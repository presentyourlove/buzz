//! Circuit Breaker for Trigger Orchestrator
//!
//! 基於 health_analyzer.rs 的 CircuitBreakerFile 實現觸發器級別的熔斷機制

use anyhow::{Context, Result};
use crate::health_analyzer::{CircuitBreakerFile, CircuitBreakerStatus, read_circuit_breaker, write_circuit_breaker};
use crate::types::AgentError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

fn default_success_threshold() -> u32 { 2 }
fn default_timeout_secs() -> u64 { 30 }

/// 熔斷器配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CircuitBreakerConfig {
    /// 連續失敗閾值，超過則觸發熔斷
    pub failure_threshold: u32,
    /// 自動重置等待時間（秒）
    pub auto_reset_after_secs: u64,
    /// 半開狀態下的探針數量
    pub half_open_probes: u32,
    /// 恢復正常所需成功次數
    #[serde(default = "default_success_threshold")]
    pub success_threshold: u32,
    /// 逾時秒數
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            auto_reset_after_secs: 30,
            half_open_probes: 3,
            success_threshold: 2,
            timeout_secs: 30,
        }
    }
}

/// 熔斷器狀態（API 面向）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerCircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl From<&CircuitBreakerStatus> for TriggerCircuitBreakerState {
    fn from(status: &CircuitBreakerStatus) -> Self {
        if status.tripped {
            if status.failure_count == 0 {
                TriggerCircuitBreakerState::HalfOpen
            } else {
                TriggerCircuitBreakerState::Open
            }
        } else {
            TriggerCircuitBreakerState::Closed
        }
    }
}

/// 觸發器熔斷器管理器
pub struct TriggerCircuitBreakerManager {
    storage_dir: PathBuf,
    breakers: Arc<RwLock<HashMap<Uuid, CircuitBreakerFile>>>,
    default_config: CircuitBreakerConfig,
}

impl TriggerCircuitBreakerManager {
    /// 建立新的熔斷器管理器
    pub fn new(storage_dir: impl Into<PathBuf>, default_config: CircuitBreakerConfig) -> Self {
        Self {
            storage_dir: storage_dir.into(),
            breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config,
        }
    }

    /// 取得或建立指定觸發器的熔斷器
    pub async fn get_or_create(
        &self,
        trigger_id: Uuid,
        trigger_name: &str,
        _config: Option<CircuitBreakerConfig>,
    ) -> Result<CircuitBreakerFile, AgentError> {
        let mut breakers = self.breakers.write().await;

        if let Some(breaker) = breakers.get(&trigger_id) {
            return Ok(breaker.clone());
        }

        let file_path = self.storage_dir.join(format!("trigger_{}.json", trigger_id));
        let breaker = if file_path.exists() {
            let status = read_circuit_breaker(&file_path).await?;
            CircuitBreakerFile {
                tripped: status.tripped,
                tripped_at: status.tripped_at,
                reason: status.reason,
                failure_count: status.failure_count,
                last_failure: status.last_failure,
                component: Some(trigger_name.to_string()),
            }
        } else {
            CircuitBreakerFile {
                tripped: false,
                tripped_at: None,
                reason: None,
                failure_count: 0,
                last_failure: None,
                component: Some(trigger_name.to_string()),
            }
        };

        breakers.insert(trigger_id, breaker.clone());
        Ok(breaker)
    }

    /// 記錄成功（重置失敗計數，若在半開狀態則關閉熔斷器）
    pub async fn record_success(&self, trigger_id: Uuid) -> Result<()> {
        let mut breakers = self.breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&trigger_id) {
            breaker.failure_count = 0;
            breaker.tripped = false;
            breaker.tripped_at = None;
            breaker.reason = None;
            breaker.last_failure = None;

            let file_path = self.storage_dir.join(format!("trigger_{}.json", trigger_id));
            write_circuit_breaker(&file_path, breaker).await
                .map_err(|e| anyhow::anyhow!("持久化熔斷器成功狀態失敗: {e}"))?;
        }
        Ok(())
    }

    /// 記錄失敗（增加失敗計數，達閾值則觸發熔斷）
    pub async fn record_failure(&self, trigger_id: Uuid, reason: String) -> Result<bool, AgentError> {
        let mut breakers = self.breakers.write().await;
        let mut tripped = false;

        if let Some(breaker) = breakers.get_mut(&trigger_id) {
            breaker.failure_count += 1;
            breaker.last_failure = Some(chrono::Utc::now().timestamp().to_string());
            breaker.reason = Some(reason.clone());

            let config = self.default_config.clone();
            if breaker.failure_count >= config.failure_threshold && !breaker.tripped {
                breaker.tripped = true;
                breaker.tripped_at = Some(chrono::Utc::now().timestamp().to_string());
                tripped = true;
            }

            let file_path = self.storage_dir.join(format!("trigger_{}.json", trigger_id));
            write_circuit_breaker(&file_path, breaker).await?;
        }
        Ok(tripped)
    }

    /// 手動重置熔斷器
    pub async fn manual_reset(&self, trigger_id: Uuid) -> Result<()> {
        let mut breakers = self.breakers.write().await;
        if let Some(breaker) = breakers.get_mut(&trigger_id) {
            breaker.tripped = false;
            breaker.tripped_at = None;
            breaker.reason = None;
            breaker.failure_count = 0;
            breaker.last_failure = None;

            let file_path = self.storage_dir.join(format!("trigger_{}.json", trigger_id));
            write_circuit_breaker(&file_path, breaker).await
                .map_err(|e| anyhow::anyhow!("持久化手動重置熔斷器失敗: {e}"))?;
        }
        Ok(())
    }

    /// 取得熔斷器狀態
    pub async fn get_status(&self, trigger_id: Uuid) -> Result<Option<TriggerCircuitBreakerState>> {
        let breakers = self.breakers.read().await;
        Ok(breakers.get(&trigger_id).map(|b| {
            let status = CircuitBreakerStatus {
                tripped: b.tripped,
                tripped_at: b.tripped_at.clone(),
                reason: b.reason.clone(),
                failure_count: b.failure_count,
                last_failure: b.last_failure.clone(),
            };
            TriggerCircuitBreakerState::from(&status)
        }))
    }

    /// 取得詳細熔斷器狀態（包含失敗計數等）
    pub async fn get_detailed_status(&self, trigger_id: Uuid) -> Result<Option<CircuitBreakerFile>> {
        let breakers = self.breakers.read().await;
        Ok(breakers.get(&trigger_id).cloned())
    }

    /// 更新熔斷器配置
    pub async fn update_config(
        &self,
        trigger_id: Uuid,
        trigger_name: &str,
        _config: CircuitBreakerConfig,
    ) -> Result<(), AgentError> {
        let mut breakers = self.breakers.write().await;

        let file_path = self.storage_dir.join(format!("trigger_{}.json", trigger_id));
        let breaker = if let Some(existing) = breakers.get(&trigger_id) {
            existing.clone()
        } else if file_path.exists() {
            let status = read_circuit_breaker(&file_path).await?;
            CircuitBreakerFile {
                tripped: status.tripped,
                tripped_at: status.tripped_at,
                reason: status.reason,
                failure_count: status.failure_count,
                last_failure: status.last_failure,
                component: Some(trigger_name.to_string()),
            }
        } else {
            CircuitBreakerFile {
                tripped: false,
                tripped_at: None,
                reason: None,
                failure_count: 0,
                last_failure: None,
                component: Some(trigger_name.to_string()),
            }
        };

        breakers.insert(trigger_id, breaker);
        Ok(())
    }

    /// 移除熔斷器（觸發器刪除時調用）
    pub async fn remove(&self, trigger_id: Uuid) -> Result<()> {
        let mut breakers = self.breakers.write().await;
        if breakers.remove(&trigger_id).is_some() {
            let file_path = self.storage_dir.join(format!("trigger_{}.json", trigger_id));
            let _ = std::fs::remove_file(file_path);
        }
        Ok(())
    }
}

/// 測試用記憶體熔斷器（支援狀態機轉換與測試介面）
pub struct TriggerCircuitBreaker {
    breakers: Arc<RwLock<HashMap<String, BreakerState>>>,
    config: CircuitBreakerConfig,
}

#[derive(Debug, Clone)]
struct BreakerState {
    failure_count: u32,
    success_count: u32,
    state: TriggerCircuitBreakerState,
    last_failure_reason: Option<String>,
}

impl TriggerCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn record_failure(&self, trigger_id: &str, _trigger_name: &str) {
        let mut breakers = self.breakers.write().await;
        let entry = breakers.entry(trigger_id.to_string()).or_insert(BreakerState {
            failure_count: 0,
            success_count: 0,
            state: TriggerCircuitBreakerState::Closed,
            last_failure_reason: None,
        });

        entry.failure_count += 1;
        entry.success_count = 0;
        if entry.failure_count >= self.config.failure_threshold {
            entry.state = TriggerCircuitBreakerState::Open;
        }
    }

    pub async fn record_success(&self, trigger_id: &str, _trigger_name: &str) {
        let mut breakers = self.breakers.write().await;
        if let Some(entry) = breakers.get_mut(trigger_id) {
            match entry.state {
                TriggerCircuitBreakerState::Open => {
                    entry.state = TriggerCircuitBreakerState::HalfOpen;
                    entry.success_count = 1;
                }
                TriggerCircuitBreakerState::HalfOpen => {
                    entry.success_count += 1;
                    if entry.success_count >= self.config.success_threshold {
                        entry.state = TriggerCircuitBreakerState::Closed;
                        entry.failure_count = 0;
                    }
                }
                TriggerCircuitBreakerState::Closed => {
                    entry.failure_count = 0;
                }
            }
        }
    }

    pub async fn get_state(&self, trigger_id: &str, _trigger_name: &str) -> TriggerCircuitBreakerState {
        let breakers = self.breakers.read().await;
        breakers.get(trigger_id).map(|s| s.state).unwrap_or(TriggerCircuitBreakerState::Closed)
    }

    pub async fn can_execute(&self, trigger_id: &str, _trigger_name: &str) -> bool {
        let breakers = self.breakers.read().await;
        breakers.get(trigger_id).map(|s| s.state != TriggerCircuitBreakerState::Open).unwrap_or(true)
    }

    pub async fn reset(&self, trigger_id: &str) {
        let mut breakers = self.breakers.write().await;
        if let Some(entry) = breakers.get_mut(trigger_id) {
            entry.state = TriggerCircuitBreakerState::Closed;
            entry.failure_count = 0;
            entry.success_count = 0;
            entry.last_failure_reason = None;
        }
    }
}
