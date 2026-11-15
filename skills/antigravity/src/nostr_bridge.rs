//! Nostr 事件橋接器 - Kind:30300 事件流

use anyhow::Result;
use nostr::{EventBuilder, EventId, Keys, Kind, Tag};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::types::*;
use crate::config::AntigravityConfig;

/// Nostr 橋接器
pub struct NostrBridge {
    keys: Keys,
    config: AntigravityConfig,
    connected: Arc<Mutex<bool>>,
}

impl NostrBridge {
    pub async fn new(config: AntigravityConfig) -> Result<Self> {
        let keys = Keys::generate();
        let bridge = Self {
            keys,
            config,
            connected: Arc::new(Mutex::new(true)),
        };
        Ok(bridge)
    }
    
    /// 發布演化報告事件
    pub async fn publish_evolution_report(&self, report: &EvolutionReport) -> Result<EventId> {
        let content = serde_json::to_string(&report)?;
        
        let mut builder = EventBuilder::new(Kind::Custom(self.config.nostr.kind), content);
        let tags = [
            ["d", &format!("daily-evolution-{}", report.date.format("%Y%m%d"))],
            ["t", "daily-evolution"],
            ["t", "architecture"],
            ["t", "automation"],
            ["channel", &self.config.monitor.alert_channel],
        ];
        for t in tags {
            if let Ok(tag) = Tag::parse(t) {
                builder = builder.tag(tag);
            }
        }
        
        let event = builder.sign_with_keys(&self.keys)?;
        info!("已生成並簽名演化報告事件: {}", event.id);
        Ok(event.id)
    }
    
    /// 發布共識項目事件
    pub async fn publish_consensus(&self, items: &[ConsensusItem]) -> Result<Vec<EventId>> {
        let mut event_ids = Vec::new();
        
        for item in items {
            let content = serde_json::to_string(item)?;
            let mut builder = EventBuilder::new(Kind::Custom(self.config.nostr.kind), content);
            
            let cat_str = format!("{:?}", item.category).to_lowercase();
            let d_str = format!("consensus-{}", item.id);
            let tags = [
                ["d", d_str.as_str()],
                ["t", "consensus"],
                ["t", cat_str.as_str()],
                ["channel", self.config.monitor.alert_channel.as_str()],
            ];
            for t in tags {
                if let Ok(tag) = Tag::parse(t) {
                    builder = builder.tag(tag);
                }
            }
            if let Some(msg_id) = &item.source_message_id {
                if let Ok(tag) = Tag::parse(["e", msg_id]) {
                    builder = builder.tag(tag);
                }
            }
            
            let event = builder.sign_with_keys(&self.keys)?;
            event_ids.push(event.id);
        }
        
        info!("已發布 {} 個共識事件", event_ids.len());
        Ok(event_ids)
    }
    
    /// 發布 RCA 發現事件
    pub async fn publish_rca_findings(&self, findings: &[RcaFinding]) -> Result<Vec<EventId>> {
        let mut event_ids = Vec::new();
        
        for finding in findings {
            let content = serde_json::to_string(finding)?;
            let mut builder = EventBuilder::new(Kind::Custom(self.config.nostr.kind), content);
            
            let d_str = format!("rca-{}", finding.id);
            let cat_str = format!("{:?}", finding.category).to_lowercase();
            let sev_str = format!("{:?}", finding.severity).to_lowercase();
            let tags = [
                ["d", d_str.as_str()],
                ["t", "rca"],
                ["t", cat_str.as_str()],
                ["t", sev_str.as_str()],
                ["channel", self.config.monitor.alert_channel.as_str()],
            ];
            for t in tags {
                if let Ok(tag) = Tag::parse(t) {
                    builder = builder.tag(tag);
                }
            }
            
            let event = builder.sign_with_keys(&self.keys)?;
            event_ids.push(event.id);
        }
        
        info!("已發布 {} 個 RCA 事件", event_ids.len());
        Ok(event_ids)
    }
    
    /// 發布健康問題事件
    pub async fn publish_health_issues(&self, issues: &[HealthIssue]) -> Result<Vec<EventId>> {
        let mut event_ids = Vec::new();
        
        for issue in issues {
            let content = serde_json::to_string(issue)?;
            let mut builder = EventBuilder::new(Kind::Custom(self.config.nostr.kind), content);
            
            let d_str = format!("health-{}", issue.id);
            let cat_str = format!("{:?}", issue.category).to_lowercase();
            let sev_str = format!("{:?}", issue.severity).to_lowercase();
            let tags = [
                ["d", d_str.as_str()],
                ["t", "health"],
                ["t", cat_str.as_str()],
                ["t", sev_str.as_str()],
                ["channel", self.config.monitor.alert_channel.as_str()],
            ];
            for t in tags {
                if let Ok(tag) = Tag::parse(t) {
                    builder = builder.tag(tag);
                }
            }
            if let Some(file) = &issue.file_path {
                if let Ok(tag) = Tag::parse(["file", file]) {
                    builder = builder.tag(tag);
                }
            }
            
            let event = builder.sign_with_keys(&self.keys)?;
            event_ids.push(event.id);
        }
        
        info!("已發布 {} 個健康問題事件", event_ids.len());
        Ok(event_ids)
    }

    /// 發布任務更新事件
    pub async fn publish_task_update(&self, task: &crate::task_sync::SyncedTask) -> Result<EventId> {
        let content = serde_json::to_string(task)?;
        let mut builder = EventBuilder::new(Kind::Custom(self.config.nostr.kind), content);
        let d_str = format!("task-{}", task.id);
        let tags = [
            ["d", d_str.as_str()],
            ["t", "task"],
            ["channel", self.config.monitor.alert_channel.as_str()],
        ];
        for t in tags {
            if let Ok(tag) = Tag::parse(t) {
                builder = builder.tag(tag);
            }
        }
        let event = builder.sign_with_keys(&self.keys)?;
        Ok(event.id)
    }

    /// 發布知識同步事件
    pub async fn publish_knowledge(&self, knowledge: &crate::knowledge_sync::SyncedKnowledge) -> Result<EventId> {
        let content = serde_json::to_string(knowledge)?;
        let mut builder = EventBuilder::new(Kind::Custom(self.config.nostr.kind), content);
        let d_str = format!("knowledge-{}", knowledge.id);
        let tags = [
            ["d", d_str.as_str()],
            ["t", "knowledge"],
            ["channel", self.config.monitor.alert_channel.as_str()],
        ];
        for t in tags {
            if let Ok(tag) = Tag::parse(t) {
                builder = builder.tag(tag);
            }
        }
        let event = builder.sign_with_keys(&self.keys)?;
        Ok(event.id)
    }
    
    /// 獲取連線狀態
    pub async fn is_connected(&self) -> bool {
        *self.connected.lock().await
    }
    
    /// 斷開連線
    pub async fn disconnect(&self) {
        *self.connected.lock().await = false;
    }
}
