//! TStack: SkillCallable 抽象介面與標準資料結構
//! 借鑒 t3dotgg/tstack 理念，為特工技能提供標準化輸入輸出、上下文傳遞與契約

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// 技能執行錯誤類型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SkillError {
    /// 執行失敗並附帶具體錯誤訊息
    ExecutionFailed(String),
    /// 缺少必要前置輸入
    MissingInput(String),
    /// 執行超時
    Timeout(String),
    /// 前置依賴缺失或失敗
    DependencyMissing(String),
    /// 依賴拓撲中檢測到環狀死結
    CycleDetected(String),
    /// 內部系統異常
    Internal(String),
}

impl fmt::Display for SkillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillError::ExecutionFailed(msg) => write!(f, "Skill execution failed: {}", msg),
            SkillError::MissingInput(param) => write!(f, "Missing required skill input: {}", param),
            SkillError::Timeout(msg) => write!(f, "Skill execution timed out: {}", msg),
            SkillError::DependencyMissing(dep) => write!(f, "Skill dependency missing or failed: {}", dep),
            SkillError::CycleDetected(cycle) => write!(f, "Cycle detected in skill dependency graph: {}", cycle),
            SkillError::Internal(msg) => write!(f, "Internal skill error: {}", msg),
        }
    }
}

impl std::error::Error for SkillError {}

/// 技能標準輸入結構
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillInput {
    /// 鍵值參數映射
    pub params: HashMap<String, Value>,
    /// 文字內容/Payload (如正文、代碼、提示詞)
    pub payload: Option<String>,
}

impl SkillInput {
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
            payload: None,
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, val: impl Into<Value>) -> Self {
        self.params.insert(key.into(), val.into());
        self
    }

    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(|v| v.as_str())
    }

    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.params.get(key)
    }
}

impl Default for SkillInput {
    fn default() -> Self {
        Self::new()
    }
}

/// 技能標準輸出結構
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillOutput {
    /// 是否成功
    pub success: bool,
    /// 產出之結構化數據
    pub data: Value,
    /// 狀態訊息或摘要說明
    pub message: String,
    /// 執行耗時 (毫秒)
    pub execution_ms: u64,
}

impl SkillOutput {
    pub fn success(data: Value, msg: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: msg.into(),
            execution_ms: 0,
        }
    }

    pub fn failure(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: Value::Null,
            message: msg.into(),
            execution_ms: 0,
        }
    }

    pub fn with_latency(mut self, ms: u64) -> Self {
        self.execution_ms = ms;
        self
    }
}

/// 跨技能執行的共享上下文
#[derive(Debug, Clone)]
pub struct SkillContext {
    pub trace_id: String,
    pub caller_agent: String,
    pub shared_memory: HashMap<String, Value>,
    pub timeout_ms: u64,
}

impl SkillContext {
    pub fn new(caller: impl Into<String>) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            caller_agent: caller.into(),
            shared_memory: HashMap::new(),
            timeout_ms: 30_000,
        }
    }

    pub fn with_memory(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.shared_memory.insert(key.into(), value.into());
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// 核心模組化技能介面 (SkillCallable Trait)
pub trait SkillCallable: Send + Sync {
    /// 技能唯一識別碼 (如 "github_search", "markdown_summarize")
    fn id(&self) -> &str;

    /// 人類可讀名稱
    fn name(&self) -> &str;

    /// 技能功能描述
    fn description(&self) -> &str;

    /// 技能分類標籤
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }

    /// 必要前置參數列表
    fn required_inputs(&self) -> &[String] {
        &[]
    }

    /// 核心執行方法
    fn execute(&self, input: &SkillInput, ctx: &SkillContext) -> Result<SkillOutput, SkillError>;
}

/// 支援閉包的輕量級 Callable 實作
pub struct ClosureSkill {
    id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    required_inputs: Vec<String>,
    handler: Arc<dyn Fn(&SkillInput, &SkillContext) -> Result<SkillOutput, SkillError> + Send + Sync>,
}

impl ClosureSkill {
    pub fn new<F>(id: impl Into<String>, name: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&SkillInput, &SkillContext) -> Result<SkillOutput, SkillError> + Send + Sync + 'static,
    {
        let id_str = id.into();
        let name_str = name.into();
        Self {
            id: id_str.clone(),
            name: name_str,
            description: format!("Closure-based skill for {}", id_str),
            tags: Vec::new(),
            required_inputs: Vec::new(),
            handler: Arc::new(handler),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_required_inputs(mut self, req: Vec<String>) -> Self {
        self.required_inputs = req;
        self
    }
}

impl SkillCallable for ClosureSkill {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tags(&self) -> Vec<String> {
        self.tags.clone()
    }

    fn required_inputs(&self) -> &[String] {
        &self.required_inputs
    }

    fn execute(&self, input: &SkillInput, ctx: &SkillContext) -> Result<SkillOutput, SkillError> {
        // 驗證必要輸入
        for req in &self.required_inputs {
            if !input.params.contains_key(req) && input.payload.is_none() {
                return Err(SkillError::MissingInput(req.clone()));
            }
        }
        (self.handler)(input, ctx)
    }
}

