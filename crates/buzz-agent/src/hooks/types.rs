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

impl HookContext {
    pub fn bare_tool_name(&self) -> &str {
        if let Some(pos) = self.tool_name.rfind("__") {
            &self.tool_name[pos + 2..]
        } else if let Some(pos) = self.tool_name.rfind('/') {
            &self.tool_name[pos + 1..]
        } else if let Some(pos) = self.tool_name.rfind(':') {
            &self.tool_name[pos + 1..]
        } else {
            &self.tool_name
        }
    }

    pub fn matches_tool(&self, candidates: &[&str]) -> bool {
        let bare = self.bare_tool_name().to_lowercase();
        let full = self.tool_name.to_lowercase();
        candidates.iter().any(|&c| {
            let cl = c.to_lowercase();
            bare == cl || full == cl
        })
    }

    pub fn get_str_arg(&self, keys: &[&str]) -> Option<String> {
        for &k in keys {
            if let Some(v) = self.tool_arguments.get(k) {
                if let Some(s) = v.as_str() {
                    return Some(s.to_string());
                }
            }
            for wrapper in &["arguments", "input", "params", "tool_input"] {
                if let Some(wrapped) = self.tool_arguments.get(wrapper) {
                    if let Some(v) = wrapped.get(k) {
                        if let Some(s) = v.as_str() {
                            return Some(s.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    pub fn has_arg(&self, keys: &[&str]) -> bool {
        for &k in keys {
            if self.tool_arguments.get(k).is_some() {
                return true;
            }
            for wrapper in &["arguments", "input", "params", "tool_input"] {
                if let Some(wrapped) = self.tool_arguments.get(wrapper) {
                    if wrapped.get(k).is_some() {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn get_i64_arg(&self, keys: &[&str]) -> Option<i64> {
        for &k in keys {
            if let Some(v) = self.tool_arguments.get(k) {
                if let Some(n) = v.as_i64() {
                    return Some(n);
                }
            }
            for wrapper in &["arguments", "input", "params", "tool_input"] {
                if let Some(wrapped) = self.tool_arguments.get(wrapper) {
                    if let Some(v) = wrapped.get(k) {
                        if let Some(n) = v.as_i64() {
                            return Some(n);
                        }
                    }
                }
            }
        }
        None
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    Error,
    Warn,
    #[default]
    Info,
}
