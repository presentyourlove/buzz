//! Stealth/Anti-Detection Features
//! 
//! Provides anti-detection measures to prevent websites from identifying
//! automated browser sessions.

use anyhow::Result;
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stealth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthConfig {
    /// Hide webdriver property
    pub hide_webdriver: bool,
    /// Override navigator.plugins
    pub mock_plugins: bool,
    /// Override navigator.languages
    pub mock_languages: bool,
    /// Mock chrome runtime
    pub mock_chrome_runtime: bool,
    /// Mock permissions
    mock_permissions: bool,
    /// Custom user agent
    pub user_agent: Option<String>,
    /// Additional init scripts
    pub extra_scripts: Vec<String>,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            hide_webdriver: true,
            mock_plugins: true,
            mock_languages: true,
            mock_chrome_runtime: true,
            mock_permissions: true,
            user_agent: None,
            extra_scripts: Vec::new(),
        }
    }
}

impl StealthConfig {
    /// Generate the init script for stealth mode
    pub fn generate_init_script(&self) -> String {
        let mut scripts = Vec::new();
        
        // Hide webdriver
        if self.hide_webdriver {
            scripts.push(r#"
                Object.defineProperty(navigator, 'webdriver', {
                    get: () => undefined,
                });
            "#.to_string());
        }
        
        // Mock plugins
        if self.mock_plugins {
            scripts.push(r#"
                Object.defineProperty(navigator, 'plugins', {
                    get: () => [
                        { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
                        { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', description: '' },
                        { name: 'Native Client', filename: 'internal-nacl-plugin', description: '' }
                    ],
                });
            "#.to_string());
        }
        
        // Mock languages
        if self.mock_languages {
            scripts.push(r#"
                Object.defineProperty(navigator, 'languages', {
                    get: () => ['en-US', 'en', 'zh-CN', 'zh'],
                });
            "#.to_string());
        }
        
        // Mock chrome runtime
        if self.mock_chrome_runtime {
            scripts.push(r#"
                window.chrome = {
                    runtime: {},
                    loadTimes: function() {},
                    csi: function() {},
                    app: {}
                };
            "#.to_string());
        }
        
        // Mock permissions
        if self.mock_permissions {
            scripts.push(r#"
                const originalQuery = window.navigator.permissions.query;
                window.navigator.permissions.query = (parameters) => (
                    parameters.name === 'notifications' ?
                        Promise.resolve({ state: Notification.permission }) :
                        originalQuery(parameters)
                );
            "#.to_string());
        }
        
        // Add extra scripts
        scripts.extend(self.extra_scripts.clone());
        
        scripts.join("\n")
    }
    
    /// Create a stealth config for maximum stealth
    pub fn max_stealth() -> Self {
        Self {
            hide_webdriver: true,
            mock_plugins: true,
            mock_languages: true,
            mock_chrome_runtime: true,
            mock_permissions: true,
            user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string()),
            extra_scripts: vec![
                // Override toString of functions to hide automation
                r#"
                const originalToString = Function.prototype.toString;
                Function.prototype.toString = function() {
                    if (this === navigator.webdriver) {
                        return 'function webdriver() { [native code] }';
                    }
                    return originalToString.call(this);
                };
                "#.to_string(),
                // Hide automation indicator in console
                r#"
                console.debug = function() {};
                "#.to_string(),
            ],
        }
    }
    
    /// Create a stealth config for minimal stealth (just hide webdriver)
    pub fn minimal() -> Self {
        Self {
            hide_webdriver: true,
            mock_plugins: false,
            mock_languages: false,
            mock_chrome_runtime: false,
            mock_permissions: false,
            user_agent: None,
            extra_scripts: Vec::new(),
        }
    }
}

/// Apply stealth to a page
pub async fn apply_stealth(page: &Page) -> Result<()> {
    let config = StealthConfig::default();
    apply_stealth_with_config(page, &config).await
}

/// Apply stealth to a page with custom config
pub async fn apply_stealth_with_config(page: &Page, config: &StealthConfig) -> Result<()> {
    let script = config.generate_init_script();
    page.add_init_script(&script).await
        .map_err(|e| anyhow::anyhow!("Failed to apply stealth script: {}", e))?;
    
    // Set custom user agent if provided
    if let Some(ua) = &config.user_agent {
        page.set_extra_http_headers(
            HashMap::from([("User-Agent".to_string(), ua.clone())])
        ).await?;
    }
    
    Ok(())
}

/// Apply stealth with maximum protection
pub async fn apply_max_stealth(page: &Page) -> Result<()> {
    let config = StealthConfig::max_stealth();
    apply_stealth_with_config(page, &config).await
}

/// Stealth utilities for common evasion techniques
pub struct StealthUtils;

impl StealthUtils {
    /// Randomize viewport slightly
    pub async fn randomize_viewport(page: &Page) -> Result<()> {
        let width = 1920 + rand::random::<i32>() % 100;
        let height = 1080 + rand::random::<i32>() % 100;
        page.set_viewport_size(playwright::protocol::browser_context::Viewport {
            width: width as u32,
            height: height as u32,
        }).await?;
        Ok(())
    }
    
    /// Add human-like mouse movements
    pub async fn human_like_mouse_move(page: &Page, x: f64, y: f64) -> Result<()> {
        // Move in steps with slight randomization
        let steps = 10 + rand::random::<u32>() % 10;
        let current: serde_json::Value = page.evaluate("() => ({ x: window.mouseX || 0, y: window.mouseY || 0 })", None::<&()>).await.unwrap_or_else(|_| serde_json::json!({ "x": 0, "y": 0 }));
        let start_x = current.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let start_y = current.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        for i in 1..=steps {
            let progress = i as f64 / steps as f64;
            let ease = progress * progress * (3.0 - 2.0 * progress); // Smoothstep
            let cx = start_x + (x - start_x) * ease + (rand::random::<f64>() - 0.5) * 2.0;
            let cy = start_y + (y - start_y) * ease + (rand::random::<f64>() - 0.5) * 2.0;
            
            page.mouse().move_to(cx, cy, None).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(10 + rand::random::<u64>() % 20)).await;
        }
        
        Ok(())
    }
    
    /// Simulate human typing with delays
    pub async fn human_type(page: &Page, selector: &str, text: &str) -> Result<()> {
        page.locator(selector).click(None).await?;
        
        for ch in text.chars() {
            page.keyboard().type_text(&ch.to_string(), None).await?;
            // Random delay between 50-200ms
            let delay = 50 + rand::random::<u64>() % 150;
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
        
        Ok(())
    }
    
    /// Random scroll behavior
    pub async fn random_scroll(page: &Page) -> Result<()> {
        let scroll_amount = 100 + rand::random::<i32>() % 500;
        let _: Option<serde_json::Value> = page.evaluate(&format!("() => window.scrollBy(0, {})", scroll_amount), None::<&()>).await.ok();
        tokio::time::sleep(tokio::time::Duration::from_millis(100 + rand::random::<u64>() % 500)).await;
        Ok(())
    }
}
