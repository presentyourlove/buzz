//! Agent Skill Crate
//! 
//! janestreet/agent_skill 概念整合技能 - 提供任務持久化、執行環境與 Nostr 事件流對接。

pub mod nostr_bridge;
pub mod task_store;
pub mod runtime;

pub use nostr_bridge::{AgentSkillTaskEvent, KIND_AGENT_SKILL_TASK};
pub use task_store::{SkillTask, TaskStatus, TaskStore};
pub use runtime::{ExecutionResult, SkillRuntime, TaskExecutor, DefaultCommandExecutor};

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// 整合服務管理器
pub struct AgentSkillService {
    runtime: Arc<SkillRuntime>,
}

impl AgentSkillService {
    /// 初始化服務
    pub fn init(storage_dir: impl AsRef<Path>) -> Result<Self> {
        let store = TaskStore::new(storage_dir)?;
        let executor = Arc::new(DefaultCommandExecutor);
        let (runtime, _) = SkillRuntime::new(store, executor);
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    /// 取得運行時
    pub fn runtime(&self) -> Arc<SkillRuntime> {
        self.runtime.clone()
    }
}

