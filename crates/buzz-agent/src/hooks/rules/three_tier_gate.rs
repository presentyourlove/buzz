// Mika / dsh-Go 式端側三檔安全門禁 (Three-Tier Security Gate)
// 實作 ReadOnly / Restricted / Elevated 實體隔離門禁
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;
use std::env;

/// 操作權限等級 (Operation Tiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OperationTier {
    /// 唯讀操作：讀取檔案、檢查狀態、查詢目錄等，無條件直接放行
    ReadOnly,
    /// 受控常規操作：檔案寫入、單元測試、代碼格式化等日常開發，審計後放行
    Restricted,
    /// 高危操作：刪除目錄、強制覆寫、Git 重置、系統級設定檔異動，未授權嚴格阻斷
    Elevated,
}

pub struct ThreeTierGateRule;

impl ThreeTierGateRule {
    /// 判斷操作的權限等級
    pub fn classify(ctx: &HookContext) -> (OperationTier, Option<&'static str>) {
        let tool_name = ctx.bare_tool_name().to_lowercase();

        // 1. 先檢驗是否命中 Tier 3 高危指令特徵
        if ctx.matches_tool(&["shell", "bash", "cmd", "powershell", "execute_command", "exec", "terminal"]) {
            if let Some(cmd) = ctx.get_str_arg(&["command", "cmd", "script", "CommandLine"]) {
                let lower = cmd.to_lowercase();
                
                // 毀滅性刪除與格式化
                let destructive_patterns = [
                    ("rm -rf", "遞迴強制刪除目錄"),
                    ("rmdir /s", "Windows 遞迴刪除目錄"),
                    ("del /s", "遞迴強制刪除檔案"),
                    ("del /f /s", "遞迴強制刪除檔案"),
                    ("remove-item -recurse", "PowerShell 遞迴刪除"),
                    ("format c:", "磁碟格式化"),
                    ("format d:", "磁碟格式化"),
                    ("mkfs.", "檔案系統格式化"),
                    (":(){ :|:& };:", "Fork Bomb 叉子炸彈"),
                    ("git reset --hard", "Git 硬重置，可能遺失未提交變更"),
                    ("git clean -fd", "Git 強制清理未追蹤檔案"),
                    ("git clean -f", "Git 強制清理檔案"),
                    ("git push --force", "Git 強制推送"),
                    ("git push -f", "Git 強制推送"),
                    ("drop database", "資料庫刪除"),
                    ("drop table", "資料表刪除"),
                    ("truncate table", "資料表清空"),
                ];

                for (pat, desc) in destructive_patterns {
                    if lower.contains(pat) {
                        return (OperationTier::Elevated, Some(desc));
                    }
                }

                // 2. 檢驗是否屬於純唯讀 Shell 指令
                let readonly_shell_prefixes = [
                    "git status", "git log", "git diff", "git branch",
                    "dir", "ls", "pwd", "echo", "cat", "type",
                    "cargo --version", "rustc --version", "node -v", "npm -v", "python --version",
                ];
                let is_pure_readonly = readonly_shell_prefixes.iter().any(|prefix| lower.trim_start().starts_with(prefix))
                    && !lower.contains('>') && !lower.contains("out-file") && !lower.contains("set-content");

                if is_pure_readonly {
                    return (OperationTier::ReadOnly, None);
                }

                return (OperationTier::Restricted, None);
            }
        }

        // 3. 檢驗是否涉及高危檔案路徑（如 .env, 私鑰, managed-agents.json 等）
        if ctx.matches_tool(&["write_file", "edit_file", "modify_file", "replace_file_content", "delete_file"]) {
            if let Some(path_str) = ctx.get_str_arg(&["path", "Path", "file_path", "target"]) {
                let path_lower = path_str.to_lowercase();
                let sensitive_files = [
                    (".env", "敏感環境變數與金鑰檔"),
                    ("managed-agents.json", "蜂群核心特工設定檔"),
                    ("id_rsa", "SSH 私鑰憑證"),
                    ("id_ed25519", "SSH 私鑰憑證"),
                    (".pem", "SSL/TLS 憑證私鑰"),
                    (".key", "密鑰憑證檔案"),
                ];

                for (sen, desc) in sensitive_files {
                    if path_lower.ends_with(sen) || path_lower.contains(&format!("{}/", sen)) || path_lower.contains(&format!("{}\\", sen)) {
                        return (OperationTier::Elevated, Some(desc));
                    }
                }
            }
            return (OperationTier::Restricted, None);
        }

        // 4. 判斷常見唯讀工具
        let readonly_tools = [
            "read_file", "view_file", "cat", "head", "tail",
            "list_dir", "list_directory", "list_files", "dir", "ls",
            "grep", "search", "search_files", "search_web", "find_file",
            "get_status", "health_check", "get_metrics",
        ];

        if readonly_tools.iter().any(|&r| tool_name == r || tool_name.ends_with(&format!("__{}", r))) {
            return (OperationTier::ReadOnly, None);
        }

        // 5. 預設為受控操作
        (OperationTier::Restricted, None)
    }

    /// 檢查是否具備授權（可透過環境變數或參數標記放行）
    pub fn is_elevated_authorized(ctx: &HookContext) -> bool {
        // 支援環境變數全域授權
        if let Ok(val) = env::var("BUZZ_ALLOW_ELEVATED") {
            if val == "1" || val.eq_ignore_ascii_case("true") {
                return true;
            }
        }

        // 支援呼叫參數中帶有授權旗標
        if let Some(auth) = ctx.tool_arguments.get("elevated_authorized") {
            if auth.as_bool().unwrap_or(false) {
                return true;
            }
        }
        if ctx.get_str_arg(&["elevated_token", "auth_token"]).is_some() {
            return true;
        }

        false
    }
}

#[async_trait]
impl Rule for ThreeTierGateRule {
    fn id(&self) -> &'static str {
        "three-tier-security-gate"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        let (tier, detail) = Self::classify(ctx);

        match tier {
            OperationTier::ReadOnly => {
                // Tier 1 唯讀：無條件放行
                Ok(HookResult::Continue)
            }
            OperationTier::Restricted => {
                // Tier 2 受限：日常安全開發，正常放行
                Ok(HookResult::Continue)
            }
            OperationTier::Elevated => {
                // Tier 3 高危：檢查是否有授權
                if Self::is_elevated_authorized(ctx) {
                    let desc = detail.unwrap_or("高危敏感操作");
                    Ok(HookResult::Warn {
                        message: format!(
                            "【Mika 三檔安全門禁警告】已確認合法授權，允許執行 Tier 3 高危操作: {}",
                            desc
                        ),
                    })
                } else {
                    let desc = detail.unwrap_or("高危不可逆操作");
                    Ok(HookResult::Block {
                        reason: format!(
                            "【Mika 三檔安全門禁攔截】檢測到未授權之 Tier 3 (Elevated) 高危操作（{}）。\
                            為防止特工誤刪重要系統檔案或覆寫關鍵設定，此操作已被 Sentry 自動阻擋！\
                            若確需執行，請在參數附加 elevated_authorized: true 或設定環境變數 BUZZ_ALLOW_ELEVATED=1。",
                            desc
                        ),
                    })
                }
            }
        }
    }
}

