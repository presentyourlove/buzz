use crate::{
    client::BuzzClient,
    error::CliError,
    validate::{generate_d_tag, parse_timestamp_or_duration, validate_not_before},
    RemindersCmd,
};
use buzz_core::kind::KIND_EVENT_REMINDER;
use nostr::{EventBuilder, JsonUtil, Kind, Tag};
use rand::Rng;
use serde_json::json;
use uuid::Uuid;

/// Dispatch reminders subcommands.
pub async fn dispatch(sub: RemindersCmd, client: &BuzzClient) -> Result<(), CliError> {
    match sub {
        RemindersCmd::Create { content, at, channel, event_id, preview, author_pubkey, dry_run } => {
            cmd_create(content, at, channel, event_id, preview, author_pubkey, dry_run, client).await
        }
        RemindersCmd::List { limit } => cmd_list(limit, client).await,
        RemindersCmd::Cancel { d_tag } => cmd_cancel(d_tag, client).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn cmd_create(
    content: String,
    at: String,
    channel: Option<String>,
    event_id: Option<String>,
    preview: Option<String>,
    author_pubkey: Option<String>,
    dry_run: bool,
    client: &BuzzClient,
) -> Result<(), CliError> {
    // Parse and validate timestamp
    let not_before = parse_timestamp_or_duration(&at)?;
    let not_before = validate_not_before(not_before)?;

    // Validate optional channel UUID
    let channel_id = if let Some(ch) = channel {
        Some(Uuid::parse_str(&ch).map_err(|_| CliError::Usage("invalid channel UUID".into()))?)
    } else {
        None
    };

    // Validate optional event_id (64-char hex)
    let event_id = if let Some(eid) = event_id {
        if eid.len() != 64 || !eid.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CliError::Usage("event-id must be 64 hex characters".into()));
        }
        Some(eid)
    } else {
        None
    };

    // Validate optional author_pubkey (64-char hex)
    let author_pubkey = if let Some(apk) = author_pubkey {
        if apk.len() != 64 || !apk.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CliError::Usage("author-pubkey must be 64 hex characters".into()));
        }
        Some(apk)
    } else {
        None
    };

    // Generate d-tag with 128-bit entropy
    let d_tag = generate_d_tag();

    // Build reminder content JSON (will be NIP-44 encrypted)
    let mut target = json!({});
    if let Some(ref ch) = channel_id {
        target["channelId"] = json!(ch.to_string());
    }
    if let Some(ref eid) = event_id {
        target["eventId"] = json!(eid);
    }
    if let Some(ref p) = preview {
        target["preview"] = json!(p);
    }
    if let Some(ref apk) = author_pubkey {
        target["authorPubkey"] = json!(apk);
    }
    // If target is empty, omit it entirely (note-only reminder per NIP-ER)
    let target = if target.as_object().map(|o| o.is_empty()).unwrap_or(false) {
        None
    } else {
        Some(target)
    };

    let content_json = json!({
        "target": target,
        "status": "pending",
        "note": content,
    });

    // Encrypt content to self via NIP-44 v2
    let ciphertext = client.nip44_encrypt_to_self(&content_json.to_string())?;

    // Build tags
    let tags = vec![
        Tag::parse(["d", &d_tag]).map_err(|e| CliError::Other(format!("d tag error: {e}")))?,
        Tag::parse(["not_before", &not_before.to_string()])
            .map_err(|e| CliError::Other(format!("not_before tag error: {e}")))?,
        Tag::parse(["alt", "Encrypted reminder"])
            .map_err(|e| CliError::Other(format!("alt tag error: {e}")))?,
    ];

    // Build event
    let keys = client.keys().ok_or_else(|| CliError::Auth("local keys required for signing".into()))?;
    let event = EventBuilder::new(Kind::Custom(KIND_EVENT_REMINDER as u16), ciphertext)
        .tags(tags)
        .sign_with_keys(keys)
        .map_err(|e| CliError::Other(format!("sign event failed: {e}")))?;

    if dry_run {
        println!("{}", event.as_json());
        return Ok(());
    }

    // Publish
    let response = client
        .submit_event(event)
        .await
        .map_err(|e| CliError::Other(format!("publish failed: {e}")))?;

    // Parse relay response
    let response_json: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| CliError::Other(format!("relay response not JSON: {e} ({response})")))?;

    let accepted = response_json
        .get("accepted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !accepted {
        let msg = response_json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(CliError::Other(format!("relay rejected: {msg}")));
    }

    if response_json
        .get("message")
        .and_then(|v| v.as_str())
        .map(|m| m == "duplicate" || m.starts_with("duplicate:"))
        .unwrap_or(false)
    {
        return Err(CliError::Conflict(
            "reminder with this d-tag already exists (duplicate)".into(),
        ));
    }

    println!("Reminder created successfully");
    println!("  d-tag: {d_tag}");
    println!("  triggers at: {} (unix: {not_before})", chrono::DateTime::from_timestamp(not_before, 0).map(|dt| dt.to_rfc3339()).unwrap_or_else(|| "invalid".into()));
    if let Some(ch) = channel_id {
        println!("  channel: {ch}");
    }
    if let Some(eid) = &event_id {
        println!("  event-id: {eid}");
    }
    if let Some(p) = &preview {
        println!("  preview: {p}");
    }
    if let Some(apk) = &author_pubkey {
        println!("  author-pubkey: {apk}");
    }
    println!("  note: {content}");

    Ok(())
}

async fn cmd_list(limit: u32, client: &BuzzClient) -> Result<(), CliError> {
    let keys = client.keys().ok_or_else(|| CliError::Auth("local keys required".into()))?;
    let pubkey = keys.public_key().to_hex();

    // Build filter for kind:30300 authored by us
    let filter = json!({
        "kinds": [KIND_EVENT_REMINDER],
        "authors": [pubkey],
        "limit": limit,
    });

    // Query relay
    let raw = client.query(&filter).await.map_err(|e| CliError::Other(format!("query failed: {e}")))?;
    let events: Vec<serde_json::Value> = serde_json::from_str(&raw)
        .map_err(|e| CliError::Other(format!("failed to parse query response: {e} ({raw})")))?;

    if events.is_empty() {
        println!("No reminders found.");
        return Ok(());
    }

    // Decrypt and display each
    for event in events {
        let d_tag = event
            .get("tags")
            .and_then(|t| t.as_array())
            .and_then(|tags| tags.iter().find(|t| t.as_array().map(|a| a.first().and_then(|v| v.as_str()) == Some("d")).unwrap_or(false)))
            .and_then(|t| t.as_array().and_then(|a| a.get(1)).and_then(|v| v.as_str()))
            .unwrap_or("?")
            .to_string();

        let not_before = event
            .get("tags")
            .and_then(|t| t.as_array())
            .and_then(|tags| tags.iter().find(|t| t.as_array().map(|a| a.first().and_then(|v| v.as_str()) == Some("not_before")).unwrap_or(false)))
            .and_then(|t| t.as_array().and_then(|a| a.get(1)).and_then(|v| v.as_str()))
            .and_then(|s| s.parse::<i64>().ok());

        let plaintext = match client.nip44_decrypt_from_self(event.get("content").and_then(|v| v.as_str()).unwrap_or("")) {
            Ok(p) => p,
            Err(_) => {
                println!("  {d_tag} [failed to decrypt]");
                continue;
            }
        };

        let content: serde_json::Value = match serde_json::from_str(&plaintext) {
            Ok(v) => v,
            Err(_) => {
                println!("  {d_tag} [malformed content]");
                continue;
            }
        };

        let status = content.get("status").and_then(|v| v.as_str()).unwrap_or("?");
        let note = content.get("note").and_then(|v| v.as_str()).unwrap_or("");
        let target = content.get("target");
        let channel_id = target.and_then(|t| t.get("channelId")).and_then(|v| v.as_str());
        let event_id = target.and_then(|t| t.get("eventId")).and_then(|v| v.as_str());
        let preview = target.and_then(|t| t.get("preview")).and_then(|v| v.as_str());
        let author_pubkey = target.and_then(|t| t.get("authorPubkey")).and_then(|v| v.as_str());

        let time_str = not_before
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "?".into());

        println!(
            "  {d_tag}  [{status}]  at: {time_str}  note: {note}{}{}{}{}",
            channel_id.map(|c| format!("  channel: {c}")).unwrap_or_default(),
            event_id.map(|e| format!("  event-id: {e}")).unwrap_or_default(),
            preview.map(|p| format!("  preview: {p}")).unwrap_or_default(),
            author_pubkey.map(|a| format!("  author-pubkey: {a}")).unwrap_or_default()
        );
    }

    Ok(())
}

async fn cmd_cancel(d_tag: String, client: &BuzzClient) -> Result<(), CliError> {
    // Validate d-tag format (32 hex chars = 128 bits)
    if d_tag.len() != 32 || !d_tag.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CliError::Usage("d-tag must be 32 hex characters (128-bit entropy)".into()));
    }

    // Fetch the existing reminder to get its content and created_at
    let keys = client.keys().ok_or_else(|| CliError::Auth("local keys required".into()))?;
    let pubkey = keys.public_key().to_hex();
    let filter = json!({
        "kinds": [KIND_EVENT_REMINDER],
        "authors": [pubkey],
        "limit": 100,
    });

    let raw = client.query(&filter).await.map_err(|e| CliError::Other(format!("query failed: {e}")))?;
    let events: Vec<serde_json::Value> = serde_json::from_str(&raw)
        .map_err(|e| CliError::Other(format!("failed to parse query response: {e} ({raw})")))?;

    let existing = events.iter().find(|e| {
        e.get("tags")
            .and_then(|t| t.as_array())
            .map(|tags| tags.iter().any(|t| {
                t.as_array()
                    .map(|a| a.first().and_then(|v| v.as_str()) == Some("d") && a.get(1).and_then(|v| v.as_str()) == Some(&d_tag))
                    .unwrap_or(false)
            }))
            .unwrap_or(false)
    });

    let (original_content, _created_at) = if let Some(ev) = existing {
        let plaintext = client.nip44_decrypt_from_self(ev.get("content").and_then(|v| v.as_str()).unwrap_or("")).ok();
        let content = plaintext.and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok());
        let created = ev.get("created_at").and_then(|v| v.as_i64()).unwrap_or_else(|| chrono::Utc::now().timestamp());
        (content, created)
    } else {
        return Err(CliError::Usage(format!("reminder with d-tag {} not found", d_tag)));
    };

    // Build cancellation content JSON (NIP-33 parameterized replacement)
    // status=cancelled, omit not_before, add expiration (jittered 30-90 days from now)
    let target = original_content
        .as_ref()
        .and_then(|c| c.get("target").cloned())
        .unwrap_or_else(|| json!({}));

    let cancel_content = json!({
        "target": target,
        "status": "cancelled",
        "note": original_content
            .as_ref()
            .and_then(|c| c.get("note").and_then(|v| v.as_str()))
            .unwrap_or(""),
    });

    // Encrypt cancellation content
    let ciphertext = client.nip44_encrypt_to_self(&cancel_content.to_string())?;

    // Jittered expiration: 30-90 days from now
    let now = chrono::Utc::now().timestamp();
    let expiration = now + (30 * 86400) + (rand::rng().next_u64() % (60 * 86400)) as i64;

    // Build tags for NIP-33 replacement: same d-tag, no not_before, add expiration
    let tags = vec![
        Tag::parse(["d", &d_tag]).map_err(|e| CliError::Other(format!("d tag error: {e}")))?,
        Tag::parse(["expiration", &expiration.to_string()])
            .map_err(|e| CliError::Other(format!("expiration tag error: {e}")))?,
        Tag::parse(["alt", "Encrypted reminder"])
            .map_err(|e| CliError::Other(format!("alt tag error: {e}")))?,
    ];

    // Build replacement event (same kind, same d-tag)
    let keys = client.keys().ok_or_else(|| CliError::Auth("local keys required".into()))?;
    let event = EventBuilder::new(Kind::Custom(KIND_EVENT_REMINDER as u16), ciphertext)
        .tags(tags)
        .sign_with_keys(keys)
        .map_err(|e| CliError::Other(format!("sign event failed: {e}")))?;

    // Publish
    let response = client
        .submit_event(event)
        .await
        .map_err(|e| CliError::Other(format!("publish failed: {e}")))?;

    // Parse relay response
    let response_json: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| CliError::Other(format!("relay response not JSON: {e} ({response})")))?;

    let accepted = response_json
        .get("accepted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !accepted {
        let msg = response_json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(CliError::Other(format!("relay rejected: {msg}")));
    }

    println!("Reminder cancelled successfully");
    println!("  d-tag: {d_tag}");

    Ok(())
}
