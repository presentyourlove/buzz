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
        // 檢查是否為 git push 或 git commit 指令
        if ctx.tool_name == "Write" || ctx.tool_name == "Edit" {
            if let Some(args) = ctx.tool_arguments.get("arguments") {
                if let Some(content) = args.get("content") {
                    let content_str = content.as_str().unwrap_or(&content.to_string());
                    
                    // 簡單的 git 指令檢測
                    if content_str.contains("git push") || content_str.contains("git commit") {
                        // 檢查是否為禁止的分支
                        let forbidden_branches = ["main", "master", "release/*"];
                        let branch_match = forbidden_branches.iter()
                            .any(|pattern| matches_branch_pattern(content_str, pattern));
                        
                        if branch_match {
                            return Ok(HookResult::Block {
                                reason: "禁止推送至 main/master/release 分支".to_string()
                            });
                        }
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}

fn matches_branch_pattern(content: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        if pattern.starts_with("release/") {
            return content.contains("release/") && (content.contains('/') || content.contains(' '));
        }
    }
    
    content.contains(pattern)
}
