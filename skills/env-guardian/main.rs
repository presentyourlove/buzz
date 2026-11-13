//! Env Guardian Skill
//! 核心功能：检查环境变量安全性，管理依赖工具状态，监控服务健康状态，提供三态熔断保护

mod env_checker;
mod dependency_tool_checker;
mod service_health_probe;
mod circuit_breaker;

use env_checker::EnvironmentChecker;
use env_checker::EnvironmentConfig;
use dependency_tool_checker::DependencyToolChecker;
use dependency_tool_checker::ToolCheckConfig;
use service_health_probe::ServiceHealthProbe;
use service_health_probe::ProbeConfig;
use circuit_breaker::CircuitBreaker;
use circuit_breaker::CircuitBreakerConfig;

/// Env Guardian 技能主类
/// 整合多个子模块，实现完整的环境检查、依赖管理、服务监控和熔断保护流程
pub struct EnvGuardianSkill {
    /// 环境检查器
    pub environment_checker: EnvironmentChecker,
    /// 依赖工具检查器
    pub dependency_checker: DependencyToolChecker,
    /// 服务健康探针
    pub health_probe: ServiceHealthProbe,
    /// 熔断器
    pub circuit_breaker: CircuitBreaker,
    /// 技能配置
    pub config: SkillConfig,
}

/// 技能配置
pub struct SkillConfig {
    /// 是否启用环境检查
    pub enable_environment_check: bool,
    /// 是否启用依赖工具检查
    pub enable_dependency_check: bool,
    /// 是否启用服务健康检查
    pub enable_health_check: bool,
    /// 是否启用熔断保护
    pub enable_circuit_breaker: bool,
    /// 检查间隔
    pub check_interval_seconds: u64,
}

impl EnvGuardianSkill {
    /// 创建新的环境守护者技能
    pub fn new(config: SkillConfig) -> Self {
        let env_config = EnvironmentConfig {
            check_sensitive: true,
            check_required: true,
            check_security_policies: true,
            check_default_values: false,
            required_vars: vec!["PATH".to_string(), "HOME".to_string()],
            sensitive_prefixes: vec!["SECRET".to_string(), "PASSWORD".to_string()],
            security_rules: vec![],
        };
        
        let dependency_config = ToolCheckConfig {
            check_required_tools: true,
            check_optional_tools: true,
            check_version_compatibility: true,
            check_tool_status: true,
            auto_fix: false,
            required_tools: vec![],
            optional_tools: vec![],
        };
        
        let health_config = ProbeConfig {
            check_network_services: true,
            check_process_status: true,
            check_resource_usage: true,
            check_service_logs: false,
            check_custom_metrics: false,
            network_services: vec![],
            process_list: vec![],
            resource_thresholds: service_health_probe::ResourceThresholds {
                cpu_usage_threshold: 80.0,
                memory_usage_threshold: 90.0,
                disk_usage_threshold: 85.0,
                network_bandwidth_threshold: 1000.0,
            },
            check_interval_seconds: 60,
        };
        
        let circuit_config = CircuitBreakerConfig {
            enabled: true,
            half_open_enabled: true,
            failure_threshold: 5,
            recovery_timeout: 60000,
            half_open_max_requests: 3,
            log_enabled: false,
            auto_reset: true,
            state_change_callback: None,
        };
        
        Self {
            environment_checker: EnvironmentChecker::new(env_config),
            dependency_checker: DependencyToolChecker::new(dependency_config),
            health_probe: ServiceHealthProbe::new(health_config),
            circuit_breaker: CircuitBreaker::new(circuit_config),
            config,
        }
    }

    /// 执行完整的安全检查
    pub fn perform_security_check(&mut self) -> SecurityCheckResult {
        let mut result = SecurityCheckResult::new();
        
        if self.config.enable_environment_check {
            let env_result = self.environment_checker.check_environment();
            result.environment_result = env_result;
        }
        
        if self.config.enable_dependency_check {
            let dep_result = self.dependency_checker.check_all_tools();
            result.dependency_result = dep_result;
        }
        
        if self.config.enable_health_check {
            let health_result = self.health_probe.run_full_check();
            result.health_result = health_result;
        }
        
        // 综合所有结果
        result.passed = result.environment_result.passed &&
                         result.dependency_result.passed &&
                         result.health_result.passed;
        
        result
    }

    /// 执行熔断保护
    pub fn activate_circuit_breaker_protection(&mut self) -> CircuitBreakerResult {
        let mut result = CircuitBreakerResult::new();
        
        if !self.config.enable_circuit_breaker {
            result.message = "熔断保护已禁用".to_string();
            return result;
        }
        
        // 检查熔断器状态
        let current_state = self.circuit_breaker.get_state();
        
        match current_state {
            circuit_breaker::CircuitBreakerState::Closed => {
                result.message = "熔断器处于关闭状态，服务正常运行".to_string();
                result.passed = true;
            }
            circuit_breaker::CircuitBreakerState::Open => {
                result.message = "熔断器处于打开状态，服务已熔断".to_string();
                result.passed = false;
                result.circuit_state = Some(current_state);
            }
            circuit_breaker::CircuitBreakerState::HalfOpen => {
                result.message = "熔断器处于半开状态，正在尝试恢复".to_string();
                result.passed = false;
                result.circuit_state = Some(current_state);
            }
        }
        
        // 获取熔断器统计信息
        let stats = self.circuit_breaker.get_stats();
        result.stats = Some(stats);
        
        result
    }

    /// 重置熔断器
    pub fn reset_circuit_breaker(&mut self) {
        self.circuit_breaker.reset();
    }

    /// 获取总体状态
    pub fn get_overall_status(&self) -> OverallStatus {
        let env_status = if self.config.enable_environment_check {
            self.environment_checker.check_environment().passed
        } else {
            true
        };
        
        let dep_status = if self.config.enable_dependency_check {
            self.dependency_checker.check_all_tools().passed
        } else {
            true
        };
        
        let health_status = if self.config.enable_health_check {
            self.health_probe.run_full_check().passed
        } else {
            true
        };
        
        let circuit_status = self.circuit_breaker.is_healthy();
        
        OverallStatus {
            environment_healthy: env_status,
            dependency_healthy: dep_status,
            health_healthy: health_status,
            circuit_healthy: circuit_status,
            overall_healthy: env_status && dep_status && health_status && circuit_status,
        }
    }
}

/// 安全检查结果
pub struct SecurityCheckResult {
    /// 环境检查结果
    pub environment_result: env_checker::EnvironmentCheckResult,
    /// 依赖检查结果
    pub dependency_result: dependency_tool_checker::ToolCheckResults,
    /// 健康检查结果
    pub health_result: service_health_probe::HealthCheckResult,
    /// 是否通过检查
    pub passed: bool,
}

impl SecurityCheckResult {
    /// 创建新的安全检查结果
    pub fn new() -> Self {
        Self {
            environment_result: env_checker::EnvironmentCheckResult {
                passed: false,
                required_vars_result: env_checker::RequiredVarsResult {
                    passed: false,
                    passed_count: 0,
                    failed_count: 0,
                    details: Vec::new(),
                },
                sensitive_vars_result: env_checker::SensitiveVarsResult {
                    passed: false,
                    sensitive_count: 0,
                    details: Vec::new(),
                },
                security_policies_result: env_checker::SecurityPoliciesResult {
                    passed: false,
                    passed_count: 0,
                    failed_count: 0,
                    details: Vec::new(),
                },
                env_vars_info: Vec::new(),
            },
            dependency_result: dependency_tool_checker::ToolCheckResults::new(),
            health_result: service_health_probe::HealthCheckResult::new(),
            passed: false,
        }
    }
}
