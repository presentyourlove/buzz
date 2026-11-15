//! 知識同步模組 - 同步文檔、指南、研究報告到本地與 Nostr

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::*;
use crate::config::AntigravityConfig;
use crate::storage::AntigravityStore;

/// 知識同步器
pub struct KnowledgeSync {
    config: AntigravityConfig,
    store: AntigravityStore,
}

impl KnowledgeSync {
    pub fn new(config: AntigravityConfig, store: AntigravityStore) -> Self {
        Self { config, store }
    }

    /// 同步知識文檔：掃描本地目錄並同步到資料庫
    pub async fn sync(&self) -> Result<Vec<SyncedKnowledge>> {
        let mut synced = Vec::new();

        // 掃描本地知識目錄（優先專案集中目錄，向下相容用戶目錄）
        let knowledge_dirs = vec![
            "C:/私人/ai agent/buzz/RESEARCH",
            "C:/私人/ai agent/buzz/GUIDES",
            "C:/私人/ai agent/buzz/PLANS",
            "C:/Users/user/.buzz/RESEARCH",
            "C:/Users/user/.buzz/GUIDES",
            "C:/Users/user/.buzz/PLANS",
        ];

        for dir in knowledge_dirs {
            let path = Path::new(dir);
            if path.exists() {
                for entry in walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md")) {
                    
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        let knowledge = SyncedKnowledge {
                            id: Uuid::new_v4(),
                            title: entry.path().file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            content,
                            category: Self::infer_category(entry.path()),
                            file_path: entry.path().to_string_lossy().to_string(),
                            created_at: Utc::now(),
                            updated_at: Utc::now(),
                            nostr_event_id: None,
                        };

                        // 檢查是否已存在（簡化：總是新增/更新）
                        self.store.save_knowledge(&knowledge).await?;
                        synced.push(knowledge);
                    }
                }
            }
        }

        info!("知識同步完成，處理 {} 個文檔", synced.len());
        Ok(synced)
    }

    fn infer_category(path: &Path) -> KnowledgeCategory {
        let path_str = path.to_string_lossy().to_lowercase();
        if path_str.contains("research") { KnowledgeCategory::Research }
        else if path_str.contains("guides") { KnowledgeCategory::Guide }
        else if path_str.contains("plans") { KnowledgeCategory::Plan }
        else { KnowledgeCategory::Other }
    }

    /// 發布知識更新到 Nostr
    pub async fn publish_knowledge(&self, knowledge: &SyncedKnowledge) -> Result<()> {
        info!("發布知識更新到 Nostr: {}", knowledge.id);
        Ok(())
    }
}

/// 同步後的知識資料結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedKnowledge {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub category: KnowledgeCategory,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub nostr_event_id: Option<String>,
}

/// 知識分類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeCategory {
    Research,
    Guide,
    Plan,
    Other,
}
