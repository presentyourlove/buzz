// Hardcoded security rule - detects hardcoded secrets in tool parameters
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct HardcodedBlockRule;

#[async_trait]
impl Rule for HardcodedBlockRule {
    fn id(&self) -> &'static str {
        "hardcoded-block"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 掃描 Write/Edit 參數中的 password、api_key、secret 等硬編碼
        if let Some(args) = ctx.tool_arguments.get("arguments") {
            if let Some(args_obj) = args.as_object() {
                // 檢查是否為 Write/Edit 類型工具
                if ctx.tool_name == "Write" || ctx.tool_name == "Edit" {
                    if let Some(content) = args_obj.get("content") {
                        let content_str = content.as_str().unwrap_or(&content.to_string());
                        
                        // 檢查是否包含敏感資訊模式
                        let sensitive_patterns = [
                            "password", "api_key", "secret", "token", "sk-", "passwd",
                            "auth", "credential", "key", "pwd"
                        ];
                        
                        for pattern in &sensitive_patterns {
                            if content_str.to_lowercase().contains(pattern) {
                                return Ok(HookResult::Block {
                                    reason: format!("硬編碼的安全資訊被阻擋: {}", pattern)
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}
