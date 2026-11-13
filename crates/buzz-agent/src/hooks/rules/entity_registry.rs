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
        if !ctx.matches_tool(&["write_file", "write", "edit_file", "edit", "str_replace", "Write", "Edit"]) {
            return Ok(HookResult::Continue);
        }

        let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) else {
            return Ok(HookResult::Continue);
        };

        if !path_str.ends_with("types.rs") {
            return Ok(HookResult::Continue);
        }

        if let Some(content_str) = ctx.get_str_arg(&["content", "Content", "new_str", "Replacement"]) {
            if content_str.contains("pub struct") || content_str.contains("pub enum") || content_str.contains("pub type") {
                if !content_str.contains("ENTITY_REGISTRY.md") {
                    return Ok(HookResult::Warn {
                        message: "新增的 struct/enum 需要更新 ENTITY_REGISTRY.md 註冊表".to_string(),
                    });
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
