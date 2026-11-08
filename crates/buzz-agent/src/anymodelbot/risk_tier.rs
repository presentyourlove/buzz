//! AnyModelBot 方案 B：四級漸進式風險模型 (RiskTier)
//!
//! 定義 Level 1 (Low) 至 Level 4 (Critical) 操作風險評級，
//! 提供不同安全門檻與確認策略。

use serde::{Deserialize, Serialize};

/// 操作風險分級列舉
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskTier {
    /// Level 1: 唯讀操作、沙箱內日常操作 -> 自動放行
    Low = 1,
    /// Level 2: 檔案新建、修改自身配置、一般套件安裝 -> 首次詢問 + 支援持久授權
    Medium = 2,
    /// Level 3: 核心代碼庫變更、外部網絡請求、系統級修改 -> 每次詢問 (TTL 10min)
    High = 3,
    /// Level 4: 非沙箱遞歸刪除、格式化磁碟、特工權限自身變更 -> 絕對禁止或強制多重認證
    Critical = 4,
}

impl std::fmt::Display for RiskTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskTier::Low => write!(f, "Level 1: 低風險 (Low - 自動放行)"),
            RiskTier::Medium => write!(f, "Level 2: 中風險 (Medium - 首次確認+記住)"),
            RiskTier::High => write!(f, "Level 3: 高風險 (High - 每次確認 10m TTL)"),
            RiskTier::Critical => write!(f, "Level 4: 極高危 (Critical - 嚴格攔截)"),
        }
    }
}

/// 風險評估器
#[derive(Debug, Default, Clone)]
pub struct RiskEvaluator;

impl RiskEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// 依據動作名稱與參數進行動態風險分級
    pub fn evaluate_action(&self, action: &str, target_path: Option<&str>) -> RiskTier {
        let action_lower = action.to_lowercase();
        let path = target_path.unwrap_or("").to_lowercase();

        // 1. Critical 極高危判定
        if action_lower.contains("format")
            || action_lower.contains("drop database")
            || (action_lower.contains("delete") && (path.is_empty() || path == "/" || path == "c:\\" || path.contains("system32")))
            || action_lower.contains("rm -rf /")
        {
            return RiskTier::Critical;
        }

        // 2. High 高風險判定
        if action_lower.contains("delete")
            || action_lower.contains("rmdir")
            || action_lower.contains("remove-item")
            || action_lower.contains("git push --force")
            || action_lower.contains("cargo update")
            || action_lower.contains("modify_permission")
            || action_lower.contains("shutdown")
        {
            return RiskTier::High;
        }

        // 3. Medium 中風險判定
        if action_lower.contains("write")
            || action_lower.contains("create")
            || action_lower.contains("edit")
            || action_lower.contains("git commit")
            || action_lower.contains("install")
        {
            return RiskTier::Medium;
        }

        // 4. 其餘預設為 Low 低風險唯讀或查詢操作
        RiskTier::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_tier_evaluation() {
        let eval = RiskEvaluator::new();

        // 唯讀與查詢 -> Low
        assert_eq!(eval.evaluate_action("read_file", Some("Cargo.toml")), RiskTier::Low);
        assert_eq!(eval.evaluate_action("search_code", None), RiskTier::Low);

        // 寫入與編輯 -> Medium
        assert_eq!(eval.evaluate_action("write_file", Some("src/test.rs")), RiskTier::Medium);
        assert_eq!(eval.evaluate_action("create_directory", Some("tmp/")), RiskTier::Medium);

        // 刪除與強制更新 -> High
        assert_eq!(eval.evaluate_action("delete_file", Some("crates/old.rs")), RiskTier::High);
        assert_eq!(eval.evaluate_action("cargo update", None), RiskTier::High);

        // 系統級危險操作 -> Critical
        assert_eq!(eval.evaluate_action("delete_directory", Some("C:\\")), RiskTier::Critical);
        assert_eq!(eval.evaluate_action("rm -rf /", None), RiskTier::Critical);
    }
}

