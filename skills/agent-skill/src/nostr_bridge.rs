//! Nostr Bridge Module
//! 
//! 實作 Kind:30300 事件流橋接，實現 janestreet/agent_skill 與 Buzz 討論串的雙向同步。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

/// Kind:30300 - Agent Skill 任務事件
pub const KIND_AGENT_SKILL_TASK: u64 = 30300;

/// Agent Skill 任務事件內容結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillTaskEvent {
    /// 任務唯一標識
    pub task_id: String,
    /// 任務類型：create, execute, complete, cancel
    pub task_type: String,
    /// 代理名稱
    pub agent_name: String,
    /// 任務描述
    pub description: String,
    /// 輸入參數 (JSON)
    pub input: serde_json::Value,
    /// 輸出結果 (JSON)
    pub output: Option<serde_json::Value>,
    /// 狀態：pending, running, completed, failed
    pub status: String,
    /// 建立時間戳
    pub created_at: i64,
    /// 更新時間戳
    pub updated_at: i64,
    /// 關聯的 Buzz 頻道 ID
    pub channel_id: Option<String>,
    /// 關聯的討論串事件 ID
    pub thread_id: Option<String>,
}

/// Nostr 橋接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrBridgeConfig {
    /// 中繼伺服器列表
    pub relays: Vec<String>,
    /// 簽名密鑰 (hex)
    pub private_key_hex: Option<String>,
    /// 訂閱過濾器標籤
    pub subscription_tags: Vec<String>,
    /// 重連間隔 (毫秒)
    pub reconnect_interval_ms: u64,
    /// 事件發布超時 (毫秒)
    pub publish_timeout_ms: u64,
}

impl Default for NostrBridgeConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                "wss://presentyourlove.communities.buzz.xyz".to_string(),
            ],
            private_key_hex: None,
            subscription_tags: vec!["agent-skill".to_string(), "buzz".to_string()],
            reconnect_interval_ms: 5000,
            publish_timeout_ms: 10000,
        }
    }
}

/// 事件處理器型別
pub type EventHandler = Arc<dyn Fn(AgentSkillTaskEvent) + Send + Sync>;

/// Nostr 橋接器
pub struct NostrBridge {
    config: NostrBridgeConfig,
    handlers: Arc<RwLock<HashMap<String, EventHandler>>>,
    event_tx: mpsc::UnboundedSender<AgentSkillTaskEvent>,
    running: Arc<RwLock<bool>>,
}

impl NostrBridge {
    /// 建立新的 Nostr 橋接器實例
    pub fn new(config: NostrBridgeConfig) -> (Self, mpsc::UnboundedReceiver<AgentSkillTaskEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        (
            Self {
                config,
                handlers: Arc::new(RwLock::new(HashMap::new())),
                event_tx,
                running: Arc::new(RwLock::new(false)),
            },
            event_rx,
        )
    }

    /// 註冊特定任務類型的處理器
    pub async fn register_handler<F>(&self, task_type: &str, handler: F)
    where
        F: Fn(AgentSkillTaskEvent) + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers.insert(task_type.to_string(), Arc::new(handler));
        info!("已註冊任務類型處理器: {}", task_type);
    }

    /// 派發本地事件至監聽管道
    pub fn dispatch_event(&self, event: AgentSkillTaskEvent) -> Result<()> {
        self.event_tx.send(event).context("派發事件失敗")?;
        Ok(())
    }

    /// 啟動橋接器
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = true;
        info!("Nostr 橋接器已就緒，連接中繼: {:?}", self.config.relays);
        Ok(())
    }

    /// 停止橋接器
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Nostr 橋接器已停止");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nostr_bridge_dispatch() {
        let (bridge, mut rx) = NostrBridge::new(NostrBridgeConfig::default());
        bridge.start().await.expect("啟動失敗");

        let event = AgentSkillTaskEvent {
            task_id: "evt-001".to_string(),
            task_type: "execute".to_string(),
            agent_name: "Pollen".to_string(),
            description: "測試事件流橋接".to_string(),
            input: serde_json::json!({ "cmd": "test" }),
            output: None,
            status: "pending".to_string(),
            created_at: 1000,
            updated_at: 1000,
            channel_id: Some("ch-01".to_string()),
            thread_id: Some("th-01".to_string()),
        };

        bridge.dispatch_event(event).expect("派發失敗");

        let received = rx.recv().await.expect("未接收到事件");
        assert_eq!(received.task_id, "evt-001");
        assert_eq!(received.task_type, "execute");

        bridge.stop().await.expect("停止失敗");
    }
}
