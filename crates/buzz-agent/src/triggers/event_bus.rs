//! 事件總線 - 發布/訂閱 + 背壓控制
//!
//! 參考 skills/agent-skill/src/task_store.rs 的並發模式

use crate::triggers::types::{BusEvent, EventSource};
use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// 訂閱者 ID
pub type SubscriberId = Uuid;

/// 事件處理器 trait
pub trait EventHandler: Send + Sync {
    fn handle(&self, event: BusEvent) -> Result<()>;
    fn event_types(&self) -> Vec<EventSource>;
    fn name(&self) -> &str;
}

/// 事件總線配置
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// 事件緩衝區大小
    pub buffer_size: usize,
    /// 背壓策略
    pub backpressure_strategy: BackpressureStrategy,
    /// 是否啟用去重
    pub enable_deduplication: bool,
    /// 去重窗口（秒）
    pub dedup_window_secs: u64,
}

/// 背壓策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureStrategy {
    /// 丟棄最舊事件
    DropOldest,
    /// 丟棄最新事件
    DropNewest,
    /// 阻塞發送者
    Block,
    /// 返回錯誤
    ReturnError,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            backpressure_strategy: BackpressureStrategy::DropOldest,
            enable_deduplication: true,
            dedup_window_secs: 60,
        }
    }
}

/// 事件總線
pub struct EventBus {
    sender: Sender<BusEvent>,
    receiver: Receiver<BusEvent>,
    subscribers: Arc<RwLock<HashMap<SubscriberId, Box<dyn EventHandler>>>>,
    config: EventBusConfig,
    // 簡單去重：記錄最近事件的雜湊
    recent_events: Arc<RwLock<HashMap<u64, chrono::DateTime<chrono::Utc>>>>,
}

impl EventBus {
    /// 建立新的事件總線
    pub fn new(config: EventBusConfig) -> Self {
        let (sender, receiver) = bounded(config.buffer_size);
        Self {
            sender,
            receiver,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            config,
            recent_events: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 發布事件（非阻塞，依背壓策略處理）
    pub fn publish(&self, event: BusEvent) -> Result<()> {
        // 去重檢查
        if self.config.enable_deduplication {
            let event_hash = self.compute_event_hash(&event);
            let now = chrono::Utc::now();
            let mut recent = self.recent_events.write().unwrap();

            // 清理過期項目
            recent.retain(|_, ts| now.signed_duration_since(*ts).num_seconds() < self.config.dedup_window_secs as i64);

            if recent.contains_key(&event_hash) {
                return Ok(()); // 去重，直接返回成功
            }

            recent.insert(event_hash, now);
        }

        match self.config.backpressure_strategy {
            BackpressureStrategy::DropOldest => {
                // 嘗試發送，若滿則移除最舊再發送
                match self.sender.try_send(event) {
                    Ok(()) => Ok(()),
                    Err(TrySendError::Full(event)) => {
                        // 移除最舊
                        let _ = self.receiver.try_recv();
                        // 再次嘗試
                        self.sender.try_send(event).map_err(|e| anyhow::anyhow!("Event bus full: {:?}", e))
                    }
                    Err(e) => Err(anyhow::anyhow!("Event bus error: {:?}", e)),
                }
            }
            BackpressureStrategy::DropNewest => {
                // 直接丟棄最新
                self.sender.try_send(event).map_err(|_| anyhow::anyhow!("Event bus full, dropped newest")).map(|_| ())
            }
            BackpressureStrategy::Block => {
                // 阻塞發送
                self.sender.send(event).map_err(|e| anyhow::anyhow!("Event bus send error: {:?}", e))
            }
            BackpressureStrategy::ReturnError => {
                self.sender.try_send(event).map_err(|e| anyhow::anyhow!("Event bus full: {:?}", e))
            }
        }
    }

    /// 發布簡單事件
    pub fn publish_simple(&self, source: EventSource, payload: serde_json::Value) -> Result<()> {
        let event = BusEvent {
            id: Uuid::new_v4(),
            source,
            payload,
            timestamp: chrono::Utc::now(),
        };
        self.publish(event)
    }

    /// 訂閱事件
    pub fn subscribe(&self, handler: Box<dyn EventHandler>) -> SubscriberId {
        let id = Uuid::new_v4();
        let mut subs = self.subscribers.write().unwrap();
        subs.insert(id, handler);
        id
    }

    /// 取消訂閱
    pub fn unsubscribe(&self, id: SubscriberId) -> bool {
        let mut subs = self.subscribers.write().unwrap();
        subs.remove(&id).is_some()
    }

    /// 獲取接收器（用於引擎主循環消費）
    pub fn receiver(&self) -> &Receiver<BusEvent> {
        &self.receiver
    }

    /// 嘗試接收事件（非阻塞）
    pub fn try_recv(&self) -> Result<Option<BusEvent>, TryRecvError> {
        self.receiver.try_recv().map(Some).map_err(|e| e.into())
    }

    /// 分發事件給所有訂閱者
    pub fn dispatch(&self, event: &BusEvent) {
        let subs = self.subscribers.read().unwrap();
        for handler in subs.values() {
            if handler.event_types().contains(&event.source) || handler.event_types().is_empty() {
                if let Err(e) = handler.handle(event.clone()) {
                    tracing::error!("Handler {} failed: {:?}", handler.name(), e);
                }
            }
        }
    }

    /// 獲取訂閱者數量
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().unwrap().len()
    }

    /// 計算事件雜湊（用於去重）
    fn compute_event_hash(&self, event: &BusEvent) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        event.source.hash(&mut hasher);
        event.payload.to_string().hash(&mut hasher);
        // 時間精度到秒級
        event.timestamp.timestamp().hash(&mut hasher);
        hasher.finish()
    }

    /// 清理過期去重記錄
    pub fn cleanup_dedup(&self) {
        let now = chrono::Utc::now();
        let mut recent = self.recent_events.write().unwrap();
        recent.retain(|_, ts| now.signed_duration_since(*ts).num_seconds() < self.config.dedup_window_secs as i64);
    }
}

/// 簡單的閉包處理器適配器
pub struct ClosureHandler<F>
where
    F: Fn(BusEvent) -> Result<()> + Send + Sync + 'static,
{
    handler: F,
    event_types: Vec<EventSource>,
    name: String,
}

impl<F> ClosureHandler<F>
where
    F: Fn(BusEvent) -> Result<()> + Send + Sync + 'static,
{
    pub fn new(name: String, event_types: Vec<EventSource>, handler: F) -> Self {
        Self {
            handler,
            event_types,
            name,
        }
    }
}

impl<F> EventHandler for ClosureHandler<F>
where
    F: Fn(BusEvent) -> Result<()> + Send + Sync + 'static,
{
    fn handle(&self, event: BusEvent) -> Result<()> {
        (self.handler)(event)
    }

    fn event_types(&self) -> Vec<EventSource> {
        self.event_types.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 建立簡單處理器的輔助函數
pub fn make_handler<F>(name: &str, event_types: Vec<EventSource>, handler: F) -> Box<dyn EventHandler>
where
    F: Fn(BusEvent) -> Result<()> + Send + Sync + 'static,
{
    Box::new(ClosureHandler::new(name.to_string(), event_types, handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_event_bus_basic_publish_subscribe() {
        let bus = EventBus::new(EventBusConfig::default());
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler_id = bus.subscribe(make_handler("test", vec![EventSource::Scheduler], move |event| {
            received_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));

        // 發布事件
        bus.publish_simple(EventSource::Scheduler, json!({"test": "data"})).unwrap();

        // 消費並分發
        if let Ok(Some(event)) = bus.try_recv() {
            bus.dispatch(&event);
        }

        // 驗證
        let msgs = received.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], json!({"test": "data"}));

        // 取消訂閱
        assert!(bus.unsubscribe(handler_id));
        assert_eq!(bus.subscriber_count(), 0);
    }
}
