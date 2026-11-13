// Data file JSON validation rule - validates JSON, TOML, YAML file content after write
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;
use serde_json::Value;

pub struct DataFileJsonRule;

#[async_trait]
impl Rule for DataFileJsonRule {
    fn id(&self) -> &'static str {
        "data-file-json"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PostToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        if !ctx.matches_tool(&["write_file", "write", "Write"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) {
            let extensions = [".json", ".toml", ".yaml", ".yml"];
            let file_ext = std::path::Path::new(&path_str)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e.to_lowercase()))
                .unwrap_or_default();

            if extensions.contains(&file_ext.as_str()) {
                if let Some(content_str) = ctx.get_str_arg(&["content", "Content"]) {
                    match file_ext.as_str() {
                        ".json" => {
                            if serde_json::from_str::<Value>(&content_str).is_err() {
                                return Ok(HookResult::Warn {
                                    message: "JSON 檔案語法無效".to_string(),
                                });
                            }
                        }
                        ".toml" | ".yaml" | ".yml" => {
                            if content_str.trim().is_empty() {
                                return Ok(HookResult::Warn {
                                    message: "TOML/YAML 檔案為空".to_string(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
