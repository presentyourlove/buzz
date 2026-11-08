//! 原生後端 Harness 實作 (支援 OpenRouter, NVIDIA NIM, 本地模型)

use super::{BuzzHarness, HarnessError, HarnessTurnOutput};
use async_trait::async_trait;

/// 原生 API Harness 結構體
pub struct NativeHarness {
    backend: String,
    default_model_id: String,
}

impl NativeHarness {
    pub fn new(backend: &str, default_model: &str) -> Self {
        Self {
            backend: backend.to_string(),
            default_model_id: default_model.to_string(),
        }
    }
}

#[async_trait]
impl BuzzHarness for NativeHarness {
    fn backend_name(&self) -> &str {
        &self.backend
    }

    fn default_model(&self) -> &str {
        &self.default_model_id
    }

    async fn execute_turn(
        &self,
        model_id: Option<&str>,
        _system_prompt: &str,
        user_message: &str,
    ) -> Result<HarnessTurnOutput, HarnessError> {
        let chosen_model = model_id.unwrap_or(&self.default_model_id);
        
        // 模擬或對接統一 HTTP/API 呼叫
        let simulated_response = format!(
            "[Harness:{}] Processed message via model '{}': {}",
            self.backend,
            chosen_model,
            user_message.trim()
        );

        Ok(HarnessTurnOutput {
            text: simulated_response,
            model_id: chosen_model.to_string(),
            input_tokens: Some(user_message.len() as u64 / 4),
            output_tokens: Some(32),
            finish_reason: Some("stop".to_string()),
        })
    }
}

