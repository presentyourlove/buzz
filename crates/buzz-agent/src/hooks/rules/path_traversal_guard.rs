// Path Traversal and Sensitive Directory Guard
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct PathTraversalGuardRule;

#[async_trait]
impl Rule for PathTraversalGuardRule {
    fn id(&self) -> &'static str {
        "path-traversal-guard"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        if let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target_path", "filepath"]) {
            let lower = path_str.to_lowercase();

            // 1. 檢查目錄逃逸 (全面檢查所有層級的相對跳脫 ../ 與 ..\)
            if lower.contains("../") || lower.contains("..\\") || lower == ".." {
                return Ok(HookResult::Block {
                    reason: format!(
                        "【PreToolUse 安全門禁攔截】檢測到目錄穿越模式 (Directory Traversal): `{}`。已被 Sentry 門禁阻擋！",
                        path_str
                    ),
                });
            }

            // 2. 檢查 Windows 保留裝置名稱注入 (nul, con, prn, aux, com1-9, lpt1-9 等，含任何副檔名如 con.txt)
            let reserved_devices = ["nul", "con", "prn", "aux", "com1", "com2", "com3", "com4", "lpt1", "lpt2"];
            let std_path = std::path::Path::new(&lower);
            let file_stem = std_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let file_name = std_path.file_name().and_then(|s| s.to_str()).unwrap_or("");

            for dev in &reserved_devices {
                if file_stem == *dev || file_name == *dev || lower == *dev || lower.ends_with(&format!("/{}", dev)) || lower.ends_with(&format!("\\{}", dev)) {
                    return Ok(HookResult::Block {
                        reason: format!(
                            "【PreToolUse 安全門禁攔截】檢測到 Windows 保留裝置名稱: `{}`。嚴禁創建或存取保留裝置檔！",
                            path_str
                        ),
                    });
                }
            }

            // 3. 檢查敏感系統目錄存取
            let sensitive_roots = [
                "c:\\windows",
                "c:/windows",
                "c:\\system32",
                "c:/system32",
                "/etc/passwd",
                "/etc/shadow",
                ".ssh/id_rsa",
                ".ssh\\id_rsa",
                ".ssh/id_ed25519",
                ".ssh\\id_ed25519",
            ];

            for root in &sensitive_roots {
                if lower.contains(root) {
                    return Ok(HookResult::Block {
                        reason: format!(
                            "【PreToolUse 安全門禁攔截】嚴禁存取受保護之系統關鍵檔案或目錄: `{}`。操作已被攔截！",
                            path_str
                        ),
                    });
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
