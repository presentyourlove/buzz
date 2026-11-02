// Entity registry rule - validates that new entities are registered
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct EntityRegistryRule;

#[async_trait]
impl Rule for EntityRegistryRule {
    fn id(&self) -> &'static str {
        "entity-registry"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 檢查是否為 types.rs 檔案
        let path_str = ctx.working_dir.to_string_lossy();
        if !path_str.contains("types.rs") {
            return Ok(HookResult::Continue);
        }
        
        // 檢查工具名稱與 Rust 代碼
        if ctx.tool_name == "Write" || ctx.tool_name == "Edit" {
            if let Some(args) = ctx.tool_arguments.get("arguments") {
                if let Some(content) = args.get("content") {
                    let content_str = content.as_str().unwrap_or(&content.to_string());
                    
                    // 簡單的 Rust 結構體檢查，檢測是否包含新的 struct 或 enum
                    if content_str.contains("pub struct") || content_str.contains("pub enum") || 
                       content_str.contains("pub type") {
                        // 檢查是否包含對應的註冊表
                        if !content_str.contains("ENTITY_REGISTRY.md") {
                            return Ok(HookResult::Warn {
                                message: "新增的 struct/enum 需要更新 ENTITY_REGISTRY.md 註冊表".to_string()
                            });
                        }
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}
