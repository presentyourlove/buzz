//! BuzzHarness 抽象化與多模型統一運行時 (Open Interpreter 架構落地)
//!
//! 提供模型無關 (Model-Agnostic) 的特工執行環境 Trait，
//! 支援 OpenAI, Anthropic, DeepSeek, Gemini, NVIDIA NIM 及本地 GGUF 後端動態熱插拔。

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// 特工回合輸出結果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessTurnOutput {
    pub text: String,
    pub model_id: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub finish_reason: Option<String>,
}

/// Harness 執行錯誤類型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HarnessError {
    ModelNotFound(String),
    NetworkError(String),
    RateLimitExceeded(String),
    AuthenticationFailed(String),
    Internal(String),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelNotFound(m) => write!(f, "Model not found: {}", m),
            Self::NetworkError(e) => write!(f, "Network error: {}", e),
            Self::RateLimitExceeded(e) => write!(f, "Rate limit exceeded: {}", e),
            Self::AuthenticationFailed(e) => write!(f, "Authentication failed: {}", e),
            Self::Internal(e) => write!(f, "Internal harness error: {}", e),
        }
    }
}

impl std::error::Error for HarnessError {}

/// 多模型統一 Harness Trait
#[async_trait::async_trait]
pub trait BuzzHarness: Send + Sync {
    /// 獲取該 Harness 所屬後端標識 (如 "openrouter", "nvidia_nim", "gemini_thought", "local_gguf")
    fn backend_name(&self) -> &str;

    /// 獲取預設模型識別碼
    fn default_model(&self) -> &str;

    /// 執行一輪特工推理與指令生成
    async fn execute_turn(
        &self,
        model_id: Option<&str>,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<HarnessTurnOutput, HarnessError>;
}

/// Harness 動態調度與熱插拔註冊表
#[derive(Default)]
pub struct HarnessRegistry {
    backends: HashMap<String, Arc<dyn BuzzHarness>>,
    default_backend: Option<String>,
}

impl HarnessRegistry {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: None,
        }
    }

    /// 註冊新 Harness 後端
    pub fn register(&mut self, harness: Arc<dyn BuzzHarness>) {
        let name = harness.backend_name().to_string();
        if self.default_backend.is_none() {
            self.default_backend = Some(name.clone());
        }
        self.backends.insert(name, harness);
    }

    /// 設定預設後端
    pub fn set_default_backend(&mut self, name: &str) -> Result<(), HarnessError> {
        if self.backends.contains_key(name) {
            self.default_backend = Some(name.to_string());
            Ok(())
        } else {
            Err(HarnessError::ModelNotFound(format!("Backend '{}' not registered", name)))
        }
    }

    /// 獲取指定或預設後端
    pub fn get(&self, name: Option<&str>) -> Option<Arc<dyn BuzzHarness>> {
        match name {
            Some(n) => self.backends.get(n).cloned(),
            None => self.default_backend.as_ref().and_then(|d| self.backends.get(d).cloned()),
        }
    }

    /// 獲取所有已註冊後端名稱
    pub fn list_backends(&self) -> Vec<String> {
        let mut list: Vec<String> = self.backends.keys().cloned().collect();
        list.sort();
        list
    }
}

pub mod native;

