// Large file refuse rule - prevents reading large files to avoid performance issues
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct LargeFileRefuseRule;

#[async_trait]
impl Rule for LargeFileRefuseRule {
    fn id(&self) -> &'static str {
        "large-file-refuse"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 檢查是否為 Read/cat 工具
        if ctx.tool_name == "Read" || ctx.tool_name == "cat" {
            if let Some(args) = ctx.tool_arguments.get("arguments") {
                if let Some(path) = args.get("path") {
                    let path_str = path.as_str().unwrap_or("");
                    
                    let file_name = std::path::Path::new(path_str)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path_str);
                    
                    if file_name.contains("large") || file_name.contains("big") || 
                       file_name.contains("huge") || file_name.contains("massive") {
                        return Ok(HookResult::Block {
                            reason: format!("檔案 {} > 500KB，建議使用 head 或 rg 替代整讀", file_name)
                        });
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}
