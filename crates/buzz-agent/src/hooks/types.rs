// 依據 crates/buzz-agent/src/types.rs:62-124 HistoryItem 結構
use std::path::PathBuf;
use crate::types::ToolResultContent;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HookContext {
    pub tool_name: String,
    pub tool_arguments: serde_json::Value,
    pub session_id: String,
    pub working_dir: PathBuf,
    pub trigger_event: HookTrigger,  // PreToolUse | PostToolUse | SessionStart | SessionEnd | SubagentStart | SubagentEnd
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrigger {
    PreToolUse,
    PostToolUse,
    SessionStart,
    SessionEnd,
    SubagentStart,
    SubagentEnd,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HookResult {
    Continue,                    // 繼續執行
    Block { reason: String },    // 阻擋並回報原因
    Modify {                     // 改寫參數/結果
        new_arguments: Option<serde_json::Value>,
        new_result: Option<ToolResultContent>,
    },
    Warn { message: String },    // 僅警告不阻擋
}

#[derive(Debug, Clone)]
pub struct RuleMatch {
    pub rule_id: String,
    pub severity: Severity,      // Error | Warn | Info
    pub message: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity { Error, Warn, Info }
