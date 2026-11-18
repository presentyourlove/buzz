//! 三態熔斷器模組

use serde::{Deserialize, Serialize};

/// 熔斷器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout_ms: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_ms: 60_000,
        }
    }
}

/// 熔斷器狀態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// 熔斷器統計資訊
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub consecutive_failures: u32,
}

/// 三態熔斷器
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub config: CircuitBreakerConfig,
    pub state: CircuitBreakerState,
    pub stats: CircuitBreakerStats,
}

impl CircuitBreaker {
    /// 建立新的熔斷器實例
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitBreakerState::Closed,
            stats: CircuitBreakerStats::default(),
        }
    }

    /// 記錄一次成功呼叫
    pub fn record_success(&mut self) {
        self.stats.total_requests += 1;
        self.stats.successful_requests += 1;
        self.stats.consecutive_failures = 0;
        self.state = CircuitBreakerState::Closed;
    }

    /// 記錄一次失敗呼叫，達閾值時跳閘開啟熔斷
    pub fn record_failure(&mut self) {
        self.stats.total_requests += 1;
        self.stats.failed_requests += 1;
        self.stats.consecutive_failures += 1;
        if self.stats.consecutive_failures >= self.config.failure_threshold {
            self.state = CircuitBreakerState::Open;
        }
    }
}
