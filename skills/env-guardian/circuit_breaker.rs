use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 三态熔断器
/// 核心功能：提供服务熔断保护机制，在服务出现故障时自动熔断，避免级联故障
pub struct CircuitBreaker {
    /// 熔断器配置
    pub config: CircuitBreakerConfig,
    /// 状态
    pub state: CircuitBreakerState,
    /// 统计信息
    pub stats: CircuitBreakerStats,
    /// 最后状态变化时间
    pub last_state_change: Instant,
    /// 熔断历史记录
    pub state_history: Vec<StateRecord>,
}

/// 熔断器配置
pub struct CircuitBreakerConfig {
    /// 是否启用熔断保护
    pub enabled: bool,
    /// 是否启用半开状态
    pub half_open_enabled: bool,
    /// 失败次数阈值
    pub failure_threshold: u32,
    /// 恢复等待时间（毫秒）
    pub recovery_timeout: u64,
    /// 半开状态最大请求数
    pub half_open_max_requests: u32,
    /// 是否记录日志
    pub log_enabled: bool,
    /// 是否自动重置
    pub auto_reset: bool,
    /// 自定义状态变化回调
    pub state_change_callback: Option<Box<dyn Fn(CircuitBreakerState)>>,n}

/// 熔断器状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// 关闭状态 - 服务正常运行
    Closed,
    /// 打开状态 - 服务故障，熔断器打开
    Open,
    /// 半开状态 - 尝试恢复服务
    HalfOpen,
}

/// 熔断器统计信息
pub struct CircuitBreakerStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 熔断器打开时间
    pub open_duration_ms: u64,
    /// 最后一次失败时间
    pub last_failure_time: Option<Instant>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 连续成功次数
    pub consecutive_successes: u32,
}

/// 状态记录
pub struct StateRecord {
    /// 状态
    pub state: CircuitBreakerState,
    /// 状态变化时间
    pub changed_at: Instant,
    /// 状态原因
    pub reason: String,
    /// 持续时间
    pub duration_ms: u64,
}

impl CircuitBreaker {
    /// 创建新的熔断器
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitBreakerState::Closed,
            stats: CircuitBreakerStats::new(),
            last_state_change: Instant::now(),
            state_history: Vec::new(),
        }
    }

    /// 执行请求
    pub fn call(&mut self) -> Result<(), CircuitBreakerError> {
        self.stats.total_requests += 1;
        
        // 检查熔断器状态
        match self.state {
            CircuitBreakerState::Closed => {
                // 关闭状态，执行请求
                if self.execute_request() {
                    self.stats.successful_requests += 1;
                    self.stats.consecutive_successes += 1;
                    self.stats.consecutive_failures = 0;
                    Ok(())
                } else {
                    self.stats.failed_requests += 1;
                    self.stats.consecutive_failures += 1;
                    self.stats.last_failure_time = Some(Instant::now());
                    
                    // 检查是否需要熔断
                    if self.stats.consecutive_failures >= self.config.failure_threshold {
                        self.transition_to_state(CircuitBreakerState::Open, "达到失败阈值");
                    }
                    
                    Err(CircuitBreakerError::ServiceUnavailable)
                }
            }
            CircuitBreakerState::Open => {
                // 打开状态，检查是否可以进入半开状态
                if self.should_attempt_recovery() {
                    self.transition_to_state(CircuitBreakerState::HalfOpen, "尝试恢复");
                    self.call()
                } else {
                    Err(CircuitBreakerError::ServiceUnavailable)
                }
            }
            CircuitBreakerState::HalfOpen => {
                // 半开状态，执行请求
                if self.stats.total_requests - self.stats.successful_requests - self.stats.failed_requests < self.config.half_open_max_requests as u64 {
                    if self.execute_request() {
                        self.stats.successful_requests += 1;
                        self.stats.consecutive_successes += 1;
                        self.stats.consecutive_failures = 0;
                        
                        // 如果连续成功达到阈值，恢复到关闭状态
                        if self.stats.consecutive_successes >= self.config.failure_threshold {
                            self.transition_to_state(CircuitBreakerState::Closed, "成功恢复");
                        }
                        
                        Ok(())
                    } else {
                        self.stats.failed_requests += 1;
                        self.stats.consecutive_failures += 1;
                        self.stats.last_failure_time = Some(Instant::now());
                        
                        // 熔断回退到打开状态
                        self.transition_to_state(CircuitBreakerState::Open, "半开状态失败");
                        
                        Err(CircuitBreakerError::ServiceUnavailable)
                    }
                } else {
                    Err(CircuitBreakerError::ServiceUnavailable)
                }
            }
        }
    }

    /// 执行请求
    fn execute_request(&self) -> bool {
        // 这里应该实现实际的服务请求逻辑
        // 由于实现复杂性，这里返回一个模拟结果
        // 在真实场景中，这里应该调用实际的服务
        true
    }

    /// 检查是否应该尝试恢复
    fn should_attempt_recovery(&self) -> bool {
        if !self.config.half_open_enabled {
            return false;
        }
        
        let elapsed = self.last_state_change.elapsed().as_millis() as u64;
        elapsed >= self.config.recovery_timeout
    }

    /// 状态转换
    fn transition_to_state(&mut self, new_state: CircuitBreakerState, reason: &str) {
        let old_state = self.state;
        let now = Instant::now();
        let duration = now.duration_since(self.last_state_change).as_millis() as u64;
        
        // 更新状态历史记录
        self.state_history.push(StateRecord {
            state: old_state,
            changed_at: self.last_state_change,
            reason: format!("{}", reason),
            duration_ms: duration,
        });
        
        // 更新状态
        self.state = new_state;
        self.last_state_change = now;
        
        // 更新统计信息
        if new_state == CircuitBreakerState::Open {
            self.stats.open_duration_ms = duration;
        }
        
        // 调用状态变化回调
        if let Some(callback) = &self.config.state_change_callback {
            callback(new_state);
        }
        
        // 记录日志
        if self.config.log_enabled {
            self.log_state_change(old_state, new_state, reason);
        }
    }

    /// 记录状态变化日志
    fn log_state_change(&self, old_state: CircuitBreakerState, new_state: CircuitBreakerState, reason: &str) {
        // 这里应该实现实际的日志记录逻辑
        // 由于实现复杂性，这里只是打印日志
        println!("熔断器状态变化: {:?} -> {:?}, 原因: {}", old_state, new_state, reason);
    }

    /// 获取当前状态
    pub fn get_state(&self) -> CircuitBreakerState {
        self.state
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> CircuitBreakerStats {
        self.stats.clone()
    }

    /// 重置熔断器
    pub fn reset(&mut self) {
        self.transition_to_state(CircuitBreakerState::Closed, "手动重置");
        self.stats = CircuitBreakerStats::new();
        self.state_history.clear();
    }

    /// 检查是否健康
    pub fn is_healthy(&self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => {
                // 检查连续失败次数是否过多
                self.stats.consecutive_failures < self.config.failure_threshold
            }
            CircuitBreakerState::Open => {
                // 检查是否可以尝试恢复
                self.should_attempt_recovery()
            }
            CircuitBreakerState::HalfOpen => {
                // 半开状态，检查请求是否成功
                let total_attempts = self.stats.total_requests - self.stats.successful_requests - self.stats.failed_requests;
                total_attempts < self.config.half_open_max_requests as u64
            }
        }
    }
}

/// 熔断器错误枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerError {
    /// 服务不可用
    ServiceUnavailable,
    /// 熔断器已打开
    CircuitOpen,
    /// 半开状态请求失败
    HalfOpenFailure,
    /// 未知错误
    UnknownError,
}

impl CircuitBreakerStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_reques
