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
        // 檢查是否為 Write 工具
        if ctx.tool_name != "Write" {
            return Ok(HookResult::Continue);
        }
        
        if let Some(args) = ctx.tool_arguments.get("arguments") {
            if let Some(path) = args.get("path") {
                let path_str = path.as_str().unwrap_or("");
                
                // 檢查檔案副檔名
                let extensions = [".json", ".toml", ".yaml", ".yml"];
                let file_ext = std::path::Path::new(path_str)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| format!(".{}", e))
                    .unwrap_or_default();
                
                if extensions.contains(&file_ext.as_str()) {
                    if let Some(content) = args.get("content") {
                        let content_str = content.as_str().unwrap_or("");
                        
                        // 根據檔案類型進行語法驗證
                        match file_ext.as_str() {
                            ".json" => {
                                if let Err(_) = serde_json::from_str::<Value>(content_str) {
                                    return Ok(HookResult::Warn {
                                        message: "JSON 檔案語法無效".to_string()
                                    });
                                }
                            }
                            ".toml" | ".yaml" | ".yml" => {
                                if content_str.trim().is_empty() {
                                    return Ok(HookResult::Warn {
                                        message: "TOML/YAML 檔案為空".to_string()
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}
