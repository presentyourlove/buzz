// Danger Command Block rule - prevents executing destructive shell commands
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct DangerCommandBlockRule;

#[async_trait]
impl Rule for DangerCommandBlockRule {
    fn id(&self) -> &'static str {
        "danger-command-block"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        if !ctx.matches_tool(&["shell", "bash", "cmd", "powershell", "execute_command", "exec", "terminal"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(cmd) = ctx.get_str_arg(&["command", "cmd", "script", "CommandLine"]) {
            let lower = cmd.to_lowercase();
            let dangerous_patterns = [
                "rm -rf /",
                "rm -rf /*",
                "rm -rf ~",
                "rmdir /s /q c:\\",
                "del /s /q c:\\",
                "del /f /s /q c:\\",
                "format c:",
                "format d:",
                "git push --force",
                "git push -f",
                "git reset --hard",
                "drop database",
                "mkfs.",
                ":(){ :|:& };:",
                "remove-item -recurse -force c:\\",
                "remove-item -recurse -force \"c:\\\"",
            ];

            for pattern in &dangerous_patterns {
                if lower.contains(pattern) {
                    return Ok(HookResult::Block {
                        reason: format!(
                            "【PreToolUse 安全門禁攔截】檢測到危險不可逆指令模式 `{}`。此操作已被 Sentry 門禁自動阻擋以維護系統安全！",
                            pattern
                        ),
                    });
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
