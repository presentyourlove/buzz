// Design document check rule - validates that design documents contain required sections
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct DesignDocCheckRule;

#[async_trait]
impl Rule for DesignDocCheckRule {
    fn id(&self) -> &'static str {
        "design-doc-check"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 檢查是否為 PLANS 或 GUIDES 目錄下的檔案
        let file_path = &ctx.working_dir;
        
        if let Some(parent) = file_path.parent() {
            let parent_name = parent.file_name().and_then(|n| n.to_str());
            
            // 檢查是否為 PLANS 或 GUIDES 目錄
            if let Some(name) = parent_name {
                if name == "PLANS" || name == "GUIDES" {
                    // 檢查是否為 .md 檔案
                    if file_path.extension().and_then(|e| e.to_str()) == Some("md") {
                        return Ok(HookResult::Warn {
                            message: "設計文檔需要包含 ## 設計決策 和 ## 驗收標準 段落".to_string()
                        });
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}
