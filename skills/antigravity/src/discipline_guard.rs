//! 開工紀律守門員

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::types::*;
use crate::config::AntigravityConfig;

/// 紀律檢查結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisciplineResult {
    pub checked_count: usize,
    pub active_count: usize,
    pub violation_count: usize,
    pub violations: Vec<PlanViolation>,
    pub checked_at: DateTime<Utc>,
}

/// 計畫違規
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanViolation {
    pub file: String,
    pub violations: Vec<String>,
}

/// 開工紀律守門員
pub struct DisciplineGuard;

impl DisciplineGuard {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn check(&self, config: &AntigravityConfig) -> Result<DisciplineResult> {
        let primary_plans = Path::new("C:/私人/ai agent/buzz/PLANS");
        let fallback_plans = Path::new("C:/Users/user/.buzz/PLANS");
        let plans_dir = if primary_plans.exists() { primary_plans } else { fallback_plans };
        
        if !plans_dir.exists() {
            return Ok(DisciplineResult {
                checked_count: 0,
                active_count: 0,
                violation_count: 0,
                violations: vec![],
                checked_at: Utc::now(),
            });
        }
        
        let mut result = DisciplineResult {
            checked_count: 0,
            active_count: 0,
            violation_count: 0,
            violations: vec![],
            checked_at: Utc::now(),
        };
        
        let entries = std::fs::read_dir(plans_dir)?;
        
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                result.checked_count += 1;
                
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if Self::is_active_plan(&content) {
                        result.active_count += 1;
                        
                        let violations = Self::check_plan_discipline(&path, &content, config);
                        if !violations.is_empty() {
                            result.violation_count += 1;
                            result.violations.push(PlanViolation {
                                file: path.file_name().unwrap().to_string_lossy().to_string(),
                                violations,
                            });
                        }
                    }
                }
            }
        }
        
        info!("紀律檢查完成: 檢核 {} 份, 活躍 {} 份, 違規 {} 份", 
              result.checked_count, result.active_count, result.violation_count);
        
        Ok(result)
    }
    
    fn is_active_plan(content: &str) -> bool {
        // 檢查 frontmatter 中的 status
        if let Some(frontmatter) = Self::extract_frontmatter(content) {
            if let Some(status_line) = frontmatter.lines().find(|l| l.trim().starts_with("status:")) {
                let status = status_line.split(':').nth(1).unwrap_or("").trim().trim_matches('"');
                return status == "active" || status == "pending-approval";
            }
        }
        false
    }
    
    fn extract_frontmatter(content: &str) -> Option<&str> {
        if content.starts_with("---") {
            if let Some(end) = content[3..].find("---") {
                return Some(&content[3..3+end]);
            }
        }
        None
    }
    
    fn check_plan_discipline(path: &Path, content: &str, _config: &AntigravityConfig) -> Vec<String> {
        let mut violations = Vec::new();
        let filename = path.file_name().unwrap().to_string_lossy();
        
        // 檢查四要素
        let has_project_binding = content.contains("buzz projects") || content.contains("project_id");
        let has_issues = content.contains("buzz issues") || content.contains("issue_id");
        let has_sentry_audit = content.contains("Sentry") && (content.contains("審核") || content.contains("audit") || content.contains("PASSED") || content.contains("FAILED"));
        let has_owner_approval = content.contains("主理人") && (content.contains("授權") || content.contains("同意") || content.contains("批准") || content.contains("approve"));
        
        if !has_project_binding {
            violations.push("缺少 buzz projects 專案綁定".to_string());
        }
        if !has_issues {
            violations.push("缺少 buzz issues 工單追蹤".to_string());
        }
        if !has_sentry_audit {
            violations.push("缺少 Sentry 架構安全審核標記".to_string());
        }
        if !has_owner_approval {
            violations.push("缺少主理人明確授權記錄".to_string());
        }
        
        // 檢查是否有實作內容但缺少授權
        if content.contains("實作") || content.contains("開工") || content.contains("編寫") {
            if !has_owner_approval {
                violations.push("偵測到實作相關內容但無主理人授權記錄".to_string());
            }
        }
        
        violations
    }
}
