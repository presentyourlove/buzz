//! 共識入庫引擎

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::*;

/// 共識入庫引擎
pub struct ConsensusIngest;

impl ConsensusIngest {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn extract(&self) -> Result<Vec<ConsensusItem>> {
        let mut items = Vec::new();
        
        // 掃描 PLANS 目錄中的決策記錄（優先專案集中目錄，向下相容用戶目錄）
        let primary_plans = Path::new("C:/私人/ai agent/buzz/PLANS");
        let fallback_plans = Path::new("C:/Users/user/.buzz/PLANS");
        let plans_dir = if primary_plans.exists() { primary_plans } else { fallback_plans };
        if plans_dir.exists() {
            for entry in std::fs::read_dir(plans_dir)?.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        items.extend(Self::extract_from_content(&path, &content)?);
                    }
                }
            }
        }
        
        // 掃描 GUIDES 目錄
        let primary_guides = Path::new("C:/私人/ai agent/buzz/GUIDES");
        let fallback_guides = Path::new("C:/Users/user/.buzz/GUIDES");
        let guides_dir = if primary_guides.exists() { primary_guides } else { fallback_guides };
        if guides_dir.exists() {
            for entry in std::fs::read_dir(guides_dir)?.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        items.extend(Self::extract_from_content(&path, &content)?);
                    }
                }
            }
        }
        
        info!("共識萃取完成: 發現 {} 項", items.len());
        Ok(items)
    }
    
    fn extract_from_content(path: &Path, content: &str) -> Result<Vec<ConsensusItem>> {
        let mut items = Vec::new();
        
        // 尋找決策標記
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            // 模式：【核心結論】、決策：、共識：、拍板：
            if trimmed.starts_with("【核心結論】") 
                || trimmed.starts_with("決策：")
                || trimmed.starts_with("共識：")
                || trimmed.starts_with("拍板：")
                || trimmed.contains("核心結論")
                || trimmed.contains("達成共識") {
                
                let title = Self::extract_title(trimmed, content, idx);
                let category = Self::infer_category(&title, content);
                
                items.push(ConsensusItem {
                    id: Uuid::new_v4(),
                    title,
                    content: Self::extract_context(content, idx, 5),
                    category,
                    source_message_id: None,
                    source_channel: None,
                    agreed_by: vec!["team".to_string()],
                    decided_at: Utc::now(),
                    status: ConsensusStatus::Agreed,
                });
            }
        }
        
        Ok(items)
    }
    
    fn extract_title(line: &str, content: &str, line_idx: usize) -> String {
        // 移除標記前綴
        let title = line
            .replace("【核心結論】", "")
            .replace("【核心結論】：", "")
            .replace("決策：", "")
            .replace("共識：", "")
            .replace("拍板：", "")
            .trim()
            .to_string();
        
        if !title.is_empty() {
            title
        } else {
            // 嘗試從下一行獲取
            content.lines().nth(line_idx + 1).unwrap_or("未命名決策").trim().to_string()
        }
    }
    
    fn extract_context(content: &str, line_idx: usize, context_lines: usize) -> String {
        let start = line_idx.saturating_sub(2);
        let end = (line_idx + context_lines).min(content.lines().count());
        content.lines().skip(start).take(end - start).collect::<Vec<_>>().join("\n")
    }
    
    fn infer_category(title: &str, content: &str) -> ConsensusCategory {
        let text = format!("{} {}", title, content).to_lowercase();
        
        if text.contains("架構") || text.contains("architecture") {
            ConsensusCategory::Architecture
        } else if text.contains("流程") || text.contains("process") || text.contains("紀律") {
            ConsensusCategory::Process
        } else if text.contains("技術債") || text.contains("refactor") || text.contains("重構") {
            ConsensusCategory::TechnicalDebt
        } else if text.contains("安全") || text.contains("security") || text.contains("audit") {
            ConsensusCategory::Security
        } else if text.contains("部署") || text.contains("deploy") || text.contains("release") {
            ConsensusCategory::Deployment
        } else if text.contains("測試") || text.contains("test") || text.contains("ci") {
            ConsensusCategory::Testing
        } else if text.contains("文檔") || text.contains("doc") || text.contains("guide") {
            ConsensusCategory::Documentation
        } else {
            ConsensusCategory::Process
        }
    }
}
