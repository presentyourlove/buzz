// Branch protection rule - prevents pushing to protected branches
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct BranchProtectionRule;

#[async_trait]
impl Rule for BranchProtectionRule {
    fn id(&self) -> &'static str {
        "branch-protection"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 僅攔截 Shell / 終端執行工具中直接對受保護分支執行 git push 的危險操作
        if !ctx.matches_tool(&[
            "run_command",
            "bash",
            "sh",
            "shell",
            "exec",
            "execute_command",
            "powershell",
            "cmd",
            "terminal",
        ]) {
            return Ok(HookResult::Continue);
        }

        let cmd_str = ctx
            .get_str_arg(&["command", "cmd", "CommandLine", "script", "input"])
            .unwrap_or_else(|| ctx.tool_arguments.to_string());

        let cmd_lower = cmd_str.to_lowercase();
        if cmd_lower.contains("git push") {
            let forbidden_patterns = [
                "origin main",
                "origin master",
                "origin release/",
                "origin head:main",
                "origin head:master",
                "--force",
                " -f ",
            ];
            if forbidden_patterns.iter().any(|p| cmd_lower.contains(p)) {
                return Ok(HookResult::Block {
                    reason: "【PreToolUse 分支保護門禁】禁止直接強推或推送至受保護分支 (main/master/release/*)，請透過 PR 流程合併。".to_string(),
                });
            }
        }

        Ok(HookResult::Continue)
    }
}
