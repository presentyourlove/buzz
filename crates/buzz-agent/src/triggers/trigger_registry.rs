//! 觸發器註冊表 - SQLite 持久化
//!
//! 參考 skills/agent-skill/src/task_store.rs 的持久化模式

use crate::triggers::condition::Condition;
use crate::triggers::types::{
    Action, DeduplicationConfig, Trigger, TriggerType,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// 觸發器註冊表
pub struct TriggerRegistry {
    db_path: PathBuf,
    triggers: Arc<RwLock<Vec<Trigger>>>,
    dirty: Arc<RwLock<bool>>,
}

impl TriggerRegistry {
    /// 建立新的觸發器註冊表（帶持久化路徑）
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        // 確保目錄存在
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("建立觸發器資料庫目錄失敗")?;
        }

        let registry = Self {
            db_path: db_path.clone(),
            triggers: Arc::new(RwLock::new(Vec::new())),
            dirty: Arc::new(RwLock::new(false)),
        };

        // 載入現有資料
        registry.load().await?;

        Ok(registry)
    }

    /// 建立新的觸發器註冊表（無持久化，用於測試/備用）
    pub fn new_in_memory() -> Self {
        Self {
            db_path: PathBuf::new(),
            triggers: Arc::new(RwLock::new(Vec::new())),
            dirty: Arc::new(RwLock::new(false)),
        }
    }
    async fn load(&self) -> Result<()> {
        if !self.db_path.exists() {
            info!("觸發器資料庫不存在，建立新檔案: {:?}", self.db_path);
            return Ok(());
        }

        let content = fs::read_to_string(&self.db_path)
            .await
            .context("讀取觸發器資料庫失敗")?;

        if content.trim().is_empty() {
            return Ok(());
        }

        let triggers: Vec<Trigger> =
            serde_json::from_str(&content).context("解析觸發器資料庫 JSON 失敗")?;

        let mut guard = self.triggers.write().await;
        *guard = triggers;
        info!("載入 {} 個觸發器", guard.len());
        Ok(())
    }

    /// 持久化觸發器資料
    pub async fn persist(&self) -> Result<()> {
        let dirty = *self.dirty.read().await;
        if !dirty {
            return Ok(());
        }

        let triggers = self.triggers.read().await;
        let content = serde_json::to_string_pretty(&*triggers).context("序列化觸發器失敗")?;

        // 原子寫入：寫入臨時檔再重命名
        let tmp_path = self.db_path.with_extension("tmp");
        fs::write(&tmp_path, content)
            .await
            .context("寫入臨時觸發器檔案失敗")?;
        fs::rename(&tmp_path, &self.db_path)
            .await
            .context("重命名觸發器檔案失敗")?;

        *self.dirty.write().await = false;
        debug!("持久化 {} 個觸發器到 {:?}", triggers.len(), self.db_path);
        Ok(())
    }

    /// 獲取所有觸發器
    pub async fn get_all(&self) -> Vec<Trigger> {
        self.triggers.read().await.clone()
    }

    /// 根據 ID 獲取觸發器
    pub async fn get(&self, id: Uuid) -> Option<Trigger> {
        self.triggers
            .read()
            .await
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    /// 根據名稱獲取觸發器
    pub async fn get_by_name(&self, name: &str) -> Option<Trigger> {
        self.triggers
            .read()
            .await
            .iter()
            .find(|t| t.name == name)
            .cloned()
    }

    /// 獲取啟用的觸發器
    pub async fn get_enabled(&self) -> Vec<Trigger> {
        self.triggers
            .read()
            .await
            .iter()
            .filter(|t| t.enabled)
            .cloned()
            .collect()
    }

    /// 插入新觸發器
    pub async fn insert(&self, trigger: Trigger) -> Result<()> {
        let mut triggers = self.triggers.write().await;

        // 檢查名稱唯一性
        if triggers.iter().any(|t| t.name == trigger.name) {
            return Err(anyhow::anyhow!("觸發器名稱已存在: {}", trigger.name));
        }

        triggers.push(trigger);
        *self.dirty.write().await = true;
        Ok(())
    }

    /// 更新觸發器
    pub async fn update(&self, trigger: Trigger) -> Result<()> {
        let mut triggers = self.triggers.write().await;

        let idx = triggers
            .iter()
            .position(|t| t.id == trigger.id)
            .ok_or_else(|| anyhow::anyhow!("觸發器不存在: {}", trigger.id))?;

        // 檢查名稱唯一性（排除自己）
        if triggers
            .iter()
            .enumerate()
            .any(|(i, t)| i != idx && t.name == trigger.name)
        {
            return Err(anyhow::anyhow!("觸發器名稱已存在: {}", trigger.name));
        }

        triggers[idx] = trigger;
        *self.dirty.write().await = true;
        Ok(())
    }

    /// 刪除觸發器
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let mut triggers = self.triggers.write().await;
        let len_before = triggers.len();
        triggers.retain(|t| t.id != id);
        let deleted = triggers.len() < len_before;
        if deleted {
            *self.dirty.write().await = true;
        }
        Ok(deleted)
    }

    /// 設置觸發器啟用/禁用狀態
    pub async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<bool> {
        let mut triggers = self.triggers.write().await;
        if let Some(trigger) = triggers.iter_mut().find(|t| t.id == id) {
            trigger.enabled = enabled;
            *self.dirty.write().await = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 獲取所有觸發器列表
    pub async fn list(&self) -> Vec<Trigger> {
        self.triggers.read().await.iter().cloned().collect()
    }

    /// 批次插入/更新（用於初始化或導入）
    pub async fn upsert_batch(&self, triggers: Vec<Trigger>) -> Result<()> {
        let mut guard = self.triggers.write().await;

        for trigger in triggers {
            if let Some(idx) = guard.iter().position(|t| t.id == trigger.id) {
                guard[idx] = trigger;
            } else {
                guard.push(trigger);
            }
        }
        *self.dirty.write().await = true;
        Ok(())
    }

    /// 獲取觸發器數量
    pub async fn count(&self) -> usize {
        self.triggers.read().await.len()
    }

    /// 觸發器總數 (同步版本，用於測試)
    pub fn trigger_count(&self) -> usize {
        self.triggers.blocking_read().len()
    }

    /// 檢查觸發器是否存在
    pub fn has_trigger(&self, id: Uuid) -> bool {
        self.triggers.blocking_read().iter().any(|t| t.id == id)
    }

    /// 獲取指定觸發器的下次觸發時間
    pub fn next_fire_time(&self, id: Uuid) -> Option<DateTime<Utc>> {
        self.triggers.blocking_read().iter().find(|t| t.id == id).and_then(|t| t.next_fire)
    }

    /// 獲取待處理觸發器數量（啟用的觸發器）
    pub fn pending_count(&self) -> usize {
        self.triggers.blocking_read().iter().filter(|t| t.enabled).count()
    }

    /// 清除所有觸發器
    pub fn clear(&self) {
        *self.triggers.blocking_write() = Vec::new();
        *self.dirty.blocking_write() = true;
    }

    /// 檢查註冊表是否正在運行（始終為 true，除非已關閉）
    pub fn is_running(&self) -> bool {
        true // 註冊表本身不會停止，只有關閉時才會
    }

    /// 獲取啟用的觸發器數量
    pub async fn count_enabled(&self) -> usize {
        self.triggers
            .read()
            .await
            .iter()
            .filter(|t| t.enabled)
            .count()
    }

    /// 同步版本：註冊新觸發器（用於 RunCtx 同步介面）
    pub fn register(&self, trigger: Trigger) -> Result<Uuid> {
        let mut triggers = self.triggers.blocking_write();
        
        // 檢查名稱唯一性
        if triggers.iter().any(|t| t.name == trigger.name) {
            return Err(anyhow::anyhow!("觸發器名稱已存在: {}", trigger.name));
        }

        let id = trigger.id;
        triggers.push(trigger);
        *self.dirty.blocking_write() = true;
        Ok(id)
    }

    /// 同步版本：註銷觸發器（用於 RunCtx 同步介面）
    pub fn unregister(&self, id: Uuid) -> bool {
        let mut triggers = self.triggers.blocking_write();
        let len_before = triggers.len();
        triggers.retain(|t| t.id != id);
        let deleted = triggers.len() < len_before;
        if deleted {
            *self.dirty.blocking_write() = true;
        }
        deleted
    }

    /// 同步版本：設置觸發器啟用/禁用狀態（用於 RunCtx 同步介面）
    pub fn set_enabled_sync(&self, id: Uuid, enabled: bool) -> bool {
        let mut triggers = self.triggers.blocking_write();
        if let Some(trigger) = triggers.iter_mut().find(|t| t.id == id) {
            trigger.enabled = enabled;
            *self.dirty.blocking_write() = true;
            true
        } else {
            false
        }
    }

    /// 同步版本：獲取所有觸發器列表（用於 RunCtx 同步介面）
    pub fn list_sync(&self) -> Vec<Trigger> {
        self.triggers.blocking_read().iter().cloned().collect()
    }

    /// 搜尋觸發器（支援條件過濾）
    pub async fn search(&self, query: &TriggerSearchQuery) -> Vec<Trigger> {
        let triggers = self.triggers.read().await;
        triggers
            .iter()
            .filter(|t| query.matches(t))
            .cloned()
            .collect()
    }

    /// 標記為需要持久化
    pub fn mark_dirty(&self) {
        *self.dirty.blocking_write() = true;
    }

    /// 關閉觸發器註冊表（優雅關機）
    pub async fn shutdown(&self) -> Result<()> {
        info!("正在關閉觸發器註冊表...");
        // 確保所有待寫入資料都已持久化
        self.persist().await?;
        info!("觸發器註冊表已關閉");
        Ok(())
    }
}

/// 觸發器搜尋查詢
#[derive(Debug, Default)]
pub struct TriggerSearchQuery {
    pub name_contains: Option<String>,
    pub enabled_only: bool,
    pub trigger_type: Option<TriggerTypeFilter>,
    pub tags: Vec<String>,
}

impl TriggerSearchQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name_contains(mut self, s: impl Into<String>) -> Self {
        self.name_contains = Some(s.into());
        self
    }

    pub fn enabled_only(mut self) -> Self {
        self.enabled_only = true;
        self
    }

    pub fn trigger_type(mut self, t: TriggerTypeFilter) -> Self {
        self.trigger_type = Some(t);
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    fn matches(&self, trigger: &Trigger) -> bool {
        if self.enabled_only && !trigger.enabled {
            return false;
        }

        if let Some(ref name_q) = self.name_contains {
            if !trigger.name.to_lowercase().contains(&name_q.to_lowercase()) {
                return false;
            }
        }

        if let Some(ref type_filter) = self.trigger_type {
            if !type_filter.matches(&trigger.trigger_type) {
                return false;
            }
        }

        if !self.tags.is_empty() {
            // 簡單的標籤匹配（可以擴展）
            let trigger_tags = self.extract_tags(trigger);
            if !self.tags.iter().all(|t| trigger_tags.contains(t)) {
                return false;
            }
        }

        true
    }

    fn extract_tags(&self, trigger: &Trigger) -> Vec<String> {
        let mut tags = Vec::new();
        match &trigger.trigger_type {
            TriggerType::Schedule { .. } => tags.push("schedule".to_string()),
            TriggerType::FileSystem { .. } => tags.push("filesystem".to_string()),
            TriggerType::Webhook { .. } => tags.push("webhook".to_string()),
            TriggerType::SystemMetric { .. } => tags.push("system".to_string()),
            TriggerType::NostrEvent { .. } => tags.push("nostr".to_string()),
        }
        tags
    }
}

/// 觸發器類型過濾器
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerTypeFilter {
    Schedule,
    FileSystem,
    Webhook,
    SystemMetric,
    NostrEvent,
}

impl TriggerTypeFilter {
    fn matches(&self, trigger_type: &TriggerType) -> bool {
        match (self, trigger_type) {
            (Self::Schedule, TriggerType::Schedule { .. }) => true,
            (Self::FileSystem, TriggerType::FileSystem { .. }) => true,
            (Self::Webhook, TriggerType::Webhook { .. }) => true,
            (Self::SystemMetric, TriggerType::SystemMetric { .. }) => true,
            (Self::NostrEvent, TriggerType::NostrEvent { .. }) => true,
            _ => false,
        }
    }
}
