//! env-guardian 配置與資料模型

use serde::{Deserialize, Serialize};

/// 環境變數驗證規範類型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvValidator {
    Url { schemes: Vec<String> },
    Hex { length: Option<usize> },
    NonEmpty,
}

/// 單一環境變數規範
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarSpec {
    pub name: String,
    pub required: bool,
    pub description: String,
    pub validator: Option<EnvValidator>,
    pub default: Option<String>,
    pub secret: bool,
}

/// 檢查狀態枚舉
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Passed,
    Warning,
    Failed,
}

/// 單一環境變數檢查報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvCheckReport {
    pub var_name: String,
    pub status: CheckStatus,
    pub value_preview: Option<String>,
    pub error: Option<String>,
    pub suggestion: Option<String>,
}

/// 完整環境變數檢查報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvFullReport {
    pub all_passed: bool,
    pub results: Vec<EnvCheckReport>,
}
