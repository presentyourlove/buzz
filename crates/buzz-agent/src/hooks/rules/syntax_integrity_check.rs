// Syntax Integrity Check - PostToolUse verification for structured files (JSON/YAML/code)
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;
use std::path::Path;

pub struct SyntaxIntegrityCheckRule;

#[async_trait]
impl Rule for SyntaxIntegrityCheckRule {
    fn id(&self) -> &'static str {
        "syntax-integrity-check"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PostToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        if !ctx.matches_tool(&["write_file", "write", "edit_file", "edit", "str_replace", "Write", "Edit"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) {
            let path = Path::new(&path_str);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

            if path.is_file() {
                if ext == "json" {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if serde_json::from_str::<serde_json::Value>(&content).is_err() {
                            return Ok(HookResult::Block {
                                reason: format!(
                                    "【PostToolUse 語法品質門禁】寫入之 JSON 檔案 `{}` 結構無效或語法解析失敗，請立即檢驗修復！",
                                    path_str
                                ),
                            });
                        }
                    }
                } else if ext == "rs" {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let open_braces = content.chars().filter(|&c| c == '{').count();
                        let close_braces = content.chars().filter(|&c| c == '}').count();
                        if open_braces != close_braces {
                            return Ok(HookResult::Warn {
                                message: format!(
                                    "【PostToolUse 語法警告】Rust 代碼 `{}` 大括號不匹配 ({{: {}, }}: {})，請確認語法完整性。",
                                    path_str, open_braces, close_braces
                                ),
                            });
                        }
                    }
                }
            }
        }

        Ok(HookResult::Continue)
    }
}

