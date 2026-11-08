//! AnyModelBot 方案 B：amb-delete-gate 遞歸刪除保護門禁
//!
//! 即使特工被授予較高權限，若非沙箱環境的遞歸刪除操作一律強制攔截，
//! 防止誤刪專案代碼庫或作業系統核心檔案。

use std::path::Path;

/// 刪除門禁攔截結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteGateResult {
    /// 允許放行 (位於合格沙箱內之臨時檔案或非遞歸安全刪除)
    Allowed,
    /// 強制攔截 (非沙箱遞歸刪除或危險系統路徑)
    Blocked(String),
}

/// 遞歸刪除門禁
#[derive(Debug, Default, Clone)]
pub struct DeleteGate;

impl DeleteGate {
    pub fn new() -> Self {
        Self
    }

    /// 檢查刪除操作是否合法
    /// 
    /// - `command_or_action`: 操作名稱或命令字串 (如 "rmdir /s /q", "rm -rf", "delete_directory")
    /// - `target_path`: 目標路徑
    /// - `is_recursive`: 是否為遞歸刪除
    pub fn inspect(&self, command_or_action: &str, target_path: &str, is_recursive: bool) -> DeleteGateResult {
        let cmd_lower = command_or_action.to_lowercase();
        let path_str = target_path.replace('\\', "/").to_lowercase();

        // 偵測命令中的遞歸標記
        let has_recursive_flag = is_recursive
            || cmd_lower.contains("rm -rf")
            || cmd_lower.contains("rmdir /s")
            || cmd_lower.contains("remove-item -recurse")
            || cmd_lower.contains("remove_dir_all");

        if !has_recursive_flag {
            // 單一檔案非遞歸刪除，由外部常規權限處理
            return DeleteGateResult::Allowed;
        }

        // 1. 絕對禁止的根目錄與系統級路徑
        let prohibited_roots = [
            "/",
            "c:/",
            "c:/windows",
            "c:/program files",
            "c:/私人",
            "c:/私人/ai agent/buzz",
        ];

        let trimmed_path = path_str.trim_end_matches('/');
        for root in &prohibited_roots {
            if trimmed_path == *root || trimmed_path.is_empty() {
                return DeleteGateResult::Blocked(format!(
                    "嚴禁在核心或根目錄執行遞歸刪除: {}",
                    target_path
                ));
            }
        }

        // 2. 含有越界路徑符號 (..)
        if path_str.contains("..") {
            return DeleteGateResult::Blocked(format!(
                "檢測到路徑越界跳脫 (..)，禁止執行遞歸刪除: {}",
                target_path
            ));
        }

        // 3. 白名單沙箱路徑檢查
        let allowed_sandbox_prefixes = [
            ".buzz/sandboxes/",
            "target/tmp/",
            "tmp/",
            "/tmp/",
            "c:/temp/",
            "c:/users/user/appdata/local/temp/",
        ];

        let is_in_sandbox = allowed_sandbox_prefixes
            .iter()
            .any(|prefix| path_str.starts_with(prefix) || path_str.contains(prefix));

        if is_in_sandbox {
            DeleteGateResult::Allowed
        } else {
            DeleteGateResult::Blocked(format!(
                "[amb-delete-gate] 門禁強制攔截：非沙箱路徑之遞歸刪除操作已被阻斷 ({})",
                target_path
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_gate_blocks_recursive_non_sandbox() {
        let gate = DeleteGate::new();

        // 攔截專案根目錄遞歸刪除
        let r1 = gate.inspect("rm -rf", "c:/私人/ai agent/buzz", true);
        assert!(matches!(r1, DeleteGateResult::Blocked(_)));

        // 攔截源碼目錄遞歸刪除
        let r2 = gate.inspect("rmdir /s /q", "crates/buzz-agent/src", true);
        assert!(matches!(r2, DeleteGateResult::Blocked(_)));

        // 攔截含 .. 跳脫之路徑
        let r3 = gate.inspect("delete_directory", ".buzz/sandboxes/../../crates", true);
        assert!(matches!(r3, DeleteGateResult::Blocked(_)));
    }

    #[test]
    fn test_delete_gate_allows_sandbox_cleanup() {
        let gate = DeleteGate::new();

        // 允許沙箱目錄內的遞歸清理
        let r1 = gate.inspect("rm -rf", ".buzz/sandboxes/task_123/tmp", true);
        assert_eq!(r1, DeleteGateResult::Allowed);

        // 允許系統臨時目錄清理
        let r2 = gate.inspect("delete_directory", "tmp/cache_build", true);
        assert_eq!(r2, DeleteGateResult::Allowed);

        // 允許單一檔案非遞歸刪除
        let r3 = gate.inspect("delete_file", "test.log", false);
        assert_eq!(r3, DeleteGateResult::Allowed);
    }
}

