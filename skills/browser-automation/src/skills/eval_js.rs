//! EvalJS Skill
//! 
//! Evaluate JavaScript in the browser context with capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info};

use crate::capability::{BrowserAction, BrowserCapability};

/// EvalJS skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalJSConfig {
    /// JavaScript code to evaluate
    pub script: String,
    /// Arguments to pass to the script
    #[serde(default)]
    pub args: Vec<Value>,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Return value as JSON (vs raw)
    #[serde(default = "default_json")]
    pub return_json: bool,
    /// Evaluate in isolated world (extension context)
    #[serde(default)]
    pub isolated_world: bool,
}

fn default_timeout() -> u64 {
    10000
}

fn default_json() -> bool {
    true
}

/// EvalJS skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalJSResult {
    /// Whether evaluation was successful
    pub success: bool,
    /// Return value
    pub value: Option<Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
    /// Console output during evaluation
    pub console_logs: Vec<ConsoleLog>,
}

/// Console log entry
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
        let mut console_logs = Vec::new();
        let console_handler = page.on_console(move |msg| {
            console_logs.push(ConsoleLog {
                level: format!("{:?}", msg.type_()).to_lowercase(),
                text: msg.text(),
                timestamp: chrono::Utc::now(),
            });
        });
        
        // Evaluate script
        let result = if config.args.is_empty() {
            if config.return_json {
                page.evaluate(&config.script).await
            } else {
                page.evaluate_handle(&config.script).await.map(|_| Value::Null)
            }
        } else {
            // For args, we need to use evaluate with serialization
            let args_json = serde_json::to_string(&config.args)?;
            let script_with_args = format!(
                "(() => {{ const args = {}; return ({})(...args); }})()",
                args_json, config.script
            );
            if config.return_json {
                page.evaluate(&script_with_args).await
            } else {
                page.evaluate_handle(&script_with_args).await.map(|_| Value::Null)
            }
        };
        
        // Drop console handler
        drop(console_handler);
        
        let duration = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(value) => {
                debug!("JavaScript evaluation completed in {}ms", duration);
                Ok(EvalJSResult {
                    success: true,
                    value: Some(value),
                    error: None,
                    duration_ms: duration,
                    console_logs,
                })
            }
            Err(e) => {
                debug!("JavaScript evaluation failed: {}", e);
                Ok(EvalJSResult {
                    success: false,
                    value: None,
                    error: Some(e.to_string()),
                    duration_ms: duration,
                    console_logs,
                })
            }
        }
    }
    
    /// Evaluate script and return a handle (for complex objects)
    pub async fn evaluate_handle(
        page: &Page,
        script: &str,
    ) -> Result<playwright::api::JSHandle> {
        page.evaluate_handle(script).await
            .context("Failed to evaluate handle")
    }
    
    /// Evaluate script in isolated world (for extension-like access)
    pub async fn evaluate_in_isolated_world(
        page: &Page,
        script: &str,
    ) -> Result<Value> {
        page.evaluate_in_isolated_world(script).await
            .context("Failed to evaluate in isolated world")
    }
    
    /// Expose a function to the page
    pub async fn expose_function(
        page: &Page,
        name: &str,
        func: impl Fn(Vec<Value>) -> Result<Value> + Send + Sync + 'static,
    ) -> Result<()> {
        page.expose_function(name, func).await
            .context("Failed to expose function")
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
            let result: bool = page.evaluate(condition).await?;
            if result {
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
        page.evaluate("JSON.stringify(performance.getEntriesByType('navigation')[0])").await
            .context("Failed to get performance metrics")
    }
    
    /// Get all cookies as JSON
    pub async fn get_cookies_json(page: &Page) -> Result<Value> {
        page.context().cookies().await
            .context("Failed to get cookies")
    }
    
    /// Set localStorage item
    pub async fn set_local_storage(page: &Page, key: &str, value: &str) -> Result<()> {
        page.evaluate(&format!(
            "localStorage.setItem('{}', '{}')",
            key.replace('\'', "\'"),
            value.replace('\'', "\'")
        )).await?;
        Ok(())
    }
    
    /// Get localStorage item
    pub async fn get_local_storage(page: &Page, key: &str) -> Result<Option<String>> {
        let result: Option<String> = page.evaluate(&format!(
            "localStorage.getItem('{}')",
            key.replace('\'', "\'")
        )).await?;
        Ok(result)
    }
    
    /// Clear localStorage
    pub async fn clear_local_storage(page: &Page) -> Result<()> {
        page.evaluate("localStorage.clear()").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
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
