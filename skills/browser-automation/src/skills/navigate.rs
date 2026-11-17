//! Navigate Skill
//! 
//! Navigate to URLs with capability validation and stealth support.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::browser_pool::IsolatedContext;
use crate::capability::{BrowserAction, BrowserCapability};

/// Navigate skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateConfig {
    /// Target URL
    pub url: String,
    /// Wait until condition
    #[serde(default = "default_wait_until")]
    pub wait_until: WaitUntil,
    /// Navigation timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Referrer URL
    pub referrer: Option<String>,
    /// Extra HTTP headers
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_wait_until() -> WaitUntil {
    WaitUntil::NetworkIdle
}

fn default_timeout() -> u64 {
    30000
}

/// Wait until condition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitUntil {
    Load,
    DomContentLoaded,
    NetworkIdle,
    Commit,
}

impl WaitUntil {
    pub fn as_str(&self) -> &'static str {
        match self {
            WaitUntil::Load => "load",
            WaitUntil::DomContentLoaded => "domcontentloaded",
            WaitUntil::NetworkIdle => "networkidle",
            WaitUntil::Commit => "commit",
        }
    }
}

/// Navigate skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateResult {
    /// Final URL after redirects
    pub final_url: String,
    /// HTTP status code
    pub status: Option<u16>,
    /// Page title
    pub title: Option<String>,
    /// Load time in milliseconds
    pub load_time_ms: u64,
    /// Whether navigation was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Navigate skill
pub struct NavigateSkill;

impl NavigateSkill {
    /// Execute navigation
    pub async fn execute(
        context: &IsolatedContext,
        config: NavigateConfig,
    ) -> Result<NavigateResult> {
        let start = std::time::Instant::now();
        
        // Validate capability
        context.capability().validate_navigation(&config.url)?;
        
        if !context.capability().allows_action(BrowserAction::Navigate) {
            return Ok(NavigateResult {
                final_url: config.url.clone(),
                status: None,
                title: None,
                load_time_ms: start.elapsed().as_millis() as u64,
                success: false,
                error: Some("Navigate action not permitted by capability".to_string()),
            });
        }
        
        info!("Navigating to: {}", config.url);
        
        // Create page and navigate
        let page = context.navigate(&config.url).await?;
        
        // Set extra headers if provided
        if !config.headers.is_empty() {
            page.set_extra_http_headers(config.headers).await?;
        }
        
        // Wait for navigation
        let response = page
            .goto(&config.url)
            .await
            .with_context(|| format!("Failed to navigate to {}", config.url))?;
        
        // Wait for the specified condition
        page.wait_for_load_state(config.wait_until.as_str())
            .await
            .with_context(|| format!("Timeout waiting for {}", config.wait_until.as_str()))?;
        
        let load_time = start.elapsed().as_millis() as u64;
        let final_url = page.url();
        let status = response.status();
        let title = page.title().await.ok();
        
        debug!("Navigation completed: {} ({}) in {}ms", final_url, status, load_time);
        
        Ok(NavigateResult {
            final_url,
            status: Some(status),
            title,
            load_time_ms: load_time,
            success: true,
            error: None,
        })
    }
    
    /// Execute with a pre-created page
    pub async fn execute_with_page(
        page: &Page,
        config: NavigateConfig,
    ) -> Result<NavigateResult> {
        let start = std::time::Instant::now();
        
        let response = page
            .goto(&config.url)
            .await
            .with_context(|| format!("Failed to navigate to {}", config.url))?;
        
        page.wait_for_load_state(config.wait_until.as_str())
            .await
            .with_context(|| format!("Timeout waiting for {}", config.wait_until.as_str()))?;
        
        let load_time = start.elapsed().as_millis() as u64;
        let final_url = page.url();
        let status = response.status();
        let title = page.title().await.ok();
        
        Ok(NavigateResult {
            final_url,
            status: Some(status),
            title,
            load_time_ms: load_time,
            success: true,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_navigate_config_defaults() {
        let config = NavigateConfig {
            url: "https://example.com".to_string(),
            wait_until: WaitUntil::NetworkIdle,
            timeout_ms: 30000,
            referrer: None,
            headers: std::collections::HashMap::new(),
        };
        
        assert_eq!(config.wait_until, WaitUntil::NetworkIdle);
        assert_eq!(config.timeout_ms, 30000);
    }
}
