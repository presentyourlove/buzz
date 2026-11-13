// Large File Guard rule - prevents reading massive files without pagination (>40KB)
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;
use std::path::Path;

pub struct LargeFileGuardRule;

#[async_trait]
impl Rule for LargeFileGuardRule {
    fn id(&self) -> &'static str {
        "large-file-guard"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        if !ctx.matches_tool(&["read_file", "read", "cat", "view", "Read"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) {
            let path = Path::new(&path_str);
            if path.is_file() {
                if let Ok(metadata) = std::fs::metadata(path) {
                    let len = metadata.len();
                    const MAX_UNPAGINATED_BYTES: u64 = 40 * 1024; // 40 KB 門禁門檻

                    if len > MAX_UNPAGINATED_BYTES {
                        // 檢查是否有指定分頁或行數範圍
                        let has_pagination = ctx.has_arg(&[
                            "offset",
                            "limit",
                            "start_line",
                            "end_line",
                            "StartLine",
                            "EndLine",
                            "byte_offset",
                            "ByteOffset",
                            "line_range",
                            "lines",
                        ]);

                        if !has_pagination {
                            let size_kb = (len as f64) / 1024.0;
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&path_str);

                            return Ok(HookResult::Block {
                                reason: format!(
                                    "【PreToolUse 大檔防爆門禁】檔案 `{}` 實體大小為 {:.1} KB（超過 40 KB 門禁上限）。未指定分頁參數（如 offset/limit 或 StartLine/EndLine），全檔整讀已被攔截以防上下文雪球崩潰！請指定行數或位元組範圍分段讀取。",
                                    file_name, size_kb
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

