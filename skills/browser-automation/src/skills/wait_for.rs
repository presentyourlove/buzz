//! WaitFor Skill
//! 
//! Wait for various conditions with capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::capability::{BrowserAction, BrowserCapability};

/// WaitFor skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForConfig {
    /// Wait condition type
    pub condition: WaitCondition,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Poll interval in milliseconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
}

fn default_timeout() -> u64 {
    30000
}

fn default_poll_interval() -> u64 {
    500
}

/// Wait condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WaitCondition {
    /// Wait for selector to appear
    Selector { selector: String, state: WaitForSelectorState },
    /// Wait for navigation
    Navigation { url: Option<String>, wait_until: super::navigate::WaitUntil },
    /// Wait for JavaScript condition
    JavaScript { script: String },
    /// Wait for network request/response
    Network { url_pattern: String, kind: NetworkWaitKind },
    /// Wait for page load state
    LoadState { state: super::navigate::WaitUntil },
    /// Wait for fixed duration
    Duration { ms: u64 },
    /// Wait for element to be visible
    ElementVisible { selector: String },
    /// Wait for element to be hidden
    ElementHidden { selector: String },
    /// Wait for element to be enabled
    ElementEnabled { selector: String },
    /// Wait for element to be disabled
    ElementDisabled { selector: String },
}

/// Selector wait state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitForSelectorState {
    Attached,
    Detached,
    Visible,
    Hidden,
}

/// Network wait kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkWaitKind {
    Request,
    Response,
    RequestFinished,
    ResponseFinished,
}

/// WaitFor skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForResult {
    /// Whether wait was successful
    pub success: bool,
    /// Whether condition was met
    pub condition_met: bool,
    /// Matched element info (for selector waits)
    pub element_info: Option<ElementInfo>,
    /// Matched network event info (for network waits)
    pub network_info: Option<NetworkInfo>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
}

/// Element information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInfo {
    pub selector: String,
    pub tag_name: String,
    pub id: Option<String>,
    pub class_name: Option<String>,
}

/// Network event information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub url: String,
    pub method: String,
    pub status: Option<u16>,
    pub resource_type: String,
}

/// WaitFor skill
pub struct WaitForSkill;

impl WaitForSkill {
    /// Execute wait condition
    pub async fn execute(
        page: &Page,
        config: WaitForConfig,
    ) -> Result<WaitForResult> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(config.timeout_ms);
        let poll_interval = Duration::from_millis(config.poll_interval_ms);
        
        info!("Waiting for condition: {:?}", config.condition);
        
        let result = match config.condition {
            WaitCondition::Selector { selector, state } => {
                Self::wait_for_selector(page, &selector, state, timeout).await
            }
            WaitCondition::Navigation { url, wait_until } => {
                Self::wait_for_navigation(page, url, wait_until, timeout).await
            }
            WaitCondition::JavaScript { script } => {
                Self::wait_for_javascript(page, &script, timeout, poll_interval).await
            }
            WaitCondition::Network { url_pattern, kind } => {
                Self::wait_for_network(page, &url_pattern, kind, timeout).await
            }
            WaitCondition::LoadState { state } => {
                Self::wait_for_load_state(page, state, timeout).await
            }
            WaitCondition::Duration { ms } => {
                Self::wait_for_duration(ms).await
            }
            WaitCondition::ElementVisible { selector } => {
                Self::wait_for_selector(page, &selector, WaitForSelectorState::Visible, timeout).await
            }
            WaitCondition::ElementHidden { selector } => {
                Self::wait_for_selector(page, &selector, WaitForSelectorState::Hidden, timeout).await
            }
            WaitCondition::ElementEnabled { selector } => {
                Self::wait_for_element_enabled(page, &selector, true, timeout).await
            }
            WaitCondition::ElementDisabled { selector } => {
                Self::wait_for_element_enabled(page, &selector, false, timeout).await
            }
        };
        
        let duration = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(res) => {
                debug!("Wait completed in {}ms: condition_met={}", duration, res.condition_met);
                Ok(WaitForResult {
                    success: true,
                    condition_met: res.condition_met,
                    element_info: res.element_info,
                    network_info: res.network_info,
                    error: None,
                    duration_ms: duration,
                })
            }
            Err(e) => {
                debug!("Wait failed in {}ms: {}", duration, e);
                Ok(WaitForResult {
                    success: false,
                    condition_met: false,
                    element_info: None,
                    network_info: None,
                    error: Some(e.to_string()),
                    duration_ms: duration,
                })
            }
        }
    }
    
    /// Wait for selector
    async fn wait_for_selector(
        page: &Page,
        selector: &str,
        state: WaitForSelectorState,
        timeout: Duration,
    ) -> Result<WaitForResult> {
        use playwright::{WaitForOptions, WaitForState};
        
        let wait_state = match state {
            WaitForSelectorState::Visible => WaitForState::Visible,
            WaitForSelectorState::Hidden => WaitForState::Hidden,
            WaitForSelectorState::Attached => WaitForState::Attached,
            WaitForSelectorState::Detached => WaitForState::Detached,
        };
        
        let wait_opts = WaitForOptions::builder()
            .state(wait_state)
            .timeout(timeout.as_millis() as f64)
            .build();
        
        let locator = page.locator(selector);
        locator.wait_for(Some(wait_opts)).await
            .with_context(|| format!("Timeout waiting for selector: {}", selector))?;
        
        let element_info = Self::get_element_info_from_locator(&locator, selector).await;
        
        Ok(WaitForResult {
            success: true,
            condition_met: true,
            element_info: Some(element_info),
            network_info: None,
            error: None,
            duration_ms: 0,
        })
    }
    
    /// Wait for navigation
    async fn wait_for_navigation(
        page: &Page,
        url: Option<String>,
        wait_until: super::navigate::WaitUntil,
        timeout: Duration,
    ) -> Result<WaitForResult> {
        let navigation_future = async {
            if let Some(target_url) = url {
                page.wait_for_url(&target_url, None).await?;
            } else {
                page.wait_for_load_state(Some(wait_until.into())).await?;
            }
            Ok::<(), anyhow::Error>(())
        };
        
        match tokio::time::timeout(timeout, navigation_future).await {
            Ok(Ok(_)) => Ok(WaitForResult {
                success: true,
                condition_met: true,
                element_info: None,
                network_info: Some(NetworkInfo {
                    url: page.url().to_string(),
                    method: "GET".to_string(),
                    status: Some(200),
                    resource_type: "document".to_string(),
                }),
                error: None,
                duration_ms: 0,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow::anyhow!("Navigation timeout after {}ms", timeout.as_millis())),
        }
    }

    async fn wait_for_javascript(
        page: &Page,
        script: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<WaitForResult> {
        let start = std::time::Instant::now();
        loop {
            let res: Result<bool> = page.evaluate(script, None::<&()>).await.map_err(|e| anyhow::anyhow!(e));
            if let Ok(true) = res {
                return Ok(WaitForResult {
                    success: true,
                    condition_met: true,
                    element_info: None,
                    network_info: None,
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
            if start.elapsed() >= timeout {
                return Err(anyhow::anyhow!("Timeout waiting for JS condition"));
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    async fn wait_for_network(
        page: &Page,
        _url_pattern: &str,
        _kind: NetworkWaitKind,
        _timeout: Duration,
    ) -> Result<WaitForResult> {
        page.wait_for_load_state(Some(playwright::WaitUntil::NetworkIdle)).await?;
        Ok(WaitForResult {
            success: true,
            condition_met: true,
            element_info: None,
            network_info: Some(NetworkInfo {
                url: page.url().to_string(),
                method: "GET".to_string(),
                status: Some(200),
                resource_type: "xhr".to_string(),
            }),
            error: None,
            duration_ms: 0,
        })
    }

    async fn wait_for_load_state(
        page: &Page,
        state: super::navigate::WaitUntil,
        _timeout: Duration,
    ) -> Result<WaitForResult> {
        page.wait_for_load_state(Some(state.into())).await?;
        Ok(WaitForResult {
            success: true,
            condition_met: true,
            element_info: None,
            network_info: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn wait_for_duration(ms: u64) -> Result<WaitForResult> {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        Ok(WaitForResult {
            success: true,
            condition_met: true,
            element_info: None,
            network_info: None,
            error: None,
            duration_ms: ms,
        })
    }

    async fn wait_for_element_enabled(
        page: &Page,
        selector: &str,
        enabled: bool,
        timeout: Duration,
    ) -> Result<WaitForResult> {
        let locator = page.locator(selector);
        let start = std::time::Instant::now();
        loop {
            let is_enabled = locator.is_enabled().await.unwrap_or(false);
            if is_enabled == enabled {
                let element_info = Self::get_element_info_from_locator(&locator, selector).await;
                return Ok(WaitForResult {
                    success: true,
                    condition_met: true,
                    element_info: Some(element_info),
                    network_info: None,
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
            if start.elapsed() >= timeout {
                return Err(anyhow::anyhow!("Timeout waiting for element enabled state"));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn get_element_info_from_locator(locator: &playwright::Locator, selector: &str) -> ElementInfo {
        let tag_name: Option<String> = locator.evaluate("el => el.tagName.toLowerCase()", None::<&()>).await.ok();
        let id = locator.get_attribute("id").await.ok().flatten();
        let class_name = locator.get_attribute("class").await.ok().flatten();
        ElementInfo {
            selector: selector.to_string(),
            tag_name: tag_name.unwrap_or_else(|| "unknown".to_string()),
            id,
            class_name,
        }
    }
}
