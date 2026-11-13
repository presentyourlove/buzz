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
        if !ctx.matches_tool(&["write_file", "write", "edit_file", "edit", "str_replace", "Write", "Edit"]) {
            return Ok(HookResult::Continue);
        }

        let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) else {
            return Ok(HookResult::Continue);
        };

        let file_path = std::path::Path::new(&path_str);
        if let Some(parent) = file_path.parent() {
            if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
                if (name == "PLANS" || name == "GUIDES")
                    && file_path.extension().and_then(|e| e.to_str()) == Some("md")
                {
                    if let Some(content) = ctx.get_str_arg(&["content", "Content", "new_str", "Replacement"]) {
                        if !content.contains("## 設計決策") || !content.contains("## 驗收標準") {
                            return Ok(HookResult::Warn {
                                message: "設計文檔需要包含 ## 設計決策 和 ## 驗收標準 段落".to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
