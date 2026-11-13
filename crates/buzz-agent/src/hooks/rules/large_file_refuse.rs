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
        if !ctx.matches_tool(&["read_file", "read", "cat", "view", "Read"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) {
            let path = std::path::Path::new(&path_str);
            if path.is_file() {
                if let Ok(meta) = std::fs::metadata(path) {
                    const HARD_MAX_BYTES: u64 = 500 * 1024; // 500 KB 硬上限
                    if meta.len() > HARD_MAX_BYTES {
                        let has_pagination = ctx.has_arg(&[
                            "offset",
                            "limit",
                            "start_line",
                            "end_line",
                            "StartLine",
                            "EndLine",
                            "byte_offset",
                            "ByteOffset",
                        ]);
                        if !has_pagination {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&path_str);
                            return Ok(HookResult::Block {
                                reason: format!(
                                    "檔案 {} 實體大小超過 500KB ({:.1} KB)，嚴禁無分頁整檔讀取，建議使用分頁參數或 rg 替代整讀",
                                    file_name,
                                    (meta.len() as f64) / 1024.0
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
