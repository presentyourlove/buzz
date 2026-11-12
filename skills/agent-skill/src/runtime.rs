//! Agent Skill Runtime Module
//! 
//! 相容於 janestreet/agent_skill 概念之輕量任務執行環境。

use anyhow::{Context, Result};
use crate::task_store::{SkillTask, TaskStatus, TaskStore};
use chrono::Utc;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// 任務執行結果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub task_id: String,
    pub success: bool,
    pub output: String,
    pub error_msg: Option<String>,
}

/// Agent Skill 執行器特徵（使用 Pin<Box<dyn Future>> 保持 dyn compatible）
pub trait TaskExecutor: Send + Sync {
    fn execute<'a>(&'a self, task: &'a SkillTask) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
}

/// 預設指令執行器
pub struct DefaultCommandExecutor;

impl TaskExecutor for DefaultCommandExecutor {
    fn execute<'a>(&'a self, task: &'a SkillTask) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            info!("執行任務 [{}]: {}", task.task_id, task.description);
            Ok(format!("任務 [{}] 於 {} 成功執行完成", task.task_id, Utc::now()))
        })
    }
}

/// Agent Skill 任務調度運行時
pub struct SkillRuntime {
    task_store: TaskStore,
    executor: Arc<dyn TaskExecutor>,
    task_queue_tx: mpsc::UnboundedSender<SkillTask>,
}

impl SkillRuntime {
    /// 建立新的運行時環境
    pub fn new(task_store: TaskStore, executor: Arc<dyn TaskExecutor>) -> (Self, mpsc::UnboundedReceiver<ExecutionResult>) {
        let (task_queue_tx, mut task_queue_rx) = mpsc::unbounded_channel::<SkillTask>();
        let (result_tx, result_rx) = mpsc::unbounded_channel::<ExecutionResult>();

        let store_clone = task_store.clone();
        let exec_clone = executor.clone();

        tokio::spawn(async move {
            while let Some(task) = task_queue_rx.recv().await {
                let task_id = task.task_id.clone();
                // 檢查是否已完成，避免重複執行
                if store_clone.is_task_processed(&task_id).await {
                    warn!("任務 {} 已在先前處理完成，跳過重放", task_id);
                    continue;
                }

                let _ = store_clone.update_status(&task_id, TaskStatus::Running, None).await;

                match exec_clone.execute(&task).await {
                    Ok(out) => {
                        let _ = store_clone.update_status(&task_id, TaskStatus::Completed, Some(out.clone())).await;
                        let _ = result_tx.send(ExecutionResult {
                            task_id,
                            success: true,
                            output: out,
                            error_msg: None,
                        });
                    }
                    Err(e) => {
                        error!("任務 {} 執行失敗: {:#}", task_id, e);
                        let _ = store_clone.update_status(&task_id, TaskStatus::Failed, Some(e.to_string())).await;
                        let _ = result_tx.send(ExecutionResult {
                            task_id,
                            success: false,
                            output: String::new(),
                            error_msg: Some(e.to_string()),
                        });
                    }
                }
            }
        });

        (
            Self {
                task_store,
                executor,
                task_queue_tx,
            },
            result_rx,
        )
    }

    /// 提交任務排程
    pub async fn submit_task(&self, task: SkillTask) -> Result<()> {
        self.task_store.insert_task(task.clone()).await.context("儲存任務失敗")?;
        self.task_queue_tx.send(task).context("派發任務失敗")?;
        Ok(())
    }

    /// 取得任務庫參照
    pub fn store(&self) -> &TaskStore {
        &self.task_store
    }

    /// 取得執行器參照
    pub fn executor(&self) -> &Arc<dyn TaskExecutor> {
        &self.executor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_execution_pipeline() {
        let test_id = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let temp_dir = std::env::temp_dir().join(format!("test_rt_{}", test_id));
        let store = TaskStore::new(&temp_dir).expect("建立 store 失敗");
        let executor = Arc::new(DefaultCommandExecutor);

        let (runtime, mut rx) = SkillRuntime::new(store, executor);

        let task = SkillTask {
            task_id: "rt-task-101".to_string(),
            task_type: "daily_evolution".to_string(),
            agent_name: "Honey2".to_string(),
            description: "生成 2026-11-12 進化提案".to_string(),
            status: TaskStatus::Pending,
            input_json: "{}".to_string(),
            output_json: None,
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        };

        runtime.submit_task(task).await.expect("提交失敗");

        if let Some(res) = rx.recv().await {
            assert_eq!(res.task_id, "rt-task-101");
            assert!(res.success);
            assert!(res.output.contains("成功執行完成"));
        } else {
            panic!("未接收到執行結果");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
