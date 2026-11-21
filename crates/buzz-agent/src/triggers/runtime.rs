//! 主動觸發器執行環境（TriggerRuntime）
//!
//! 整合 Scheduler（排程）、TriggerRegistry（持久化註冊表）、
//! ConditionEvaluator（條件求值）與 EventBus（事件總線）的背景狀態機服務。

use crate::triggers::{
    condition::{evaluate_condition, ConditionEvaluator, EvaluationContext, FactProvider, SimpleProvider},
    event_bus::EventBus,
    scheduler::Scheduler,
    trigger_registry::TriggerRegistry,
    types::{
        Action, BusEvent, EventSource, Trigger,
    },
};
use anyhow::Result;
pub use crate::triggers::types::{TriggerRuntimeConfig, TriggerRuntimeHealth};
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info};
use uuid::Uuid;

/// 背景觸發器執行環境狀態機
pub struct TriggerRuntime {
    pub config: TriggerRuntimeConfig,
    scheduler: Scheduler,
    trigger_registry: Arc<TriggerRegistry>,
    condition_evaluator: ConditionEvaluator<SimpleProvider>,
    event_bus: EventBus,
    fact_provider: Arc<dyn FactProvider + Send + Sync>,
    is_running: Arc<AtomicBool>,
    events_processed: Arc<AtomicU64>,
    errors_count: Arc<AtomicU64>,
    start_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    worker_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    shutdown_tx: Arc<RwLock<Option<mpsc::Sender<()>>>>,
}

impl TriggerRuntime {
    /// 建立預設執行環境
    pub fn new() -> Self {
        Self::with_config(TriggerRuntimeConfig::default())
    }

    /// 根據配置建立執行環境
    pub fn with_config(config: TriggerRuntimeConfig) -> Self {
        let scheduler = Scheduler::new(config.scheduler_config.clone());
        let trigger_registry = Arc::new(TriggerRegistry::new_in_memory());
        let fact_provider = Arc::new(SimpleProvider);
        let condition_evaluator = ConditionEvaluator::new(SimpleProvider);
        let event_bus = EventBus::new(config.event_bus_config.clone());

        Self {
            config,
            scheduler,
            trigger_registry,
            condition_evaluator,
            event_bus,
            fact_provider,
            is_running: Arc::new(AtomicBool::new(false)),
            events_processed: Arc::new(AtomicU64::new(0)),
            errors_count: Arc::new(AtomicU64::new(0)),
            start_time: Arc::new(RwLock::new(None)),
            worker_handle: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// 註冊觸發器
    pub async fn register_trigger(&self, trigger: Trigger) -> Result<Uuid> {
        let trigger_id = trigger.id;
        
        // 若啟用則同步註冊至排程器
        if trigger.enabled {
            let _ = self.scheduler.add_trigger(trigger.clone());
        }

        // 存入註冊表
        if self.trigger_registry.get(trigger_id).await.is_some() {
            self.trigger_registry.update(trigger).await?;
        } else {
            self.trigger_registry.insert(trigger).await?;
        }

        Ok(trigger_id)
    }

    /// 註銷觸發器
    pub async fn unregister_trigger(&self, trigger_id: Uuid) -> Result<()> {
        self.scheduler.remove_trigger(trigger_id);
        self.trigger_registry.delete(trigger_id).await?;
        Ok(())
    }

    /// 啟動執行環境（等冪設計）
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            // 已在運行中，等冪直接成功返回
            return Ok(());
        }

        *self.start_time.write().unwrap() = Some(Utc::now());

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.write().unwrap() = Some(shutdown_tx);

        let scheduler = self.scheduler.clone();
        let trigger_registry = self.trigger_registry.clone();
        let fact_provider = self.fact_provider.clone();
        let event_bus = self.event_bus.clone();
        let is_running = self.is_running.clone();
        let events_processed = self.events_processed.clone();
        let errors_count = self.errors_count.clone();

        // 確保非零檢查間隔
        let check_interval_secs = if self.config.check_interval_secs == 0 {
            1
        } else {
            self.config.check_interval_secs
        };
        let check_interval = Duration::from_millis(check_interval_secs.max(1) * 100);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !is_running.load(Ordering::SeqCst) {
                            break;
                        }

                        let due_triggers = scheduler.pop_due();

                        for trigger in due_triggers {
                            let trigger_id = trigger.id;
                            let ctx = EvaluationContext::new();
                            let mut condition_passed = Ok(true);
                            for cond in &trigger.conditions {
                                match evaluate_condition(cond, &ctx) {
                                    Ok(true) => continue,
                                    Ok(false) => {
                                        condition_passed = Ok(false);
                                        break;
                                    }
                                    Err(e) => {
                                        condition_passed = Err(e);
                                        break;
                                    }
                                }
                            }

                            match condition_passed {
                                Ok(true) => {
                                    for action in &trigger.actions {
                                        let event = BusEvent {
                                            id: Uuid::new_v4(),
                                            source: EventSource::Scheduler,
                                            payload: serde_json::to_value(action).unwrap_or_default(),
                                            timestamp: Utc::now(),
                                        };

                                        if let Err(e) = event_bus.publish(event) {
                                            errors_count.fetch_add(1, Ordering::SeqCst);
                                            error!("發布觸發事件失敗: {}", e);
                                        }
                                    }

                                    let _ = trigger_registry.set_enabled(trigger_id, true).await;
                                    events_processed.fetch_add(1, Ordering::SeqCst);
                                }
                                Ok(false) => {
                                    // 條件未滿足，重新加回排程
                                    let _ = scheduler.add_trigger(trigger);
                                }
                                Err(e) => {
                                    errors_count.fetch_add(1, Ordering::SeqCst);
                                    error!("觸發器 {} 條件求值異常: {}", trigger_id, e);
                                }
                            }
                        }

                        // 排空接收總線事件
                        while let Ok(Some(_)) = event_bus.try_recv() {
                            events_processed.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        *self.worker_handle.write().unwrap() = Some(handle);
        info!("TriggerRuntime 服務已啟動");
        Ok(())
    }

    /// 停止執行環境（等冪設計）
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.swap(false, Ordering::SeqCst) {
            // 未在運行中，等冪直接成功返回
            return Ok(());
        }

        let tx = self.shutdown_tx.write().unwrap().take();
        if let Some(tx) = tx {
            let _ = tx.send(()).await;
        }

        let handle = self.worker_handle.write().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.await;
        }

        let _ = self.event_bus.shutdown();
        info!("TriggerRuntime 服務已停止");
        Ok(())
    }

    /// 查詢是否在運行中
    pub async fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// 獲取健康狀態
    pub async fn health_check(&self) -> TriggerRuntimeHealth {
        let triggers = self.trigger_registry.get_all().await;
        let active_count = triggers.iter().filter(|t| t.enabled).count();
        let running_status = self.is_running().await;
        let start_time = *self.start_time.read().unwrap();
        let uptime_secs = start_time
            .map(|st| (Utc::now() - st).num_seconds().max(0) as u64)
            .unwrap_or(0);

        TriggerRuntimeHealth {
            is_running: running_status,
            running: running_status,
            registered_triggers: triggers.len(),
            active_triggers: active_count,
            trigger_count: triggers.len(),
            last_check: Utc::now(),
            events_processed: self.events_processed.load(Ordering::SeqCst),
            errors_count: self.errors_count.load(Ordering::SeqCst),
            uptime_secs,
        }
    }

    /// 獲取配置引用
    pub fn config(&self) -> &TriggerRuntimeConfig {
        &self.config
    }
}

impl Default for TriggerRuntime {
    fn default() -> Self {
        Self::new()
    }
}
