//! env-guardian 技能库入口
//! 
//! 环境变量检查与依赖工具管理技能 - 确保系统环境安全完整，依赖工具正常运行
//! 
//! 模块结构：
//! - `env_checker`: 环境变数检查器
//! - `dependency_tool_checker`: 依赖工具检查器
//! - `service_health_probe`: 服务健康探针
//! - `circuit_breaker`: 三态熔断器
//! - `EnvGuardianSkill`: 技能主入口点

pub mod env_checker;
pub mod dependency_tool_checker;
pub mod service_health_probe;
pub mod circuit_breaker;

use std::sync::Arc;
use tokio::sync::RwLock;
use buzz_agent::skill::{Skill, SkillContext, SkillResult, SkillMetadata};
use buzz_core::AgentEvent;

/// env-guardian 技能主入口
pub struct EnvGuardianSkill {
    /// 环境变数检查器
    env_checker: Arc<RwLock<env_checker::EnvChecker>>,
    /// 依赖工具检查器
    tool_checker: Arc<RwLock<dependency_tool_checker::DependencyToolChecker>>,
    /// 服务健康探针
    health_probe: Arc<RwLock<service_health_probe::ServiceHealthProbe>>,
    /// 熔断器管理
    circuit_breakers: Arc<RwLock<std::collections::HashMap<String, circuit_breaker::CircuitBreaker>>>,
}

impl EnvGuardianSkill {
    /// 创建新的 env-guardian 技能实例
    pub fn new() -> Self {
        Self {
            env_checker: Arc::new(RwLock::new(env_checker::EnvChecker::default())),
            tool_checker: Arc::new(RwLock::new(dependency_tool_checker::DependencyToolChecker::default())),
            health_probe: Arc::new(RwLock::new(service_health_probe::ServiceHealthProbe::default())),
            circuit_breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// 执行完整环境检查
    pub async fn run_full_check(&self) -> SkillResult {
        let mut results = Vec::new();
        
        // 1. 环境变数检查
        let env_report = {
            let checker = self.env_checker.read().await;
            checker.check_all().await
        };
        results.push(serde_json::json!({
            "check_type": "environment_variables",
            "passed": env_report.all_passed,
            "details": env_report.results
        }));
        
        // 2. 依赖工具检查
        let tool_report = {
            let checker = self.tool_checker.read().await;
            checker.check_all().await
        };
        results.push(serde_json::json!({
            "check_type": "dependency_tools",
            "passed": tool_report.all_passed,
            "details": tool_report.results
        }));
        
        // 3. 服务健康探针
        let health_report = {
            let probe = self.health_probe.read().await;
            probe.check_all().await
        };
        results.push(serde_json::json!({
            "check_type": "service_health",
            "passed": health_report.all_healthy,
            "details": health_report.results
        }));
        
        // 4. 熔断器状态
        let cb_status = {
            let cbs = self.circuit_breakers.read().await;
            cbs.iter().map(|(name, cb)| {
                serde_json::json!({
                    "name": name,
                    "state": format!("{:?}", cb.state),
                    "stats": {
                        "total_requests": cb.stats.total_requests,
                        "successful": cb.stats.successful_requests,
                        "failed": cb.stats.failed_requests,
                        "consecutive_failures": cb.stats.consecutive_failures,
                    }
                })
            }).collect::<Vec<_>>()
        };
        results.push(serde_json::json!({
            "check_type": "circuit_breakers",
            "details": cb_status
        }));
        
        let all_passed = results.iter().all(|r| r.get("passed").map(|v| v.as_bool().unwrap_or(true)).unwrap_or(true));
        
        SkillResult::success(serde_json::json!({
            "summary": {
                "all_passed": all_passed,
                "checks_count": results.len(),
            },
            "results": results
        }))
    }
    
    /// 获取或创建熔断器
    pub async fn get_or_create_circuit_breaker(&self, name: &str, config: circuit_breaker::CircuitBreakerConfig) -> circuit_breaker::CircuitBreaker {
        let mut cbs = self.circuit_breakers.write().await;
        if let Some(cb) = cbs.get(name) {
            cb.clone()
        } else {
            let cb = circuit_breaker::CircuitBreaker::new(config);
            cbs.insert(name.to_string(), cb.clone());
            cb
        }
    }
}

#[async_trait::async_trait]
impl Skill for EnvGuardianSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "env-guardian".to_string(),
            version: "0.1.0".to_string(),
            description: "环境变量检查与依赖工具管理技能 - 确保系统环境安全完整，依赖工具正常运行".to_string(),
            author: "Pollen".to_string(),
            tags: vec!["environment", "validation", "health-check", "circuit-breaker".to_string()],
            required_permissions: vec!["system.env.read".to_string(), "system.process.execute".to_string()],
        }
    }
    
    async fn initialize(&mut self, _context: &SkillContext) -> SkillResult {
        SkillResult::success(serde_json::json!({
            "message": "env-guardian skill initialized"
        }))
    }
    
    async fn handle_event(&mut self, event: &AgentEvent, _context: &SkillContext) -> SkillResult {
        match event.kind {
            30300 => {
                // 技能调用事件
                if let Some(payload) = event.content.get("payload") {
                    if let Some(action) = payload.get("action").and_then(|v| v.as_str()) {
                        match action {
                            "full_check" => self.run_full_check().await,
                            "env_check" => {
                                let checker = self.env_checker.read().await;
                                let report = checker.check_all().await;
                                SkillResult::success(serde_json::to_value(report).unwrap())
                            }
                            "tool_check" => {
                                let checker = self.tool_checker.read().await;
                                let report = checker.check_all().await;
                                SkillResult::success(serde_json::to_value(report).unwrap())
                            }
                            "health_check" => {
                                let probe = self.health_probe.read().await;
                                let report = probe.check_all().await;
                                SkillResult::success(serde_json::to_value(report).unwrap())
                            }
                            "circuit_breaker_status" => {
                                let cbs = self.circuit_breakers.read().await;
                                let status: Vec<_> = cbs.iter().map(|(name, cb)| {
                                    serde_json::json!({
                                        "name": name,
                                        "state": format!("{:?}", cb.state),
                                    })
                                }).collect();
                                SkillResult::success(serde_json::json!({ "circuit_breakers": status }))
                            }
                            _ => SkillResult::error(format!("Unknown action: {}", action)),
                        }
                    } else {
                        SkillResult::error("Missing action in payload")
                    }
                } else {
                    SkillResult::error("Missing payload in event")
                }
            }
            _ => SkillResult::ignored(),
        }
    }
    
    async fn shutdown(&mut self, _context: &SkillContext) -> SkillResult {
        SkillResult::success(serde_json::json!({
            "message": "env-guardian skill shutdown"
        }))
    }
}

impl Default for EnvGuardianSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_env_guardian_skill_creation() {
        let skill = EnvGuardianSkill::new();
        let metadata = skill.metadata();
        assert_eq!(metadata.name, "env-guardian");
        assert_eq!(metadata.version, "0.1.0");
    }
    
    #[tokio::test]
    async fn test_full_c
