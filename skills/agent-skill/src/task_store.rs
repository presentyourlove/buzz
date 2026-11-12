//! Agent Skill Task Store Module
//! 
//! 本機任務持久化與冪等防重放模組。

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// 任務狀態列舉
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// 任務實體定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTask {
    pub task_id: String,
    pub task_type: String,
    pub agent_name: String,
    pub description: String,
    pub status: TaskStatus,
    pub input_json: String,
    pub output_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 任務存儲庫
#[derive(Clone)]
pub struct TaskStore {
    memory_cache: Arc<RwLock<HashMap<String, SkillTask>>>,
    storage_path: PathBuf,
}

impl TaskStore {
    /// 建立新的任務存儲庫
    pub fn new(storage_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = storage_dir.as_ref();
        if !dir.exists() {
            std::fs::create_dir_all(dir).context("建立任務存儲目錄失敗")?;
        }
        let storage_path = dir.join("agent_skill_tasks.json");

        let mut initial_cache = HashMap::new();
        if storage_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&storage_path) {
                if let Ok(tasks) = serde_json::from_str::<Vec<SkillTask>>(&content) {
                    for task in tasks {
                        initial_cache.insert(task.task_id.clone(), task);
                    }
                }
            }
        }

        Ok(Self {
            memory_cache: Arc::new(RwLock::new(initial_cache)),
            storage_path,
        })
    }

    /// 檢查任務是否已處理過（冪等防重放）
    pub async fn is_task_processed(&self, task_id: &str) -> bool {
        let cache = self.memory_cache.read().await;
        if let Some(task) = cache.get(task_id) {
            matches!(task.status, TaskStatus::Completed | TaskStatus::Cancelled)
        } else {
            false
        }
    }

    /// 插入新任務
    pub async fn insert_task(&self, task: SkillTask) -> Result<()> {
        let mut cache = self.memory_cache.write().await;
        cache.insert(task.task_id.clone(), task);
        self.persist_unlocked(&cache)?;
        Ok(())
    }

    /// 更新任務狀態
    pub async fn update_status(&self, task_id: &str, status: TaskStatus, output: Option<String>) -> Result<()> {
        let mut cache = self.memory_cache.write().await;
        if let Some(task) = cache.get_mut(task_id) {
            task.status = status;
            task.output_json = output;
            task.updated_at = Utc::now().timestamp();
            let new_status_str = task.status.to_string();
            self.persist_unlocked(&cache)?;
            info!("任務 {} 狀態已更新為 {}", task_id, new_status_str);
        }
        Ok(())
    }

    /// 取得任務
    pub async fn get_task(&self, task_id: &str) -> Option<SkillTask> {
        let cache = self.memory_cache.read().await;
        cache.get(task_id).cloned()
    }

    /// 取得所有任務列表
    pub async fn list_tasks(&self) -> Vec<SkillTask> {
        let cache = self.memory_cache.read().await;
        cache.values().cloned().collect()
    }

    /// 內部持久化至磁碟
    fn persist_unlocked(&self, cache: &HashMap<String, SkillTask>) -> Result<()> {
        let tasks: Vec<&SkillTask> = cache.values().collect();
        let content = serde_json::to_string_pretty(&tasks).context("序列化任務庫失敗")?;
        std::fs::write(&self.storage_path, content).context("寫入任務庫檔案失敗")?;
        debug!("任務庫已持久化至 {:?}", self.storage_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_store_lifecycle() {
        let test_id = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let temp_dir = std::env::temp_dir().join(format!("test_store_{}", test_id));
        let store = TaskStore::new(&temp_dir).expect("建立 store 失敗");

        let task = SkillTask {
            task_id: "task-001".to_string(),
            task_type: "execute".to_string(),
            agent_name: "Pollen".to_string(),
            description: "整合 janestreet/agent_skill 模組".to_string(),
            status: TaskStatus::Pending,
            input_json: "{}".to_string(),
            output_json: None,
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        };

        store.insert_task(task).await.expect("插入任務失敗");
        assert!(!store.is_task_processed("task-001").await);

        store.update_status("task-001", TaskStatus::Completed, Some("ok".to_string())).await.expect("更新狀態失敗");
        assert!(store.is_task_processed("task-001").await);

        let retrieved = store.get_task("task-001").await.expect("找不到任務");
        assert_eq!(retrieved.status, TaskStatus::Completed);
        assert_eq!(retrieved.output_json, Some("ok".to_string()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
