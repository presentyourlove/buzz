//! 依賴工具檢查器模組

use std::process::Command;
use serde::{Deserialize, Serialize};

/// 單個依賴工具檢查結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCheckItem {
    pub name: String,
    pub available: bool,
    pub required: bool,
    pub version: Option<String>,
}

/// 依賴工具總體檢查報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCheckReport {
    pub all_passed: bool,
    pub results: Vec<ToolCheckItem>,
}

/// 依賴工具檢查器
#[derive(Debug, Clone)]
pub struct DependencyToolChecker {
    tools: Vec<(&'static str, &'static str, bool)>,
}

impl Default for DependencyToolChecker {
    fn default() -> Self {
        Self {
            tools: vec![
                ("git", "--version", false),
                ("cargo", "--version", false),
                ("python", "--version", false),
            ],
        }
    }
}

impl DependencyToolChecker {
    /// 非同步檢查所有預設系統依賴工具
    pub async fn check_all(&self) -> ToolCheckReport {
        let mut results = Vec::new();
        let mut all_passed = true;

        for &(name, flag, required) in &self.tools {
            let output = Command::new(name).arg(flag).output();
            let (available, version) = match output {
                Ok(out) if out.status.success() => {
                    let v = String::from_utf8_lossy(&out.stdout).lines().next().map(|s| s.trim().to_string());
                    (true, v)
                }
                _ => (false, None),
            };
            if required && !available {
                all_passed = false;
            }
            results.push(ToolCheckItem {
                name: name.to_string(),
                available,
                required,
                version,
            });
        }

        ToolCheckReport { all_passed, results }
    }
}
