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
        // 驗證 arguments 必須為有效 JSON Object
        if !ctx.tool_arguments.is_object() {
            return Ok(HookResult::Block {
                reason: "參數必須為有效 JSON".to_string(),
            });
        }

        // 檢查是否為寫入工具，驗證 JSON 語法
        if ctx.matches_tool(&["write_file", "write", "Write"]) {
            if let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) {
                if path_str.to_lowercase().ends_with(".json") {
                    if let Some(content_str) = ctx.get_str_arg(&["content", "Content"]) {
                        if serde_json::from_str::<Value>(&content_str).is_err() {
                            return Ok(HookResult::Block {
                                reason: "JSON 檔案語法無效".to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
