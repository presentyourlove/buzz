//! 代碼健康檢查

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::*;

/// 代碼健康檢查器
pub struct CodeHealthCheck;

impl CodeHealthCheck {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn check(&self) -> Result<Vec<HealthIssue>> {
        let mut issues = Vec::new();
        
        // 檢查編譯警告
        issues.extend(self.check_compilation_warnings().await?);
        
        // 檢查依賴漏洞
        issues.extend(self.check_dependency_vulnerabilities().await?);
        
        // 檢查測試覆蓋率
        issues.extend(self.check_test_coverage().await?);
        
        // 檢查代碼複雜度
        issues.extend(self.check_code_complexity().await?);
        
        // 檢查死代碼
        issues.extend(self.check_dead_code().await?);
        
        // 檢查未使用依賴
        issues.extend(self.check_unused_dependencies().await?);
        
        // 安全審計
        issues.extend(self.check_security_audit().await?);
        
        info!("代碼健康檢查完成: 發現 {} 項問題", issues.len());
        Ok(issues)
    }
    
    async fn check_compilation_warnings(&self) -> Result<Vec<HealthIssue>> {
        let mut issues = Vec::new();
        
        let output = std::process::Command::new("cargo")
            .args(["check", "--workspace", "--message-format=json"])
            .current_dir("c:/私人/ai agent/buzz")
            .output()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    if json["reason"] == "compiler-message" {
                        let level = json["message"]["level"].as_str().unwrap_or("");
                        if level == "warning" {
                            let message = json["message"]["message"].as_str().unwrap_or("");
                            let code = json["message"]["code"]["code"].as_str().unwrap_or("");
                            let file = json["message"]["spans"][0]["file_name"].as_str().unwrap_or("");
                            let line_num = json["message"]["spans"][0]["line_start"].as_u64().unwrap_or(0);
                            
                            issues.push(HealthIssue {
                                id: Uuid::new_v4(),
                                title: format!("編譯警告 [{}]", code),
                                description: message.to_string(),
                                category: HealthCategory::CompilationWarning,
                                severity: HealthSeverity::Warning,
                                file_path: Some(file.to_string()),
                                line_number: Some(line_num as usize),
                                rule_id: Some(code.to_string()),
                                detected_at: Utc::now(),
                                status: HealthStatus::Open,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(issues)
    }
    
    async fn check_dependency_vulnerabilities(&self) -> Result<Vec<HealthIssue>> {
        let mut issues = Vec::new();
        
        let output = std::process::Command::new("cargo")
            .args(["audit", "--json"])
            .current_dir("c:/私人/ai agent/buzz")
            .output();
        
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    if json["type"] == "vulnerability" {
                        let vuln = &json["vulnerability"];
                        issues.push(HealthIssue {
                            id: Uuid::new_v4(),
                            title: format!("依賴漏洞: {}", vuln["id"].as_str().unwrap_or("unknown")),
                            description: vuln["description"].as_str().unwrap_or("").to_string(),
                            category: HealthCategory::DependencyVulnerability,
                            severity: match vuln["cvss"]["v3_score"].as_f64().unwrap_or(0.0) {
                                s if s >= 9.0 => HealthSeverity::Critical,
                                s if s >= 7.0 => HealthSeverity::Error,
                                s if s >= 4.0 => HealthSeverity::Warning,
                                _ => HealthSeverity::Info,
                            },
                            file_path: None,
                            line_number: None,
                            rule_id: Some(vuln["id"].as_str().unwrap_or("").to_string()),
                            detected_at: Utc::now(),
                            status: HealthStatus::Open,
                        });
                    }
                }
            }
        }
        
        Ok(issues)
    }
    
    async fn check_test_coverage(&self) -> Result<Vec<HealthIssue>> {
        let mut issues = Vec::new();
        
        // 統計測試檔案
        let repo_path = Path::new("c:/私人/ai agent/buzz");
        let mut src_files = 0;
        let mut test_files = 0;
        
        for entry in walkdir::WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs")) {
            let path = entry.path();
            if path.to_string_lossy().contains("test") || path.to_string_lossy().contains("tests") {
                test_files += 1;
            } else {
                src_files += 1;
            }
        }
        
        if src_files > 0 && test_files == 0 {
            issues.push(HealthIssue {
                id: Uuid::new_v4(),
                title: "缺少測試檔案".to_string(),
                description: format!("發現 {} 個源碼檔案但無測試檔案", src_files),
                category: HealthCategory::TestCoverage,
                severity: HealthSeverity::Warning,
                file_path: None,
                line_number: None,
                rule_id: None,
                detected_at: Utc::now(),
                status: HealthStatus::Open,
            });
        } else if src_files > 0 {
            let ratio = test_files as f64 / src_files as f64;
            if ratio < 0.3 {
                issues.push(HealthIssue {
                    id: Uuid::new_v4(),
                    title: "測試覆蓋率可能不足".to_string(),
                    description: format!("源碼檔案: {}, 測試檔案: {}, 比例: {:.1}%", src_files, test_files, ratio * 100.0),
                    category: HealthCategory::TestCoverage,
                    severity: HealthSeverity::Info,
                    file_path: None,
                    line_number: None,
                    rule_id: None,
                    detected_at: Utc::now(),
                    status: HealthStatus::Open,
                });
            }
        }
        
        Ok(issues)
    }
    
    async fn check_code_complexity(&self) -> Result<Vec<HealthIssue>> {
        let mut issues = Vec::new();
        
        // 簡單檢查：行數過長的函數
        for entry in walkdir::WalkDir::new("c:/私人/ai agent/buzz")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs")) {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                let mut in_fn = false;
                let mut fn_start = 0;
                let mut brace_count = 0;
                
                for (idx, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") || trimmed.starts_with("async fn ") {
                        in_fn = true;
                        fn_start = idx;
                        brace_count = 0;
                    }
                    
                    if in_fn {
                        brace_count += trimmed.matches('{').count() as i32;
                        brace_count -= trimmed.matches('}').count() as i32;
                        
                        if brace_count == 0 {
                            let fn_length = idx - fn_start + 1;
                            if fn_length > 100 {
                                issues.push(HealthIssue {
                                    id: Uuid::new_v4(),
                                    title: format!("函數過長: {} 行", fn_length),
                                    description: format!("檔案 {} 中的函數超過 100 行", entry.path().display()),
                                    category: HealthCategory::CodeComplexity,
                                    severity: HealthSeverity::Warning,
                                    file_path: Some(entry.path().to_string_lossy().to_string()),
                                    line_number: Some(fn_start + 1),
                                    rule_id: Some("FN_TOO_LONG".to_string()),
                                    detected_at: Utc::now(),
                                    status: HealthStatus::Open,
                                });
                            }
                            in_fn = false;
                        }
                    }
                }
            }
        }
        
        Ok(issues)
    }

    async fn check_dead_code(&self) -> Result<Vec<HealthIssue>> {
        Ok(Vec::new())
    }

    async fn check_unused_dependencies(&self) -> Result<Vec<HealthIssue>> {
        Ok(Vec::new())
    }

    async fn check_security_audit(&self) -> Result<Vec<HealthIssue>> {
        Ok(Vec::new())
    }
}
    
