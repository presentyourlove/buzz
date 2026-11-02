// JSON validation rule - validates JSON parameters and file content
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;
use serde_json::Value;

pub struct JsonValidationRule;

#[async_trait]
impl Rule for JsonValidationRule {
    fn id(&self) -> &'static str {
        "json-validation"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 驗證 arguments 必須為有效 JSON
        if !ctx.tool_arguments.is_object() {
            return Ok(HookResult::Block {
                reason: "參數必須為有效 JSON".to_string()
            });
        }
        
        // 檢查是否為 Write 工具，驗證 JSON 語法
        if ctx.tool_name == "Write" {
            if let Some(args) = ctx.tool_arguments.get("arguments") {
                if let Some(path) = args.get("path") {
                    let path_str = path.as_str().unwrap_or("");
                    if path_str.ends_with(".json") {
                        if let Some(content) = args.get("content") {
                            // 驗證 JSON 語法
                            if let Err(_) = serde_json::from_str::<Value>(content.as_str().unwrap_or("")) {
                                return Ok(HookResult::Block {
                                    reason: "JSON 檔案語法無效".to_string()
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
