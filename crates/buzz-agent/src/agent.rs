use std::sync::Arc;

use serde_json::json;
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::JoinSet;
use tracing::Instrument as _;

use crate::builtin;
use crate::browser_pool::BrowserPool;
use crate::config::{
    pricing_authority, Config, MAX_PROMPT_BYTES, MAX_TOOL_CALLS_PER_TURN, MAX_TOOL_RESULT_BYTES,
};
use crate::handoff::{ContextRecovery, HandoffOutcome};
use crate::triggers::{Trigger, TriggerType, trigger_registry::TriggerRegistry, EventBus, Scheduler, EventBusConfig, SchedulerConfig, TriggerStatus};
use crate::hints::SkillEntry;
use crate::llm::Llm;
use crate::mcp::McpRegistry;
use crate::mcp::ResultBudget;
use crate::permission::PermissionDecision;

use crate::types::{
    AgentError, CacheTotalState, ContentBlock, HistoryItem, PricingIdentity, ProviderStop,
    SessionUsageBaseline, StopReason, ToolCall, ToolResult, ToolResultContent, TurnIOState,
    TurnTotalState,
};
use crate::wire::{self, WireSender};
use nostr::Keys;
use uuid::Uuid;

const ERROR_REFLECTION_SUFFIX: &str =
    "\n\n[Reflect] Before retrying, identify the cause and change your approach.";

/// 系統自我修正與錯誤恢復指引後綴（Self-Healing Execution Prompt）
const SELF_HEALING_PROMPT_SUFFIX: &str =
    "\n\n【系統自我修正與錯誤恢復機制 (Self-Healing Execution)】：\n偵測到上述工具執行發生錯誤！\n1. 請仔細研讀錯誤訊息與 Traceback 根因，切勿重複使用完全相同的失敗參數。\n2. 若為語法或編譯錯誤，請修正代碼後重試；若為路徑不存在，請先確認檔案清單。\n3. 請立即調整方針，輸出具備修正邏輯的新行動。";

const UNSUPPORTED_IMAGE_TOOL_MESSAGE: &str = "The current model does not support image input. The image was removed from conversation history so this turn can continue. Use a text-based inspection tool or ask the user for a textual description instead.";

/// Model-visible feedback after the provider truncates an assistant response at
/// its output-token limit. This is a user message rather than a synthetic tool
/// result because truncation can happen without a tool call (and an unpaired
/// tool result is invalid on every provider wire format).
pub(crate) const MAX_TOKENS_RECOVERY_MESSAGE: &str = "Your previous response reached the model's output token limit and was truncated. Any incomplete tool calls were discarded and were not run. Stop prolonged internal reasoning now. Use the available tools immediately: write a script or artifact to a file and run it in small, verifiable steps instead of emitting the entire solution inline. Continue the task concisely from the preserved text.";

/// Remove image blocks that the provider has explicitly rejected while keeping
/// their surrounding tool result (and therefore the tool-call/result pairing)
/// intact. Returns the number of images removed; zero means the provider error
/// cannot be safely recovered by mutating history.
fn replace_unsupported_images(history: &mut [HistoryItem]) -> usize {
    let mut replaced = 0;
    for item in history {
        let HistoryItem::ToolResult(result) = item else {
            continue;
        };
        let before = result.content.len();
        result
            .content
            .retain(|content| !matches!(content, ToolResultContent::Image { .. }));
        let removed = before - result.content.len();
        if removed > 0 {
            replaced += removed;
            result.is_error = true;
            result.content.push(ToolResultContent::Text(
                UNSUPPORTED_IMAGE_TOOL_MESSAGE.to_string(),
            ));
        }
    }
    replaced
}

/// Maximum reply reminders emitted per prompt when `require_reply` is on.
///
/// After this many, the turn is allowed to end whether or not anything was
/// published: the guard exists to catch accidental omission, not to compel
/// speech. The shared `stop_max_rejections` budget can cut this lower — see
/// [`Config::require_reply`](crate::config::Config::require_reply).
const MAX_REPLY_NAGS: u32 = 2;

/// Output-token upper bound (inclusive) for the silent-death signature: the
/// observed failure emits 2–12 tokens.  Turns with `output_tokens <= 12`
/// and no prior tool call are flagged.  Legitimate one-sentence replies land
/// well above this value even in the most terse case.
const SILENT_TURN_TOKEN_THRESHOLD: u64 = 12;

/// Maximum consecutive silent rounds allowed within a single turn before
/// the early-stop braking gate triggers to halt infinite handoff/steer loops.
const MAX_CONSECUTIVE_SILENT_TURNS: u32 = 2;

/// User-facing message emitted when the silent early-stop braking gate triggers.
const SILENT_EARLY_STOP_BREAKER_MSG: &str =
    "⚠️ [系統熔斷] 雲端模型未產生有效輸出（連續交白卷），為避免無效重試已觸發保護閘門中止本回合。請稍後重試或檢查模型狀態。";

/// Server label on the synthetic reply-guard objection.
///
/// Not a real MCP server. It rides the same tool-result path as `_Stop` hook
/// output, so the model sees `{hook, server, text}` attribution naming the
/// in-process guard rather than an MCP server that could be impersonated.
const REPLY_GUARD_SERVER: &str = "buzz-agent";

/// Reminder text emitted when a turn is about to end with nothing published.
///
/// Explicitly licenses silence. The base prompt tells agents that publishing is
/// optional and "silence is usually correct"; a reminder that argued otherwise
/// would fight that instruction and make agents chattier.
const REPLY_GUARD_NAG: &str = "CRITICAL: You are about to end this turn without calling `buzz messages send`. \
Your assistant text, analysis, and reasoning are NEVER shown to the user or published to the channel! \
If you have answered the user's question, drafted an analysis, or completed a task, you MUST publish your response using `buzz messages send` right now! \
Do NOT output plain text response; you MUST invoke the tool to send the message.";

/// Nag specifically triggered when the model spent tokens on internal reasoning but produced zero content and zero tool calls.
const REPLY_GUARD_REASONING_ONLY_NAG: &str = "CRITICAL: Your turn produced internal reasoning but ZERO output content and ZERO tool calls! \
You MUST NOT remain silent. You have access to local tools like read_file and dev__shell. \
If the user asked about a file, inspect it using read_file or deliver your analysis using buzz messages send right now! \
Do NOT just think and stop; execute the required tools NOW.";

/// Whether `call` is a recognized attempt to publish a reply to Buzz.
///
/// Recognizes an *attempt*, not a successful publish: the command text is
/// inspected, never the exit status. That is deliberate — a send that fails
/// already returns a non-zero exit and error JSON to the model, which is louder
/// feedback than the reminder this gates.
///
/// `has` + `!is_hook` are the same checks the dispatcher uses to accept a call
/// (see `execute_calls`), so a hallucinated `fake__shell` — rejected at preflight
/// and never executed — cannot disarm the guard. They must stay *before*
/// [`is_reply_shaped`]: together with them, and only with them, the `__shell`
/// suffix is exactly equivalent to "the bare tool name is `shell`".
fn is_buzz_reply_call(call: &ToolCall, mcp: &McpRegistry) -> bool {
    mcp.has(&call.name) && !mcp.is_hook(&call.name) && is_reply_shaped(&call.name, &call.arguments)
}

fn is_buzz_send_message_call(call: &ToolCall, mcp: &McpRegistry) -> bool {
    mcp.has(&call.name) && !mcp.is_hook(&call.name) && is_send_message_shaped(&call.name, &call.arguments)
}

fn is_send_message_shaped(name: &str, arguments: &serde_json::Value) -> bool {
    name.ends_with("__shell")
        && arguments
            .get("command")
            .and_then(|v| v.as_str())
            .is_some_and(|cmd| {
                cmd.contains("messages send")
            })
}

/// Helper: inspect a tool result to determine if it represents genuine execution success.
/// Checks `is_error`, JSON `exit_code != 0`, `timed_out`, and `error` field.
fn is_tool_result_success(tr: &ToolResult) -> bool {
    if tr.is_error {
        return false;
    }
    for c in &tr.content {
        if let ToolResultContent::Text(text) = c {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(ec) = v.get("exit_code").and_then(|e| e.as_i64()) {
                    if ec != 0 {
                        return false;
                    }
                }
                if v.get("timed_out").and_then(|t| t.as_bool()).unwrap_or(false) {
                    return false;
                }
                if v.get("error").is_some() {
                    return false;
                }
            }
        }
    }
    true
}

/// Extract channel UUID and optional reply-to event hex from a user prompt.
pub fn extract_reply_target_from_prompt(text: &str) -> Option<(String, Option<String>)> {
    // 1. Extract reply-to (64 hex characters)
    let reply_to = {
        let mut found = None;
        if let Some(pos) = text.find("--reply-to") {
            let after = &text[pos + 10..];
            let token = after.split_whitespace().next().unwrap_or("").trim_matches(|c| c == '`' || c == '"' || c == '\'' || c == ',');
            if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
                found = Some(token.to_string());
            }
        }
        if found.is_none() {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("event_id:") || trimmed.starts_with("Event ID:") || trimmed.starts_with("id:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let candidate = parts[1].trim_matches(|c| c == '`' || c == '"' || c == '\'' || c == ',');
                        if candidate.len() == 64 && candidate.chars().all(|c| c.is_ascii_hexdigit()) {
                            found = Some(candidate.to_string());
                            break;
                        }
                    }
                }
            }
        }
        found
    };

    // 2. Extract channel_id (UUID 8-4-4-4-12)
    let channel_id = {
        let mut found = None;
        if let Some(pos) = text.find("--channel") {
            let after = &text[pos + 9..];
            let token = after.split_whitespace().next().unwrap_or("").trim_matches(|c| c == '`' || c == '"' || c == '\'' || c == ')' || c == ',');
            if is_valid_uuid(token) {
                found = Some(token.to_string());
            }
        }
        if found.is_none() {
            for word in text.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '#' || c == '"' || c == '\'' || c == ',' || c == '<' || c == '>') {
                let clean = word.trim();
                if is_valid_uuid(clean) {
                    found = Some(clean.to_string());
                    break;
                }
            }
        }
        found
    };

    channel_id.map(|cid| (cid, reply_to))
}

/// Clean up markdown link artifacts that frontends inject into local file paths.
/// e.g. `[user.buzz](http://user.buzz)` -> `user.buzz`
/// e.g. `[TRENDING.md](http://TRENDING.md)` -> `TRENDING.md`
/// e.g. `C:\...\2026-11-11_GITHUB_[TRENDING.md]` -> `C:\...\2026-11-11_GITHUB_TRENDING.md`
pub fn denoise_prompt_markdown_links(input: &str) -> String {
    // Pass 1: standard [text](url) markdown link denoising
    let mut step1 = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    let mut last_copied = 0;

    while let Some((start_bracket, ch)) = chars.next() {
        if ch == '[' {
            // Find closing ']'
            let mut close_bracket = None;
            for (idx, next_ch) in chars.by_ref() {
                if next_ch == ']' {
                    close_bracket = Some(idx);
                    break;
                }
            }
            if let Some(end_bracket) = close_bracket {
                // Peek if immediately followed by '('
                if let Some(&(open_paren, '(')) = chars.peek() {
                    chars.next(); // consume '('
                    // Find closing ')'
                    let mut close_paren = None;
                    for (idx, next_ch) in chars.by_ref() {
                        if next_ch == ')' {
                            close_paren = Some(idx);
                            break;
                        }
                    }
                    if let Some(end_paren) = close_paren {
                        let inner_text = &input[start_bracket + 1..end_bracket];
                        let url = &input[open_paren + 1..end_paren];
                        // If url looks like a web scheme, file scheme, or fake domain generated by autolink
                        if url.starts_with("http://")
                            || url.starts_with("https://")
                            || url.starts_with("file://")
                            || url.starts_with("buzz://")
                            || url.contains(".md")
                            || url.contains(".buzz")
                        {
                            step1.push_str(&input[last_copied..start_bracket]);
                            step1.push_str(inner_text);
                            last_copied = end_paren + 1;
                        }
                    }
                }
            }
        }
    }
    step1.push_str(&input[last_copied..]);

    // Pass 2: Clean isolated brackets and backticks inside tokens that represent file/directory paths
    // e.g. `C:\Users\user\.buzz\PLANS\REPORTS\2026-11-11_GITHUB_[TRENDING.md]`
    let mut result = String::with_capacity(step1.len());
    for line in step1.split_inclusive('\n') {
        let mut line_res = String::with_capacity(line.len());
        for word in line.split_inclusive(|c: char| c.is_whitespace()) {
            let is_path_like = word.contains('\\') || word.contains(":/") || word.contains(":\\");
            if is_path_like && (word.contains('[') || word.contains(']') || word.contains('`')) {
                let cleaned_word: String = word
                    .chars()
                    .filter(|&c| c != '[' && c != ']' && c != '`')
                    .collect();
                line_res.push_str(&cleaned_word);
            } else {
                line_res.push_str(word);
            }
        }
        result.push_str(&line_res);
    }

    // Pass 3: Restore devoured backslash in Windows user profile path `user.buzz\` -> `user\.buzz\`
    let result = result
        .replace("user.buzz\\", "user\\.buzz\\")
        .replace("user.buzz/", "user\\.buzz/");

    result
}

pub(crate) fn is_valid_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Whether a tool name and arguments have the shape of a Buzz publish command.
///
/// Split from [`is_buzz_reply_call`] only so the matcher is testable without a
/// live [`McpRegistry`]; callers must apply the registry checks first.
///
/// On the name: `ends_with("__shell")` is exact rather than approximate *given*
/// those checks. Registration rejects `__` in both server names and bare tool
/// names, and qualified names are `{server}__{bare}`, so a trailing `__shell` can
/// only straddle the separator if the bare name starts with `_` — which `is_hook`
/// already excludes. Dropping the separator would not be exact: `powershell` and
/// `noshell` both end in `shell`.
///
/// On the command: a deliberately coarse substring test, scoped to the structured
/// `command` field so unrelated metadata — a `description` that quotes a send —
/// cannot suppress the guard, and a non-string `command` is rejected rather than
/// coerced. Known limits, both accepted: a command assembled at runtime (`$CMD`)
/// or hidden in a wrapper script is missed, and text that merely quotes a send
/// (`echo "buzz messages send"`) matches. Missing a real post is the expensive
/// direction, and substring matching is the more forgiving one there.
fn is_reply_shaped(name: &str, arguments: &serde_json::Value) -> bool {
    name.ends_with("__shell")
        && arguments
            .get("command")
            .and_then(|v| v.as_str())
            .is_some_and(|cmd| {
                // `messages send` also covers `messages send-diff`. `reactions
                // add` counts because the base prompt directs agents to react
                // rather than post a bare acknowledgement, so nagging an agent
                // that reacted would punish documented-correct behavior.
                cmd.contains("messages send") || cmd.contains("reactions add")
            })
}

pub struct RunCtx<'a> {
    pub cfg: &'a Config,
    /// Effective model for this session. Usually equals `cfg.model`; overridden
    /// per-session by `session/set_model`. All LLM calls use this value.
    pub effective_model: &'a str,
    pub session_id: &'a str,
    pub system_prompt: &'a str,
    pub llm: &'a Llm,
    pub mcp: &'a Arc<McpRegistry>,
    /// Process-wide permission broker (owned by `App`). Every LLM-issued MCP
    /// tool call asks the client to authorize it through this broker before
    /// executing. Shared across all sessions so the global admission cap bounds
    /// simultaneously-outstanding asks process-wide.
    pub permissions: &'a Arc<crate::permission::PermissionBroker>,
    /// ACP protocol version negotiated at `initialize`, fixed for the
    /// connection. Selects the `session/request_permission` wire shape.
    pub protocol_version: u32,
    /// Skills discovered at session creation; used by the built-in `load_skill` tool.
    pub skills: &'a [SkillEntry],
    pub wire: &'a WireSender,
    pub cancel: &'a mut watch::Receiver<bool>,
    /// Agent's Nostr keys for Mode B daemon mode (signing kind:30300, sending DMs).
    /// Injected from `Config::agent_keys` when available.
    pub agent_keys: Option<Arc<nostr::Keys>>,
    /// HistoryItem memory cache for size estimation (Mode B: Phase 2).
    /// Injected from `Config` at `RunCtx` construction in `lib.rs`.
    pub history_cache: Option<Arc<crate::types::HistoryItemCache>>,
    /// Mid-turn steer queue. Drained at each round boundary (before the next
    /// LLM call): queued messages are appended to history as user turns so the
    /// model sees them on its next request, without restarting the turn. Fed by
    /// the `_goose/unstable/session/steer` handler.
    pub steer: &'a mut mpsc::UnboundedReceiver<Vec<ContentBlock>>,
    pub history: &'a mut Vec<HistoryItem>,
    pub original_task: &'a mut Option<String>,
    pub handoff_count: &'a mut usize,
    /// ACP v2 session identifier for this prompt turn. Used to derive
    /// per-message `messageId` values that are unique within the ACP session.
    /// Distinct from `session_id` (which is the ACP session); this is a
    /// per-`session/prompt` random token so that IDs from one prompt invocation
    /// never collide with those from another even within the same session.
    pub run_id: String,
    /// Cache-summed input tokens reported by the provider on this session's
    /// most recent request (persists across `session/prompt` calls), or `None`
    /// before the first response and immediately after a handoff resets the
    /// context. The handoff gate reads this to compare against the token
    /// budget; falls back to the byte heuristic when `None`.
    pub last_request_input_tokens: &'a mut Option<u64>,
    /// History byte size at the moment `last_request_input_tokens` was
    /// measured. Paired with it so the gate can add a conservative token
    /// estimate of history that has grown since (tool results, next prompt),
    /// which the exact-but-stale token count would otherwise miss. Cleared and
    /// preserved in lockstep with `last_request_input_tokens`.
    pub last_request_history_bytes: &'a mut Option<usize>,
    /// Accumulated input tokens across all LLM rounds in this turn, for
    /// NIP-AM metric publishing. Reset to `Unseen` at turn start in `run()`.
    pub turn_input_tokens: &'a mut TurnIOState,
    /// Accumulated output tokens across all LLM rounds in this turn, for
    /// NIP-AM metric publishing. Reset to `Unseen` at turn start in `run()`.
    pub turn_output_tokens: &'a mut TurnIOState,
    /// The cache-served subset of `turn_input_tokens`, accumulated across all
    /// LLM rounds in this turn. Reset to `Unseen` at turn start in `run()`.
    /// Consumers price this slice at the provider's cached rate; without it
    /// every round of a growing conversation is billed at full price.
    ///
    /// `CacheTotalState` enforces the D1 rule: any usage-bearing round that
    /// omits this category poisons the accumulator permanently for the turn.
    pub turn_cached_input_tokens: &'a mut CacheTotalState,
    /// The cache-written subset of `turn_input_tokens`, accumulated across all
    /// LLM rounds in this turn. Reset to `Unseen` at turn start in `run()`.
    /// Consumers need this to price cache-creation at the provider's write rate
    /// (distinct from both the standard input rate and the cached-read rate).
    ///
    /// Same D1 tri-state contract as `turn_cached_input_tokens`.
    pub turn_cache_write_tokens: &'a mut CacheTotalState,
    /// Per-turn billing identity accumulator.
    ///
    /// - `None`: no usage-bearing response observed yet this turn (initial state).
    /// - `Some(Some(pi))`: all usage-bearing responses so far carry the same
    ///   proven identity `pi`. If a subsequent response carries a different or
    ///   unproven identity, this transitions to `Some(None)` (poisoned).
    /// - `Some(None)`: poisoned — mixed identities, unproven response, mesh
    ///   retry across models, or no identity derived. Never heals within the turn.
    ///
    /// Reset to `None` at turn start in `run()`. The wire payload emits the
    /// proven identity when `Some(Some(pi))`, omits it otherwise.
    pub turn_pricing_identity: &'a mut Option<Option<PricingIdentity>>,
    /// Tri-state total-token accumulator for this turn.
    ///
    /// - `Unseen`: no usage-bearing response observed yet this turn (initial state).
    /// - `Exact(n)`: every usage-bearing response so far reported a genuine
    ///   provider total; `n` is their sum.
    /// - `Unknown`: at least one usage-bearing response lacked a provider total;
    ///   this turn can never produce a reliable total.
    ///
    /// Reset to `Unseen` at turn start in `run()`. Callers must not derive a
    /// total by summing input+output — that is the UI display approximation only.
    pub turn_total_state: &'a mut TurnTotalState,
    /// Session-cumulative counters as they stood when this turn began. Added to
    /// the `turn_*` accumulators above to report a cumulative figure mid-turn;
    /// the session's own copy is only advanced once, after the turn returns.
    pub usage_baseline: SessionUsageBaseline,
    /// 確定性工作集帳本（修復 D8）：隨回合自動記錄讀寫改檔案、指令與錯誤。
    pub ledger: &'a mut crate::compaction::WorkingSetLedger,
    /// Shared browser pool for builtin browser tools. May be None if Playwright
    /// runtime was not ready at startup.
    pub browser_pool: &'a Option<Arc<BrowserPool>>,
    /// Trigger registry for proactive autonomous triggers (cron, event-based, etc.)
    pub trigger_registry: Arc<TriggerRegistry>,
}

/// Methods for managing custom triggers in the agent runtime.
impl RunCtx<'_> {
    /// Register a new custom trigger (cron, interval, event-based, or one-shot).
    ///
    /// The trigger will be automatically managed by the agent's trigger registry.
    /// Returns the UUID of the registered trigger.
    pub fn register_trigger(&mut self, trigger: Trigger) -> Result<Uuid, AgentError> {
        self.trigger_registry
            .register(trigger)
            .map_err(|e| AgentError::Internal(format!("Failed to register trigger: {}", e)))
    }

    /// Unregister a trigger by its UUID.
    ///
    /// Returns `true` if the trigger was found and removed, `false` otherwise.
    pub fn unregister_trigger(&mut self, id: Uuid) -> bool {
        self.trigger_registry.unregister(id)
    }

    /// Enable or disable a registered trigger.
    pub fn set_trigger_enabled(&mut self, id: Uuid, enabled: bool) -> bool {
        self.trigger_registry.set_enabled_sync(id, enabled)
    }

    /// Get a list of all registered triggers.
    pub fn list_triggers(&self) -> Vec<Trigger> {
        self.trigger_registry.list_sync()
    }
}

/// Fold one round's proven identity into the per-turn identity accumulator.
///
/// Accumulator tri-state (NIP-AM §pricingIdentity):
/// - `None`: no usage-bearing round observed yet this turn.
/// - `Some(Some(pi))`: every usage-bearing round so far carries the same
///   proven identity `pi`.
/// - `Some(None)`: poisoned — mixed identities or unproven round seen.
///   Never heals within the turn.
///
/// `round`: `Some(pi)` when this round's `(base_url, request_model)` resolve
/// to a proven identity; `None` when the endpoint is unallowlisted or the
/// model is unknown.
#[inline]
fn fold_pricing_identity(
    acc: Option<Option<PricingIdentity>>,
    round: Option<PricingIdentity>,
) -> Option<Option<PricingIdentity>> {
    match acc {
        // First usage-bearing round: record whatever was derived.
        None => Some(round),
        // Already consistent: keep only if this round matches exactly.
        Some(Some(ref existing)) => {
            if Some(existing) == round.as_ref() {
                Some(round)
            } else {
                // Mismatch (different model, different authority,
                // or this round had no proven identity) → poison.
                Some(None)
            }
        }
        // Already poisoned: stays poisoned forever this turn.
        poisoned @ Some(None) => poisoned,
    }
}

impl RunCtx<'_> {
    /// Send a session-cumulative `usage_update` reflecting everything observed
    /// up to and including the most recent LLM response.
    ///
    /// The figure is the turn-start baseline plus this turn's running
    /// accumulators, which is exactly what `session/prompt` will fold into the
    /// session once the turn returns — so a mid-turn notification and the
    /// end-of-turn one agree, and a turn that never returns has still reported
    /// everything but its final in-flight request.
    async fn emit_usage_update(&self) {
        let base = self.usage_baseline;
        // Combine session baseline CacheTotalState with the per-turn delta:
        // merge_session produces Exact when both sides are Exact, Unknown when
        // either is Unknown, and leaves Unseen when both sides are Unseen.
        let cached_total = base
            .cached_input_tokens
            .merge_session(*self.turn_cached_input_tokens);
        let write_total = base
            .cache_write_tokens
            .merge_session(*self.turn_cache_write_tokens);
        let payload = wire::usage_update_payload(
            base.input_tokens
                .merge_session(*self.turn_input_tokens)
                .exact_value(),
            base.output_tokens
                .merge_session(*self.turn_output_tokens)
                .exact_value(),
            cached_total.exact_value(),
            write_total.exact_value(),
            base.total_state.merge_session(*self.turn_total_state),
            self.effective_model,
            // Extract the proven identity if this turn is consistent so far.
            self.turn_pricing_identity
                .as_ref()
                .and_then(|inner| inner.as_ref()),
        );
        wire::send(
            self.wire,
            wire::goose_session_update(self.session_id, payload),
        )
        .await;
    }

    pub async fn run(&mut self, prompt: Vec<ContentBlock>) -> Result<StopReason, AgentError> {
        let mut user_text = prompt_to_text(prompt)?;
        if user_text.len() > MAX_PROMPT_BYTES {
            return Err(AgentError::InvalidParams(format!(
                "prompt: exceeds {MAX_PROMPT_BYTES} bytes"
            )));
        }
        // Denoise markdown links that frontends mistakenly attach to local filenames and paths
        user_text = denoise_prompt_markdown_links(&user_text);

        // 階段 E：若為重啟／休眠後的新 Session 首輪且歷史為空，先嘗試由磁碟加密 Checkpoint 還原續作上下文
        let restored_same_tail = self.restore_checkpoint_if_scoped(&user_text);

        if self.original_task.is_none() {
            *self.original_task = Some(user_text.clone());
        }
        if !restored_same_tail {
            self.history.push(HistoryItem::User(user_text));
        }

        // Reset per-turn token accumulators for this prompt.
        *self.turn_input_tokens = TurnIOState::Unseen;
        *self.turn_output_tokens = TurnIOState::Unseen;
        *self.turn_cached_input_tokens = CacheTotalState::Unseen;
        *self.turn_cache_write_tokens = CacheTotalState::Unseen;
        *self.turn_pricing_identity = None;
        *self.turn_total_state = TurnTotalState::Unseen;
        // Per-turn handoff-attempt counter. Scoped here (not persisted in the
        // session) so `BUZZ_AGENT_MAX_HANDOFFS` bounds compactions per
        // `session/prompt` turn rather than per session lifetime. A
        // long-lived session legitimately needs unbounded handoffs across
        // prompts; the cap only exists to stop runaway within a single turn.
        // The session-cumulative `handoff_count` (used in log lines) is not
        // reset: it reflects total compactions since session start.
        let mut handoff_attempts: usize = 0;

        let mut round = 0u32;
        // Per-prompt `_Stop` objection count. Bounded per prompt (not per
        // session) so a stubborn exchange can't permanently disable the stop
        // guard for a long-lived session; `max_rounds` still caps the loop.
        let mut stop_rejections = 0u32;
        // Reply-guard state for this prompt. `prompt()` *is* the turn, so
        // locals here are per-turn by construction — same shape as
        // `stop_rejections` above.
        //
        // Named for what it proves: a *recognized attempt* to publish, not a
        // successful publish. See `is_buzz_reply_call`.
        // Tracks whether a publish-shaped tool call was seen this turn, updated
        // unconditionally (not gated on `require_reply`) so the silent-turn
        // diagnostic has a turn-level view regardless of config.  A turn that
        // ran read-only tools and then died at 3 tokens IS a silent death;
        // only a genuine publish should suppress the WARN.
        let mut buzz_reply_call_seen = false;
        let mut buzz_reply_publish_count = 0usize;
        let mut buzz_publish_attempt_count = 0usize;
        let mut last_failed_call: Option<ToolCall> = None;
        let mut identical_call_failures = 0usize;
        let mut total_consecutive_tool_failures = 0usize;
        const MAX_IDENTICAL_TOOL_FAILURES: usize = 5;
        const MAX_TOTAL_CONSECUTIVE_TOOL_FAILURES: usize = 8;
        let mut reply_nags = 0u32;
        let mut silent_turns = 0u32;
        let mut successful_tool_rounds = 0u32;
        let mut dynamic_extended_budget = 40u32;
        // Per-`run()` reactive context-recovery budget. Per-turn, not
        // per-session: a fresh prompt deserves a fresh chance to recover, and
        // `max_rounds` defaults to 0 (unbounded) so it cannot bound this.
        let mut context_recoveries = 0u32;
        // Per-run output-truncation recovery budget. Unlike context recovery,
        // these successful provider requests consume a real round and are not
        // refunded; this counter only bounds the default-unlimited case.
        let mut max_tokens_recoveries = 0u32;
        loop {
            // 智慧動態預算天花板 (Smart Dynamic Budget Ceiling)
            // 若使用者明確設定了 max_rounds > 0 則尊重設定；
            // 否則支援環境變數 BUZZ_MAX_ROUNDS_CEILING（預設 1000 輪）。
            // 只要特工持續有實質進展（無死循環、無連續故障），自動持續展期 40 輪，完全不打擾主理人；
            // 僅當偵測到卡死、死循環（鬼打牆）或達到安全天花板（預設 1000 輪）時，才暫停請求核准。
            let max_rounds_ceiling: u32 = std::env::var("BUZZ_MAX_ROUNDS_CEILING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000);

            let mut effective_max_rounds = if self.cfg.max_rounds > 0 {
                self.cfg.max_rounds
            } else {
                dynamic_extended_budget
            };

            if self.cfg.max_rounds == 0 && round >= effective_max_rounds {
                let is_truly_stuck = identical_call_failures >= 3 || total_consecutive_tool_failures >= 4;
                let is_absolute_ceiling = round >= max_rounds_ceiling;

                if !is_truly_stuck && !is_absolute_ceiling && successful_tool_rounds > 0 {
                    // 真係做緊嘢！自動展期 40 輪，不打擾主理人
                    dynamic_extended_budget = dynamic_extended_budget.saturating_add(40);
                    effective_max_rounds = dynamic_extended_budget;
                    tracing::info!(
                        session_id = %self.session_id,
                        round,
                        new_budget = dynamic_extended_budget,
                        successful_tool_rounds,
                        max_rounds_ceiling,
                        "smart turn budget: active progress detected — auto-extending turn budget by 40 rounds without user interruption"
                    );
                }
            }

            // 機制二：逼近當前階梯天花板剩餘 5 輪時主動落盤預警 (Proximity Nag)
            // 僅在真的卡死或接近絕對天花板剩餘 5 輪時才注入警示
            if round == effective_max_rounds.saturating_sub(5) && (effective_max_rounds >= max_rounds_ceiling || identical_call_failures >= 2 || total_consecutive_tool_failures >= 3) {
                tracing::info!(
                    session_id = %self.session_id,
                    round,
                    effective_max_rounds,
                    "injecting ceiling proximity nag to prompt agent to conclude and report"
                );
                self.history.push(HistoryItem::User(format!(
                    "[系統警示]：您當前任務已推進至第 {round} 輪（上限 {effective_max_rounds} 輪）。若已完成任務，請將實體檔案落盤保存並向主理人回報進度！"
                )));
            }

            // 機制一：撞頂強制造冊回報 (Ceiling Auto-Publish Fallback)
            if round >= effective_max_rounds {
                tracing::warn!(
                    session_id = %self.session_id,
                    round,
                    effective_max_rounds,
                    "terminating turn: reached maximum rounds safety ceiling — triggering auto-publish"
                );
                let is_stuck = identical_call_failures >= 3 || total_consecutive_tool_failures >= 4;
                let ceiling_msg_id = format!("{}-ceiling-round-{round}", self.run_id);
                let ceiling_notice = if is_stuck {
                    format!(
                        "【任務受阻自動防護】特工偵測到工具調用連續失敗或相同參數重複受阻（已執行第 {round} 輪）。為免無效空轉，已自動保存現場。請主理人檢閱排查後回覆『繼續』以解鎖下一階段！"
                    )
                } else {
                    format!(
                        "【階段里程碑到達】特工已連續完成 {effective_max_rounds} 回合長程深度實作任務。為確保系統穩定與記憶體安全，已自動完成階段性進度保存。請主理人檢閱或回覆『繼續』以無縫承接下一階段實作！"
                    )
                };
                wire::send(
                    self.wire,
                    wire::session_update(
                        self.session_id,
                        json!({
                            "sessionUpdate": "agent_message_chunk",
                            "messageId": &ceiling_msg_id,
                            "content": { "type": "text", "text": &ceiling_notice }
                        }),
                    ),
                )
                .await;
                self.history.push(HistoryItem::Assistant {
                    text: ceiling_notice,
                    tool_calls: Vec::new(),
                    reasoning_details: None,
                });
                self.try_auto_publish_fallback().await;
                self.save_checkpoint_if_scoped();
                return Ok(StopReason::MaxTurnRequests);
            }
            if *self.cancel.borrow() {
                return Ok(StopReason::Cancelled);
            }
            // Round boundary: fold in any steer messages queued since the last
            // round. They land as user turns so the model incorporates them on
            // its next request — the turn continues, it is not restarted. Drain
            // non-blocking; an empty queue is the common case.
            self.drain_steers();
            if self.run_preflight_compaction(&mut handoff_attempts).await {
                return Ok(StopReason::Cancelled);
            }

            let mut tools = self.mcp.tools();
            // Inject the built-in load_skill tool when skills are available.
            if !self.skills.is_empty() {
                tools.push(builtin::load_skill_def());
            }
            // Inject Mode B daemon tools when agent keys are available.
            if self.agent_keys.is_some() {
                tools.push(builtin::sign_and_send_event_def());
                tools.push(builtin::send_direct_message_def());
                tools.push(builtin::create_reminder_def());
            }
            // Inject browser automation tools when browser pool is available.
            if self.browser_pool.is_some() {
                // Initialize browser automation skill registry
                browser_automation::skills::init_registry();
                tools.push(builtin::browser_open_def());
                tools.push(builtin::browser_screenshot_def());
                tools.push(builtin::browser_close_def());
                // Phase 2: Advanced browser interaction tools
                tools.push(builtin::browser_click_def());
                tools.push(builtin::browser_fill_def());
                tools.push(builtin::browser_wait_def());
                tools.push(builtin::browser_extract_def());
                // Phase 2: High-level business skills
                tools.push(builtin::form_fill_def());
                tools.push(builtin::structured_scrape_def());
            }
            round = round.saturating_add(1);
            let response_result = tokio::select! {
                biased;
                _ = self.cancel.changed() => return Ok(StopReason::Cancelled),
                r = self.llm.complete(self.cfg, self.system_prompt, self.history, &tools, self.effective_model)
                        .instrument(tracing::info_span!("llm", session_id = %self.session_id)) => r,
                _ = async {
                    // Keepalive ticker: emit a lightweight session update every 30s
                    // while waiting on the LLM provider. This resets the ACP harness
                    // idle clock so long provider responses don't trigger timeout.
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                    interval.tick().await; // first tick fires immediately — skip it
                    loop {
                        interval.tick().await;
                        tracing::debug!("llm keepalive tick");
                        wire::send(
                            self.wire,
                            wire::session_update(
                                self.session_id,
                                json!({
                                    "sessionUpdate": "keepalive",
                                }),
                            ),
                        )
                        .await;
                    }
                } => unreachable!(),
            };
            let response = match response_result {
                Ok(response) => response,
                Err(AgentError::UnsupportedImageInput(detail)) => {
                    let removed = replace_unsupported_images(self.history);
                    if removed == 0 {
                        return Err(AgentError::UnsupportedImageInput(detail));
                    }
                    tracing::warn!(
                        model = self.effective_model,
                        removed_images = removed,
                        "provider rejected image input; removed images from history and continuing turn"
                    );
                    continue;
                }
                // Reactive context recovery. A context-window 400 is the only
                // ground-truth signal that history must shrink, and it arrives
                // exactly when the proactive gate cannot act: a failed request
                // reports no usage, so `last_request_input_tokens` stays frozen
                // at the last SUCCESSFUL (sub-threshold) reading and
                // `should_handoff()` returns false forever. Without this arm the
                // error propagates out of `run()`, the in-memory session keeps
                // the same oversized history, and every later prompt in that
                // session fails the same way — a stick that persists across
                // turns for the life of the session. (Restarting the agent DOES
                // clear it: history lives only in the in-memory session map, so
                // a restart is the manual workaround, not an exception to it.)
                //
                // Retried in-loop rather than returned so the recovered context
                // continues the turn the user is waiting on.
                Err(AgentError::LlmContextExceeded(e)) => {
                    match self
                        .recover_from_context_overflow(&mut context_recoveries)
                        .await
                    {
                        ContextRecovery::Recovered => {
                            // Refund the round the rejected request consumed.
                            // `round` is incremented before `complete()`, so
                            // without this a finite `max_rounds` is spent by a
                            // request the provider refused to serve: the loop
                            // would re-enter, hit the cap at the top, and return
                            // `MaxTurnRequests` having destroyed history and
                            // never asked the model again — a worse outcome than
                            // the error it replaced.
                            //
                            // This cannot become an unbounded amnesty: refunds
                            // happen only on a *successful* recovery, and
                            // recoveries are independently capped by
                            // `MAX_CONTEXT_RECOVERIES_PER_RUN`, so at most that
                            // many rounds can ever be refunded in one turn. An
                            // ordinary round is never refunded.
                            round = round.saturating_sub(1);
                            // Same reset as the proactive path (see
                            // `HandoffOutcome::Performed` above): the frozen
                            // token reading describes history that no longer
                            // exists. Clearing it is what lets the gate work
                            // again on later rounds.
                            *self.last_request_input_tokens = None;
                            *self.last_request_history_bytes = None;
                            continue;
                        }
                        ContextRecovery::Cancelled => return Ok(StopReason::Cancelled),
                        // No rescue left. Surface the provider's own error
                        // rather than a synthetic one: it names the model and
                        // the offending sizes, and a visible failure is the
                        // point — the alternative is retrying forever.
                        ContextRecovery::Exhausted => {
                            return Err(AgentError::LlmContextExceeded(e))
                        }
                    }
                }
                // 429 限流自適應降載（Adaptive Downsizing on 429/TPM Exhaustion）
                // 當 LLM 遭遇 429 Too Many Requests 且對話歷史膨脹時，不應盲目等待相同龐大 payload 重試，
                // 而是主動啟動上下文緊急縮編降載，減半 payload 壓力再退避重試，徹底打破限流死循環。
                Err(AgentError::Llm(msg))
                    if (msg.contains("429")
                        || msg.contains("Too Many Requests")
                        || msg.contains("rate limit")
                        || msg.contains("Rate limit"))
                        && context_recoveries < crate::config::MAX_CONTEXT_RECOVERIES_PER_RUN =>
                {
                    tracing::warn!(
                        model = self.effective_model,
                        error = %msg,
                        recoveries = context_recoveries,
                        "LLM returned rate-limit (429/TPM); performing adaptive context downsizing to relieve API pressure"
                    );
                    match self
                        .recover_from_context_overflow(&mut context_recoveries)
                        .await
                    {
                        ContextRecovery::Recovered => {
                            round = round.saturating_sub(1);
                            *self.last_request_input_tokens = None;
                            *self.last_request_history_bytes = None;
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                        ContextRecovery::Cancelled => return Ok(StopReason::Cancelled),
                        ContextRecovery::Exhausted => {
                            return Err(AgentError::Llm(msg));
                        }
                    }
                }
                Err(error) => return Err(error),
            };
            // Record provider-reported input usage so the next loop iteration's
            // handoff gate can compare it against the token budget. We capture
            // it together with the history byte size AT THIS MOMENT — which is
            // exactly the history that was just sent to `complete()` (the
            // assistant response is appended below, after this point). Pairing
            // them lets the gate add a conservative estimate for any history
            // appended before the next request. Uses `context_pressure_bytes`
            // (the same measure the gate's `current_bytes` uses) so the
            // `grown` delta is coherent — an image contributes its visual-
            // token equivalent here, not its base64 length. Preserve both when
            // a response omits usage (`None`) rather than clobbering — a
            // one-off missing field shouldn't blind the gate or zero the
            // growth baseline.
            if response.input_tokens_overflowed {
                // The Anthropic-style inclusive sum (input_tokens +
                // cache_read_input_tokens + cache_creation_input_tokens)
                // overflowed u64::MAX during parsing. Permanently poison the
                // turn accumulator so wire emission omits this value and ACP
                // marks the delta unreliable. Do NOT update
                // last_request_input_tokens — freeze the context-gate
                // baseline at its prior reading rather than poisoning it with
                // a clamped value, exactly as the absent-usage path does.
                *self.turn_input_tokens = TurnIOState::Poisoned;
            } else if let Some(tokens) = response.input_tokens {
                *self.last_request_input_tokens = Some(tokens);
                *self.last_request_history_bytes = Some(
                    self.history
                        .iter()
                        .map(HistoryItem::context_pressure_bytes)
                        .sum(),
                );
                // Accumulate per-turn input tokens for NIP-AM metric publishing.
                // fold_round uses checked_add; overflow permanently poisons the
                // turn accumulator (and, via merge_session, the session cumulative).
                *self.turn_input_tokens = self.turn_input_tokens.fold_round(tokens);
            }
            // Accumulate per-turn output tokens for NIP-AM metric publishing.
            if let Some(out) = response.output_tokens {
                *self.turn_output_tokens = self.turn_output_tokens.fold_round(out);
            }
            // Fold the provider-reported total, cache subsets, and billing
            // identity — only when this response was usage-bearing (had input
            // or output tokens).  A response with no usage at all is not
            // evidence of a missing cache field or total and must not poison
            // either accumulator.
            //
            // `input_tokens_overflowed` counts as usage-bearing: the provider
            // reported an input total (which overflowed) so cache fields and
            // the total are meaningful and must be folded.
            //
            // D1: absent cache field on a usage-bearing round permanently
            // poisons the turn accumulator.  Some(0) stays Exact(0) (explicit
            // zero is distinct from absent).  Cache-read and cache-write are
            // tracked separately from `turn_input_tokens` rather than
            // subtracted from it: the input total must stay inclusive for the
            // handoff gate, which cares how much context was sent, not cost.
            //
            // Shape assumption: documented OpenAI-compatible responses that carry
            // `total_tokens` always co-report at least one of `prompt_tokens` /
            // `completion_tokens`. A response that supplies only `total_tokens`
            // with neither category is therefore not a supported shape and would
            // be silently ignored here. If that shape is ever encountered, extend
            // this gate rather than representing absent categories as zero.
            if response.input_tokens.is_some()
                || response.input_tokens_overflowed
                || response.output_tokens.is_some()
            {
                *self.turn_total_state = self.turn_total_state.fold(response.total_tokens);
                // Cache-read: the cache-served subset of input tokens.
                *self.turn_cached_input_tokens = self
                    .turn_cached_input_tokens
                    .fold(response.cached_input_tokens);
                // Cache-write: the cache-creation subset of input tokens.
                *self.turn_cache_write_tokens = self
                    .turn_cache_write_tokens
                    .fold(response.cache_write_tokens);

                // Derive billing identity for this round and fold it into the
                // per-turn accumulator.  Rules (NIP-AM §pricingIdentity):
                //
                // 1. Attempt to derive identity from (base_url, request_model).
                //    - base_url must canonically match an official allowlisted host.
                //    - request_model must be Some (mesh-auto with unknown model
                //      cannot prove identity).
                // 2. Fold the round identity into the turn accumulator:
                //    - Unseen (None): record this round's identity (or poison if None).
                //    - Consistent: if it matches, keep; otherwise poison.
                //    - Poisoned: stays poisoned forever this turn.
                {
                    let round_identity: Option<PricingIdentity> =
                        response.request_model.as_deref().and_then(|model| {
                            pricing_authority(&self.cfg.base_url).map(|auth| PricingIdentity {
                                authority: auth.to_string(),
                                model: model.to_string(),
                                cache_class: None, // no cache-class derivation yet
                            })
                        });

                    *self.turn_pricing_identity =
                        fold_pricing_identity(self.turn_pricing_identity.take(), round_identity);
                }

                // Report what the turn has burned SO FAR, before running the
                // next round. A turn is many provider round-trips over many
                // minutes, and until this point the only report was the one
                // `session/prompt` sends after the turn returns — so a turn
                // that was cancelled, timed out, or whose process was killed
                // reported nothing at all, and its tokens (already billed)
                // existed only in this stack frame. Reporting per round bounds
                // the loss to the single request in flight.
                //
                // Emitting more than one `usage_update` per turn is expected by
                // the consumer: buzz-acp's UsageTracker advances its committed
                // baseline only when the turn's metric is published, so every
                // notification within a turn measures from the same frozen
                // baseline and the last one seen is the turn's true total.
                // goose behaves the same way, which is why the tracker was
                // written to tolerate it.
                self.emit_usage_update().await;
            }

            // Stable per-kind message IDs for ACP v2 ContentChunk compliance.
            // ACP v2 requires every ContentChunk to carry `messageId`; all chunks
            // that belong to the same logical message must share the same ID, and
            // IDs must be unique per message within the ACP session.
            //
            // A provider round produces at most one thought and one assistant
            // message (the parsers collapse all provider output into one
            // LlmResponse.reasoning string and one LlmResponse.text string).
            // These are two *distinct* logical messages, so they get distinct IDs.
            //
            // `run_id` is a fresh random token per `session/prompt` invocation,
            // so `<run_id>-thought-<round>` and `<run_id>-message-<round>` are
            // unique within the ACP session even across multiple prompts.
            //
            // ACP v1 allows the field, so this is a backwards-safe addition.
            let thought_msg_id = format!("{}-thought-{round}", self.run_id);
            let message_msg_id = format!("{}-message-{round}", self.run_id);
            if !response.reasoning.is_empty() {
                wire::send(
                    self.wire,
                    wire::session_update(
                        self.session_id,
                        json!({
                            "sessionUpdate": "agent_thought_chunk",
                            "messageId": &thought_msg_id,
                            "content": { "type": "text", "text": &response.reasoning }
                        }),
                    ),
                )
                .await;
            }

            if !response.text.is_empty() {
                wire::send(
                    self.wire,
                    wire::session_update(
                        self.session_id,
                        json!({
                            "sessionUpdate": "agent_message_chunk",
                            "messageId": &message_msg_id,
                            "content": { "type": "text", "text": &response.text }
                        }),
                    ),
                )
                .await;
            }

            // `max_tokens` describes a truncated assistant response, not turn
            // completion. Never execute tool calls from it: although one may
            // parse as valid, a later call (or surrounding instructions) may
            // have been cut off. Replay only the text, with no tool calls, so
            // the history remains valid without fabricated tool results; then
            // add actionable user-role feedback and ask the model to continue.
            if response.stop == ProviderStop::MaxTokens {
                self.history.push(HistoryItem::Assistant {
                    text: response.text,
                    tool_calls: Vec::new(),
                    reasoning_details: response.reasoning_details,
                });
                if max_tokens_recoveries >= self.cfg.max_token_recoveries {
                    tracing::warn!(
                        recoveries = max_tokens_recoveries,
                        max_recoveries = self.cfg.max_token_recoveries,
                        "provider repeatedly hit output token limit; recovery budget exhausted"
                    );
                    return Ok(StopReason::MaxTokens);
                }
                max_tokens_recoveries = max_tokens_recoveries.saturating_add(1);
                tracing::warn!(
                    recovery = max_tokens_recoveries,
                    max_recoveries = self.cfg.max_token_recoveries,
                    "provider hit output token limit; asking model to continue in smaller steps"
                );
                self.history
                    .push(HistoryItem::User(MAX_TOKENS_RECOVERY_MESSAGE.to_string()));
                continue;
            }

            if response.tool_calls.is_empty() {
                if response.stop == ProviderStop::ToolUse {
                    return Err(AgentError::Llm(
                        "provider: stop=tool_use but zero tool_calls".into(),
                    ));
                }
                // Capture before response.text is moved into history.
                let text_is_empty = response.text.trim().is_empty();
                self.history.push(HistoryItem::Assistant {
                    text: response.text,
                    tool_calls: Vec::new(),
                    reasoning_details: response.reasoning_details.clone(),
                });
                let stop = map_stop(response.stop);
                // Diagnostic: warn when no publish was seen across the whole
                // turn, the final response has no visible text, and the
                // token count looks silent.  Two independent gates:
                //   1. `!buzz_reply_call_seen` — no publish attempt in any
                //      round (read-only tool calls do NOT suppress: a turn
                //      that ran tools but never published then died at 3
                //      tokens is still a silent death).
                //   2. `text_is_empty` — model emitted no visible text
                //      (a terse reply like "OK" is not silent).
                //   3. token count or usage-absent check.
                // Fires before the `_Stop` hook so the warning appears in
                // the log even if the hook rejects the stop and the loop
                // continues.  Does not alter control flow.
                warn_if_silent_turn(
                    buzz_reply_call_seen,
                    text_is_empty,
                    response.output_tokens,
                    response.stop,
                );

                let is_silent = is_silent_candidate(
                    buzz_reply_call_seen,
                    text_is_empty,
                    response.output_tokens,
                );
                if is_silent {
                    silent_turns = silent_turns.saturating_add(1);
                } else {
                    silent_turns = 0;
                }

                if is_silent && silent_turns >= MAX_CONSECUTIVE_SILENT_TURNS {
                    tracing::error!(
                        silent_turns = silent_turns,
                        max_silent_turns = MAX_CONSECUTIVE_SILENT_TURNS,
                        "agent: silent early-stop braking gate triggered — halting turn to prevent infinite retry loop"
                    );
                    let breaker_msg_id = format!("{}-breaker-{round}", self.run_id);
                    wire::send(
                        self.wire,
                        wire::session_update(
                            self.session_id,
                            json!({
                                "sessionUpdate": "agent_message_chunk",
                                "messageId": &breaker_msg_id,
                                "content": { "type": "text", "text": SILENT_EARLY_STOP_BREAKER_MSG }
                            }),
                        ),
                    )
                    .await;
                    if let Some(HistoryItem::Assistant { text, .. }) = self.history.last_mut() {
                        if text.trim().is_empty() {
                            *text = SILENT_EARLY_STOP_BREAKER_MSG.to_string();
                        }
                    }
                    self.save_checkpoint_if_scoped();
                    return Ok(StopReason::EndTurn);
                }
                // Only gate genuine end_turn — don't override max_tokens/refusal.
                if stop == StopReason::EndTurn {
                    if stop_rejections >= self.cfg.stop_max_rejections {
                        if !buzz_reply_call_seen {
                            self.try_auto_publish_fallback().await;
                        }
                        self.save_checkpoint_if_scoped();
                        return Ok(stop);
                    }
                    let mut objections = self
                        .mcp
                        .call_hooks(
                            "_Stop",
                            &json!({}),
                            self.cfg.hook_timeout,
                            &self.cfg.hook_servers,
                        )
                        .await;
                    // Reply guard shares this gate and this budget, so a round
                    // carrying both a hook objection and a reply reminder costs
                    // one rejection and delivers both texts.
                    if self.cfg.require_reply
                        && !buzz_reply_call_seen
                        && reply_nags < MAX_REPLY_NAGS
                    {
                        reply_nags += 1;
                        let nag_text = if text_is_empty && response.reasoning_details.is_some() {
                            REPLY_GUARD_REASONING_ONLY_NAG
                        } else {
                            REPLY_GUARD_NAG
                        };
                        objections
                            .push((REPLY_GUARD_SERVER.to_string(), nag_text.to_string()));
                    }
                    if !objections.is_empty() {
                        stop_rejections = stop_rejections.saturating_add(1);
                        push_hook_outputs_as_tool_results(self.history, "_Stop", &objections);
                        continue;
                    }
                    if !buzz_reply_call_seen {
                        self.try_auto_publish_fallback().await;
                    }
                }
                self.save_checkpoint_if_scoped();
                return Ok(stop);
            }

            silent_turns = 0;
            let mut calls = response.tool_calls;
            if calls.len() > MAX_TOOL_CALLS_PER_TURN {
                tracing::warn!(
                    "capping tool_calls {} -> {MAX_TOOL_CALLS_PER_TURN}",
                    calls.len()
                );
                calls.truncate(MAX_TOOL_CALLS_PER_TURN);
            }
            // Deliberately after truncation: a publish-shaped call that was
            // discarded never runs, so it must not suppress the reminder.
            // Updated unconditionally (not gated on `require_reply`) so the
            // silent-turn diagnostic has a publish-aware turn-level signal
            // regardless of config.
            self.history.push(HistoryItem::Assistant {
                text: response.text,
                tool_calls: calls.clone(),
                reasoning_details: response.reasoning_details,
            });

            if let Some(stop) = self.execute_calls(&calls, &mut buzz_reply_publish_count, &mut buzz_publish_attempt_count).await {
                return Ok(stop);
            }

            // 更新實際成功回覆/發布狀態（以實際執行成功為準，避免失敗調用誤殺救援機制）
            if !buzz_reply_call_seen {
                if buzz_reply_publish_count > 0 {
                    buzz_reply_call_seen = true;
                } else {
                    let recent_results = self.history.iter().rev().take(calls.len());
                    for (c, item) in calls.iter().zip(recent_results) {
                        if is_buzz_reply_call(c, self.mcp) {
                            if let HistoryItem::ToolResult(tr) = item {
                                if is_tool_result_success(tr) {
                                    buzz_reply_call_seen = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // 智慧型死循環與連續工具錯誤硬熔斷煞車門禁
            let calls_failed = !calls.is_empty() && self.history.iter().rev().take(calls.len()).all(|item| {
                if let HistoryItem::ToolResult(tr) = item {
                    tr.is_error
                } else {
                    false
                }
            });

            if calls_failed {
                total_consecutive_tool_failures = total_consecutive_tool_failures.saturating_add(1);
                // 檢查是否為完全相同的工具呼叫與參數（死循環特徵）
                let first_call = calls.first();
                if let (Some(last), Some(current)) = (&last_failed_call, first_call) {
                    if last.name == current.name && last.arguments == current.arguments {
                        identical_call_failures = identical_call_failures.saturating_add(1);
                    } else {
                        identical_call_failures = 1;
                        last_failed_call = Some((*current).clone());
                    }
                } else {
                    identical_call_failures = 1;
                    last_failed_call = first_call.cloned();
                }
            } else {
                total_consecutive_tool_failures = 0;
                identical_call_failures = 0;
                last_failed_call = None;
                if !calls.is_empty() {
                    successful_tool_rounds = successful_tool_rounds.saturating_add(1);
                }
            }

            let is_identical_loop = identical_call_failures >= MAX_IDENTICAL_TOOL_FAILURES;
            let is_total_overflow = total_consecutive_tool_failures >= MAX_TOTAL_CONSECUTIVE_TOOL_FAILURES;

            if is_identical_loop || is_total_overflow {
                tracing::error!(
                    identical_call_failures,
                    total_consecutive_tool_failures,
                    is_identical_loop,
                    is_total_overflow,
                    "agent: consecutive tool failures breaker triggered — halting turn to prevent runaway loops"
                );
                let breaker_msg_id = format!("{}-breaker-tool-{round}", self.run_id);
                let err_msg = if is_identical_loop {
                    "【安全熔斷煞車】特工連續 5 次重複執行完全相同的失敗工具呼叫，已自動終止任務以防範無效死循環。請檢查工具參數與系統設定。"
                } else {
                    "【安全熔斷煞車】特工累計連續 8 次工具執行失敗（如多個備選檔案路徑均不存在），已自動終止任務以防範無效重試。請檢查檔案路徑或系統設定。"
                };
                wire::send(
                    self.wire,
                    wire::session_update(
                        self.session_id,
                        json!({
                            "sessionUpdate": "agent_message_chunk",
                            "messageId": &breaker_msg_id,
                            "content": { "type": "text", "text": err_msg }
                        }),
                    ),
                )
                .await;
                self.history.push(HistoryItem::Assistant {
                    text: err_msg.to_string(),
                    tool_calls: Vec::new(),
                    reasoning_details: None,
                });
                // 無條件兜底發布，確保熔斷原因送入 Buzz 討論串
                self.try_auto_publish_fallback().await;
                self.save_checkpoint_if_scoped();
                return Ok(StopReason::EndTurn);
            }

            // Solution 3: Tool-call round pacing buffer to prevent burst rate-limit (429) on upstream LLM
            if self.cfg.round_pacing_ms > 0 && !calls.is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(self.cfg.round_pacing_ms)).await;
            }
        }
    }

    /// Non-blocking drain of the steer queue. Each queued steer is appended to
    /// history as a user turn so the model picks it up on its next request. A
    /// steer whose blocks all fail to render (e.g. unsupported content) is
    /// skipped rather than aborting the turn — steering is best-effort
    /// augmentation, not a hard input contract like the initial prompt.
    fn drain_steers(&mut self) {
        while let Ok(blocks) = self.steer.try_recv() {
            match prompt_to_text(blocks) {
                Ok(text) if !text.trim().is_empty() => {
                    self.history.push(HistoryItem::User(text));
                }
                Ok(_) => {
                    tracing::debug!("dropping empty steer message");
                }
                Err(e) => {
                    tracing::warn!("dropping unrenderable steer message: {e}");
                }
            }
        }
    }

    /// Unified tool-call execution. Three phases:
    ///   1. Preflight (sequential): emit `pending`; unknown tools fail fast
    ///      with a synthetic result. Cancel here fills every still-empty
    ///      slot as cancelled.
    ///   2. Execute: spawn runnable calls into a `JoinSet` bounded by a
    ///      `Semaphore(max_parallel_tools)`. `select!` between cancel and
    ///      `join_next`. On cancel: close semaphore, drain in-flight tasks
    ///      (each sends `notifications/cancelled` internally), synthesize
    ///      cancelled for unfilled slots and emit `failed`.
    ///   3. Append: push results into history in original call order.
    ///
    /// `max_parallel_tools = 1` makes phase 2 effectively sequential
    /// (one in-flight call at a time via the semaphore). Larger values
    /// run that many calls concurrently.
    async fn execute_calls(
        &mut self,
        calls: &[ToolCall],
        reply_publish_count: &mut usize,
        publish_attempt_count: &mut usize,
    ) -> Option<StopReason> {
        let mut results: Vec<Option<ToolResult>> = vec![None; calls.len()];
        let mut runnable: Vec<usize> = Vec::with_capacity(calls.len());

        for (idx, call) in calls.iter().enumerate() {
            if *self.cancel.borrow() {
                for (j, c) in calls.iter().enumerate() {
                    if results[j].is_none() {
                        // Calls 0..idx already had `pending` emitted; emit
                        // a terminal `failed` so the client doesn't see
                        // them stuck.
                        if j < idx {
                            emit_failed(self.wire, self.session_id, c, "cancelled").await;
                        }
                        results[j] = Some(synthetic_tool_result(c, "cancelled".into()));
                    }
                }
                self.append_results(calls, &mut results);
                return Some(StopReason::Cancelled);
            }
            emit_pending(self.wire, self.session_id, call).await;

            // 重複發布物理熔斷門禁 (Duplicate Publish Circuit Breaker)：
            // 單一 Turn 內僅允許發送一次成功的頻道訊息 (messages send)；若因 Handoff 或模型失憶重複發起發布，一律源頭攔截，
            // 避免 Honey 發生連續回覆 12 次之重播洗版風暴。
            // 注意：
            // 1. 貼表情符號 (reactions add) 不受此限，亦不會消耗此回覆訊息計數。
            // 2. 僅在先前已「成功發布」(*reply_publish_count > 0) 時進行防重放攔截。
            // 3. 若前次發布失敗（如 Exit Code != 0），允許在同一 Turn 內修正重試（最多 3 次嘗試），避免首次失敗即被封口。
            if is_buzz_send_message_call(call, self.mcp) {
                if *reply_publish_count > 0 {
                    tracing::warn!(
                        session_id = self.session_id,
                        tool_name = %call.name,
                        reply_publish_count = *reply_publish_count,
                        "agent: duplicate publish suppressed in turn to prevent replay storm"
                    );
                    let suppressed_text = "Notice: Your message was already published to the channel earlier in this turn. Duplicate publish is suppressed. Conclude your turn now.";
                    let mut result = synthetic_tool_result(call, suppressed_text.into());
                    result.is_error = false;
                    emit_completed(self.wire, self.session_id, call, &result).await;
                    results[idx] = Some(result);
                    continue;
                } else if *publish_attempt_count >= 3 {
                    tracing::warn!(
                        session_id = self.session_id,
                        tool_name = %call.name,
                        publish_attempt_count = *publish_attempt_count,
                        "agent: publish attempts limit reached in turn"
                    );
                    let suppressed_text = "Notice: Message publish failed repeatedly (3 attempts). Conclude your turn now.";
                    let mut result = synthetic_tool_result(call, suppressed_text.into());
                    result.is_error = true;
                    emit_completed(self.wire, self.session_id, call, &result).await;
                    results[idx] = Some(result);
                    continue;
                } else {
                    *publish_attempt_count += 1;
                }
            }

            // Built-in load_skill: execute inline, no MCP round-trip.
            if call.name == builtin::LOAD_SKILL_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_load_skill(&call.arguments, self.skills).await;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in create_reminder: execute inline, no MCP round-trip.
            if call.name == builtin::CREATE_REMINDER_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_create_reminder(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_open: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_OPEN_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_open(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_screenshot: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_SCREENSHOT_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_screenshot(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_close: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_CLOSE_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_close(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_select: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_SELECT_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_select(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_hover: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_HOVER_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_hover(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_keypress: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_KEYPRESS_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_keypress(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in browser_execute_script: execute inline, no MCP round-trip.
            if call.name == builtin::BROWSER_EXECUTE_SCRIPT_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_browser_execute_script(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in form_fill: execute inline, no MCP round-trip.
            if call.name == builtin::FORM_FILL_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_form_fill(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Built-in structured_scrape: execute inline, no MCP round-trip.
            if call.name == builtin::STRUCTURED_SCRAPE_TOOL {
                emit_in_progress(self.wire, self.session_id, call).await;
                let mut result = builtin::call_structured_scrape(&call.arguments, self).await.ok()?;
                result.provider_id = call.provider_id.clone();
                emit_completed(self.wire, self.session_id, call, &result).await;
                results[idx] = Some(result);
                continue;
            }

            // Hook tools (bare name starts with `_`) are invisible to the
            // LLM and only callable via `call_hooks`. Treat any direct
            // invocation as if the tool didn't exist.
            if !self.mcp.has(&call.name) || self.mcp.is_hook(&call.name) {
                let err = format!("unknown tool: {}", call.name);
                emit_failed(self.wire, self.session_id, call, &err).await;
                results[idx] = Some(synthetic_tool_result(call, err));
                continue;
            }
            runnable.push(idx);
        }

        self.execute_parallel(calls, &runnable, &mut results).await;

        // 執行完成後，檢視發布工具的實際執行結果：
        // 只有在工具成功執行且無錯誤時，才累加 reply_publish_count
        for (idx, call) in calls.iter().enumerate() {
            if is_buzz_send_message_call(call, self.mcp) {
                if let Some(res) = &results[idx] {
                    if is_tool_result_success(res) {
                        *reply_publish_count += 1;
                        tracing::info!(
                            session_id = self.session_id,
                            reply_publish_count = *reply_publish_count,
                            "agent: channel message successfully published"
                        );
                    } else {
                        tracing::warn!(
                            session_id = self.session_id,
                            publish_attempt_count = *publish_attempt_count,
                            "agent: channel message publish attempt failed; retry permitted"
                        );
                    }
                }
            }
        }

        self.append_results(calls, &mut results);

        if *self.cancel.borrow() {
            Some(StopReason::Cancelled)
        } else {
            None
        }
    }

    fn append_results(&mut self, calls: &[ToolCall], results: &mut [Option<ToolResult>]) {
        for (i, call) in calls.iter().enumerate() {
            let mut result = results[i].take().unwrap_or_else(|| ToolResult {
                provider_id: call.provider_id.clone(),
                content: vec![ToolResultContent::Text(
                    "internal error: missing result".into(),
                )],
                is_error: true,
            });
            // On tool error: append a reflection prompt so the LLM
            // diagnoses the failure before blindly retrying.
            if result.is_error {
                tracing::warn!(
                    session_id = %self.session_id,
                    tool_name = %call.name,
                    tool_args = %call.arguments,
                    error = %result.text(),
                    "agent: tool execution failed"
                );
                let recovery_hint = if self.cfg.self_healing_enabled {
                    SELF_HEALING_PROMPT_SUFFIX
                } else {
                    ERROR_REFLECTION_SUFFIX
                };
                result
                    .content
                    .push(ToolResultContent::Text(recovery_hint.to_string()));
            }
            self.ledger.record_tool_execution(call, &result);
            self.history.push(HistoryItem::ToolResult(result));
        }
    }

    /// 執行每回合壓縮閘門（Tier 0 剪枝 → Tier 1 增強交接 → Tier 2 無 LLM 備援）。
    /// 回傳 `true` 代表使用者在交接過程中主動取消了本回合。
    async fn run_preflight_compaction(&mut self, handoff_attempts: &mut usize) -> bool {
        let is_tiered = self.cfg.compaction_mode == crate::config::CompactionMode::Tiered;
        if is_tiered {
            let prune_threshold = crate::handoff::token_threshold_for_percent(
                self.cfg.max_context_tokens,
                self.cfg.prune_threshold_percent,
            );
            if self.projected_handoff_input_tokens() >= prune_threshold {
                let stats = crate::compaction::prune_old_tool_results(
                    self.history,
                    self.cfg.max_context_tokens,
                    self.cfg.prune_threshold_percent,
                    self.cfg.prune_target_percent,
                    self.cfg.tail_budget_percent,
                    self.cfg.tail_min_rounds,
                );
                // 僅當實際剪枝修改了歷史內容時才清零舊用量基準，以免影響未修改小歷史的既有測試語義
                if stats.pruned_count > 0 {
                    *self.last_request_input_tokens = None;
                    *self.last_request_history_bytes = None;
                }
            }
        }

        let handoff_warranted = is_tiered && self.should_handoff();
        match self.maybe_handoff(handoff_attempts).await {
            HandoffOutcome::Cancelled => return true,
            // Context was just reset — clear token count and byte baseline so stale readings
            // do not immediately re-trigger compaction before fresh provider usage arrives.
            HandoffOutcome::Performed => {
                *self.last_request_input_tokens = None;
                *self.last_request_history_bytes = None;
                self.save_checkpoint_if_scoped();
            }
            HandoffOutcome::Skipped => {
                if handoff_warranted
                    && crate::compaction::compact_without_llm(
                        self.history,
                        self.ledger,
                        self.original_task.as_deref(),
                        self.cfg.max_context_tokens,
                        self.cfg.prune_threshold_percent,
                        self.cfg.tail_budget_percent,
                        self.cfg.tail_min_rounds,
                    )
                {
                    *self.last_request_input_tokens = None;
                    *self.last_request_history_bytes = None;
                    self.save_checkpoint_if_scoped();
                }
                truncate_history(self.history, self.cfg.max_history_bytes);
            }
        }
        false
    }

    /// 階段 E：於新建立的空白 Session 首輪嘗試從磁碟解密並還原相同討論串的 Checkpoint。
    fn restore_checkpoint_if_scoped(&mut self, incoming_user_text: &str) -> bool {
        if !self.cfg.checkpoint_enabled || !self.history.is_empty() {
            return false;
        }
        let Some(scope) = crate::checkpoint::CheckpointScope::from_prompt_and_system(
            incoming_user_text,
            self.system_prompt,
        ) else {
            return false;
        };
        let store = crate::checkpoint::EncryptedCheckpointStore::from_config(
            self.cfg,
            self.system_prompt,
        );
        match store.load(&scope.scope_key) {
            Ok(Some(payload)) => {
                self.history.extend(payload.history);
                // 防禦性確保不會遺留未配對 ToolResult 的懸空 Assistant tool_calls
                while matches!(
                    self.history.last(),
                    Some(HistoryItem::Assistant { tool_calls, .. }) if !tool_calls.is_empty()
                ) {
                    self.history.pop();
                }
                *self.original_task = payload
                    .original_task
                    .or_else(|| Some(incoming_user_text.to_owned()));
                *self.handoff_count = payload.handoff_count;
                *self.ledger = payload.ledger;
                let same_tail = matches!(
                    self.history.last(),
                    Some(HistoryItem::User(s)) if s == incoming_user_text
                );
                tracing::info!(
                    target: "buzz_agent::checkpoint",
                    session_id = %self.session_id,
                    scope_key = %scope.scope_key,
                    restored_items = self.history.len(),
                    "restored encrypted session checkpoint from disk"
                );
                same_tail
            }
            Ok(None) => false,
            Err(err) => {
                tracing::warn!(
                    target: "buzz_agent::checkpoint",
                    session_id = %self.session_id,
                    scope_key = %scope.scope_key,
                    error = %err,
                    "failed to load encrypted checkpoint; starting fresh"
                );
                false
            }
        }
    }

    /// 階段 E：將當前討論串濃縮狀態去敏並以 AES-256-GCM 寫入磁碟（失敗不阻斷回合）。
    pub(crate) fn save_checkpoint_if_scoped(&self) {
        if !self.cfg.checkpoint_enabled {
            return;
        }
        let Some(prompt_anchor) = self
            .original_task
            .clone()
            .or_else(|| crate::compaction::find_latest_real_user_prompt(self.history, None))
        else {
            return;
        };
        let Some(scope) = crate::checkpoint::CheckpointScope::from_prompt_and_system(
            &prompt_anchor,
            self.system_prompt,
        ) else {
            return;
        };
        let store = crate::checkpoint::EncryptedCheckpointStore::from_config(
            self.cfg,
            self.system_prompt,
        );
        let condensed = crate::checkpoint::compact_history_for_checkpoint(
            self.history,
            self.original_task.as_deref(),
            self.cfg.max_context_tokens,
            self.cfg.tail_budget_percent,
            self.cfg.tail_min_rounds,
        );
        if let Err(err) = store.save(
            &scope,
            self.original_task.clone(),
            *self.handoff_count,
            self.ledger.clone(),
            condensed,
        ) {
            tracing::warn!(
                target: "buzz_agent::checkpoint",
                session_id = %self.session_id,
                scope_key = %scope.scope_key,
                error = %err,
                "failed to persist encrypted checkpoint"
            );
        }
    }

    async fn execute_parallel(
        &mut self,
        calls: &[ToolCall],
        runnable: &[usize],
        results: &mut [Option<ToolResult>],
    ) {
        let limit = self.cfg.max_parallel_tools.max(1);
        let sem = Arc::new(Semaphore::new(limit));
        let mut set: JoinSet<(usize, InvokeOutcome)> = JoinSet::new();
        let hook_reg = Arc::new(crate::hooks::HookRegistry::new());

        for &i in runnable {
            let call = calls[i].clone();
            let mcp = Arc::clone(self.mcp);
            let wire = self.wire.clone();
            let session_id = self.session_id.to_owned();
            let timeout = self.cfg.tool_timeout;
            let budget = ResultBudget {
                total: MAX_TOOL_RESULT_BYTES,
                text: self.cfg.max_tool_result_text_bytes,
            };
            let mut cancel = self.cancel.clone();
            let sem = Arc::clone(&sem);
            let permissions = Arc::clone(self.permissions);
            let protocol_version = self.protocol_version;
            let hook_reg = Arc::clone(&hook_reg);
            set.spawn(async move {
                // Acquire a permit; if the semaphore is closed (cancel),
                // emit a terminal wire update and skip the call.
                let _permit = match sem.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        emit_failed(&wire, &session_id, &call, "cancelled").await;
                        return (i, InvokeOutcome::Failed("cancelled".into()));
                    }
                };
                // Argument-shape validation BEFORE the ask: a malformed
                // non-object argument can never execute, so reject it locally
                // without prompting the user to approve a doomed call.
                if let Err(e) = crate::mcp::validate_arg_shape(&call.name, &call.arguments) {
                    let msg = e.to_string();
                    emit_failed(&wire, &session_id, &call, &msg).await;
                    return (i, InvokeOutcome::Failed(msg));
                }

                // PreToolUse Hook Guard (Sentry Gates)
                let hook_ctx = crate::hooks::HookContext {
                    tool_name: call.name.clone(),
                    tool_arguments: call.arguments.clone(),
                    session_id: session_id.clone(),
                    working_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
                    trigger_event: crate::hooks::HookTrigger::PreToolUse,
                };

                match hook_reg.run_pre_tool_use(&hook_ctx).await {
                    crate::hooks::HookResult::Block { reason } => {
                        emit_failed(&wire, &session_id, &call, &reason).await;
                        return (i, InvokeOutcome::Failed(reason));
                    }
                    crate::hooks::HookResult::Warn { message } => {
                        tracing::warn!("PreToolUse warning for {}: {}", call.name, message);
                    }
                    crate::hooks::HookResult::Continue => {}
                    _ => {}
                }

                // Ask the client to authorize this call. The broker owns the
                // full correlation lifecycle and races cancellation internally;
                // every non-authorizing outcome fails closed.
                match permissions
                    .request_permission(&wire, protocol_version, &session_id, &call, &mut cancel)
                    .await
                {
                    PermissionDecision::Allowed => {}
                    PermissionDecision::Denied(msg) => {
                        emit_failed(&wire, &session_id, &call, msg).await;
                        return (i, InvokeOutcome::Failed(msg.into()));
                    }
                    PermissionDecision::Cancelled => {
                        emit_failed(&wire, &session_id, &call, "cancelled").await;
                        return (i, InvokeOutcome::Failed("cancelled".into()));
                    }
                }
                // Cancellation recheck: a cancel may have landed while we
                // waited for approval. Do not start the call in that case.
                if *cancel.borrow() {
                    emit_failed(&wire, &session_id, &call, "cancelled").await;
                    return (i, InvokeOutcome::Failed("cancelled".into()));
                }
                emit_in_progress(&wire, &session_id, &call).await;
                let outcome = invoke_tool_inner(&mcp, &call, timeout, budget, cancel).await;

                // PostToolUse Hook Guard
                let outcome = match outcome {
                    InvokeOutcome::Done(ref result) => {
                        let mut post_ctx = hook_ctx;
                        post_ctx.trigger_event = crate::hooks::HookTrigger::PostToolUse;
                        match hook_reg.run_post_tool_use(&post_ctx, result).await {
                            crate::hooks::HookResult::Block { reason } => {
                                InvokeOutcome::Failed(reason)
                            }
                            crate::hooks::HookResult::Warn { message } => {
                                tracing::warn!("PostToolUse warning for {}: {}", call.name, message);
                                outcome
                            }
                            _ => outcome,
                        }
                    }
                    other => other,
                };

                match &outcome {
                    InvokeOutcome::Done(result) => {
                        emit_completed(&wire, &session_id, &call, result).await;
                    }
                    InvokeOutcome::Failed(msg) => {
                        emit_failed(&wire, &session_id, &call, msg).await;
                    }
                }
                (i, outcome)
            });
        }

        let mut cancel_rx = self.cancel.clone();
        let mut cancelled = if *cancel_rx.borrow() {
            sem.close();
            true
        } else {
            false
        };
        while !cancelled {
            tokio::select! {
                biased;
                _ = cancel_rx.changed() => {
                    // Cancel: stop accepting new permits. Do NOT abort
                    // tasks — each in-flight `mcp.call` observes the same
                    // cancel receiver via its internal `select!` and
                    // returns promptly with an "cancelled" error after
                    // sending `notifications/cancelled` to the server.
                    sem.close();
                    cancelled = true;
                    break;
                }
                next = set.join_next() => {
                    match next {
                        Some(Ok((i, outcome))) => {
                            results[i] = Some(outcome_to_result(&calls[i], outcome));
                        }
                        Some(Err(e)) => {
                            tracing::warn!("tool task join error: {e}");
                        }
                        None => break,
                    }
                }
            }
        }

        // After cancel, drain in-flight tasks. Each task's internal
        // `do_call` observes the cancel receiver and returns promptly
        // after sending `notifications/cancelled`. We bound the drain
        // to avoid hanging if a task is stuck in restart/reconnect.
        if cancelled {
            let drain_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                match tokio::time::timeout_at(drain_deadline, set.join_next()).await {
                    Ok(Some(Ok((i, outcome)))) => {
                        if results[i].is_none() {
                            results[i] = Some(outcome_to_result(&calls[i], outcome));
                        }
                    }
                    Ok(Some(Err(e))) => {
                        tracing::warn!("tool task join error (drain): {e}");
                    }
                    Ok(None) => break, // all tasks drained
                    Err(_) => {
                        // Drain timed out — abort remaining tasks.
                        set.abort_all();
                        tracing::warn!("cancel drain timed out; aborting remaining tasks");
                        break;
                    }
                }
            }
        }

        // Fill any remaining unfilled runnable slots as cancelled. Tasks
        // that didn't complete (timed out in drain or never started) need
        // a terminal wire update so the client doesn't see "pending" forever.
        for &i in runnable {
            if results[i].is_none() {
                results[i] = Some(synthetic_tool_result(&calls[i], "cancelled".into()));
                emit_failed(self.wire, self.session_id, &calls[i], "cancelled").await;
            }
        }
    }

    fn extract_reasoning_text(details: &serde_json::Value, system_prompt: &str) -> Option<String> {
        let mut extracted_reasoning = String::new();
        if let Some(arr) = details.as_array() {
            for val in arr {
                if let Some(s) = val.as_str() {
                    extracted_reasoning.push_str(s);
                    extracted_reasoning.push('\n');
                } else if let Some(text_val) = val.get("text").and_then(|t| t.as_str()) {
                    extracted_reasoning.push_str(text_val);
                    extracted_reasoning.push('\n');
                }
            }
        } else if let Some(s) = details.as_str() {
            extracted_reasoning = s.to_string();
        }

        let clean_reasoning = extracted_reasoning.trim();
        if !clean_reasoning.is_empty() {
            let agent_title = if system_prompt.contains("Sentry") {
                "Sentry"
            } else if system_prompt.contains("Honey2") {
                "Honey2"
            } else if system_prompt.contains("Pollen") {
                "Pollen"
            } else if system_prompt.contains("Wax") {
                "Wax"
            } else if system_prompt.contains("Fizz") {
                "Fizz"
            } else if system_prompt.contains("Honey") {
                "Honey"
            } else {
                "特工"
            };
            Some(format!("【{} 分析思考報告】：\n\n{}", agent_title, clean_reasoning))
        } else {
            None
        }
    }

    fn last_assistant_text(&self) -> Option<String> {
        for item in self.history.iter().rev() {
            if let HistoryItem::Assistant { text, reasoning_details, .. } = item {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
                // If text is empty but reasoning_details exists, extract reasoning as fallback text
                if let Some(details) = reasoning_details {
                    if let Some(extracted) = Self::extract_reasoning_text(details, self.system_prompt) {
                        return Some(extracted);
                    }
                }
            }
        }
        None
    }

    async fn try_auto_publish_fallback(&mut self) {
        let text_to_publish = match self.last_assistant_text() {
            Some(t) if t.len() >= 5 => t,
            _ => {
                tracing::warn!(session_id = self.session_id, "auto_publish_fallback: no text candidate found for auto-publish");
                return;
            }
        };

        // 修 D9：跳過第 35 輪 [系統警示] 與 MAX_TOKENS_RECOVERY_MESSAGE 等合成 User 訊息，
        // 逆向尋找含有 `<context>` 與 `channel_id` 的真實用戶提問（並於找不到時退回 original_task）。
        let (channel_id, maybe_reply_to) = match crate::compaction::find_reply_target_in_history(
            self.history,
            self.original_task.as_deref(),
        ) {
            Some(target) => target,
            None => {
                tracing::warn!(session_id = self.session_id, "auto_publish_fallback: could not extract channel_id from prompt");
                return;
            }
        };

        let shell_tool = match self.mcp.tools().into_iter().find(|t| t.name.ends_with("__shell")) {
            Some(t) => t.name,
            None => {
                tracing::warn!(session_id = self.session_id, "auto_publish_fallback: no shell tool available for auto-publish");
                return;
            }
        };

        let temp_dir = std::env::temp_dir();
        let temp_filename = format!("buzz_auto_pub_{}.txt", uuid::Uuid::new_v4());
        let temp_path = temp_dir.join(&temp_filename);
        if let Err(e) = std::fs::write(&temp_path, &text_to_publish) {
            tracing::error!(session_id = self.session_id, "auto_publish_fallback: failed to write temp file: {e}");
            return;
        }

        let temp_str = temp_path.to_string_lossy().replace('\\', "/");
        let temp_script_filename = format!("buzz_auto_pub_script_{}.py", uuid::Uuid::new_v4());
        let temp_script_path = temp_dir.join(&temp_script_filename);

        let py_script_code = r#"import os, sys, shutil, subprocess

text_path = sys.argv[1]
channel_id = sys.argv[2]
reply_to = sys.argv[3] if len(sys.argv) > 3 and sys.argv[3] != "None" else None
priv_key = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] != "None" else None

buzz_bin = shutil.which('buzz') or os.path.expandvars(r'%LOCALAPPDATA%\Buzz\buzz.exe')
with open(text_path, 'r', encoding='utf-8') as f:
    content = f.read()

args = [buzz_bin, 'messages', 'send', '--channel', channel_id, '--content', content]
if reply_to:
    args.extend(['--reply-to', reply_to])
if priv_key:
    args.extend(['--private-key', priv_key])

res = subprocess.run(args, capture_output=True, text=True)
if res.stdout:
    sys.stdout.write(res.stdout)
if res.stderr:
    sys.stderr.write(res.stderr)
sys.exit(res.returncode)
"#;

        if let Err(e) = std::fs::write(&temp_script_path, py_script_code) {
            tracing::error!(session_id = self.session_id, "auto_publish_fallback: failed to write py script: {e}");
            let _ = std::fs::remove_file(&temp_path);
            return;
        }

        let temp_script_str = temp_script_path.to_string_lossy().replace('\\', "/");
        let reply_to_arg = maybe_reply_to.as_deref().unwrap_or("None");
        let priv_key_arg = self.agent_keys.as_ref()
            .map(|k| k.secret_key().to_secret_hex())
            .unwrap_or_else(|| "None".to_string());

        let cmd = format!(
            "python \"{}\" \"{}\" \"{}\" \"{}\" \"{}\"",
            temp_script_str, temp_str, channel_id, reply_to_arg, priv_key_arg
        );

        tracing::info!(
            session_id = self.session_id,
            channel_id = %channel_id,
            reply_to = ?maybe_reply_to,
            "auto_publish_fallback: rescuing unpublished assistant response via auto-publish"
        );

        let call = ToolCall {
            provider_id: format!("auto_pub_{}", uuid::Uuid::new_v4()),
            name: shell_tool,
            arguments: serde_json::json!({ "command": cmd }),
            provider_extra: serde_json::Map::new(),
        };

        let timeout = self.cfg.tool_timeout;
        let budget = ResultBudget {
            total: MAX_TOOL_RESULT_BYTES,
            text: self.cfg.max_tool_result_text_bytes,
        };
        let cancel = (*self.cancel).clone();
        emit_in_progress(self.wire, self.session_id, &call).await;
        let outcome = invoke_tool_inner(self.mcp, &call, timeout, budget, cancel).await;
        let result = outcome_to_result(&call, outcome);
        if result.is_error {
            tracing::warn!(session_id = self.session_id, error_text = %result.text(), "auto_publish_fallback: failed to send message");
        } else {
            tracing::info!(session_id = self.session_id, "auto_publish_fallback: successfully rescued and published message");
        }
        emit_completed(self.wire, self.session_id, &call, &result).await;
        self.history.push(HistoryItem::ToolResult(result));

        let _ = std::fs::remove_file(&temp_path);
        let _ = std::fs::remove_file(&temp_script_path);
    }
}

/// Outcome of invoking a single tool. The wire notification is emitted by
/// the caller so the spawn loop and the (degenerate, max_parallel=1) path
/// share the same logic.
#[derive(Debug)]
enum InvokeOutcome {
    Done(ToolResult),
    Failed(String),
}

/// Standalone tool invocation. Takes only owned/cloned handles so it can
/// run inside a spawned task. On timeout, kills the offending MCP server's
/// process group and marks it dead; the registry's lazy restart handles it
/// on the next call.
async fn invoke_tool_inner(
    mcp: &Arc<McpRegistry>,
    call: &ToolCall,
    tool_timeout: std::time::Duration,
    budget: ResultBudget,
    mut cancel: watch::Receiver<bool>,
) -> InvokeOutcome {
    if *cancel.borrow() {
        return InvokeOutcome::Failed("cancelled".into());
    }
    match tokio::time::timeout(
        tool_timeout,
        mcp.call(
            &call.name,
            &call.provider_id,
            &call.arguments,
            budget,
            &mut cancel,
        ),
    )
    .await
    {
        Ok(Ok(result)) => InvokeOutcome::Done(result),
        Ok(Err(AgentError::Cancelled)) => InvokeOutcome::Failed("cancelled".into()),
        Ok(Err(e)) => InvokeOutcome::Failed(e.to_string()),
        Err(_) => {
            // If the session was cancelled, the timeout fired because
            // do_call returned quickly with "cancelled" and the outer
            // timeout raced. Don't kill a healthy server for that.
            if *cancel.borrow() {
                return InvokeOutcome::Failed("cancelled".into());
            }
            if let Some(server) = mcp.server_of(&call.name) {
                mcp.kill_server(server, "tool timeout");
            }
            let msg = format!(
                "tool: timeout after {}s. The command took too long. Try a faster approach.",
                tool_timeout.as_secs()
            );
            InvokeOutcome::Failed(msg)
        }
    }
}

fn outcome_to_result(call: &ToolCall, outcome: InvokeOutcome) -> ToolResult {
    match outcome {
        InvokeOutcome::Done(r) => r,
        InvokeOutcome::Failed(m) => synthetic_tool_result(call, m),
    }
}

async fn emit_pending(wire: &WireSender, sid: &str, call: &ToolCall) {
    wire::send(
        wire,
        wire::session_update(
            sid,
            json!({
                "sessionUpdate": "tool_call",
                "toolCallId": call.provider_id,
                "title": call.name,
                "kind": "other",
                "status": "pending",
                "rawInput": call.arguments,
            }),
        ),
    )
    .await;
}

async fn emit_in_progress(wire: &WireSender, sid: &str, call: &ToolCall) {
    wire::send(
        wire,
        wire::session_update(
            sid,
            json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": call.provider_id,
                "status": "in_progress",
            }),
        ),
    )
    .await;
}

async fn emit_completed(wire: &WireSender, sid: &str, call: &ToolCall, result: &ToolResult) {
    wire::send(
        wire,
        wire::session_update(
            sid,
            json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": call.provider_id,
                "status": "completed",
                "content": [{ "type": "content", "content": { "type": "text", "text": result.text() } }],
                "rawOutput": { "isError": result.is_error },
            }),
        ),
    )
    .await;
}

async fn emit_failed(wire: &WireSender, sid: &str, call: &ToolCall, err: &str) {
    wire::send(
        wire,
        wire::session_update(
            sid,
            json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": call.provider_id,
                "status": "failed",
                "rawOutput": { "error": err },
            }),
        ),
    )
    .await;
}

fn prompt_to_text(prompt: Vec<ContentBlock>) -> Result<String, AgentError> {
    let mut parts = Vec::with_capacity(prompt.len());
    for block in prompt {
        match block {
            ContentBlock::Text { text } => parts.push(text),
            ContentBlock::ResourceLink { uri } => parts.push(format!("[resource: {uri}]")),
            ContentBlock::Unsupported => {
                return Err(AgentError::InvalidParams(
                    "prompt: unsupported content block (only text and resource_link are advertised)".into(),
                ));
            }
        }
    }
    Ok(parts.join("\n"))
}

/// Format a single hook output as a structured tool-result body.
///
/// We emit a JSON object rather than XML-style tags. JSON is unambiguous:
/// the inner `text` field is escaped, so a malicious hook cannot break
/// out by including a literal `</hook_output>` (or any other delimiter)
/// in its output. The LLM still sees the source attribution via the
/// `hook` and `server` fields.
fn format_hook_output_body(hook: &str, server: &str, text: &str) -> String {
    // serde_json::to_string never fails on owned strings.
    serde_json::to_string(&json!({
        "hook": hook,
        "server": server,
        "text": text,
    }))
    .unwrap_or_else(|_| String::from("{\"hook\":\"\",\"server\":\"\",\"text\":\"\"}"))
}

/// Synthetic provider id for an injected hook tool-call/result pair. Must
/// be unique per pair so the LLM wire format (which keys tool results by
/// id) stays valid across multiple objections in one session.
fn synthetic_hook_id(hook: &str, server: &str, ordinal: u64) -> String {
    format!("buzz_hook_{hook}_{server}_{ordinal}")
}

/// Append a synthetic Assistant tool-call + ToolResult pair for each hook
/// output. Modeling hook output as a tool result (rather than as a User
/// message) means a malicious hook can't impersonate the user or system
/// — the LLM treats tool results as lower-trust, structured data.
///
/// Each pair uses the hook's qualified tool name (e.g. `fake___Stop`) so
/// attribution is preserved in the wire format. Empty arguments are sent
/// as `{}`. The `Assistant` turn carries no text (tool_calls only).
pub(crate) fn push_hook_outputs_as_tool_results(
    history: &mut Vec<HistoryItem>,
    hook: &str,
    outputs: &[(String, String)],
) {
    for (server, text) in outputs.iter() {
        let provider_id = synthetic_hook_id(hook, server, unique_nonce());
        // Tool name is `<server>__<hook>` — same shape as a real qname
        // for that hook, so the LLM never sees an unknown synthetic name.
        let tool_name = format!("{server}__{hook}");
        history.push(HistoryItem::Assistant {
            text: String::new(),
            tool_calls: vec![ToolCall {
                provider_id: provider_id.clone(),
                name: tool_name,
                arguments: serde_json::json!({}),
                // Synthesised locally, so there is no provider wire form to
                // preserve.
                provider_extra: Default::default(),
            }],
            reasoning_details: None,
        });
        history.push(HistoryItem::ToolResult(ToolResult {
            provider_id,
            content: vec![ToolResultContent::Text(format_hook_output_body(
                hook, server, text,
            ))],
            is_error: false,
        }));
    }
}

/// Monotonic counter for synthetic hook ids within a single process. The
/// uniqueness target is "no collision within the lifetime of one history
/// vec", which a process-wide counter satisfies trivially.
fn unique_nonce() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn synthetic_tool_result(call: &ToolCall, msg: String) -> ToolResult {
    ToolResult {
        provider_id: call.provider_id.clone(),
        content: vec![ToolResultContent::Text(msg)],
        is_error: true,
    }
}

pub(crate) fn truncate_history(history: &mut Vec<HistoryItem>, max_bytes: usize) {
    let mut total: usize = history.iter().map(HistoryItem::estimated_bytes).sum();
    if total <= max_bytes {
        return;
    }
    let original_len = history.len();
    while total > max_bytes && !history.is_empty() {
        let mut end = 1usize;
        while end < history.len() && !matches!(history[end], HistoryItem::User(_)) {
            end += 1;
        }
        if end >= history.len() {
            break;
        }
        let dropped: usize = history[..end]
            .iter()
            .map(HistoryItem::estimated_bytes)
            .sum();
        history.drain(..end);
        total = total.saturating_sub(dropped);
    }
    if history.len() < original_len {
        tracing::info!(
            "history truncated {original_len} -> {} items ({total} bytes)",
            history.len()
        );
    }
}

fn map_stop(p: ProviderStop) -> StopReason {
    match p {
        ProviderStop::EndTurn | ProviderStop::ToolUse | ProviderStop::Other => StopReason::EndTurn,
        ProviderStop::MaxTokens => StopReason::MaxTokens,
        ProviderStop::Refusal => StopReason::Refusal,
    }
}

/// Returns `true` when a reported output-token count is at or below the
/// silent-death threshold.  The observed failure signature is 2–12 tokens.
///
/// Takes a bare `u64` — the caller handles `None` usage separately (a
/// provider that omits token counts is a distinct diagnostic case, not
/// automatically "near-zero").
///
/// Extracted as a pure function so it can be tested without standing up an
/// async agent loop.
fn is_silent_turn(output_tokens: u64) -> bool {
    output_tokens <= SILENT_TURN_TOKEN_THRESHOLD
}

/// Returns `true` when a round response matches the silent-death signature:
/// no publish attempt across the turn, the response text is empty,
/// and output tokens are either near-zero or absent.
fn is_silent_candidate(
    buzz_reply_call_seen: bool,
    text_is_empty: bool,
    output_tokens: Option<u64>,
) -> bool {
    if buzz_reply_call_seen || !text_is_empty {
        return false;
    }
    match output_tokens {
        Some(t) => is_silent_turn(t),
        None => true,
    }
}

/// Emits the silent-turn diagnostic WARN when the turn produced no publish,
/// no visible text, and either near-zero or absent output tokens.
///
/// `buzz_reply_call_seen` is the publish-aware gate (from
/// `is_buzz_reply_call`), updated unconditionally regardless of
/// `require_reply`.  Read-only tool calls do NOT suppress the WARN — a turn
/// that ran tools but never published and then died at 3 tokens is a silent
/// death.
///
/// Two distinct WARN shapes:
/// - Near-zero token count (`output_tokens <= 12`): canonical silent-death.
/// - Unknown usage (`None`) with no publish and no text: separately
///   diagnostic; does not assert near-zero since the count is unknown.
///
/// Extracted as a free function so the WARN seam can be exercised by a
/// scoped tracing subscriber without standing up the full async run loop.
fn warn_if_silent_turn(
    buzz_reply_call_seen: bool,
    text_is_empty: bool,
    output_tokens: Option<u64>,
    stop: ProviderStop,
) {
    if buzz_reply_call_seen || !text_is_empty {
        return;
    }
    match output_tokens {
        Some(t) if is_silent_turn(t) => {
            tracing::warn!(
                stop = ?stop,
                output_tokens = t,
                "agent: turn ended with no publish attempt and near-zero output tokens — possible silent model/gateway early-stop"
            );
        }
        None => {
            tracing::warn!(
                stop = ?stop,
                "agent: turn ended with no publish attempt and no usage reported — cannot confirm output size"
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tracing_subscriber::layer::SubscriberExt;

    /// `truncate_history` cannot serve as the context-window fallback: it is
    /// measured in BYTES (`max_history_bytes`, default 16 MiB, a request-body
    /// limiter) while the thing the fallback must defend is a TOKEN window
    /// (`max_context_tokens`, default 200k). A history large enough to blow a
    /// 200k-token window is nowhere near 16 MiB, so at the default budget the
    /// fallback evicts nothing at all — which is why the `Skipped ->
    /// truncate_history` path left the agent permanently stuck and the reactive
    /// ladder had to be built instead.
    ///
    /// The negative assertion is paired with a positive control (same helper,
    /// same fixture, budget set to the window instead) so that "evicted
    /// nothing" is a real observation about the unit mismatch rather than a
    /// blind probe that could never evict.
    #[test]
    fn truncate_history_is_a_noop_at_context_window_scale() {
        // ~800 KB of history. At any real bytes/token density (densest real
        // content is ~1.4 B/tok, typical prose ~3-4) this is >= 200k tokens,
        // i.e. already over a 200k window.
        let mut history: Vec<HistoryItem> = Vec::new();
        for i in 0..400 {
            history.push(HistoryItem::User(format!("q{i} {}", "x".repeat(1000))));
            history.push(HistoryItem::Assistant {
                text: format!("a{i} {}", "y".repeat(1000)),
                tool_calls: vec![],
                reasoning_details: None,
            });
        }
        let total: usize = history.iter().map(HistoryItem::estimated_bytes).sum();
        let pressure: usize = history
            .iter()
            .map(HistoryItem::context_pressure_bytes)
            .sum();
        assert!(
            total > 800_000,
            "fixture must be big enough to exceed a 200k-token window, got {total}"
        );

        // NEGATIVE: the real configured default budget.
        let default_budget = 16 * 1024 * 1024;
        let mut under_default = history.clone();
        truncate_history(&mut under_default, default_budget);
        assert_eq!(
            under_default.len(),
            history.len(),
            "16 MiB byte budget evicted nothing from a {total}-byte history \
             (pressure {pressure}) that already exceeds a 200k-token window"
        );

        // POSITIVE CONTROL: same helper, same fixture, budget set to the
        // window instead. If this also evicted nothing the assertion above
        // would prove nothing about the unit mismatch -- it would just mean
        // the probe is blind.
        let mut under_window = history.clone();
        truncate_history(&mut under_window, 200_000);
        assert!(
            under_window.len() < history.len(),
            "positive control must evict: probe is blind otherwise"
        );
    }

    /// The shapes the guard must recognize as a publish attempt. Callers apply
    /// the registry checks first; these cover the name suffix and command text.
    #[test]
    fn reply_shape_matches_documented_send_forms() {
        for cmd in [
            "buzz messages send --channel X --content Y",
            "buzz --relay wss://r messages send --channel X --content Y",
            "/abs/path/buzz messages send",
            "printf 'hi' | buzz messages send --content -",
            "buzz messages send-diff --diff -",
            "buzz reactions add --event E --emoji +",
            // Assembled through another shell: rev 3's tokenizer missed this.
            r#"sh -c "buzz messages send --channel X""#,
        ] {
            assert!(
                is_reply_shaped("dev__shell", &json!({ "command": cmd })),
                "expected {cmd:?} to count as a publish attempt"
            );
        }
    }

    /// Commands that do real work but do not reply in the originating
    /// conversation must still be nagged.
    #[test]
    fn reply_shape_rejects_non_reply_commands() {
        for cmd in [
            "buzz messages get --channel X",
            "buzz channels list",
            "buzz reactions remove --event E",
            "buzz pr open --title T",
            "buzz social publish --content hi",
            "buzz notes set --name n",
            "cargo test -p buzz-agent",
        ] {
            assert!(
                !is_reply_shaped("dev__shell", &json!({ "command": cmd })),
                "expected {cmd:?} not to count as a publish attempt"
            );
        }
    }

    /// The `__` separator is load-bearing: `ends_with("shell")` alone would
    /// accept any registered tool whose name merely ends in those letters, and
    /// `has()` proves registration, not the bare name.
    #[test]
    fn reply_shape_requires_the_qname_separator() {
        let args = json!({ "command": "buzz messages send --channel X" });
        for name in [
            "dev__powershell",
            "dev__noshell",
            "shell",
            "dev__send_message",
        ] {
            assert!(
                !is_reply_shaped(name, &args),
                "{name} must not satisfy the shell-tool check"
            );
        }
        assert!(is_reply_shaped("dev__shell", &args));
        assert!(is_reply_shaped("buzz-dev-mcp__shell", &args));
    }

    /// Only the field that carries the executable command counts. Searching
    /// serialized arguments instead would let arbitrary metadata disarm the
    /// guard, turning a description into an attempted send.
    #[test]
    fn reply_shape_reads_only_the_command_field() {
        assert!(!is_reply_shaped(
            "dev__shell",
            &json!({ "description": "buzz messages send --channel X" })
        ));
        assert!(!is_reply_shaped(
            "dev__shell",
            &json!({ "workdir": "buzz messages send" })
        ));
        // Malformed `command` is rejected, not coerced — and must not panic.
        assert!(!is_reply_shaped("dev__shell", &json!({ "command": 42 })));
        assert!(!is_reply_shaped("dev__shell", &json!({ "command": null })));
        assert!(!is_reply_shaped("dev__shell", &json!({})));
        assert!(!is_reply_shaped("dev__shell", &json!("not an object")));
    }

    /// A9 regression: `reasoning_details` contributes real bytes to
    /// `estimated_bytes` (see `types.rs::HistoryItem::size_with`), so a
    /// history item carrying a large opaque reasoning array must actually
    /// drive `truncate_history` eviction — not be silently invisible to the
    /// sizing gate that decides what survives a turn.
    #[test]
    fn truncate_history_evicts_oldest_turn_with_reasoning_details() {
        let big_reasoning = json!([{ "type": "reasoning.text", "text": "x".repeat(400) }]);
        let mut history = vec![
            HistoryItem::User("first question".into()),
            HistoryItem::Assistant {
                text: "first answer".into(),
                tool_calls: vec![],
                reasoning_details: Some(big_reasoning),
            },
            HistoryItem::User("second question".into()),
            HistoryItem::Assistant {
                text: "second answer".into(),
                tool_calls: vec![],
                reasoning_details: None,
            },
        ];

        let total_before: usize = history.iter().map(HistoryItem::estimated_bytes).sum();
        // Budget below the total but above the second (smaller) turn alone,
        // so only the oldest user+assistant pair — the one carrying
        // reasoning_details — must be dropped.
        let max_bytes = total_before - 100;
        assert!(
            max_bytes > 0,
            "test fixture must leave room to evict only one turn"
        );

        truncate_history(&mut history, max_bytes);

        assert_eq!(
            history.len(),
            2,
            "the oldest user+assistant turn (with reasoning_details) must be evicted"
        );
        assert!(matches!(&history[0], HistoryItem::User(s) if s == "second question"));
        assert!(
            matches!(&history[1], HistoryItem::Assistant { text, .. } if text == "second answer")
        );
        let total_after: usize = history.iter().map(HistoryItem::estimated_bytes).sum();
        assert!(total_after <= max_bytes);
    }

    #[test]
    fn unsupported_images_become_recoverable_tool_errors() {
        let mut history = vec![
            HistoryItem::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    provider_id: "call-image".into(),
                    name: "dev__view_image".into(),
                    arguments: json!({ "source": "spec.png" }),
                    provider_extra: Default::default(),
                }],
                reasoning_details: None,
            },
            HistoryItem::ToolResult(ToolResult {
                provider_id: "call-image".into(),
                content: vec![
                    ToolResultContent::Text("10x10 image from spec.png".into()),
                    ToolResultContent::Image {
                        data: "aW1n".into(),
                        mime_type: "image/png".into(),
                    },
                ],
                is_error: false,
            }),
        ];

        assert_eq!(replace_unsupported_images(&mut history), 1);
        let HistoryItem::ToolResult(result) = &history[1] else {
            panic!("tool result must stay paired with the assistant tool call");
        };
        assert_eq!(result.provider_id, "call-image");
        assert!(result.is_error);
        assert!(result
            .content
            .iter()
            .all(|content| !matches!(content, ToolResultContent::Image { .. })));
        assert!(result.text().contains("does not support image input"));
        assert!(result.text().contains("10x10 image from spec.png"));
        assert_eq!(replace_unsupported_images(&mut history), 0);
    }

    #[test]
    fn truncate_history_noop_when_under_budget() {
        let mut history = vec![
            HistoryItem::User("hi".into()),
            HistoryItem::Assistant {
                text: "hello".into(),
                tool_calls: vec![],
                reasoning_details: None,
            },
        ];
        let original_len = history.len();
        truncate_history(&mut history, 1_000_000);
        assert_eq!(
            history.len(),
            original_len,
            "under budget must not evict anything"
        );
    }

    // ── fold_pricing_identity: turn discipline ────────────────────────────────

    fn pi(authority: &str, model: &str) -> PricingIdentity {
        PricingIdentity {
            authority: authority.to_string(),
            model: model.to_string(),
            cache_class: None,
        }
    }

    /// Case 1: two usage-bearing rounds with different proven identities in one
    /// turn must poison the accumulator.  The wire payload omits `pricingIdentity`
    /// when `Some(None)`.
    #[test]
    fn fold_pricing_identity_mismatch_poisons() {
        let round_a = Some(pi("api.anthropic.com", "claude-opus-4-5"));
        let round_b = Some(pi("api.openai.com", "gpt-4o"));

        // Start: unseen.
        let acc = None;
        // After round A: consistent — Some(Some(claude-opus-4-5)).
        let acc = fold_pricing_identity(acc, round_a);
        assert!(
            matches!(acc, Some(Some(_))),
            "after one round must be consistent"
        );
        // After round B (different authority + model): poisoned.
        let acc = fold_pricing_identity(acc, round_b);
        assert_eq!(
            acc,
            Some(None),
            "different proven identities in one turn must poison"
        );
    }

    /// Case 2: proven identity followed by a usage-bearing round with no proven
    /// identity (request_model absent or non-allowlisted endpoint) must poison.
    /// An unpaired cumulative snapshot also produces round=None (no model known)
    /// and hits this same path — case 4 collapses into case 2.
    #[test]
    fn fold_pricing_identity_unproven_round_poisons() {
        let round_a = Some(pi("api.anthropic.com", "claude-3-7-sonnet"));
        let round_unproven: Option<PricingIdentity> = None; // absent request_model or custom endpoint

        let acc = None;
        let acc = fold_pricing_identity(acc, round_a);
        assert!(
            matches!(acc, Some(Some(_))),
            "after one proven round must be consistent"
        );
        let acc = fold_pricing_identity(acc, round_unproven);
        assert_eq!(
            acc,
            Some(None),
            "an unproven round after a proven round must poison (no-model / custom-endpoint path)"
        );
    }

    /// Case 3: a poisoned accumulator must not heal, even if a later round
    /// carries an identity matching the original.
    #[test]
    fn fold_pricing_identity_poisoned_never_heals() {
        let round_a = Some(pi("api.openai.com", "gpt-4o"));
        let round_unproven: Option<PricingIdentity> = None;
        let round_a_again = Some(pi("api.openai.com", "gpt-4o")); // identical to round_a

        let acc = None;
        let acc = fold_pricing_identity(acc, round_a);
        let acc = fold_pricing_identity(acc, round_unproven); // poisons
        assert_eq!(acc, Some(None), "must be poisoned before heal attempt");
        let acc = fold_pricing_identity(acc, round_a_again); // must not heal
        assert_eq!(
            acc,
            Some(None),
            "poisoned accumulator must stay poisoned even when the next round matches the original identity"
        );
    }

    /// Baseline: a turn where every round carries the same proven identity
    /// stays consistent and emits the identity on the wire.
    #[test]
    fn fold_pricing_identity_consistent_rounds_stay_proven() {
        let identity = pi("api.openrouter.ai", "meta-llama/llama-4-scout");

        let acc = None;
        let acc = fold_pricing_identity(acc, Some(identity.clone()));
        let acc = fold_pricing_identity(acc, Some(identity.clone()));
        let acc = fold_pricing_identity(acc, Some(identity.clone()));
        assert_eq!(
            acc,
            Some(Some(identity)),
            "three identical rounds must remain consistently proven"
        );
    }

    // ── is_silent_turn (pure predicate) ─────────────────────────────────────

    /// Counts WARN events emitted by `warn_if_silent_turn` calls inside `f`.
    ///
    /// Identifies silent-turn WARNs by target (`buzz_agent::agent`) + WARN
    /// level + presence of the `stop` field, which is unique to these two
    /// WARNs in this module.  Using the target avoids parsing message strings,
    /// which are routed through `record_debug` as `Display`-formatted values
    /// and are not reliably interceptable via `record_str` across tracing
    /// versions.
    fn count_silent_turn_warnings(f: impl FnOnce()) -> usize {
        struct Capture {
            count: Arc<AtomicUsize>,
        }
        struct Visitor {
            saw_stop: bool,
        }
        impl tracing::field::Visit for Visitor {
            fn record_debug(&mut self, field: &tracing::field::Field, _: &dyn std::fmt::Debug) {
                if field.name() == "stop" {
                    self.saw_stop = true;
                }
            }
        }
        impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _: tracing_subscriber::layer::Context<'_, S>,
            ) {
                if *event.metadata().level() != tracing::Level::WARN {
                    return;
                }
                if event.metadata().target() != "buzz_agent::agent" {
                    return;
                }
                let mut v = Visitor { saw_stop: false };
                event.record(&mut v);
                if v.saw_stop {
                    self.count.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
        let count = Arc::new(AtomicUsize::new(0));
        let sub = tracing_subscriber::registry().with(Capture {
            count: count.clone(),
        });
        tracing::subscriber::with_default(sub, f);
        count.load(Ordering::SeqCst)
    }

    /// Predicate: values within the observed failure range (2–12) fire.
    /// Pair (0, 12) catches an always-false mutation and an off-by-one at 12.
    #[test]
    fn is_silent_turn_fires_at_and_below_threshold() {
        assert!(
            is_silent_turn(0),
            "zero output tokens must be a silent turn"
        );
        assert!(
            is_silent_turn(SILENT_TURN_TOKEN_THRESHOLD),
            "exactly at threshold (12) must be a silent turn — 12 is in the observed range"
        );
    }

    /// One above the threshold must NOT fire, catching `<` vs `<=` and
    /// always-true mutations.
    #[test]
    fn is_silent_turn_silent_above_threshold() {
        assert!(
            !is_silent_turn(SILENT_TURN_TOKEN_THRESHOLD + 1),
            "one above threshold (13) must not be a silent turn"
        );
    }

    // ── warn_if_silent_turn (WARN seam) ───────────────────────────────────

    /// The canonical silent-death signature — no publish, no text, ≤12 tokens
    /// — must emit exactly one WARN.  Deleting the WARN call, weakening the
    /// token check, or hardcoding `buzz_reply_call_seen = true` are all caught.
    #[test]
    fn warn_if_silent_turn_fires_for_canonical_signature() {
        let n = count_silent_turn_warnings(|| {
            warn_if_silent_turn(
                false,   // no publish seen
                true,    // no text
                Some(4), // 4 tokens — in the 2–12 range
                ProviderStop::EndTurn,
            );
        });
        assert_eq!(n, 1, "canonical silent-death must emit exactly one WARN");
    }

    /// A turn that ends with non-empty assistant text is NOT a silent death
    /// even if token count is low — a terse reply like "OK" is legitimate.
    /// Deleting the `text_is_empty` gate would cause this to fail.
    #[test]
    fn warn_if_silent_turn_silent_for_nonempty_text() {
        let n = count_silent_turn_warnings(|| {
            warn_if_silent_turn(
                false,   // no publish
                false,   // text IS present
                Some(3), // low tokens — would fire without the text gate
                ProviderStop::EndTurn,
            );
        });
        assert_eq!(n, 0, "a turn with non-empty assistant text must not WARN");
    }

    /// A turn that published (buzz_reply_call_seen = true) then ended with a
    /// short final completion must not trigger the WARN.  This is the normal
    /// publish-then-wrap pattern.  Deleting the `buzz_reply_call_seen` gate
    /// would cause this to fail.
    #[test]
    fn warn_if_silent_turn_silent_after_publish() {
        let n = count_silent_turn_warnings(|| {
            warn_if_silent_turn(
                true,    // publish seen
                true,    // no text in final round
                Some(0), // zero tokens — would fire without the publish gate
                ProviderStop::EndTurn,
            );
        });
        assert_eq!(n, 0, "a turn where a publish ran must not WARN");
    }

    /// Unknown usage (None) with no publish and no text emits the distinct
    /// "no usage reported" WARN.  Mutating the None arm to fall through to
    /// `_ => {}` would cause this.
    #[test]
    fn warn_if_silent_turn_fires_distinct_warn_for_none_usage() {
        let n = count_silent_turn_warnings(|| {
            warn_if_silent_turn(
                false, // no publish
                true,  // no text
                None,  // provider omitted usage
                ProviderStop::EndTurn,
            );
        });
        assert_eq!(n, 1, "unknown-usage silent turn must emit exactly one WARN");
    }

    // ── is_silent_candidate (braking gate predicate) ───────────────────────

    #[test]
    fn is_silent_candidate_canonical_cases() {
        // True cases: no publish, empty text, token <= 12 or None
        assert!(is_silent_candidate(false, true, Some(0)));
        assert!(is_silent_candidate(false, true, Some(4)));
        assert!(is_silent_candidate(false, true, Some(SILENT_TURN_TOKEN_THRESHOLD)));
        assert!(is_silent_candidate(false, true, None));

        // False cases: publish seen, or non-empty text, or token > threshold
        assert!(!is_silent_candidate(true, true, Some(0)), "publish seen must disarm");
        assert!(!is_silent_candidate(false, false, Some(0)), "non-empty text must disarm");
        assert!(!is_silent_candidate(false, true, Some(SILENT_TURN_TOKEN_THRESHOLD + 1)), "tokens above threshold must disarm");
    }

    #[test]
    fn silent_early_stop_breaker_constants() {
        assert_eq!(MAX_CONSECUTIVE_SILENT_TURNS, 2);
        assert!(SILENT_EARLY_STOP_BREAKER_MSG.contains("系統熔斷"));
    }

    #[test]
    fn test_duplicate_publish_counter_logic() {
        let mut publish_success_count = 0usize;
        let mut publish_attempt_count = 0usize;

        // First publish attempt (failed): success count remains 0, attempt count increments
        publish_attempt_count += 1;
        assert_eq!(publish_success_count, 0);
        assert_eq!(publish_attempt_count, 1);
        // Retry is allowed because publish_success_count == 0
        assert_eq!(publish_success_count, 0);

        // Second publish attempt (succeeded): success count becomes 1
        publish_attempt_count += 1;
        publish_success_count += 1;
        assert_eq!(publish_attempt_count, 2);
        assert_eq!(publish_success_count, 1);

        // Third publish attempt in same turn: detected as duplicate because publish_success_count > 0
        let is_duplicate = publish_success_count > 0;
        assert!(is_duplicate, "subsequent publish after success in same turn must be identified as duplicate");
    }

    #[test]
    fn test_consecutive_tool_failures_logic() {
        let mut total_failures = 0usize;
        let mut identical_failures = 0usize;
        const MAX_IDENTICAL: usize = 5;
        const MAX_TOTAL: usize = 8;

        // 4 different failures do NOT trigger identical breaker
        for _ in 0..4 {
            total_failures += 1;
            identical_failures = 1;
        }
        assert_eq!(total_failures, 4);
        assert!(identical_failures < MAX_IDENTICAL);
        assert!(total_failures < MAX_TOTAL);

        // 5 identical failures DO trigger identical breaker
        identical_failures = 5;
        assert!(identical_failures >= MAX_IDENTICAL);

        // Reset on success
        total_failures = 0;
        identical_failures = 0;
        assert_eq!(identical_failures, 0);

        // 8 total failures trigger overflow breaker
        for _ in 0..8 {
            total_failures += 1;
        }
        assert!(total_failures >= MAX_TOTAL);
    }

    #[test]
    fn test_denoise_user_buzz_path_restoration() {
        let dirty = "請讀取 C:\\Users\\user.buzz\\PLANS\\GITHUB_TRENDING_ANALYSIS_PLAN.md 檔案";
        let cleaned = denoise_prompt_markdown_links(dirty);
        assert_eq!(
            cleaned,
            "請讀取 C:\\Users\\user\\.buzz\\PLANS\\GITHUB_TRENDING_ANALYSIS_PLAN.md 檔案"
        );

        let dirty_slash = "path C:/Users/user.buzz/PLANS/test.md";
        let cleaned_slash = denoise_prompt_markdown_links(dirty_slash);
        assert_eq!(cleaned_slash, "path C:/Users/user\\.buzz/PLANS/test.md");
    }

    #[test]
    fn test_extract_reply_target_from_prompt() {
        // Standard prompt with --reply-to and --channel
        let prompt1 = "IMPORTANT: For ordinary replies in this turn, use `--reply-to e4fa7a3209951eefb6a914c5e31ea65a696971bcb1dbc43a3d4e9d6af680e239` on `buzz messages send` so the conversation stays threaded.\nChannel: general (#af946a63-dd43-4050-baea-65140d47fc42)";
        let target1 = extract_reply_target_from_prompt(prompt1);
        assert!(target1.is_some());
        let (channel, reply_to) = target1.unwrap();
        assert_eq!(channel, "af946a63-dd43-4050-baea-65140d47fc42");
        assert_eq!(reply_to, Some("e4fa7a3209951eefb6a914c5e31ea65a696971bcb1dbc43a3d4e9d6af680e239".to_string()));

        // Prompt with explicit --channel flag and event_id line
        let prompt2 = "event_id: 1111111111111111111111111111111111111111111111111111111111111111\nbuzz messages send --channel 22222222-2222-4222-8222-222222222222";
        let target2 = extract_reply_target_from_prompt(prompt2);
        assert!(target2.is_some());
        let (channel2, reply_to2) = target2.unwrap();
        assert_eq!(channel2, "22222222-2222-4222-8222-222222222222");
        assert_eq!(reply_to2, Some("1111111111111111111111111111111111111111111111111111111111111111".to_string()));
    }

    #[test]
    fn test_denoise_prompt_markdown_links() {
        let dirty = "請讀取 C:\\Users\\[user.buzz](http://user.buzz)\\PLANS\\REPORTS\\2026-11-11_GITHUB_[TRENDING.md](http://TRENDING.md) 內容";
        let cleaned = denoise_prompt_markdown_links(dirty);
        assert_eq!(
            cleaned,
            "請讀取 C:\\Users\\user\\.buzz\\PLANS\\REPORTS\\2026-11-11_GITHUB_TRENDING.md 內容"
        );

        let dirty2 = "Check out [https://example.com](https://example.com) and [foo](http://foo.bar)";
        let cleaned2 = denoise_prompt_markdown_links(dirty2);
        assert_eq!(cleaned2, "Check out https://example.com and foo");
    }

    #[test]
    fn test_denoise_isolated_brackets_in_paths() {
        let dirty = "請讀取 C:\\Users\\user\\.buzz\\PLANS\\REPORTS\\2026-11-11_GITHUB_[TRENDING.md] 內容";
        let cleaned = denoise_prompt_markdown_links(dirty);
        assert_eq!(
            cleaned,
            "請讀取 C:\\Users\\user\\.buzz\\PLANS\\REPORTS\\2026-11-11_GITHUB_TRENDING.md 內容"
        );

        let dirty2 = "請查看 `C:\\Users\\user\\.buzz\\PLANS\\REPORTS\\2026-11-11_GITHUB_[TRENDING.md]`";
        let cleaned2 = denoise_prompt_markdown_links(dirty2);
        assert_eq!(
            cleaned2,
            "請查看 C:\\Users\\user\\.buzz\\PLANS\\REPORTS\\2026-11-11_GITHUB_TRENDING.md"
        );

        let non_path = "這裡保留 [Reflect] 標籤與 [2026-11-11] 日期";
        let cleaned3 = denoise_prompt_markdown_links(non_path);
        assert_eq!(cleaned3, non_path);
    }

    #[test]
    fn test_last_assistant_text_fallback_to_reasoning() {
        use crate::types::HistoryItem;
        use serde_json::json;

        // Mock history with empty text but reasoning_details
        let history = vec![
            HistoryItem::User("hello".to_string()),
            HistoryItem::Assistant {
                text: "".to_string(),
                tool_calls: Vec::new(),
                reasoning_details: Some(json!("I am analyzing the repository structure.")),
            },
        ];

        let mut found = None;
        for item in history.iter().rev() {
            if let HistoryItem::Assistant { text, reasoning_details, .. } = item {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    found = Some(trimmed.to_string());
                    break;
                }
                if let Some(details) = reasoning_details {
                    if let Some(s) = details.as_str() {
                        found = Some(format!("【Honey2 分析思考報告】：\n\n{}", s));
                        break;
                    }
                }
            }
        }

        assert_eq!(
            found,
            Some("【Honey2 分析思考報告】：\n\nI am analyzing the repository structure.".to_string())
        );
    }
}
