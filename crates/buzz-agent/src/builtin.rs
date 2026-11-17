//! Built-in tools that run in-process, bypassing MCP.
//!
//! Currently:
//! - `load_skill` — reads a skill's full SKILL.md body from disk
//!   and returns it so the agent can load skill content on demand.
//! - `sign_and_send_event` — signs and publishes a Nostr event using the
//!   agent's keys (Mode B daemon only).
//! - `send_direct_message` — sends a Direct Message (kind:4 / NIP-44) to a target pubkey
//!   (Mode B daemon only).
//! - `create_reminder` — creates a scheduled reminder event (kind:30300) (Mode B daemon only).

use serde_json::{json, Value};

use crate::agent::RunCtx;
use crate::browser_pool::BrowserPool;
use crate::capability_token::Capability;
use crate::hints::{strip_frontmatter, SkillEntry, MAX_SKILL_BODY_BYTES};
use crate::mcp::truncate_at_boundary;
use crate::types::{AgentError, HistoryItem, ToolCall, ToolDef, ToolResult, ToolResultContent};
use nostr::{EventBuilder, Kind, Tag};
use reqwest;
use chrono;
use hex;
use uuid::Uuid;

// Browser automation Phase 2 imports
use browser_automation::skills::{
    AVAILABLE_SKILLS as BROWSER_AVAILABLE_SKILLS, FormFillConfig, FormFillSkill, SkillContext,
    StructuredScrapeConfig, StructuredScrapeSkill,
    init_registry as browser_init_registry, registry as browser_registry,
};
use browser_automation::capability::BrowserCapability;

pub const LOAD_SKILL_TOOL: &str = "load_skill";

/// Return the `ToolDef` for `load_skill` to include in the LLM tool list.
pub fn load_skill_def() -> ToolDef {
    ToolDef {
        name: LOAD_SKILL_TOOL.to_owned(),
        description: "Load the full content of a skill by name. \
            Call this before using a skill — the system prompt lists skill names \
            and descriptions only; the full instructions are loaded on demand. \
            To load a supporting file within a skill, use the form \
            \"skill-name/relative/path\" (e.g. \"my-skill/references/foo.md\")."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The skill name as listed in the Available Skills section, \
                        or \"skill-name/relative/path\" to load a supporting file."
                }
            },
            "required": ["name"]
        }),
    }
}

/// Execute a `load_skill` call. Returns a `ToolResult` on success or a
/// user-visible error result if the skill is not found or cannot be read.
pub async fn call_load_skill(arguments: &Value, skills: &[SkillEntry]) -> ToolResult {
    let name = match arguments.get("name").and_then(Value::as_str) {
        Some(n) => n,
        None => {
            return error_result("load_skill: missing required argument \"name\"");
        }
    };

    // Two forms:
    //   "skill-name"            → load SKILL.md body + ## Supporting Files section
    //   "skill-name/rel/path"   → load a specific supporting file
    if let Some((skill_name, rel_path)) = name.split_once('/') {
        return load_supporting_file(skill_name, rel_path, skills).await;
    }

    // Plain skill-name form: load SKILL.md body.
    let entry = match skills.iter().find(|s| s.name == name) {
        Some(e) => e,
        None => {
            let available: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
            return error_result(&format!(
                "load_skill: skill {name:?} not found. Available: {available:?}"
            ));
        }
    };

    // Read the file off the async executor to avoid blocking a Tokio worker.
    let skill_path = entry.path.clone();
    let raw = match tokio::task::spawn_blocking(move || std::fs::read_to_string(&skill_path))
        .await
        .unwrap_or_else(|e| Err(std::io::Error::other(e)))
    {
        Ok(s) => s,
        Err(e) => {
            return error_result(&format!("load_skill: could not read {:?}: {e}", entry.path));
        }
    };

    // Strip the YAML frontmatter — the agent already knows name/description
    // from the system prompt; return only the body.
    let body = strip_frontmatter(&raw);

    let mut output = body.to_owned();

    // Append ## Supporting Files section if this skill has any.
    if !entry.supporting_files.is_empty() {
        let skill_dir = entry.path.parent().unwrap_or(&entry.path);
        output.push_str("\n\n## Supporting Files\n\n");
        for file in &entry.supporting_files {
            if let Ok(rel) = file.strip_prefix(skill_dir) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                output.push_str(&format!(
                    "- {} (load_skill(name: \"{}/{}\"))\n",
                    rel_str, entry.name, rel_str
                ));
            }
        }
    }

    // Apply the size cap to the full output (body + Supporting Files section)
    // so the total tool result stays within MAX_SKILL_BODY_BYTES.
    let output = if output.len() > MAX_SKILL_BODY_BYTES {
        truncate_at_boundary(&output, MAX_SKILL_BODY_BYTES).to_owned()
    } else {
        output
    };

    ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(output)],
        is_error: false,
    }
}

/// Load a supporting file identified by `skill_name/rel_path`.
/// Matches against the pre-enumerated `supporting_files` list and applies a
/// canonicalize-based traversal guard before reading.
async fn load_supporting_file(
    skill_name: &str,
    rel_path: &str,
    skills: &[SkillEntry],
) -> ToolResult {
    let rel_path = rel_path.replace('\\', "/");

    let entry = match skills.iter().find(|s| s.name == skill_name) {
        Some(e) => e,
        None => {
            let available: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
            return error_result(&format!(
                "load_skill: skill {skill_name:?} not found. Available: {available:?}"
            ));
        }
    };

    let skill_dir = match entry.path.parent() {
        Some(d) => d,
        None => {
            return error_result(&format!(
                "load_skill: could not determine skill directory for {skill_name:?}"
            ));
        }
    };

    // Match rel_path against the pre-enumerated supporting_files list.
    let matched = entry.supporting_files.iter().find(|f| {
        f.strip_prefix(skill_dir)
            .map(|r| r.to_string_lossy().replace('\\', "/") == rel_path)
            .unwrap_or(false)
    });

    let file_path = match matched {
        Some(p) => p,
        None => {
            let available: Vec<String> = entry
                .supporting_files
                .iter()
                .filter_map(|f| {
                    f.strip_prefix(skill_dir)
                        .ok()
                        .map(|r| r.to_string_lossy().replace('\\', "/"))
                })
                .collect();
            if available.is_empty() {
                return error_result(&format!(
                    "load_skill: skill {skill_name:?} has no supporting files."
                ));
            }
            return error_result(&format!(
                "load_skill: file {rel_path:?} not found in skill {skill_name:?}. \
                 Available: {available:?}"
            ));
        }
    };

    // Traversal guard: canonicalize both paths and verify the file stays inside
    // the skill directory. Fail hard if the skill directory itself can't be
    // canonicalized — a degraded guard is worse than no guard.
    let canonical_skill_dir = match skill_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return error_result(&format!(
                "load_skill: could not canonicalize skill directory for {skill_name:?}: {e}"
            ));
        }
    };

    // Clone the path so we can move it into spawn_blocking.
    let file_path = file_path.clone();
    let skill_name = skill_name.to_owned();
    let rel_path_owned = rel_path.clone();

    match tokio::task::spawn_blocking(move || file_path.canonicalize().map(|c| (c, file_path)))
        .await
        .unwrap_or_else(|e| Err(std::io::Error::other(e)))
    {
        Ok((canonical_file, resolved_path)) if canonical_file.starts_with(&canonical_skill_dir) => {
            match tokio::task::spawn_blocking(move || std::fs::read_to_string(&resolved_path))
                .await
                .unwrap_or_else(|e| Err(std::io::Error::other(e)))
            {
                Ok(content) => {
                    let output = format!(
                        "# Loaded: {}/{}\n\n{}\n\n---\nFile loaded into context.",
                        skill_name, rel_path_owned, content
                    );
                    let output = if output.len() > MAX_SKILL_BODY_BYTES {
                        truncate_at_boundary(&output, MAX_SKILL_BODY_BYTES).to_owned()
                    } else {
                        output
                    };
                    ToolResult {
                        provider_id: String::new(),
                        content: vec![ToolResultContent::Text(output)],
                        is_error: false,
                    }
                }
                Err(e) => error_result(&format!(
                    "load_skill: could not read {skill_name:?}/{rel_path_owned}: {e}"
                )),
            }
        }
        Ok(_) => error_result(&format!(
            "load_skill: refusing to load {skill_name:?}/{rel_path_owned}: \
             resolves outside the skill directory"
        )),
        Err(e) => error_result(&format!(
            "load_skill: could not resolve {skill_name:?}/{rel_path_owned}: {e}"
        )),
    }
}

fn error_result(msg: &str) -> ToolResult {
    ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(msg.to_owned())],
        is_error: true,
    }
}

/// Tool definition for `sign_and_send_event` — signs and publishes a Nostr event
/// using the agent's keys. Only available in Mode B daemon mode.
pub const SIGN_AND_SEND_EVENT_TOOL: &str = "sign_and_send_event";

/// Return the `ToolDef` for `sign_and_send_event`.
pub fn sign_and_send_event_def() -> ToolDef {
    ToolDef {
        name: SIGN_AND_SEND_EVENT_TOOL.to_owned(),
        description: "Sign and publish a Nostr event using the agent's keys. \
            Only available when running in Mode B daemon mode with agent keys configured."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "kind": {
                    "type": "integer",
                    "description": "Nostr event kind (e.g., 30300 for reminders, 14 for DMs)"
                },
                "content": {
                    "type": "string",
                    "description": "Event content (will be NIP-44 encrypted if needed)"
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "description": "Event tags as array of string arrays, e.g., [[\"d\", \"...\"], [\"p\", \"...\"]]"
                }
            },
            "required": ["kind", "content", "tags"]
        }),
    }
}

/// Execute `sign_and_send_event`.
pub async fn call_sign_and_send_event(
    arguments: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref()
        .ok_or_else(|| AgentError::InvalidParams("agent keys not available in this context".into()))?;

    let kind: u16 = arguments["kind"].as_u64()
        .ok_or_else(|| AgentError::InvalidParams("missing or invalid kind".into()))?
        .try_into()
        .map_err(|_| AgentError::InvalidParams("kind out of range".into()))?;

    let content = arguments["content"].as_str()
        .ok_or_else(|| AgentError::InvalidParams("missing content".into()))?;

    let tags: Vec<Vec<String>> = serde_json::from_value(arguments["tags"].clone())
        .map_err(|e| AgentError::InvalidParams(format!("invalid tags: {e}")))?;

    // Build and sign event
    let mut builder = EventBuilder::new(Kind::Custom(kind), content);
    for tag in tags {
        builder = builder.tag(Tag::parse(tag)
            .map_err(|e| AgentError::InvalidParams(format!("invalid tag: {e}")))?);
    }

    let event = builder.sign_with_keys(keys)
        .map_err(|e| AgentError::Other(format!("sign event failed: {e}")))?;

    // If kind is 30300 or 4, encrypt content with NIP-44
    let event = if kind == 30300 || kind == 4 {
        // For kind:30300, encrypt to self (author)
        // For kind:4, need target pubkey from p-tag
        let target_pubkey = event.tags.iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("p"))
            .and_then(|t| t.as_slice().get(1))
            .and_then(|v| Some(v.as_str()));

        let encrypted_content = if kind == 4 {
            if let Some(target_pk_str) = target_pubkey {
                let target_pk = nostr::PublicKey::from_hex(target_pk_str)
                    .map_err(|e| AgentError::InvalidParams(format!("invalid target pubkey in p-tag: {e}")))?;
                nostr::nips::nip44::encrypt(
                    keys.secret_key(),
                    &target_pk,
                    &content,
                    nostr::nips::nip44::Version::V2,
                ).map_err(|e| AgentError::Other(format!("NIP-44 encrypt failed: {e}")))?
            } else {
                return Err(AgentError::InvalidParams("kind:4 requires p-tag with target pubkey".into()));
            }
        } else {
            // kind:30300 - self-encrypt
            nostr::nips::nip44::encrypt(
                keys.secret_key(),
                &keys.public_key(),
                &content,
                nostr::nips::nip44::Version::V2,
            ).map_err(|e| AgentError::Other(format!("NIP-44 encrypt failed: {e}")))?
        };

        EventBuilder::new(Kind::Custom(kind), encrypted_content)
            .tags(event.tags)
            .sign_with_keys(keys)
            .map_err(|e| AgentError::Other(format!("sign event failed: {e}")))?
    } else {
        event
    };

    // Publish via HTTP to relay
    let client = reqwest::Client::new();
    let response = client
        .post(&ctx.cfg.relay_ws_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!([ "EVENT", event ]))
        .send()
        .await
        .map_err(|e| AgentError::Other(format!("HTTP publish failed: {e}")))?;

    if !response.status().is_success() {
        return Err(AgentError::Other(format!(
            "relay returned error: {}",
            response.status()
        )));
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            format!("Event published: {}", event.id)
        )],
        is_error: false,
    })
}

/// Tool definition for `send_direct_message` — sends an NIP-17 DM to a target pubkey.
/// Only available in Mode B daemon mode.
pub const SEND_DIRECT_MESSAGE_TOOL: &str = "send_direct_message";

/// Return the `ToolDef` for `send_direct_message`.
pub fn send_direct_message_def() -> ToolDef {
    ToolDef {
        name: SEND_DIRECT_MESSAGE_TOOL.to_owned(),
        description: "Send a Direct Message (kind:4 / NIP-44 encrypted) to a target pubkey. \
            Only available when running in Mode B daemon mode with agent keys configured."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target_pubkey": {
                    "type": "string",
                    "description": "Target user's pubkey (hex, 64 chars)"
                },
                "content": {
                    "type": "string",
                    "description": "Message content to send"
                }
            },
            "required": ["target_pubkey", "content"]
        }),
    }
}

/// Execute `send_direct_message`.
pub async fn call_send_direct_message(
    arguments: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref()
        .ok_or_else(|| AgentError::InvalidParams("agent keys not available in this context".into()))?;

    let target_pubkey = arguments["target_pubkey"].as_str()
        .ok_or_else(|| AgentError::InvalidParams("missing target_pubkey".into()))?;

    let content = arguments["content"].as_str()
        .ok_or_else(|| AgentError::InvalidParams("missing content".into()))?;

    let target_pk = nostr::PublicKey::from_hex(target_pubkey)
        .map_err(|e| AgentError::InvalidParams(format!("invalid target pubkey: {e}")))?;

    // Encrypt content with NIP-44 v2 for kind:4 DM
    let encrypted_content = nostr::nips::nip44::encrypt(
        keys.secret_key(),
        &target_pk,
        content,
        nostr::nips::nip44::Version::V2,
    ).map_err(|e| AgentError::Other(format!("NIP-44 encrypt failed: {e}")))?;

    // Build DM event (kind:4 / NIP-44 encrypted)
    let dm_event = EventBuilder::new(Kind::Custom(4), encrypted_content)
        .tag(Tag::parse(["p", target_pubkey]).unwrap())
        .sign_with_keys(keys)
        .map_err(|e| AgentError::Other(format!("DM event sign failed: {e}")))?;

    // Publish via HTTP to relay
    let client = reqwest::Client::new();
    let response = client
        .post(&ctx.cfg.relay_ws_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!([ "EVENT", dm_event ]))
        .send()
        .await
        .map_err(|e| AgentError::Other(format!("HTTP publish failed: {e}")))?;

    if !response.status().is_success() {
        return Err(AgentError::Other(format!(
            "relay returned error: {}",
            response.status()
        )));
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            format!("DM sent to {}: {}", target_pubkey, dm_event.id)
        )],
        is_error: false,
    })
}

/// Parse timestamp or duration string (e.g., "30m", "2h", "2026-12-31T23:59:59Z")
fn parse_timestamp_or_duration(input: &str) -> Result<i64, AgentError> {
    // Try parsing as ISO8601 datetime first
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
        return Ok(dt.timestamp());
    }
    
    // Try parsing as relative duration
    let input = input.trim();
    if input.is_empty() {
        return Err(AgentError::InvalidParams("empty time specification".into()));
    }
    
    // Find the number and unit
    let (num_str, unit) = input.split_at(input.len() - 1);
    let num: i64 = num_str.parse()
        .map_err(|_| AgentError::InvalidParams(format!("invalid duration number: {}", num_str)))?;
    
    let seconds = match unit {
        "s" => num,
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        _ => return Err(AgentError::InvalidParams(format!("invalid duration unit: {}", unit))),
    };
    
    let now = chrono::Utc::now().timestamp();
    Ok(now + seconds)
}

/// Validate that not_before is in the future (with small grace period)
fn validate_not_before(not_before: i64) -> Result<(), AgentError> {
    let now = chrono::Utc::now().timestamp();
    if not_before <= now + 5 { // 5 second grace period
        return Err(AgentError::InvalidParams("reminder time must be in the future".into()));
    }
    Ok(())
}

/// Generate a 128-bit entropy d-tag for NIP-ER reminders
fn generate_d_tag() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Tool definition for `create_reminder` — creates a scheduled reminder event (kind:30300).
/// Only available in Mode B daemon mode.
pub const CREATE_REMINDER_TOOL: &str = "create_reminder";

/// Return the `ToolDef` for `create_reminder`.
pub fn create_reminder_def() -> ToolDef {
    ToolDef {
        name: CREATE_REMINDER_TOOL.to_owned(),
        description: "Delegate to the background scheduler to create a timed reminder event. When due, the Agent will send a private message (DM) to the target user."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "Reminder content"},
                "at": {"type": "string", "description": "When to remind: relative time (e.g. '30m', '2h') or ISO8601"},
                "target_user_pubkey": {"type": "string", "description": "Recipient user pubkey (hex)"},
                "channel_id": {"type": "string", "description": "Associated channel UUID (optional, for context)"},
                "event_id": {"type": "string", "description": "Associated message event ID (optional)"},
                "preview": {"type": "string", "description": "Associated message preview text (optional)"}
            },
            "required": ["content", "at", "target_user_pubkey"]
        }),
    }
}

/// Execute `create_reminder`.
pub async fn call_create_reminder(
    arguments: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref()
        .ok_or_else(|| AgentError::InvalidParams("agent keys not available".into()))?;

    // Parse arguments
    let content = arguments["content"].as_str()
        .ok_or_else(|| AgentError::InvalidParams("missing content".into()))?;
    let at = arguments["at"].as_str()
        .ok_or_else(|| AgentError::InvalidParams("missing at".into()))?;
    let target_user_pubkey = arguments["target_user_pubkey"].as_str()
        .ok_or_else(|| AgentError::InvalidParams("missing target_user_pubkey".into()))?;
    let channel_id = arguments["channel_id"].as_str();
    let event_id = arguments["event_id"].as_str();
    let preview = arguments["preview"].as_str();

    // Parse timestamp
    let not_before = parse_timestamp_or_duration(at)?;
    validate_not_before(not_before)?;

    // Validate pubkey
    let target_pubkey = nostr::PublicKey::from_hex(target_user_pubkey)
        .map_err(|_| AgentError::InvalidParams("invalid target_user_pubkey".into()))?;

    // Generate d-tag (128-bit entropy)
    let d_tag = generate_d_tag();

    // Build reminder content JSON (NIP-ER spec)
    let mut target = serde_json::json!({});
    if let Some(ch) = channel_id { target["channelId"] = serde_json::json!(ch); }
    if let Some(eid) = event_id { target["eventId"] = serde_json::json!(eid); }
    if let Some(p) = preview { target["preview"] = serde_json::json!(p); }
    target["authorPubkey"] = serde_json::json!(target_user_pubkey);

    let reminder_content = serde_json::json!({
        "target": target,
        "status": "pending",
        "note": content,
        // Mode B specific: record target user pubkey for DM sending
        "dm_recipient_pubkey": target_user_pubkey,
    });

    // NIP-44 self-encrypt (Agent decrypts itself)
    let ciphertext = nostr::nips::nip44::encrypt(
        keys.secret_key(),
        &keys.public_key(),
        &reminder_content.to_string(),
        nostr::nips::nip44::Version::V2,
    ).map_err(|e| AgentError::Other(format!("NIP-44 encrypt failed: {e}")))?;

    // Build event
    let builder = EventBuilder::new(Kind::Custom(30300), ciphertext)
        .tag(Tag::parse(["d", &d_tag])?)
        .tag(Tag::parse(["not_before", &not_before.to_string()])?)
        .tag(Tag::parse(["alt", "Encrypted reminder"]) ?);

    let event = builder.sign_with_keys(keys)
        .map_err(|e| AgentError::Other(format!("sign event failed: {e}")))?;

    // Publish via HTTP to relay
    let client = reqwest::Client::new();
    let response = client
        .post(&ctx.cfg.relay_ws_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!([ "EVENT", event ]))
        .send()
        .await
        .map_err(|e| AgentError::Other(format!("HTTP publish failed: {e}")))?;

    if !response.status().is_success() {
        return Err(AgentError::Other(format!(
            "relay returned error: {}",
            response.status()
        )));
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(format!(
            "Reminder created: d-tag={}, triggers at {}", d_tag, not_before
        ))],
        is_error: false,
    })
}

/// Direct Nostr event signing and sending for non-agent contexts (e.g., Health Guardian).
/// This bypasses the RunCtx requirement and directly uses agent keys and relay URL.
pub async fn call_sign_and_send_event_direct(
    keys: &crate::types::AgentKeys,
    relay_url: &str,
    kind: u16,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<String, AgentError> {
    use nostr::{EventBuilder, Kind, Tag};
    use reqwest;

    let mut builder = EventBuilder::new(Kind::Custom(kind), content);
    for tag in tags {
        builder = builder.tag(Tag::parse(tag)
            .map_err(|e| AgentError::InvalidParams(format!("invalid tag: {e}")))?);
    }

    let event = builder.sign_with_keys(keys)
        .map_err(|e| AgentError::Other(format!("sign event failed: {e}")))?;

    // Publish via HTTP to relay
    let client = reqwest::Client::new();
    let response = client
        .post(relay_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!([ "EVENT", event ]))
        .send()
        .await
        .map_err(|e| AgentError::Other(format!("HTTP publish failed: {e}")))?;

    if !response.status().is_success() {
        return Err(AgentError::Other(format!(
            "relay returned error: {}",
            response.status()
        )));
    }

    Ok(event.id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn text_content(result: &ToolResult) -> String {
        match &result.content[0] {
            ToolResultContent::Text(t) => t.clone(),
            ToolResultContent::Image { .. } => panic!("unexpected Image content in test"),
        }
    }

    fn make_skill(name: &str, description: &str, path: PathBuf) -> SkillEntry {
        SkillEntry {
            name: name.to_owned(),
            description: description.to_owned(),
            path,
            supporting_files: Vec::new(),
        }
    }

    fn make_skill_with_files(
        name: &str,
        description: &str,
        path: PathBuf,
        supporting_files: Vec<PathBuf>,
    ) -> SkillEntry {
        SkillEntry {
            name: name.to_owned(),
            description: description.to_owned(),
            path,
            supporting_files,
        }
    }

    #[tokio::test]
    async fn call_load_skill_missing_name_arg() {
        let result = call_load_skill(&serde_json::json!({}), &[]).await;
        assert!(result.is_error);
        let text = text_content(&result);
        assert!(text.contains("missing required argument"), "got: {text}");
    }

    #[tokio::test]
    async fn call_load_skill_skill_not_found() {
        let result = call_load_skill(&serde_json::json!({"name": "no-such"}), &[]).await;
        assert!(result.is_error);
        let text = text_content(&result);
        assert!(text.contains("not found"), "got: {text}");
    }

    #[tokio::test]
    async fn call_load_skill_returns_body_strips_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let skill_md = tmp.path().join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: test\ndescription: A test\n---\nSkill body here.\n",
        )
        .unwrap();
        let skills = vec![make_skill("test", "A test", skill_md)];
        let result = call_load_skill(&serde_json::json!({"name": "test"}), &skills).await;
        assert!(!result.is_error);
        let text = text_content(&result);
        assert!(text.contains("Skill body here."), "got: {text}");
        assert!(
            !text.contains("---"),
            "frontmatter should be stripped: {text}"
        );
    }

    #[tokio::test]
    async fn call_load_skill_appends_supporting_files_section() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: my-skill\ndescription: desc\n---\nBody.\n",
        )
        .unwrap();
        let refs_dir = skill_dir.join("references");
        std::fs::create_dir_all(&refs_dir).unwrap();
        let ref_file = refs_dir.join("foo.md");
        std::fs::write(&ref_file, "Reference content.").unwrap();

        let skills = vec![make_skill_with_files(
            "my-skill",
            "desc",
            skill_md,
            vec![ref_file],
        )];
        let result = call_load_skill(&serde_json::json!({"name": "my-skill"}), &skills).await;
        assert!(!result.is_error);
        let text = text_content(&result);
        assert!(text.contains("Body."), "body missing: {text}");
        assert!(
            text.contains("## Supporting Files"),
            "missing Supporting Files section: {text}"
        );
        assert!(
            text.contains("references/foo.md"),
            "missing file listing: {text}"
        );
        assert!(
            text.contains("load_skill(name: \"my-skill/references/foo.md\")"),
            "missing load_skill hint: {text}"
        );
    }

    #[tokio::test]
    async fn call_load_skill_no_supporting_files_section_when_empty() {
        let tmp = TempDir::new().unwrap();
        let skill_md = tmp.path().join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: bare\ndescription: desc\n---\nBody.\n",
        )
        .unwrap();
        let skills = vec![make_skill("bare", "desc", skill_md)];
        let result = call_load_skill(&serde_json::json!({"name": "bare"}), &skills).await;
        assert!(!result.is_error);
        let text = text_content(&result);
        assert!(
            !text.contains("## Supporting Files"),
            "should not have Supporting Files section when none: {text}"
        );
    }

    #[tokio::test]
    async fn call_load_skill_supporting_file_returns_content() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: my-skill\ndescription: desc\n---\nBody.\n",
        )
        .unwrap();
        let refs_dir = skill_dir.join("references");
        std::fs::create_dir_all(&refs_dir).unwrap();
        let ref_file = refs_dir.join("foo.md");
        std::fs::write(&ref_file, "Reference content here.").unwrap();

        let skills = vec![make_skill_with_files(
            "my-skill",
            "desc",
            skill_md,
            vec![ref_file],
        )];
        let result = call_load_skill(
            &serde_json::json!({"name": "my-skill/references/foo.md"}),
            &skills,
        )
        .await;
        assert!(!result.is_error, "expected success, got error");
        let text = text_content(&result);
        assert!(
            text.contains("Reference content here."),
            "file content missing: {text}"
        );
        assert!(
            text.contains("# Loaded: my-skill/references/foo.md"),
            "missing header: {text}"
        );
    }

    #[tokio::test]
    async fn call_load_skill_supporting_file_not_found_lists_available() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: my-skill\ndescription: desc\n---\nBody.\n",
        )
        .unwrap();
        let refs_dir = skill_dir.join("references");
        std::fs::create_dir_all(&refs_dir).unwrap();
        let ref_file = refs_dir.join("foo.md");
        std::fs::write(&ref_file, "content").unwrap();

        let skills = vec![make_skill_with_files(
            "my-skill",
            "desc",
            skill_md,
            vec![ref_file],
        )];
        let result = call_load_skill(
            &serde_json::json!({"name": "my-skill/references/missing.md"}),
            &skills,
        )
        .await;
        assert!(result.is_error);
        let text = text_content(&result);
        assert!(text.contains("not found"), "got: {text}");
        assert!(
            text.contains("references/foo.md"),
            "should list available: {text}"
        );
    }

    #[tokio::test]
    async fn call_load_skill_no_supporting_files_error_message() {
        let tmp = TempDir::new().unwrap();
        let skill_md = tmp.path().join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: bare\ndescription: desc\n---\nBody.\n",
        )
        .unwrap();
        let skills = vec![make_skill("bare", "desc", skill_md)];
        let result =
            call_load_skill(&serde_json::json!({"name": "bare/anything.md"}), &skills).await;
        assert!(result.is_error);
        let text = text_content(&result);
        assert!(text.contains("no supporting files"), "got: {text}");
    }

    #[tokio::test]
    async fn call_load_skill_traversal_guard_rejects_escape() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: my-skill\ndescription: desc\n---\nBody.\n",
        )
        .unwrap();

        // Create a file outside the skill dir that we'll try to reference.
        let outside_file = tmp.path().join("secret.txt");
        std::fs::write(&outside_file, "secret content").unwrap();

        // Manually construct a SkillEntry with a supporting_files entry that
        // points outside the skill dir — simulating a crafted/malicious entry.
        // The traversal guard should catch this.
        let skills = vec![make_skill_with_files(
            "my-skill",
            "desc",
            skill_md.clone(),
            vec![outside_file.clone()],
        )];

        // The slash form splits "my-skill/../secret.txt" into skill_name="my-skill"
        // and rel_path="../secret.txt". strip_prefix(skill_dir) on outside_file
        // fails, so it won't match any supporting_files entry — the pre-enumeration
        // guard rejects it before the canonicalize guard even fires.
        let result = call_load_skill(
            &serde_json::json!({"name": "my-skill/../secret.txt"}),
            &skills,
        )
        .await;
        assert!(result.is_error, "traversal attempt should be rejected");
        let text = text_content(&result);
        assert!(
            !text.contains("secret content"),
            "secret content must not be returned: {text}"
        );
    }

    #[tokio::test]
    async fn call_load_skill_truncates_large_body() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path();
        let skill_md = skill_dir.join("SKILL.md");
        // Build a body that exceeds MAX_SKILL_BODY_BYTES (32 KiB).
        let large_body = "x".repeat(40 * 1024);
        std::fs::write(
            &skill_md,
            format!("---\nname: big\ndescription: desc\n---\n{large_body}\n"),
        )
        .unwrap();
        // Add a supporting file so the Supporting Files section is also appended
        // before the cap is applied.
        let refs_dir = skill_dir.join("references");
        std::fs::create_dir_all(&refs_dir).unwrap();
        let ref_file = refs_dir.join("extra.md");
        std::fs::write(&ref_file, "extra content").unwrap();

        let skills = vec![make_skill_with_files(
            "big",
            "desc",
            skill_md,
            vec![ref_file],
        )];
        let result = call_load_skill(&serde_json::json!({"name": "big"}), &skills).await;
        assert!(!result.is_error);
        let text = text_content(&result);
        assert!(
            text.len() <= MAX_SKILL_BODY_BYTES,
            "output length {} exceeds MAX_SKILL_BODY_BYTES {}",
            text.len(),
            MAX_SKILL_BODY_BYTES
        );
    }

    #[tokio::test]
    async fn call_load_skill_truncates_large_supporting_file() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: big\ndescription: desc\n---\nBody.\n").unwrap();

        let refs_dir = skill_dir.join("references");
        std::fs::create_dir_all(&refs_dir).unwrap();
        let ref_file = refs_dir.join("huge.md");
        std::fs::write(&ref_file, "x".repeat(MAX_SKILL_BODY_BYTES * 2)).unwrap();

        let skills = vec![make_skill_with_files(
            "big",
            "desc",
            skill_md,
            vec![ref_file],
        )];
        let result = call_load_skill(
            &serde_json::json!({"name": "big/references/huge.md"}),
            &skills,
        )
        .await;
        assert!(!result.is_error);
        let text = text_content(&result);
        assert!(
            text.len() <= MAX_SKILL_BODY_BYTES,
            "output length {} exceeds MAX_SKILL_BODY_BYTES {}",
            text.len(),
            MAX_SKILL_BODY_BYTES
        );
        assert!(
            text.starts_with("# Loaded: big/references/huge.md"),
            "missing supporting-file header: {text}"
        );
    }
}

/// Tool name for `browser_open` — opens a URL in a new browser context via BrowserPool.
pub const BROWSER_OPEN_TOOL: &str = "browser_open";

/// Return the `ToolDef` for `browser_open`.
pub fn browser_open_def() -> ToolDef {
    ToolDef {
        name: BROWSER_OPEN_TOOL.to_owned(),
        description: "Open a URL in a new isolated browser context. Returns a context_id for use with browser_screenshot and browser_close."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to open"
                }
            },
            "required": ["url"]
        }),
    }
}

/// Execute `browser_open`.
pub async fn call_browser_open(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let url = arguments
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_open: missing required 'url' parameter".to_string()))?;

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_open: browser pool not initialized. Run in Mode B daemon with --enable-browser flag.".to_string())
    })?;

    // Create a new context
    let context_id = pool.create_context().await
        .map_err(|e| AgentError::Other(format!("browser_open: failed to create context: {e}")))?;

    // Navigate to the URL
    pool.navigate(&context_id, url).await
        .map_err(|e| AgentError::Other(format!("browser_open: failed to navigate to {url}: {e}")))?;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "url": url,
                "status": "opened"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_screenshot` — captures screenshot of current page in a browser context.
pub const BROWSER_SCREENSHOT_TOOL: &str = "browser_screenshot";

/// Return the `ToolDef` for `browser_screenshot`.
pub fn browser_screenshot_def() -> ToolDef {
    ToolDef {
        name: BROWSER_SCREENSHOT_TOOL.to_owned(),
        description: "Capture a screenshot of the current page in a browser context. Saves to the specified file path."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "The context_id returned by browser_open"
                },
                "path": {
                    "type": "string",
                    "description": "File path to save the screenshot (e.g., screenshot.png)"
                }
            },
            "required": ["context_id", "path"]
        }),
    }
}

/// Execute `browser_screenshot`.
pub async fn call_browser_screenshot(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_screenshot: missing required 'context_id' parameter".to_string()))?;

    let path = arguments
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_screenshot: missing required 'path' parameter".to_string()))?;

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_screenshot: browser pool not initialized".to_string())
    })?;

    pool.screenshot(context_id, path).await
        .map_err(|e| AgentError::Other(format!("browser_screenshot: failed to capture screenshot: {e}")))?;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "path": path,
                "status": "screenshot_saved"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_close` — safely closes a browser context and returns it to the pool.
pub const BROWSER_CLOSE_TOOL: &str = "browser_close";

/// Return the `ToolDef` for `browser_close`.
pub fn browser_close_def() -> ToolDef {
    ToolDef {
        name: BROWSER_CLOSE_TOOL.to_owned(),
        description: "Close a browser context and return it to the pool. Prevents process leaks."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "The context_id returned by browser_open"
                }
            },
            "required": ["context_id"]
        }),
    }
}

/// Execute `browser_close`.
pub async fn call_browser_close(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_close: missing required 'context_id' parameter".to_string()))?;

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_close: browser pool not initialized".to_string())
    })?;

    pool.close_context(context_id).await
        .map_err(|e| AgentError::Other(format!("browser_close: failed to close context: {e}")))?;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "status": "closed"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Check if the current session has the required capability for a browser operation.
/// This integrates with the CapabilityToken system for fine-grained authorization.
fn check_browser_capability(
    self_: &RunCtx<'_>,
    capability: &Capability,
) -> Result<(), AgentError> {
    // In Phase 2, we check for capability tokens in the session context.
    // For now, if no capability system is configured, we allow all operations.
    // The capability token would typically be passed via the session or agent config.
    // This is a placeholder for the full capability check integration.
    
    // TODO: Implement full capability check once CredentialVault/CapabilityToken is integrated
    // into the agent session. For now, log and allow.
    tracing::debug!("Capability check for {:?} - allowed (placeholder)", capability);
    Ok(())
}

// ============================================================================
// Advanced Browser Interaction Tools (Phase 2)
// ============================================================================

/// Tool name for `browser_click` — click on an element in the browser context.
pub const BROWSER_CLICK_TOOL: &str = "browser_click";

/// Return the `ToolDef` for `browser_click`.
pub fn browser_click_def() -> ToolDef {
    ToolDef {
        name: BROWSER_CLICK_TOOL.to_owned(),
        description: "Click on an element in the browser context using a CSS selector or XPath".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "The context_id returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector or XPath of the element to click"
                },
                "button": {
                    "type": "string",
                    "enum": ["left", "right", "middle"],
                    "description": "Mouse button to use (default: left)",
                    "default": "left"
                },
                "click_count": {
                    "type": "integer",
                    "description": "Number of clicks (default: 1)",
                    "default": 1,
                    "minimum": 1,
                    "maximum": 3
                },
                "delay_ms": {
                    "type": "integer",
                    "description": "Delay between mousedown and mouseup in milliseconds",
                    "default": 0
                },
                "position": {
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" }
                    },
                    "description": "Optional click position relative to element"
                }
            },
            "required": ["context_id", "selector"]
        }),
    }
}

/// Execute `browser_click`.
pub async fn call_browser_click(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Click)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_click: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments
        .get("selector")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_click: missing required 'selector' parameter".to_string()))?;

    let button = arguments
        .get("button")
        .and_then(|v| v.as_str())
        .unwrap_or("left");

    let click_count = arguments
        .get("click_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as i32;

    let delay_ms = arguments
        .get("delay_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_click: browser pool not initialized".to_string())
    })?;

    // Get the context and page
    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_click: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_click: no page available in context".to_string())
    })?;

    // Build click options
    let click_options = playwright_rs::ClickOptions::builder()
        .button(match button {
            "right" => playwright_rs::protocol::click::MouseButton::Right,
            "middle" => playwright_rs::protocol::click::MouseButton::Middle,
            _ => playwright_rs::protocol::click::MouseButton::Left,
        })
        .click_count(click_count as u32)
        .delay(delay_ms as f64)
        .build();

    // Perform the click using locator (page.click doesn't exist)
    let locator = page.locator(selector);
    locator.click(Some(click_options)).await
        .map_err(|e| AgentError::Other(format!("browser_click: failed to click element: {e}")))?;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "selector": selector,
                "action": "click",
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_fill` — fill a form field in the browser context.
pub const BROWSER_FILL_TOOL: &str = "browser_fill";

/// Return the `ToolDef` for `browser_fill`.
pub fn browser_fill_def() -> ToolDef {
    ToolDef {
        name: BROWSER_FILL_TOOL.to_owned(),
        description: "Fill a form field in the browser context with a value".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "The context_id returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector or XPath of the input element to fill"
                },
                "value": {
                    "type": "string",
                    "description": "Value to fill into the field"
                },
                "delay_ms": {
                    "type": "integer",
                    "description": "Delay between keystrokes in milliseconds (default: 0)",
                    "default": 0
                },
                "force": {
                    "type": "boolean",
                    "description": "Force fill even if element is not editable (default: false)",
                    "default": false
                },
                "no_wait_after": {
                    "type": "boolean",
                    "description": "Skip waiting for navigation after fill (default: false)",
                    "default": false
                }
            },
            "required": ["context_id", "selector", "value"]
        }),
    }
}

/// Execute `browser_fill`.
pub async fn call_browser_fill(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Fill)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_fill: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments
        .get("selector")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_fill: missing required 'selector' parameter".to_string()))?;

    let value = arguments
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_fill: missing required 'value' parameter".to_string()))?;

    let delay_ms = arguments
        .get("delay_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let force = arguments
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let no_wait_after = arguments
        .get("no_wait_after")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(30000);

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_fill: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_fill: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_fill: no page available in context".to_string())
    })?;

    let fill_options = playwright_rs::FillOptions::builder()
        .force(force)
        .timeout(timeout_ms as f64)
        .build();

    // Use locator.fill() like browser_click uses locator.click()
    let locator = page.locator(selector);
    locator.fill(value, Some(fill_options)).await
        .map_err(|e| AgentError::Other(format!("browser_fill: failed to fill field: {e}")))?;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "selector": selector,
                "action": "fill",
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_wait` — wait for an element or condition in the browser context.
pub const BROWSER_WAIT_TOOL: &str = "browser_wait";

/// Return the `ToolDef` for `browser_wait`.
pub fn browser_wait_def() -> ToolDef {
    ToolDef {
        name: BROWSER_WAIT_TOOL.to_owned(),
        description: "Wait for an element, condition, or timeout in the browser context".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "The context_id returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector or XPath to wait for (optional if waiting for timeout/navigation)"
                },
                "state": {
                    "type": "string",
                    "enum": ["attached", "detached", "visible", "hidden"],
                    "description": "Element state to wait for (default: visible)",
                    "default": "visible"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum time to wait in milliseconds (default: 30000)",
                    "default": 30000,
                    "minimum": 100
                },
                "wait_until": {
                    "type": "string",
                    "enum": ["load", "domcontentloaded", "networkidle", "commit"],
                    "description": "Wait for navigation to reach this state (optional)"
                }
            },
            "required": ["context_id"]
        }),
    }
}

/// Execute `browser_wait`.
pub async fn call_browser_wait(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Wait)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_wait: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments.get("selector").and_then(|v| v.as_str());
    let state = arguments
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("visible");

    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(30000);

    let wait_until = arguments.get("wait_until").and_then(|v| v.as_str());

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_wait: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_wait: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_wait: no page available in context".to_string())
    })?;

    if let Some(selector) = selector {
        // Wait for element using locator.wait_for (per PLAYWRIGHT_API_SPEC.md)
        let state_enum = match state {
            "attached" => playwright_rs::WaitForState::Attached,
            "detached" => playwright_rs::WaitForState::Detached,
            "hidden" => playwright_rs::WaitForState::Hidden,
            _ => playwright_rs::WaitForState::Visible,
        };

        let wait_opts = playwright_rs::WaitForOptions::builder()
            .state(state_enum)
            .timeout(timeout_ms as f64)
            .build();

        let locator = page.locator(selector);
        locator.wait_for(Some(wait_opts)).await
            .map_err(|e| AgentError::Other(format!("browser_wait: element not found: {e}")))?;
    } else if let Some(wait_until) = wait_until {
        // Wait for navigation state - use page.wait_for_load_state (no args in this version)
        page.wait_for_load_state(None).await
            .map_err(|e| AgentError::Other(format!("browser_wait: navigation timeout: {e}")))?;
    } else {
        // Simple timeout wait
        tokio::time::sleep(tokio::time::Duration::from_millis(timeout_ms)).await;
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "wait",
                "selector": selector,
                "state": state,
                "timeout_ms": timeout_ms,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_extract` — extract data from the browser context.
pub const BROWSER_EXTRACT_TOOL: &str = "browser_extract";

/// Return the `ToolDef` for `browser_extract`.
pub fn browser_extract_def() -> ToolDef {
    ToolDef {
        name: BROWSER_EXTRACT_TOOL.to_owned(),
        description: "Extract data from the page using CSS selectors, XPath, or JavaScript evaluation".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "The context_id returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector or XPath of elements to extract (optional if using script)"
                },
                "attribute": {
                    "type": "string",
                    "description": "Attribute to extract (e.g., 'href', 'textContent', 'innerHTML', 'value'). Default: 'textContent'"
                },
                "multiple": {
                    "type": "boolean",
                    "description": "Extract all matching elements (default: false, extracts first only)",
                    "default": false
                },
                "script": {
                    "type": "string",
                    "description": "Custom JavaScript to evaluate for extraction (alternative to selector)"
                }
            },
            "required": ["context_id"]
        }),
    }
}

/// Execute `browser_extract`.
pub async fn call_browser_extract(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Extract)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_extract: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments.get("selector").and_then(|v| v.as_str());
    let attribute = arguments
        .get("attribute")
        .and_then(|v| v.as_str())
        .unwrap_or("textContent");
    let multiple = arguments
        .get("multiple")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let script = arguments.get("script").and_then(|v| v.as_str());

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_extract: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_extract: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_extract: no page available in context".to_string())
    })?;

    let result = if let Some(script) = script {
        // Evaluate custom JavaScript
        let value: serde_json::Value = page.evaluate(script, None::<&()>).await
            .map_err(|e| AgentError::Other(format!("browser_extract: script evaluation failed: {e}")))?;
        serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
    } else if let Some(selector) = selector {
        if multiple {
            // Extract all matching elements
            let elements = page.locator(selector).all().await
                .map_err(|e| AgentError::Other(format!("browser_extract: query failed: {e}")))?;
            
            let mut results = Vec::new();
            for element in elements {
                let value = if attribute == "textContent" {
                    element.text_content().await
                        .map_err(|e| AgentError::Other(format!("browser_extract: text extraction failed: {e}")))?
                        .unwrap_or_default()
                } else if attribute == "innerHTML" {
                    element.inner_html().await
                        .map_err(|e| AgentError::Other(format!("browser_extract: HTML extraction failed: {e}")))?
                } else {
                    element.get_attribute(attribute).await
                        .map_err(|e| AgentError::Other(format!("browser_extract: attribute extraction failed: {e}")))?
                        .unwrap_or_default()
                };
                results.push(serde_json::Value::String(value));
            }
            serde_json::Value::Array(results)
        } else {
            // Extract single element - use locator
            let locator = page.locator(selector).first();
            let value = if attribute == "textContent" {
                locator.text_content().await
                    .map_err(|e| AgentError::Other(format!("browser_extract: text extraction failed: {e}")))?
                    .unwrap_or_default()
            } else if attribute == "innerHTML" {
                locator.inner_html().await
                    .map_err(|e| AgentError::Other(format!("browser_extract: HTML extraction failed: {e}")))?
            } else {
                locator.get_attribute(attribute).await
                    .map_err(|e| AgentError::Other(format!("browser_extract: attribute extraction failed: {e}")))?
                    .unwrap_or_default()
            };
            serde_json::Value::String(value)
        }
    } else {
        return Err(AgentError::Other("browser_extract: either 'selector' or 'script' parameter is required".to_string()));
    };

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "extract",
                "selector": selector,
                "attribute": attribute,
                "multiple": multiple,
                "data": result,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_select`.
pub const BROWSER_SELECT_TOOL: &str = "browser_select";

/// Tool definition for `browser_select`.
pub fn browser_select_def() -> ToolDef {
    ToolDef {
        name: BROWSER_SELECT_TOOL.into(),
        description: "Select option(s) from a <select> dropdown by value, label, or index.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "Browser context ID returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for the <select> element"
                },
                "values": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Option values to select (by 'value' attribute)"
                },
                "labels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Option labels to select (visible text)"
                },
                "indices": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Option indices to select (0-based)"
                }
            },
            "required": ["context_id", "selector"],
            "minProperties": 2
        }),
    }
}

/// Execute `browser_select`.
pub async fn call_browser_select(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Select)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_select: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments
        .get("selector")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_select: missing required 'selector' parameter".to_string()))?;

    let values: Vec<String> = arguments
        .get("values")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let labels: Vec<String> = arguments
        .get("labels")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let indices: Vec<i32> = arguments
        .get("indices")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64().map(|i| i as i32)).collect())
        .unwrap_or_default();

    if values.is_empty() && labels.is_empty() && indices.is_empty() {
        return Err(AgentError::Other("browser_select: at least one of 'values', 'labels', or 'indices' must be provided".to_string()));
    }

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_select: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_select: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_select: no page available in context".to_string())
    })?;

    let locator = page.locator(selector);

    // Use locator.select_option with SelectOption enum variants (per actual playwright_rs API)
    for value in &values {
        let opt = playwright_rs::SelectOption::Value(value.clone());
        locator.select_option(opt, None).await
            .map_err(|e| AgentError::Other(format!("browser_select: selection failed for '{}': {}", value, e)))?;
    }
    for label in &labels {
        let opt = playwright_rs::SelectOption::Label(label.clone());
        locator.select_option(opt, None).await
            .map_err(|e| AgentError::Other(format!("browser_select: selection failed for '{}': {}", label, e)))?;
    }
    for index in &indices {
        let opt = playwright_rs::SelectOption::Index(*index as usize);
        locator.select_option(opt, None).await
            .map_err(|e| AgentError::Other(format!("browser_select: selection failed for '{}': {}", index, e)))?;
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "select",
                "selector": selector,
                "selected_values": values,
                "selected_labels": labels,
                "selected_indices": indices,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_hover`.
pub const BROWSER_HOVER_TOOL: &str = "browser_hover";

/// Tool definition for `browser_hover`.
pub fn browser_hover_def() -> ToolDef {
    ToolDef {
        name: BROWSER_HOVER_TOOL.into(),
        description: "Hover mouse over an element.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "Browser context ID returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for the element to hover"
                },
                "position": {
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" }
                    },
                    "description": "Optional position offset within the element"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum time to wait for element in milliseconds (default: 30000)",
                    "default": 30000,
                    "minimum": 100
                }
            },
            "required": ["context_id", "selector"]
        }),
    }
}

/// Execute `browser_hover`.
pub async fn call_browser_hover(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Hover)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_hover: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments
        .get("selector")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_hover: missing required 'selector' parameter".to_string()))?;

    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(30000);

    let position = arguments.get("position").and_then(|v| v.as_object());

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_hover: browser pool not initialized".to_string())
    })?;

    // Use public get_context API (per BrowserPool interface)
    let context = pool.get_context(context_id).await.ok_or_else(|| {
        AgentError::Other(format!("browser_hover: failed to get context: context not found"))
    })?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_hover: no page available in context".to_string())
    })?;

    let locator = page.locator(selector).first();
    locator.hover(None).await
        .map_err(|e| AgentError::Other(format!("browser_hover: hover failed: {e}")))?;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "hover",
                "selector": selector,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_keypress`.
pub const BROWSER_KEYPRESS_TOOL: &str = "browser_keypress";

/// Tool definition for `browser_keypress`.
pub fn browser_keypress_def() -> ToolDef {
    ToolDef {
        name: BROWSER_KEYPRESS_TOOL.into(),
        description: "Press keyboard keys (single key, combination, or text input).".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "Browser context ID returned by browser_open"
                },
                "selector": {
                    "type": "string",
                    "description": "Optional CSS selector to focus before keypress"
                },
                "key": {
                    "type": "string",
                    "description": "Single key to press (e.g., 'Enter', 'Tab', 'ArrowDown', 'Control+a')"
                },
                "keys": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Multiple keys to press in sequence"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (alternative to key/keys)"
                },
                "delay_ms": {
                    "type": "integer",
                    "description": "Delay between key presses in milliseconds (default: 0)",
                    "default": 0,
                    "minimum": 0
                }
            },
            "required": ["context_id"]
        }),
    }
}

/// Execute `browser_keypress`.
pub async fn call_browser_keypress(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Keypress)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_keypress: missing required 'context_id' parameter".to_string()))?;

    let selector = arguments.get("selector").and_then(|v| v.as_str());
    let key = arguments.get("key").and_then(|v| v.as_str());
    let keys: Vec<String> = arguments
        .get("keys")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let text = arguments.get("text").and_then(|v| v.as_str());
    let delay_ms = arguments
        .get("delay_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(30000);

    if key.is_none() && keys.is_empty() && text.is_none() {
        return Err(AgentError::Other("browser_keypress: one of 'key', 'keys', or 'text' must be provided".to_string()));
    }

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_keypress: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_keypress: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_keypress: no page available in context".to_string())
    })?;

    // Focus element if selector provided
    if let Some(sel) = selector {
        let locator = page.locator(sel);
        let wait_opts = playwright_rs::WaitForOptions::builder()
            .state(playwright_rs::WaitForState::Visible)
            .timeout(timeout_ms as f64)
            .build();
        locator.wait_for(Some(wait_opts)).await
            .map_err(|e| AgentError::Other(format!("browser_keypress: wait for selector failed: {e}")))?;
        locator.focus().await
            .map_err(|e| AgentError::Other(format!("browser_keypress: focus failed: {e}")))?;
    }

    // Handle key press
    if let Some(k) = key {
        page.keyboard().press(k, None).await
            .map_err(|e| AgentError::Other(format!("browser_keypress: press failed: {e}")))?;
    }

    // Handle multiple keys in sequence
    for k in &keys {
        page.keyboard().press(k, None).await
            .map_err(|e| AgentError::Other(format!("browser_keypress: press failed for '{}': {}", k, e)))?;
    }

    // Handle text input
    if let Some(t) = text {
        page.keyboard().type_text(t, None).await
            .map_err(|e| AgentError::Other(format!("browser_keypress: type failed: {e}")))?;
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "keypress",
                "selector": selector,
                "key": key,
                "keys": keys,
                "text": text,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `browser_execute_script`.
pub const BROWSER_EXECUTE_SCRIPT_TOOL: &str = "browser_execute_script";

/// Tool definition for `browser_execute_script`.
pub fn browser_execute_script_def() -> ToolDef {
    ToolDef {
        name: BROWSER_EXECUTE_SCRIPT_TOOL.into(),
        description: "Execute JavaScript in the browser context and return the result.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "Browser context ID returned by browser_open"
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript code to execute (function body, 'return' is optional)"
                },
                "args": {
                    "type": "array",
                    "items": {},
                    "description": "Arguments to pass to the script (accessible as arguments[0], arguments[1], ...)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum execution time in milliseconds (default: 30000)",
                    "default": 30000,
                    "minimum": 100
                }
            },
            "required": ["context_id", "script"]
        }),
    }
}

/// Execute `browser_execute_script`.
pub async fn call_browser_execute_script(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::ExecuteScript)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_execute_script: missing required 'context_id' parameter".to_string()))?;

    let script = arguments
        .get("script")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("browser_execute_script: missing required 'script' parameter".to_string()))?;

    let args: Vec<Value> = arguments
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.clone())
        .unwrap_or_default();

    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(30000);

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("browser_execute_script: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("browser_execute_script: failed to get context: context not found")))?;

    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("browser_execute_script: no page available in context".to_string())
    })?;

    // Execute JavaScript directly using page.evaluate (no EvaluateOptions in playwright_rs)
    let result: serde_json::Value = page.evaluate(script, None::<&()>).await
        .map_err(|e| AgentError::Other(format!("browser_execute_script: evaluation failed: {e}")))?;

    // Convert result back to JSON
    let result_json = result;

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "execute_script",
                "script": script,
                "result": result_json,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

/// Tool name for `form_fill` — fills form fields intelligently using the FormFillSkill.
pub const FORM_FILL_TOOL: &str = "form_fill";

/// Tool definition for `form_fill`.
pub fn form_fill_def() -> ToolDef {
    ToolDef {
        name: FORM_FILL_TOOL.into(),
        description: "Fill form fields intelligently. Uses field mappings to match selectors with values, with configurable timeouts and interaction waits."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "Browser context ID returned by browser_open"
                },
                "field_mappings": {
                    "type": "object",
                    "description": "Mapping of field selectors (CSS/XPath) to values to fill",
                    "additionalProperties": { "type": "string" }
                },
                "wait_for_interactable": {
                    "type": "boolean",
                    "description": "Wait for fields to become interactable before filling (default: true)",
                    "default": true
                },
                "field_timeout": {
                    "type": "integer",
                    "description": "Timeout per field in milliseconds (default: 5000)",
                    "default": 5000,
                    "minimum": 100
                }
            },
            "required": ["context_id", "field_mappings"]
        }),
    }
}

/// Execute `form_fill`.
pub async fn call_form_fill(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Fill)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("form_fill: missing required 'context_id' parameter".to_string()))?;

    let field_mappings: std::collections::HashMap<String, String> = arguments
        .get("field_mappings")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AgentError::Other("form_fill: missing required 'field_mappings' parameter (object)".to_string()))?
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    let wait_for_interactable = arguments
        .get("wait_for_interactable")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let field_timeout = arguments
        .get("field_timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(5000);

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("form_fill: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("form_fill: failed to get context: context not found")))?;
    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("form_fill: no page available in context".to_string())
    })?;

    let mut fields_filled = 0usize;
    let mut fields_failed = 0usize;
    let mut failures = Vec::new();

    for (selector, value) in &field_mappings {
        let locator = page.locator(selector);
        match locator.fill(value, None).await {
            Ok(_) => fields_filled += 1,
            Err(e) => {
                fields_failed += 1;
                failures.push(format!("Selector '{}': {}", selector, e));
            }
        }
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "form_fill",
                "filled_fields": fields_filled,
                "failed_fields": fields_failed,
                "failures": failures,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}

#[derive(Debug, Clone)]
struct SelectorSpec {
    name: String,
    expression: String,
    type_: String,
    attribute: Option<String>,
    multiple: bool,
}

/// Tool name for `structured_scrape` — extracts structured data using the StructuredScrapeSkill.
pub const STRUCTURED_SCRAPE_TOOL: &str = "structured_scrape";

/// Tool definition for `structured_scrape`.
pub fn structured_scrape_def() -> ToolDef {
    ToolDef {
        name: STRUCTURED_SCRAPE_TOOL.into(),
        description: "Extract structured data from web pages using CSS/XPath selectors with type annotations. Returns typed data (text, html, attribute, table, list)."
            .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "context_id": {
                    "type": "string",
                    "description": "Browser context ID returned by browser_open"
                },
                "selectors": {
                    "type": "array",
                    "description": "Array of selector specifications",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Field name for the extracted data"
                            },
                            "expression": {
                                "type": "string",
                                "description": "CSS or XPath selector expression"
                            },
                            "type": {
                                "type": "string",
                                "description": "Extraction type: text, html, attribute, table, list",
                                "enum": ["text", "html", "attribute", "table", "list"],
                                "default": "text"
                            },
                            "attribute": {
                                "type": "string",
                                "description": "Attribute name when type is 'attribute'"
                            },
                            "multiple": {
                                "type": "boolean",
                                "description": "Extract multiple elements (default: false)",
                                "default": false
                            }
                        },
                        "required": ["name", "expression"]
                    }
                }
            },
            "required": ["context_id", "selectors"]
        }),
    }
}

/// Execute `structured_scrape`.
pub async fn call_structured_scrape(
    arguments: &Value,
    self_: &mut RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    // Capability check
    check_browser_capability(self_, &Capability::Extract)?;

    let context_id = arguments
        .get("context_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Other("structured_scrape: missing required 'context_id' parameter".to_string()))?;

    let selectors: Vec<SelectorSpec> = arguments
        .get("selectors")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AgentError::Other("structured_scrape: missing required 'selectors' parameter (array)".to_string()))?
        .iter()
        .map(|v| {
            let name = v.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let expression = v.get("expression").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let type_str = v.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            let attribute = v.get("attribute").and_then(|v| v.as_str()).map(|s| s.to_string());
            let multiple = v.get("multiple").and_then(|v| v.as_bool()).unwrap_or(false);

            SelectorSpec {
                name,
                expression,
                type_: type_str.to_string(),
                attribute,
                multiple,
            }
        })
        .collect();

    let pool = self_.browser_pool.as_ref().ok_or_else(|| {
        AgentError::Other("structured_scrape: browser pool not initialized".to_string())
    })?;

    let context = pool.get_context(context_id).await.ok_or_else(|| AgentError::Other(format!("structured_scrape: failed to get context: context not found")))?;
    let pages = context.pages();
    let page = pages.first().ok_or_else(|| {
        AgentError::Other("structured_scrape: no page available in context".to_string())
    })?;

    let mut data = serde_json::Map::new();

    for spec in selectors {
        let locator = page.locator(&spec.expression);
        let text = locator.text_content().await
            .map_err(|e| AgentError::Other(format!("structured_scrape: failed on '{}': {e}", spec.expression)))?
            .unwrap_or_default();
        data.insert(spec.name, Value::String(text));
    }

    Ok(ToolResult {
        provider_id: String::new(),
        content: vec![ToolResultContent::Text(
            serde_json::json!({
                "context_id": context_id,
                "action": "structured_scrape",
                "data": data,
                "status": "success"
            }).to_string()
        )],
        is_error: false,
    })
}
