//! Orchestrator State
//!
//! 整合 TriggerRuntime、TriggerRegistry、TriggerCircuitBreakerManager、EventBus 的應用狀態

use crate::triggers::{
    runtime::TriggerRuntime,
    trigger_registry::TriggerRegistry,
    orchestrator::circuit_breaker::TriggerCircuitBreakerManager,
    event_bus::EventBus,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 應用狀態，整合所有核心組件
#[derive(Clone)]
pub struct AppState {
    /// TriggerRuntime 背景服務
    pub trigger_runtime: Arc<RwLock<Option<Arc<TriggerRuntime>>>>,
    /// 觸發器註冊表
    pub trigger_registry: Arc<TriggerRegistry>,
    /// 熔斷器管理器
    pub circuit_breaker_manager: Arc<TriggerCircuitBreakerManager>,
    /// 事件總線
    pub event_bus: Arc<EventBus>,
}

impl AppState {
    /// 建立新的應用狀態
    pub fn new(
        trigger_registry: Arc<TriggerRegistry>,
        circuit_breaker_manager: Arc<TriggerCircuitBreakerManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            trigger_runtime: Arc::new(RwLock::new(None)),
            trigger_registry,
            circuit_breaker_manager,
            event_bus,
        }
    }

    /// 設定 TriggerRuntime
    pub async fn set_trigger_runtime(&self, runtime: TriggerRuntime) {
        *self.trigger_runtime.write().await = Some(Arc::new(runtime));
    }

    /// 取得 TriggerRuntime
    pub async fn get_trigger_runtime(&self) -> Option<Arc<TriggerRuntime>> {
        self.trigger_runtime.read().await.clone()
    }

    /// 檢查 TriggerRuntime 是否已啟動
    pub async fn is_runtime_running(&self) -> bool {
        self.trigger_runtime.read().await.is_some()
    }
}
