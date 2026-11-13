use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// 服务健康探针
/// 核心功能：检查关键服务的运行状态、健康指标和可用性，确保系统服务正常运行
pub struct ServiceHealthProbe {
    /// 检查配置
    pub config: ProbeConfig,
    /// 服务状态缓存
    pub service_status_cache: HashMap<String, ServiceStatus>,
    /// 检查历史记录
    pub check_history: Vec<ProbeRecord>,
}

/// 探针配置
pub struct ProbeConfig {
    /// 是否检查网络服务
    pub check_network_services: bool,
    /// 是否检查进程状态
    pub check_process_status: bool,
    /// 是否检查资源使用情况
    pub check_resource_usage: bool,
    /// 是否检查服务日志
    pub check_service_logs: bool,
    /// 是否检查自定义指标
    pub check_custom_metrics: bool,
    /// 网络服务列表
    pub network_services: Vec<NetworkService>,
    /// 进程列表
    pub process_list: Vec<ProcessConfig>,
    /// 资源阈值
    pub resource_thresholds: ResourceThresholds,
    /// 检查间隔
    pub check_interval_seconds: u64,
}

/// 网络服务配置
pub struct NetworkService {
    /// 服务名称
    pub name: String,
    /// 服务主机
    pub host: String,
    /// 服务端口
    pub port: u16,
    /// 是否必需
    pub required: bool,
    /// 是否启用
    pub enabled: bool,
    /// 检查超时
    pub timeout_seconds: u64,
    /// 检查选项
    pub check_options: NetworkCheckOptions,
}

/// 网络检查选项
pub struct NetworkCheckOptions {
    /// 是否检查连接状态
    pub check_connectivity: bool,
    /// 是否检查响应时间
    pub check_response_time: bool,
    /// 是否检查服务版本
    pub check_service_version: bool,
    /// 是否检查服务状态
    pub check_service_status: bool,
}

/// 进程配置
pub struct ProcessConfig {
    /// 进程名称
    pub name: String,
    /// 是否必需
    pub required: bool,
    /// 是否启用
    pub enabled: bool,
    /// 检查选项
    pub check_options: ProcessCheckOptions,
}

/// 进程检查选项
pub struct ProcessCheckOptions {
    /// 是否检查进程是否存在
    pub check_exists: bool,
    /// 是否检查进程状态
    pub check_status: bool,
    /// 是否检查CPU使用率
    pub check_cpu_usage: bool,
    /// 是否检查内存使用率
    pub check_memory_usage: bool,
    /// 是否检查进程启动时间
    pub check_uptime: bool,
}

/// 资源阈值
pub struct ResourceThresholds {
    /// CPU使用率阈值 (0-100)
    pub cpu_usage_threshold: f64,
    /// 内存使用率阈值 (0-100)
    pub memory_usage_threshold: f64,
    /// 磁盘使用率阈值 (0-100)
    pub disk_usage_threshold: f64,
    /// 网络带宽阈值 (Mbps)
    pub network_bandwidth_threshold: f64,
}

/// 服务状态
pub struct ServiceStatus {
    /// 服务名称
    pub name: String,
    /// 是否运行
    pub running: bool,
    /// 健康状态
    pub health_status: HealthStatus,
    /// 最后检查时间
    pub last_checked_at: chrono::DateTime<chrono::Utc>,
    /// 检查结果
    pub check_result: CheckResult,
}

/// 健康状态枚举
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 错误
    Error,
    /// 未知
    Unknown,
}

/// 检查结果
pub struct CheckResult {
    /// 检查是否通过
    pub passed: bool,
    /// 错误列表
    pub errors: Vec<String>,
    /// 警告列表
    pub warnings: Vec<String>,
    /// 信息列表
    pub infos: Vec<String>,
    /// 指标数据
    pub metrics: HashMap<String, f64>,
}

/// 探针记录
pub struct ProbeRecord {
    /// 记录ID
    pub id: String,
    /// 服务名称
    pub service_name: String,
    /// 检查时间
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// 检查结果
    pub result: CheckResult,
    /// 检查持续时间
    pub duration_ms: u64,
}

impl ServiceHealthProbe {
    /// 创建新的服务健康探针
    pub fn new(config: ProbeConfig) -> Self {
        Self {
            config,
            service_status_cache: HashMap::new(),
            check_history: Vec::new(),
        }
    }

    /// 运行完整检查
    pub fn run_full_check(&mut self) -> HealthCheckResult {
        let mut all_results = HealthCheckResult::new();
        
        // 检查网络服务
        if self.config.check_network_services {
            let network_results = self.check_network_services();
            all_results.network_results.extend(network_results);
        }
        
        // 检查进程状态
        if self.config.check_process_status {
            let process_results = self.check_processes();
            all_results.process_results.extend(process_results);
        }
        
        // 检查资源使用情况
        if self.config.check_resource_usage {
            let resource_results = self.check_resource_usage();
            all_results.resource_results = resource_results;
        }
        
        // 检查服务日志
        if self.config.check_service_logs {
            let log_results = self.check_service_logs();
            all_results.log_results = log_results;
        }
        
        // 检查自定义指标
        if self.config.check_custom_metrics {
            let custom_results = self.check_custom_metrics();
            all_results.custom_results = custom_results;
        }
        
        // 计算总体结果
        all_results.passed = all_results.network_results.iter().all(|r| r.check_result.passed) &&
                             all_results.process_results.iter().all(|r| r.check_result.passed);
        
        all_results
    }

    /// 检查网络服务
    fn check_network_services(&mut self) -> Vec<ServiceStatus> {
        let mut results = Vec::new();
        
        for service in &self.config.network_services {
            if !service.enabled {
                continue;
            }
            
            let status = self.check_network_service(service);
            results.push(status);
        }
        
        results
    }

    /// 检查单个网络服务
    fn check_network_service(&mut self, service: &NetworkService) -> ServiceStatus {
        let start_time = Instant::now();
        let mut check_result = CheckResult {
            passed: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
            metrics: HashMap::new(),
        };
        
        let mut health_status = HealthStatus::Unknown;
        
        // 检查连接状态
        if service.check_options.check_connectivity {
            let address = format!("{}:{}", service.host, service.port);
            match TcpStream::connect_timeout(&address.parse().unwrap(), Duration::from_secs(service.timeout_seconds)) {
                Ok(_) => {
                    check_result.infos.push(format!("服务 {} 连接成功", service.name));
                    if health_status == HealthStatus::Unknown {
                        health_status = HealthStatus::Healthy;
                    }
                }
                Err(e) => {
                    check_result.errors.push(format!("服务 {} 连接失败: {}", service.name, e));
                    check_result.passed = false;
                    if health_status == HealthStatus::Unknown {
                        health_status = HealthStatus::Error;
                    }
                }
            }
        }
        
        // 检查响应时间
        if service.check_options.check_response_time {
            let address = format!("{}:{}", service.host, service.port);
            let start = Instant::now();
            match TcpStream::connect(&address) {
                Ok(_) => {
                    let duration = start.elapsed().as_millis() as f64;
                    check_result.metrics.insert("response_time_ms".to_string(), duration);
                    
                    // 根据响应时间评估健康状态
                    if duration > service.timeout_seconds as f64 * 1000.0 {
                        check_result.warnings.push(format!("服务 {} 响应时间过长: {} ms", service.name, duration));
                        if health_status == HealthStatus::Healthy {
                            health_status = HealthStatus::Warning;
                        }
                    }
                }
                Err(e) => {
                    check_result.errors.push(format!("服务 {} 连接失败: {}", service.name, e));
                    check_result.passed = false;
                    if health_status == HealthStatus::Healthy {
                        health_status = HealthStatus::Error;
                    }
                }
            }
        }
        
        let status = ServiceStatus {
            name: service.name.clone(),
            running: check_result.passed,
            health_status,
            last_checked_at: chrono::Utc::now(),
            check_result,
        };
        
        // 缓存状态
        self.service_status_cache.insert(service.name.clone(), status.clone());
        status
    }
}
