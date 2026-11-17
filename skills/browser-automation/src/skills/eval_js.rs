//! EvalJS Skill
//! 
//! Executes JavaScript expressions and functions in page context with
//! result serialization, error handling, and console log capturing.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// EvalJS skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalJSConfig {
    /// JavaScript code or expression to evaluate
    pub script: String,
    /// Arguments to pass to the function (if script is a function)
    #[serde(default)]
    pub args: Vec<Value>,
    /// Execution timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Whether to serialize return value to JSON
    #[serde(default = "default_true")]
    pub return_json: bool,
    /// Run in isolated world (content script context)
    #[serde(default)]
    pub isolated_world: bool,
}

fn default_timeout() -> u64 {
    30000
}

fn default_true() -> bool {
    true
}

/// EvalJS skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalJSResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Serialized return value
    pub value: Option<Value>,
    /// Error message if execution failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Console logs captured during execution
    pub console_logs: Vec<ConsoleLog>,
}

/// Captured console log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleLog {
    pub level: String,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// EvalJS skill
pub struct EvalJSSkill;

impl EvalJSSkill {
    /// Execute JavaScript evaluation
    pub async fn execute(
        page: &Page,
        config: EvalJSConfig,
    ) -> Result<EvalJSResult> {
        let start = std::time::Instant::now();
        
        info!("Evaluating JavaScript: {} chars", config.script.len());
        
        // Capture console logs
        let console_logs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let logs_clone = console_logs.clone();
        let _ = page.on_console(move |msg| {
            let logs = logs_clone.clone();
            async move {
                if let Ok(mut lock) = logs.lock() {
                    lock.push(ConsoleLog {
                        level: msg.type_().to_string(),
                        text: msg.text().to_string(),
                        timestamp: chrono::Utc::now(),
                    });
                }
                Ok(())
            }
        }).await;
        
        // Evaluate script
        let result: Result<Value> = if config.args.is_empty() {
            page.evaluate(&config.script, None::<&()>).await.map_err(|e| anyhow::anyhow!(e))
        } else {
            let args_json = serde_json::to_string(&config.args)?;
            let script_with_args = format!(
                "(() => {{ const args = {}; return ({})(...args); }})()",
                args_json, config.script
            );
            page.evaluate(&script_with_args, None::<&()>).await.map_err(|e| anyhow::anyhow!(e))
        };
        
        let duration = start.elapsed().as_millis() as u64;
        let logs = console_logs.lock().map(|l| l.clone()).unwrap_or_default();
        
        match result {
            Ok(value) => {
                debug!("JavaScript evaluation completed in {}ms", duration);
                Ok(EvalJSResult {
                    success: true,
                    value: Some(value),
                    error: None,
                    duration_ms: duration,
                    console_logs: logs,
                })
            }
            Err(e) => {
                debug!("JavaScript evaluation failed: {}", e);
                Ok(EvalJSResult {
                    success: false,
                    value: None,
                    error: Some(e.to_string()),
                    duration_ms: duration,
                    console_logs: logs,
                })
            }
        }
    }
    
    /// Add a script to run on every page load
    pub async fn add_init_script(
        page: &Page,
        script: &str,
    ) -> Result<()> {
        page.add_init_script(script).await
            .context("Failed to add init script")
    }
    
    /// Execute a file as script
    pub async fn execute_file(
        page: &Page,
        path: &str,
        args: Vec<Value>,
    ) -> Result<EvalJSResult> {
        let script = tokio::fs::read_to_string(path).await
            .context("Failed to read script file")?;
        
        let config = EvalJSConfig {
            script,
            args,
            timeout_ms: 30000,
            return_json: true,
            isolated_world: false,
        };
        
        Self::execute(page, config).await
    }
    
    /// Wait for a JavaScript condition
    pub async fn wait_for_condition(
        page: &Page,
        condition: &str,
        timeout_ms: u64,
        poll_interval_ms: u64,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        
        loop {
            let result: Result<bool> = page.evaluate(condition, None::<&()>).await.map_err(|e| anyhow::anyhow!(e));
            if let Ok(true) = result {
                return Ok(true);
            }
            
            if start.elapsed() >= timeout {
                return Ok(false);
            }
            
            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
        }
    }
    
    /// Get performance metrics
    pub async fn get_performance_metrics(page: &Page) -> Result<Value> {
        page.evaluate("() => JSON.parse(JSON.stringify(performance.getEntriesByType('navigation')[0] || {}))", None::<&()>)
            .await
            .context("Failed to get performance metrics")
    }
    
    /// Get all cookies as JSON
    pub async fn get_cookies_json(page: &Page) -> Result<Value> {
        let cookies = page.context()?.cookies(None).await
            .context("Failed to get cookies")?;
        serde_json::to_value(cookies).context("Failed to serialize cookies")
    }
    
    /// Set localStorage item
    pub async fn set_local_storage(page: &Page, key: &str, value: &str) -> Result<()> {
        let script = format!(
            "localStorage.setItem('{}', '{}')",
            key.replace('\'', "\\'"),
            value.replace('\'', "\\'")
        );
        page.evaluate::<(), Option<()>>(&script, None).await?;
        Ok(())
    }
    
    /// Get localStorage item
    pub async fn get_local_storage(page: &Page, key: &str) -> Result<Option<String>> {
        let script = format!(
            "localStorage.getItem('{}')",
            key.replace('\'', "\\'")
        );
        let result: Option<String> = page.evaluate(&script, None::<&()>).await?;
        Ok(result)
    }
    
    /// Clear localStorage
    pub async fn clear_local_storage(page: &Page) -> Result<()> {
        page.evaluate::<(), Option<()>>("localStorage.clear()", None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_eval_js_config_defaults() {
        let config = EvalJSConfig {
            script: "1 + 1".to_string(),
            args: vec![],
            timeout_ms: 10000,
            return_json: true,
            isolated_world: false,
        };
        
        assert!(config.return_json);
        assert!(!config.isolated_world);
        assert_eq!(config.timeout_ms, 10000);
    }
}
