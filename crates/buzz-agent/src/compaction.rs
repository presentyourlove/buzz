//! 四層漸進上下文壓縮核心模組（Tier 0 剪枝、Tier 2 無 LLM 壓縮備援、WorkingSetLedger、D9 合成訊息識別）。
//!
//! 本模組僅包含純確定性邏輯，嚴格禁止 `unsafe` 程式碼與魔術數字（Magic Numbers）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{PRUNE_MIN_TOOL_RESULT_BYTES, SNIPPET_ARG_MAX_BYTES};
use crate::handoff::estimate_history_tokens;
use crate::types::{HistoryItem, ToolCall, ToolResult, ToolResultContent};

/// 工作集帳本追蹤之最多檔案紀錄筆數（採 LRU 滑動保留最新狀態）。
pub const MAX_LEDGER_FILES: usize = 32;
/// 工作集帳本追蹤之最多近期指令紀錄筆數。
pub const MAX_LEDGER_COMMANDS: usize = 15;
/// 工作集帳本記錄最近錯誤摘要的最大位元組數。
pub const MAX_LEDGER_ERROR_BYTES: usize = 256;
/// Tier 1 交接保留逐字尾端之絕對上限（16 KiB），避免極端大視窗過度膨脹交接後上下文。
pub const HANDOFF_VERBATIM_TAIL_CAP_BYTES: usize = 16 * 1024;
/// 滾動摘要萃取前次 `[Context Handoff]` 摘要時的最大位元組預算（16 KiB）。
pub const PREVIOUS_SUMMARY_MAX_BYTES: usize = 16 * 1024;

/// Tier 0 剪枝佔位符前綴，用於冪等檢查避免重複剪枝已摘要之 ToolResult。
pub const PRUNED_RESULT_PREFIX: &str = "[已剪枝:";
/// Tier 1 LLM 交接摘要訊息前綴。
pub const CONTEXT_HANDOFF_PREFIX: &str = "[Context Handoff]";
/// Tier 2 無 LLM 確定性壓縮錨點前綴。
pub const DETERMINISTIC_COMPACTION_PREFIX: &str = "[Deterministic Context Compaction]";
/// 第 35 輪 proximity warning 注入之系統警示前綴（屬合成 User 訊息）。
pub const SYNTHETIC_ALERT_PREFIX: &str = "[系統警示]";
/// 壓縮後附加於用戶提問末尾的守衛標記開頭，多次交接時需先剝除以防重複疊加。
pub const SYSTEM_NOTE_MARKER: &str = "\n\n[System Note: Context has been compacted.";

/// 未能於 ToolCall 對應表中找到名稱時的預設顯示標籤。
const UNKNOWN_TOOL_NAME: &str = "unknown_tool";
/// 摘要工具參數時依序檢查的高價值欄位鍵名。
const ARG_SUMMARY_KEYS: &[&str] = &[
    "path",
    "file_path",
    "filepath",
    "TargetFile",
    "DirectoryPath",
    "SearchPath",
    "command",
    "cmd",
    "pattern",
    "query",
    "url",
    "skill",
];

/// 以 UTF-8 字元邊界安全截斷字串至 `max_bytes`（含省略號），絕不切斷多 Byte 中文字元或 Emoji。
pub(crate) fn clamp_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_owned();
    }
    if max_bytes < 4 {
        let mut cut = max_bytes.min(s.len());
        while cut > 0 && !s.is_char_boundary(cut) {
            cut -= 1;
        }
        return s[..cut].to_owned();
    }
    let target = max_bytes - "…".len();
    let mut cut = target;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &s[..cut])
}

/// 將字串壓成單行並依 UTF-8 安全邊界截斷至 `max_bytes`，供快照與帳本條目使用。
pub(crate) fn sanitize_single_line(s: &str, max_bytes: usize) -> String {
    let single_line: String = s
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let collapsed = single_line.split_whitespace().collect::<Vec<_>>().join(" ");
    clamp_bytes(&collapsed, max_bytes)
}

/// 判斷 `HistoryItem::User(text)` 是否為系統自動注入的合成提示訊息（D9 修復核心）。
///
/// 因為迴圈警示與截斷恢復訊息也是透過 `HistoryItem::User` 送進模型，若未加以區分，
/// 後續交接與自動發送補救會把系統訊息當成用戶提問，導致原始任務與 `<context>` 中的 `channel_id` 遺失。
pub fn is_synthetic_system_user_message(text: &str) -> bool {
    text.starts_with(CONTEXT_HANDOFF_PREFIX)
        || text.starts_with(SYNTHETIC_ALERT_PREFIX)
        || text == crate::agent::MAX_TOKENS_RECOVERY_MESSAGE
        || text.starts_with(DETERMINISTIC_COMPACTION_PREFIX)
}

/// 剝除先前壓縮附加的 `[System Note: Context has been compacted...]` 後綴，
/// 避免歷經多次交接後在原始提問後方堆疊重複的系統守衛字串。
pub fn strip_compaction_system_note(text: &str) -> &str {
    match text.find(SYSTEM_NOTE_MARKER) {
        Some(idx) => &text[..idx],
        None => text,
    }
}

/// 由新到舊搜尋最後一則「真實用戶提問」（跳過交接摘要、第 35 輪警示等合成訊息）；
/// 若歷史內找不到（例如經極端截斷），則退回 session 保留之 `fallback_original_task`。
pub fn find_latest_real_user_prompt(
    history: &[HistoryItem],
    fallback_original_task: Option<&str>,
) -> Option<String> {
    history
        .iter()
        .rev()
        .find_map(|item| match item {
            HistoryItem::User(s) if !is_synthetic_system_user_message(s) => {
                Some(strip_compaction_system_note(s).to_owned())
            }
            _ => None,
        })
        .or_else(|| fallback_original_task.map(|s| strip_compaction_system_note(s).to_owned()))
}

/// 由新到舊掃描非合成 `User` 訊息以提取發訊頻道目標 `(channel_id, reply_to)`；
/// 若仍找不到（例如高輪數壓縮後），退回檢查 `original_task`（D9 修復）。
pub fn find_reply_target_in_history(
    history: &[HistoryItem],
    original_task: Option<&str>,
) -> Option<(String, Option<String>)> {
    for item in history.iter().rev() {
        if let HistoryItem::User(s) = item {
            if is_synthetic_system_user_message(s) {
                continue;
            }
            if let Some(target) = crate::agent::extract_reply_target_from_prompt(s) {
                return Some(target);
            }
        }
    }
    original_task.and_then(crate::agent::extract_reply_target_from_prompt)
}

/// 從 `ToolCall.arguments` 提取關鍵參數摘要（修 D2：`path`、`command`、`pattern` 等），
/// 以單行且 UTF-8 安全的方式限制在 `max_bytes` 內。
pub fn summarize_tool_args(_tool_name: &str, args: &Value, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    let Some(map) = args.as_object() else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for key in ARG_SUMMARY_KEYS {
        if let Some(val) = map.get(*key) {
            let formatted = match val {
                Value::String(s) if !s.trim().is_empty() => {
                    let clean = sanitize_single_line(s, max_bytes);
                    format!("{key}=\"{clean}\"")
                }
                Value::Number(n) => format!("{key}={n}"),
                Value::Bool(b) => format!("{key}={b}"),
                _ => continue,
            };
            parts.push(formatted);
            // 僅取前 2 個關鍵參數避免單一工具呼叫佔用過多摘要空間
            if parts.len() >= 2 {
                break;
            }
        }
    }
    if parts.is_empty() {
        for (k, v) in map {
            if let Value::String(s) = v {
                if !s.trim().is_empty() {
                    let clean = sanitize_single_line(s, max_bytes);
                    parts.push(format!("{k}=\"{clean}\""));
                    break;
                }
            }
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        clamp_bytes(&parts.join(", "), max_bytes)
    }
}

/// 工作集帳本中的檔案操作紀錄項目。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerFileEntry {
    pub path: String,
    pub op: String,
    pub status: String,
}

/// 工作集帳本中的近期指令執行紀錄項目。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerCommandEntry {
    pub command_summary: String,
    pub status: String,
}

/// 確定性工作集帳本（修復 D8）：由系統程式直接記錄已觸碰檔案、指令與近期錯誤，不依賴 LLM 主動更新 todo。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkingSetLedger {
    pub files: Vec<LedgerFileEntry>,
    pub commands: Vec<LedgerCommandEntry>,
    pub last_error: Option<String>,
}

impl WorkingSetLedger {
    /// 依據已完成執行的 `ToolCall` 與其 `ToolResult` 確定性更新工作集帳本。
    pub fn record_tool_execution(&mut self, call: &ToolCall, result: &ToolResult) {
        let status = if result.is_error { "error" } else { "ok" };
        let bare = call.name.rsplit("__").next().unwrap_or(call.name.as_str());

        match bare {
            "read_file" | "view_file" | "list_dir" | "find_by_name" | "grep_search" => {
                if let Some(path) = extract_path_arg(&call.arguments) {
                    self.upsert_file(path, "read", status);
                }
            }
            "write_file" | "write_to_file" => {
                if let Some(path) = extract_path_arg(&call.arguments) {
                    self.upsert_file(path, "write", status);
                }
            }
            "edit_file" | "replace_file_content" | "multi_replace_file_content" | "apply_patch" => {
                if let Some(path) = extract_path_arg(&call.arguments) {
                    self.upsert_file(path, "edit", status);
                }
            }
            "shell" | "run_command" | "bash" => {
                if let Some(cmd) = extract_command_arg(&call.arguments) {
                    self.push_command(cmd, status);
                }
            }
            _ => {}
        }

        if result.is_error {
            let err_text = sanitize_single_line(&result.text(), MAX_LEDGER_ERROR_BYTES);
            if !err_text.is_empty() {
                self.last_error = Some(err_text);
            }
        }
    }

    /// 由既有對話歷史重放重建工作集帳本（例如由舊版無帳本 history 補建）。
    pub fn rebuild_from_history(history: &[HistoryItem]) -> Self {
        let mut ledger = Self::default();
        let mut call_map: HashMap<String, &ToolCall> = HashMap::new();
        for item in history {
            match item {
                HistoryItem::Assistant { tool_calls, .. } => {
                    for call in tool_calls {
                        call_map.insert(call.provider_id.clone(), call);
                    }
                }
                HistoryItem::ToolResult(result) => {
                    if let Some(call) = call_map.get(&result.provider_id) {
                        ledger.record_tool_execution(call, result);
                    }
                }
                HistoryItem::User(_) => {}
            }
        }
        ledger
    }

    /// 帳本是否尚無任何紀錄（空帳本在摘要／交接時不輸出多餘段落，維持舊測試相容性）。
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.commands.is_empty() && self.last_error.is_none()
    }

    /// 將工作集帳本格式化為可讀性高的 Markdown 區段，供注入交接摘要與 Checkpoint。
    pub fn render_markdown(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut out = String::from("[Working Set Ledger]");
        if !self.files.is_empty() {
            out.push_str("\n### Touched Files");
            for f in &self.files {
                out.push_str(&format!("\n- {} [{}]: {}", f.op, f.status, f.path));
            }
        }
        if !self.commands.is_empty() {
            out.push_str("\n### Recent Commands");
            for c in &self.commands {
                out.push_str(&format!("\n- [{}]: {}", c.status, c.command_summary));
            }
        }
        if let Some(err) = &self.last_error {
            out.push_str("\n### Last Tool Error\n- ");
            out.push_str(err);
        }
        out
    }

    fn upsert_file(&mut self, path: String, op: &str, status: &str) {
        self.files.retain(|f| f.path != path);
        self.files.push(LedgerFileEntry {
            path,
            op: op.to_owned(),
            status: status.to_owned(),
        });
        if self.files.len() > MAX_LEDGER_FILES {
            let overflow = self.files.len() - MAX_LEDGER_FILES;
            self.files.drain(0..overflow);
        }
    }

    fn push_command(&mut self, command_summary: String, status: &str) {
        self.commands.push(LedgerCommandEntry {
            command_summary,
            status: status.to_owned(),
        });
        if self.commands.len() > MAX_LEDGER_COMMANDS {
            let overflow = self.commands.len() - MAX_LEDGER_COMMANDS;
            self.commands.drain(0..overflow);
        }
    }
}

fn extract_path_arg(args: &Value) -> Option<String> {
    for key in [
        "path",
        "file_path",
        "filepath",
        "TargetFile",
        "DirectoryPath",
        "SearchPath",
    ] {
        if let Some(val) = args.get(key).and_then(|v| v.as_str()) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(sanitize_single_line(trimmed, SNIPPET_ARG_MAX_BYTES));
            }
        }
    }
    None
}

fn extract_command_arg(args: &Value) -> Option<String> {
    for key in ["command", "cmd"] {
        if let Some(val) = args.get(key).and_then(|v| v.as_str()) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(sanitize_single_line(trimmed, SNIPPET_ARG_MAX_BYTES));
            }
        }
    }
    None
}

/// 不可分割的完整交互單元：`Assistant` 及其緊隨的所有 `ToolResult`，或單一 `User` 項目。
#[derive(Debug, Clone, Copy)]
struct RoundGroup {
    start: usize,
    end: usize,
    is_assistant: bool,
}

fn group_history_rounds(history: &[HistoryItem]) -> Vec<RoundGroup> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < history.len() {
        match &history[i] {
            HistoryItem::Assistant { .. } => {
                let start = i;
                i += 1;
                while i < history.len() && matches!(history[i], HistoryItem::ToolResult(_)) {
                    i += 1;
                }
                groups.push(RoundGroup {
                    start,
                    end: i,
                    is_assistant: true,
                });
            }
            HistoryItem::User(_) => {
                groups.push(RoundGroup {
                    start: i,
                    end: i + 1,
                    is_assistant: false,
                });
                i += 1;
            }
            HistoryItem::ToolResult(_) => {
                // 若出現孤立的 ToolResult，併入前一群組邊界避免切分後再製造孤兒
                if let Some(last) = groups.last_mut() {
                    last.end = i + 1;
                } else {
                    groups.push(RoundGroup {
                        start: i,
                        end: i + 1,
                        is_assistant: false,
                    });
                }
                i += 1;
            }
        }
    }
    groups
}

fn group_pressure_bytes(history: &[HistoryItem], group: RoundGroup) -> usize {
    history[group.start..group.end]
        .iter()
        .map(HistoryItem::context_pressure_bytes)
        .sum()
}

/// 計算保留逐字尾端的起始索引（對齊完整回合群組邊界，絕不拆散 `Assistant` 與 `ToolResult`）。
///
/// 由新到舊（若最後一項為當前 `User` 提問則先行納入保護並自前一個群組開始計算）納入完整群組，
/// 直到已納入至少 `min_rounds` 個 Assistant 群組且再加入下一群組將超過 `tail_budget_bytes`。
pub fn compute_protected_tail_start(
    history: &[HistoryItem],
    tail_budget_bytes: usize,
    min_rounds: usize,
) -> usize {
    if history.is_empty() {
        return 0;
    }
    let groups = group_history_rounds(history);
    if groups.is_empty() {
        return 0;
    }

    // 若最後一個群組是當前 User 提問，一律納入 tail 保護，但不計入 Assistant 回合配額
    let mut scan_end = groups.len();
    let mut tail_start = history.len();
    if let Some(last) = groups.last() {
        if !last.is_assistant {
            tail_start = last.start;
            scan_end -= 1;
        }
    }

    let mut bytes_used = 0usize;
    let mut assistant_rounds = 0usize;
    for group in groups[..scan_end].iter().rev() {
        let g_bytes = group_pressure_bytes(history, *group);
        let next_bytes = bytes_used.saturating_add(g_bytes);
        if assistant_rounds >= min_rounds && next_bytes > tail_budget_bytes {
            break;
        }
        bytes_used = next_bytes;
        if group.is_assistant {
            assistant_rounds += 1;
        }
        tail_start = group.start;
    }

    tail_start
}

/// 供 Tier 1 `handoff()` 提取置於 `[Context Handoff]` 後方的近期逐字尾端項目（D3 修復）。
///
/// 扣除最後即將由 `handoff()` 加上 `[System Note]` 重新注入的真實 `User` 提問與舊有交接錨點後，
/// 在 `min(tail_budget_bytes, HANDOFF_VERBATIM_TAIL_CAP_BYTES)` 預算內按完整回合群組提取近期內容。
pub fn extract_verbatim_tail_for_handoff(
    history: &[HistoryItem],
    tail_budget_bytes: usize,
    min_rounds: usize,
    current_prompt: Option<&str>,
) -> Vec<HistoryItem> {
    if history.is_empty() {
        return Vec::new();
    }
    let effective_budget = tail_budget_bytes.min(HANDOFF_VERBATIM_TAIL_CAP_BYTES);
    if effective_budget == 0 {
        return Vec::new();
    }

    // 若最末項就是即將由 handoff() 重注的當前提問，先扣除以免與 guarded_prompt 重複
    let end_idx = match history.last() {
        Some(HistoryItem::User(s))
            if !is_synthetic_system_user_message(s)
                && current_prompt.is_some_and(|p| strip_compaction_system_note(s) == p) =>
        {
            history.len() - 1
        }
        _ => history.len(),
    };
    let candidate = &history[..end_idx];
    if candidate.is_empty() {
        return Vec::new();
    }

    let groups = group_history_rounds(candidate);
    let mut chosen_groups: Vec<RoundGroup> = Vec::new();
    let mut bytes_used = 0usize;
    let mut assistant_rounds = 0usize;

    for group in groups.iter().rev() {
        // 舊的交接錨點將合併至新版 # Previous Handoff Summary，不再重複放進逐字尾端
        let is_old_anchor = group.end == group.start + 1
            && matches!(
                &candidate[group.start],
                HistoryItem::User(s)
                    if s.starts_with(CONTEXT_HANDOFF_PREFIX)
                        || s.starts_with(DETERMINISTIC_COMPACTION_PREFIX)
            );
        if is_old_anchor {
            continue;
        }

        let g_bytes = group_pressure_bytes(candidate, *group);
        let next_bytes = bytes_used.saturating_add(g_bytes);
        if next_bytes > effective_budget {
            // 若加入此群組會超出預算，停止納入手中較舊群組以防撐爆小視窗測試或上限
            break;
        }
        chosen_groups.push(*group);
        bytes_used = next_bytes;
        if group.is_assistant {
            assistant_rounds += 1;
        }
        if assistant_rounds >= min_rounds && bytes_used >= effective_budget {
            break;
        }
    }

    chosen_groups.reverse();
    let mut out = Vec::new();
    for g in chosen_groups {
        // 防禦性保證：絕不輸出以孤兒 ToolResult 開頭的片段
        if out.is_empty() && matches!(candidate[g.start], HistoryItem::ToolResult(_)) {
            continue;
        }
        out.extend(candidate[g.start..g.end].iter().cloned());
    }
    out
}

/// Tier 0 確定性剪枝執行統計資訊（供可觀測性結構化日誌記錄）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneStats {
    pub pruned_count: usize,
    pub bytes_before: usize,
    pub bytes_after: usize,
}

/// Tier 0 確定性微壓縮剪枝（修復 D7，零 LLM 成本）：
/// 當總 context pressure bytes 達 `prune_threshold_pct`（預設 60%）時，
/// 對保護尾端之前的舊 `ToolResult`（長度 > 512 bytes）進行批次剪枝至 `prune_target_pct`（預設 40%）水位。
/// 全程保留 `provider_id`、`is_error` 及對應 `Assistant` 物件，100% 維持協定與 Cache 一致性。
pub fn prune_old_tool_results(
    history: &mut [HistoryItem],
    max_context_tokens: u64,
    prune_threshold_pct: u64,
    prune_target_pct: u64,
    tail_budget_pct: u64,
    tail_min_rounds: usize,
) -> PruneStats {
    let bytes_before: usize = history
        .iter()
        .map(HistoryItem::context_pressure_bytes)
        .sum();
    let trigger_bytes = percent_of_tokens_as_bytes(max_context_tokens, prune_threshold_pct);
    let target_bytes = percent_of_tokens_as_bytes(max_context_tokens, prune_target_pct);
    if bytes_before < trigger_bytes || bytes_before <= target_bytes {
        return PruneStats {
            pruned_count: 0,
            bytes_before,
            bytes_after: bytes_before,
        };
    }

    let call_lookup = build_tool_call_lookup(history);
    let tail_budget_bytes = percent_of_tokens_as_bytes(max_context_tokens, tail_budget_pct);
    let tail_start = compute_protected_tail_start(history, tail_budget_bytes, tail_min_rounds);

    let mut current_bytes = bytes_before;
    let mut pruned_count = 0usize;

    for item in history[..tail_start].iter_mut() {
        if current_bytes <= target_bytes {
            break;
        }
        let HistoryItem::ToolResult(tr) = item else {
            continue;
        };
        if is_already_pruned_result(tr) {
            continue;
        }
        let orig_bytes = HistoryItem::ToolResult(tr.clone()).context_pressure_bytes();
        if orig_bytes <= PRUNE_MIN_TOOL_RESULT_BYTES {
            continue;
        }

        let (tool_name, args_summary) = call_lookup
            .get(tr.provider_id.as_str())
            .cloned()
            .unwrap_or_else(|| (UNKNOWN_TOOL_NAME.to_owned(), String::new()));
        let args_display = if args_summary.is_empty() {
            "none".to_owned()
        } else {
            args_summary
        };
        let placeholder = format!(
            "{PRUNED_RESULT_PREFIX} tool={tool_name} args={args_display} original_bytes={orig_bytes}; 若需完整內容請重新讀取]"
        );
        let new_bytes = tr.provider_id.len() + placeholder.len();
        tr.content = vec![ToolResultContent::Text(placeholder)];
        current_bytes = current_bytes
            .saturating_sub(orig_bytes)
            .saturating_add(new_bytes);
        pruned_count += 1;
    }

    if pruned_count > 0 {
        tracing::info!(
            target: "buzz_agent::compaction",
            tier = "tier0_prune",
            pruned_count,
            bytes_before,
            bytes_after = current_bytes,
            "tier0 deterministic pruning completed"
        );
    }

    PruneStats {
        pruned_count,
        bytes_before,
        bytes_after: current_bytes,
    }
}

fn build_tool_call_lookup(history: &[HistoryItem]) -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    for item in history {
        if let HistoryItem::Assistant { tool_calls, .. } = item {
            for call in tool_calls {
                let args = summarize_tool_args(&call.name, &call.arguments, SNIPPET_ARG_MAX_BYTES);
                map.insert(call.provider_id.clone(), (call.name.clone(), args));
            }
        }
    }
    map
}

fn is_already_pruned_result(tr: &ToolResult) -> bool {
    tr.content.len() == 1
        && matches!(
            &tr.content[0],
            ToolResultContent::Text(s) if s.starts_with(PRUNED_RESULT_PREFIX)
        )
}

fn percent_of_tokens_as_bytes(max_context_tokens: u64, percent: u64) -> usize {
    let tokens = max_context_tokens / 100 * percent;
    usize::try_from(tokens).unwrap_or(usize::MAX)
}

/// Tier 2 無 LLM 確定性壓縮備援（修復 D4、D5）：
/// 當摘要呼叫失敗（429 / 網路錯誤 / 空回應）或達 `max_handoffs` 上限時，
/// 檢查實際估算 token 是否確實超過目標水位；若超標則強制剪枝、提取帳本與前次摘要錨點，
/// 並按完整回合邊界移除中間最舊回合直到降至目標水位以下。
///
/// 若 `history` 本身的實際估算 token 未超標（例如以模擬高 usage 測試小歷史），則不改動歷史（回傳 `false`）。
pub fn compact_without_llm(
    history: &mut Vec<HistoryItem>,
    ledger: &WorkingSetLedger,
    original_task: Option<&str>,
    max_context_tokens: u64,
    target_pct: u64,
    tail_budget_pct: u64,
    tail_min_rounds: usize,
) -> bool {
    let target_tokens = max_context_tokens / 100 * target_pct;
    let initial_tokens = estimate_history_tokens(history, None);
    if initial_tokens <= target_tokens {
        return false;
    }

    // 步驟 1：先強制對保護尾端外的所有大型 ToolResult 執行剪枝（門檻設為 0 強制檢查）
    let mut changed = prune_old_tool_results(
        history,
        max_context_tokens,
        0,
        target_pct,
        tail_budget_pct,
        tail_min_rounds,
    )
    .pruned_count
        > 0;

    if estimate_history_tokens(history, None) <= target_tokens {
        return changed;
    }

    // 步驟 2：提取前次摘要與最近真實 User 提問，按回合邊界淘汰最舊群組
    let prev_summary = extract_previous_handoff_summary(history);
    let latest_prompt = find_latest_real_user_prompt(history, original_task);
    let tail_budget_bytes = percent_of_tokens_as_bytes(max_context_tokens, tail_budget_pct);
    let mut kept_tail = extract_verbatim_tail_for_handoff(
        history,
        tail_budget_bytes,
        tail_min_rounds,
        latest_prompt.as_deref(),
    );

    let anchor = build_deterministic_anchor_message(prev_summary.as_deref(), ledger, original_task);

    // 若錨點 + 尾端仍超出 target_tokens，自 kept_tail 最舊的完整回合群組開始淘汰
    let anchor_bytes = HistoryItem::User(anchor.clone()).context_pressure_bytes();
    let prompt_bytes = latest_prompt
        .as_ref()
        .map(|p| HistoryItem::User(p.clone()).context_pressure_bytes())
        .unwrap_or(0);
    let target_bytes = usize::try_from(target_tokens).unwrap_or(usize::MAX);

    loop {
        let tail_bytes: usize = kept_tail
            .iter()
            .map(HistoryItem::context_pressure_bytes)
            .sum();
        if anchor_bytes + tail_bytes + prompt_bytes <= target_bytes || kept_tail.is_empty() {
            break;
        }
        let groups = group_history_rounds(&kept_tail);
        if let Some(first) = groups.first() {
            kept_tail.drain(0..first.end);
        } else {
            break;
        }
    }

    history.clear();
    history.push(HistoryItem::User(anchor));
    history.extend(kept_tail);
    if let Some(prompt) = latest_prompt {
        history.push(HistoryItem::User(prompt));
    }
    changed = true;

    let final_tokens = estimate_history_tokens(history, None);
    tracing::warn!(
        target: "buzz_agent::compaction",
        tier = "tier2_deterministic_fallback",
        tokens_before = initial_tokens,
        tokens_after = final_tokens,
        remaining_items = history.len(),
        "tier2 deterministic compaction applied without LLM"
    );

    changed
}

/// 自歷史中尋找最新一筆既有交接或確定性壓縮摘要，供滾動式合併使用（修復 D1）。
pub(crate) fn extract_previous_handoff_summary(history: &[HistoryItem]) -> Option<String> {
    history.iter().rev().find_map(|item| match item {
        HistoryItem::User(s)
            if s.starts_with(CONTEXT_HANDOFF_PREFIX)
                || s.starts_with(DETERMINISTIC_COMPACTION_PREFIX) =>
        {
            Some(clamp_bytes(s, PREVIOUS_SUMMARY_MAX_BYTES))
        }
        _ => None,
    })
}

fn build_deterministic_anchor_message(
    prev_summary: Option<&str>,
    ledger: &WorkingSetLedger,
    original_task: Option<&str>,
) -> String {
    let mut parts = vec![DETERMINISTIC_COMPACTION_PREFIX.to_owned()];
    if let Some(task) = original_task {
        parts.push(format!(
            "Original Task Anchor:\n{}",
            clamp_bytes(
                strip_compaction_system_note(task),
                HANDOFF_VERBATIM_TAIL_CAP_BYTES / 4
            )
        ));
    }
    if let Some(prev) = prev_summary {
        parts.push(format!(
            "Previous Summary:\n{}",
            clamp_bytes(prev, PREVIOUS_SUMMARY_MAX_BYTES / 2)
        ));
    }
    if !ledger.is_empty() {
        parts.push(ledger.render_markdown());
    }
    parts.join("\n\n")
}

/// Tier 3 緊急最小保底壓縮：當反應式減半階梯皆已失敗但 `history` 仍超過 2 項時，
/// 不再依賴可能高估的 `max_context_tokens`，直接壓縮為「確定性帳本錨點 +（若安全）最後 1 回合 + 最新用戶提問」，
/// 以挽救因摘要模型過載或視窗低估導致的 Context 400。
pub fn emergency_minimal_compaction(
    history: &mut Vec<HistoryItem>,
    ledger: &WorkingSetLedger,
    original_task: Option<&str>,
) -> bool {
    const EMERGENCY_LAST_ROUND_CAP_BYTES: usize = 2048;
    if history.len() <= 2 {
        return false;
    }
    let prev_summary = extract_previous_handoff_summary(history);
    let latest_prompt = find_latest_real_user_prompt(history, original_task);
    let last_round = extract_verbatim_tail_for_handoff(
        history,
        EMERGENCY_LAST_ROUND_CAP_BYTES,
        1,
        latest_prompt.as_deref(),
    );
    let anchor = build_deterministic_anchor_message(prev_summary.as_deref(), ledger, original_task);

    let before_len = history.len();
    history.clear();
    history.push(HistoryItem::User(anchor));
    history.extend(last_round);
    if let Some(prompt) = latest_prompt {
        history.push(HistoryItem::User(prompt));
    }
    history.len() < before_len || before_len > 2
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map};

    use super::*;
    use crate::agent::MAX_TOKENS_RECOVERY_MESSAGE;

    fn make_tool_call(id: &str, name: &str, args: Value) -> ToolCall {
        ToolCall {
            provider_id: id.into(),
            name: name.into(),
            arguments: args,
            provider_extra: Map::new(),
        }
    }

    fn make_tool_result(id: &str, text: &str, is_error: bool) -> ToolResult {
        ToolResult {
            provider_id: id.into(),
            content: vec![ToolResultContent::Text(text.into())],
            is_error,
        }
    }

    fn make_round(idx: usize, payload_len: usize) -> Vec<HistoryItem> {
        let id = format!("call_{idx}");
        let body = "x".repeat(payload_len);
        vec![
            HistoryItem::Assistant {
                text: format!("checking {idx}"),
                tool_calls: vec![make_tool_call(
                    &id,
                    "dev__read_file",
                    json!({"path": format!("src/file_{idx}.rs")}),
                )],
                reasoning_details: None,
            },
            HistoryItem::ToolResult(make_tool_result(&id, &body, false)),
        ]
    }

    #[test]
    fn d2_summarize_tool_args_extracts_priority_keys_and_clamps_utf8() {
        let args = json!({
            "path": "crates/buzz-agent/src/compaction.rs",
            "command": "cargo test\n--lib",
            "extra": "ignored"
        });
        let s = summarize_tool_args("dev__read_file", &args, SNIPPET_ARG_MAX_BYTES);
        assert!(s.contains(r#"path="crates/buzz-agent/src/compaction.rs""#));
        assert!(s.contains(r#"command="cargo test --lib""#));

        // 驗證多 Byte 中文字元截斷不會 panic
        let cn_args =
            json!({"query": "這是一段很長的繁體中文測試文字用來驗證多字節邊界截斷安全性"});
        let clamped = summarize_tool_args("search", &cn_args, 24);
        assert!(clamped.len() <= 24);
    }

    #[test]
    fn d3_protected_tail_never_splits_assistant_tool_result_pair() {
        let mut history = vec![HistoryItem::User("start".into())];
        for i in 0..5 {
            history.extend(make_round(i, 300));
        }
        history.push(HistoryItem::User("current question".into()));

        // tail_budget_bytes=400, min_rounds=2 -> 保留至少最後 2 個 Assistant+ToolResult 群組 + 當前 User
        let tail_start = compute_protected_tail_start(&history, 400, 2);
        assert!(
            !matches!(history[tail_start], HistoryItem::ToolResult(_)),
            "tail_start ({tail_start}) must never land on an orphan ToolResult"
        );
        // 確認從 tail_start 開始的第一項是 Assistant（或 User），而不是 ToolResult
        assert!(matches!(
            history[tail_start],
            HistoryItem::Assistant { .. } | HistoryItem::User(_)
        ));
    }

    #[test]
    fn d7_tier0_prunes_old_tool_results_down_to_target_keeping_metadata() {
        let mut history = vec![HistoryItem::User("initial task".into())];
        // 前 4 回合各含 1500 bytes 工具輸出；最後 3 回合含 200 bytes
        for i in 0..4 {
            history.extend(make_round(i, 1500));
        }
        for i in 4..7 {
            history.extend(make_round(i, 200));
        }
        history.push(HistoryItem::User("latest".into()));

        let stats = prune_old_tool_results(&mut history, 10_000, 60, 40, 20, 3);
        assert!(stats.bytes_before >= 6_000);
        assert!(stats.pruned_count >= 2);
        assert!(stats.bytes_after < stats.bytes_before);
        assert!(
            stats.bytes_after <= 4_000,
            "bytes_after={} should drop <= 40%",
            stats.bytes_after
        );

        // 確認舊結果保留了 provider_id 與剪枝佔位符，而近期 3 回合未被剪枝
        if let HistoryItem::ToolResult(first_tr) = &history[2] {
            assert_eq!(first_tr.provider_id, "call_0");
            assert!(first_tr.text().starts_with(PRUNED_RESULT_PREFIX));
            assert!(first_tr.text().contains("dev__read_file"));
            assert!(first_tr.text().contains("src/file_0.rs"));
        } else {
            panic!("expected ToolResult at index 2");
        }
        if let HistoryItem::ToolResult(recent_tr) = &history[history.len() - 2] {
            assert!(!recent_tr.text().starts_with(PRUNED_RESULT_PREFIX));
        }
    }

    #[test]
    fn d8_working_set_ledger_tracks_files_commands_and_errors() {
        let mut ledger = WorkingSetLedger::default();
        let c1 = make_tool_call("c1", "dev__read_file", json!({"path": "src/lib.rs"}));
        let r1 = make_tool_result("c1", "content", false);
        let c2 = make_tool_call("c2", "dev__edit_file", json!({"path": "src/lib.rs"}));
        let r2 = make_tool_result("c2", "ok", false);
        let c3 = make_tool_call("c3", "dev__shell", json!({"command": "cargo test --lib"}));
        let r3 = make_tool_result("c3", "exit code 1: compile failed", true);

        ledger.record_tool_execution(&c1, &r1);
        ledger.record_tool_execution(&c2, &r2);
        ledger.record_tool_execution(&c3, &r3);

        // 同路徑應該去重保留最新 op="edit"
        assert_eq!(ledger.files.len(), 1);
        assert_eq!(ledger.files[0].op, "edit");
        assert_eq!(ledger.commands.len(), 1);
        assert_eq!(ledger.commands[0].status, "error");
        assert!(ledger
            .last_error
            .as_deref()
            .unwrap()
            .contains("compile failed"));

        let md = ledger.render_markdown();
        assert!(md.starts_with("[Working Set Ledger]"));
        assert!(md.contains("src/lib.rs"));
        assert!(md.contains("cargo test --lib"));
    }

    #[test]
    fn d9_synthetic_messages_never_shadow_real_user_prompt_or_channel_id() {
        let real_prompt = "<context>\nChannel: ops (#5ca019f6-1a27-592a-b015-4baf0eb33572)\n</context>\nPlease analyze pollen";
        let history = vec![
            HistoryItem::User(format!("{real_prompt}{SYSTEM_NOTE_MARKER} Keep going.]")),
            HistoryItem::Assistant {
                text: "working".into(),
                tool_calls: vec![],
                reasoning_details: None,
            },
            HistoryItem::User(
                "[系統警示]：您已執行 35 輪工具呼叫，接近系統最大執行上限（40 輪）。".into(),
            ),
            HistoryItem::User(MAX_TOKENS_RECOVERY_MESSAGE.into()),
        ];

        assert!(is_synthetic_system_user_message("[系統警示] foo"));
        assert!(is_synthetic_system_user_message(
            MAX_TOKENS_RECOVERY_MESSAGE
        ));
        assert!(is_synthetic_system_user_message("[Context Handoff]\nsum"));
        assert!(!is_synthetic_system_user_message(real_prompt));

        let found_prompt = find_latest_real_user_prompt(&history, None).unwrap();
        assert_eq!(found_prompt, real_prompt);

        let target = find_reply_target_in_history(&history, None).unwrap();
        assert_eq!(target.0, "5ca019f6-1a27-592a-b015-4baf0eb33572");
    }

    #[test]
    fn d4_d5_tier2_compact_without_llm_shrinks_oversize_and_spares_tiny_history() {
        let ledger = WorkingSetLedger {
            files: vec![LedgerFileEntry {
                path: "src/main.rs".into(),
                op: "edit".into(),
                status: "ok".into(),
            }],
            ..Default::default()
        };

        // 1. 小歷史（例如 regressions 中只用高假 usage=950 觸發）不應刪除 STEER-CANARY
        let mut tiny = vec![
            HistoryItem::User("turn 1".into()),
            HistoryItem::User("STEER-CANARY".into()),
        ];
        assert!(!compact_without_llm(
            &mut tiny,
            &ledger,
            Some("turn 1"),
            1_000,
            60,
            20,
            3
        ));
        assert_eq!(tiny.len(), 2);

        // 2. 僅工具輸出超標時，步驟 1 剪枝即足以壓至 <= 60% 並保留尾端
        let mut prunable = vec![HistoryItem::User("main task".into())];
        for i in 0..8 {
            prunable.extend(make_round(i, 1200));
        }
        prunable.push(HistoryItem::User("STEER-CANARY".into()));
        assert!(compact_without_llm(
            &mut prunable,
            &ledger,
            Some("main task"),
            10_000,
            60,
            20,
            3
        ));
        assert!(estimate_history_tokens(&prunable, None) <= 6_000);
        assert!(matches!(prunable.last(), Some(HistoryItem::User(s)) if s == "STEER-CANARY"));

        // 3. 即使工具結果剪枝後仍超標（含大量不可剪枝的 Assistant 文字），步驟 2 應建立帳本錨點並淘汰最舊回合
        let mut big = vec![HistoryItem::User("main task".into())];
        for i in 0..10 {
            big.push(HistoryItem::Assistant {
                text: format!("analysis-{i}: {}", "x".repeat(700)),
                tool_calls: vec![ToolCall {
                    provider_id: format!("c-{i}"),
                    name: "dev__read_file".into(),
                    arguments: json!({"path": format!("src/f{i}.rs")}),
                    provider_extra: Map::new(),
                }],
                reasoning_details: None,
            });
            big.push(HistoryItem::ToolResult(ToolResult {
                provider_id: format!("c-{i}"),
                content: vec![ToolResultContent::Text("r".repeat(1200))],
                is_error: false,
            }));
        }
        big.push(HistoryItem::User("STEER-CANARY".into()));
        assert!(compact_without_llm(
            &mut big,
            &ledger,
            Some("main task"),
            10_000,
            60,
            20,
            3
        ));
        assert!(estimate_history_tokens(&big, None) <= 6_000);
        assert!(
            matches!(big.first(), Some(HistoryItem::User(s)) if s.contains("[Working Set Ledger]"))
        );
        assert!(matches!(big.last(), Some(HistoryItem::User(s)) if s == "STEER-CANARY"));
    }
}
