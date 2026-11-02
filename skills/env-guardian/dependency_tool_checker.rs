use std::process::Command;
use std::collections::HashMap;

/// 依赖工具检查器
/// 核心功能：检查关键依赖工具的版本、安装状态和可用性，确保系统依赖环境正常
pub struct DependencyToolChecker {
    /// 检查配置
    pub config: ToolCheckConfig,
    /// 工具状态缓存
    pub tool_status_cache: HashMap<String, ToolStatus>,
    /// 检查历史记录
    pub check_history: Vec<CheckRecord>,
}

/// 工具检查配置
pub struct ToolCheckConfig {
    /// 是否检查必需工具
    pub check_required_tools: bool,
    /// 是否检查可选工具
    pub check_optional_tools: bool,
    /// 是否检查版本兼容性
    pub check_version_compatibility: bool,
    /// 是否检查工具状态
    pub check_tool_status: bool,
    /// 是否自动修复
    pub auto_fix: bool,
    /// 必需工具列表
    pub required_tools: Vec<ToolConfig>,
    /// 可选工具列表
    pub optional_tools: Vec<ToolConfig>,
}

/// 工具配置
pub struct ToolConfig {
    /// 工具名称
    pub name: String,
    /// 期望的最小版本
    pub min_version: Option<String>,
    /// 期望的最大版本
    pub max_version: Option<String>,
    /// 是否必需
    pub required: bool,
    /// 版本约束
    pub version_constraint: Option<String>,
    /// 检查选项
    pub check_options: CheckOptions,
}

/// 检查选项
pub struct CheckOptions {
    /// 是否检查是否存在
    pub check_exists: bool,
    /// 是否检查版本
    pub check_version: bool,
    /// 是否检查可执行性
    pub check_executable: bool,
    /// 是否检查路径
    pub check_path: bool,
    /// 是否检查环境变量
    pub check_env_vars: bool,
}

/// 工具状态
pub struct ToolStatus {
    /// 工具名称
    pub name: String,
    /// 是否存在
    pub exists: bool,
    /// 版本信息
    pub version: Option<String>,
    /// 可执行性
    pub executable: bool,
    /// 路径
    pub path: Option<String>,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 状态
    pub status: ToolStatusEnum,
    /// 检查时间
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// 工具状态枚举
pub enum ToolStatusEnum {
    /// 正常
    Normal,
    /// 缺失
    Missing,
    /// 版本错误
    VersionError,
    /// 可执行性错误
    ExecutableError,
    /// 权限错误
    PermissionError,
    /// 未知错误
    UnknownError,
}

/// 检查记录
pub struct CheckRecord {
    /// 检查ID
    pub id: String,
    /// 工具名称
    pub tool_name: String,
    /// 检查时间
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// 检查结果
    pub result: CheckResult,
    /// 检查持续时间
    pub duration_ms: u64,
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
}

/// 工具检查结果
pub struct ToolCheckResult {
    /// 工具名称
    pub tool_name: String,
    /// 检查结果
    pub result: CheckResult,
    /// 工具状态
    pub tool_status: ToolStatus,
}

/// 工具检查结果集合
pub struct ToolCheckResults {
    /// 必需工具检查结果
    pub required_tools_results: Vec<ToolCheckResult>,
    /// 可选工具检查结果
    pub optional_tools_results: Vec<ToolCheckResult>,
    /// 是否通过检查
    pub passed: bool,
}

impl DependencyToolChecker {
    /// 创建新的依赖工具检查器
    pub fn new(config: ToolCheckConfig) -> Self {
        Self {
            config,
            tool_status_cache: HashMap::new(),
            check_history: Vec::new(),
        }
    }

    /// 检查所有工具
    pub fn check_all_tools(&mut self) -> ToolCheckResults {
        let mut all_results = ToolCheckResults::new();
        
        // 检查必需工具
        if self.config.check_required_tools {
            let required_results = self.check_required_tools();
            all_results.required_tools_results.extend(required_results);
        }
        
        // 检查可选工具
        if self.config.check_optional_tools {
            let optional_results = self.check_optional_tools();
            all_results.optional_tools_results.extend(optional_results);
        }
        
        // 计算总体结果
        all_results.passed = all_results.required_tools_results.iter().all(|r| r.result.passed) &&
                             all_results.optional_tools_results.iter().all(|r| r.result.passed);
        
        all_results
    }

    /// 检查必需工具
    fn check_required_tools(&mut self) -> Vec<ToolCheckResult> {
        let mut results = Vec::new();
        
        for tool_config in &self.config.required_tools {
            let result = self.check_single_tool(&tool_config);
            results.push(result);
        }
        
        results
    }

    /// 检查可选工具
    fn check_optional_tools(&mut self) -> Vec<ToolCheckResult> {
        let mut results = Vec::new();
        
        for tool_config in &self.config.optional_tools {
            let result = self.check_single_tool(&tool_config);
            results.push(result);
        }
        
        results
    }

    /// 检查单个工具
    fn check_single_tool(&mut self, tool_config: &ToolConfig) -> ToolCheckResult {
        let start_time = std::time::Instant::now();
        let tool_name = tool_config.name.clone();
        
        let mut result = CheckResult {
            passed: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
        };
        
        let mut tool_status = ToolStatus {
            name: tool_name.clone(),
            exists: false,
            version: None,
            executable: false,
            path: None,
            env_vars: HashMap::new(),
            status: ToolStatusEnum::Missing,
            checked_at: chrono::Utc::now(),
        };
        
        // 检查工具是否存在
        if tool_config.check_exists {
            match self.find_tool(&tool_name) {
                Some(path) => {
                    tool_status.exists = true;
                    tool_status.path = Some(path);
                    
                    // 检查可执行性
                    if tool_config.check_executable {
                        if self.is_executable(&path) {
                            tool_status.executable = true;
                        } else {
                            tool_status.status = ToolStatusEnum::ExecutableError;
                            result.errors.push(format!("工具 '{}' 不可执行", tool_name));
                            result.passed = false;
                        }
                    }
                    
                    // 检查版本
                    if tool_config.check_version || tool_config.min_version.is_some() || tool_config.max_version.is_some() {
                        if let Some(version) = self.get_tool_version(&tool_name) {
                            tool_status.version = Some(version.clone());
                            
                            // 检查版本兼容性
                            if let Err(e) = self.check_version_compatibility(&tool_config, &version) {
                                result.errors.push(e);
                                result.passed = false;
                            }
                        } else {
                            tool_status.status = ToolStatusEnum::VersionError;
                            result.errors.push(format!("无法获取工具 '{}' 的版本", tool_name));
                            result.passed = false;
                        }
                    }
                    
                    // 检查环境变量
                    if tool_config.check_env_vars {
                        let env_vars = self.get_tool_env_vars(&tool_name);
                        tool_status.env_vars = env_vars;
                    }
                }
                None => {
                    tool_status.status = ToolStatusEnum::Missing;
                    result.errors.push(format!("工具 '{}' 不存在", tool_name));
                    result.passed = false;
                }
            }
        }
        
        // 检查路径
        if tool_config.check_path {
            if let Some(path) = &tool_status.path {
                if !self.is_valid_path(path) {
                    result.warnings.push(format!("工具 '{}' 的路径无效: {}", tool_name, path));
                }
            }
        }
        
        // 缓存工具状态
        self.tool_status_cache.insert(tool_name.clone(), tool_status);
        
        // 记录检查历史
        let duration = start_time.elapsed().as_millis() as u64;
        self.check_history.push(CheckRecord {
            id: format!("{}_{}", tool_name, chrono::Utc::now().timestamp()),
            tool_name: tool_name.clone(),
       
