//! 任務同步模組 - 同步頻道任務到本地資料庫與 Nostr

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::*;
use crate::config::AntigravityConfig;
use crate::storage::AntigravityStore;

/// 任務同步器
pub struct TaskSync {
    config: AntigravityConfig,
    store: AntigravityStore,
}

impl TaskSync {
    pub fn new(config: AntigravityConfig, store: AntigravityStore) -> Self {
        Self { config, store }
    }

    /// 同步任務：從頻道/資料庫抓取並更新本地
    pub async fn sync(&self) -> Result<Vec<SyncedTask>> {
        let mut synced_tasks = Vec::new();

        // 從資料庫載入現有任務
        let existing_tasks = self.store.get_all_tasks().await?;
        
        // 模擬從 Nostr 頻道抓取新任務（實際應用中會解析 Kind:30300 事件）
        let channel_tasks = self.fetch_from_nostr().await?;

        for task in channel_tasks {
            // 檢查是否已存在
            if !existing_tasks.iter().any(|t| t.id == task.id) {
                // 新任務：存入資料庫
                self.store.save_task(&task).await?;
                synced_tasks.push(task.clone());
                info!("同步新任務: {} ({})", task.subject, task.id);
            }
        }

        info!("任務同步完成，新增 {} 個任務", synced_tasks.len());
        Ok(synced_tasks)
    }

    /// 從 Nostr 抓取任務事件
    async fn fetch_from_nostr(&self) -> Result<Vec<SyncedTask>> {
        // 實際實作中會連接 Nostr relay 並查詢 Kind:30300 事件
        // 這裡返回空向量作為佔位
        Ok(Vec::new())
    }

    /// 發布任務更新到 Nostr
    pub async fn publish_task_update(&self, task: &SyncedTask) -> Result<()> {
        // 實際實作中會發送 Kind:30300 事件
        info!("發布任務更新到 Nostr: {}", task.id);
        Ok(())
    }
}

/// 同步後的任務資料結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedTask {
    pub id: Uuid,
    pub subject: String,
    pub content: String,
    pub status: TaskStatus,
    pub assignee: Option<String>,
    pub channel_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub nostr_event_id: Option<String>,
}

/// 任務狀態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Open,
    InProgress,
    InReview,
    Done,
    Blocked,
    Cancelled,
}
