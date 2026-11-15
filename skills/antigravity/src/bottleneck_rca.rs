//! 瓶頸根因分析

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::*;

/// 瓶頸 RCA 分析器
pub struct BottleneckRca;

impl BottleneckRca {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn analyze(&self) -> Result<Vec<RcaFinding>> {
        let mut findings = Vec::new();
        
        // 分析編譯警告
        findings.extend(self.analyze_compilation_warnings().await?);
        
        // 分析測試覆蓋率
        findings.extend(self.analyze_test_coverage().await?);
        
        // 分析依賴漏洞
        findings.extend(self.analyze_dependencies().await?);
        
        // 分析效能熱點
        findings.extend(self.analyze_performance().await?);
        
        // 分析錯誤模式
        findings.extend(self.analyze_error_patterns().await?);
        
        info!("RCA 分析完成: 發現 {} 項", findings.len());
        Ok(findings)
    }
    
    async fn analyze_error_patterns(&self) -> Result<Vec<RcaFinding>> {
        Ok(Vec::new())
    }
    
    async fn analyze_compilation_warnings(&self) -> Result<Vec<RcaFinding>> {
        let mut findings = Vec::new();
        
        // 執行 cargo check 獲取警告
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
                            
                            findings.push(RcaFinding {
                                id: Uuid::new_v4(),
                                title: format!("編譯警告: {}", code),
                                description: message.to_string(),
                                root_cause: format!("代碼觸發編譯器警告 {}", code),
                                impact: "可能導致未來版本不兼容或運行時問題".to_string(),
                                severity: RcaSeverity::Low,
                                category: RcaCategory::Maintainability,
                                affected_components: vec![file.to_string()],
                                recommended_actions: vec![
                                    "修復警告內容".to_string(),
                                    "考慮在 CI 中啟用 deny(warnings)".to_string(),
                                ],
                                discovered_at: Utc::now(),
                                status: RcaStatus::Open,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(findings)
    }
    
    async fn analyze_test_coverage(&self) -> Result<Vec<RcaFinding>> {
        let mut findings = Vec::new();
        
        // 檢查測試覆蓋率
        let output = std::process::Command::new("cargo")
            .args(["llvm-cov", "--workspace", "--summary-only"])
            .current_dir("c:/私人/ai agent/buzz")
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // 解析覆蓋率輸出
                for line in stdout.lines() {
                    if line.contains("%") && line.contains("coverage") {
                        // 這裡可以解析具體覆蓋率數值
                        if line.contains("0%") || line.contains("1%") || line.contains("2%") 
                            || line.contains("3%") || line.contains("4%") || line.contains("5%")
                            || line.contains("10%") || line.contains("15%") || line.contains("20%") {
                            findings.push(RcaFinding {
                                id: Uuid::new_v4(),
                                title: "測試覆蓋率過低".to_string(),
                                description: format!("檢測到低覆蓋率: {}", line),
                                root_cause: "缺少單元測試或整合測試".to_string(),
                                impact: "重構風險高，回歸缺陷難以發現".to_string(),
                                severity: RcaSeverity::Medium,
                                category: RcaCategory::Reliability,
                                affected_components: vec!["全專案".to_string()],
                                recommended_actions: vec![
                                    "為核心模組添加單元測試".to_string(),
                                    "設置最低覆蓋率門檻".to_string(),
                                ],
                                discovered_at: Utc::now(),
                                status: RcaStatus::Open,
                            });
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(findings)
    }
    
    async fn analyze_dependencies(&self) -> Result<Vec<RcaFinding>> {
        let mut findings = Vec::new();
        
        // 執行 cargo audit
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
                        findings.push(RcaFinding {
                            id: Uuid::new_v4(),
                            title: format!("依賴漏洞: {}", vuln["id"].as_str().unwrap_or("unknown")),
                            description: vuln["description"].as_str().unwrap_or("").to_string(),
                            root_cause: "使用了有已知漏洞的依賴版本".to_string(),
                            impact: "安全風險，可能被利用".to_string(),
                            severity: match vuln["cvss"]["v3_score"].as_f64().unwrap_or(0.0) {
                                s if s >= 9.0 => RcaSeverity::Critical,
                                s if s >= 7.0 => RcaSeverity::High,
                                s if s >= 4.0 => RcaSeverity::Medium,
                                _ => RcaSeverity::Low,
                            },
                            category: RcaCategory::Security,
                            affected_components: vec![vuln["package"]["name"].as_str().unwrap_or("").to_string()],
                            recommended_actions: vec![
                                "升級到修復版本".to_string(),
                                "若無修復版本，尋找替代方案".to_string(),
                            ],
                            discovered_at: Utc::now(),
                            status: RcaStatus::Open,
                        });
                    }
                }
            }
        }
        
        Ok(findings)
    }
    
    async fn analyze_performance(&self) -> Result<Vec<RcaFinding>> {
        let mut findings = Vec::new();
        
        // 檢查大型檔案（可能的效能熱點）
        let repo_path = Path::new("c:/私人/ai agent/buzz");
        let mut large_files = Vec::new();
        
        for entry in walkdir::WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs")) {
            if let Ok(metadata) = entry.metadata() {
                if metadata.len() > 50_000 { // > 50KB
                    large_files.push((entry.path().to_path_buf(), metadata.len()));
                }
            }
        }
        
        if !large_files.is_empty() {
            for (path, size) in large_files.iter().take(10) {
                findings.push(RcaFinding {
                    id: Uuid::new_v4(),
                    title: format!("大型檔案可能影響編譯效能: {}", path.display()),
                    description: format!("檔案大小: {} bytes，超過 50KB 閾值", size),
                    root_cause: "檔案行數或位元組過大導致編譯器單元展開耗時".to_string(),
                    impact: "降低增量編譯速度並增加 IDE 解析負擔".to_string(),
                    severity: RcaSeverity::Low,
                    category: RcaCategory::Performance,
                    affected_components: vec![path.to_string_lossy().to_string()],
                    recommended_actions: vec![
                        "考慮拆分為多個較小模組".to_string(),
                        "檢查是否有生成的代碼可移除".to_string(),
                    ],
                    discovered_at: Utc::now(),
                    status: RcaStatus::Open,
                });
            }
        }
        
        Ok(findings)
    }
}
      
