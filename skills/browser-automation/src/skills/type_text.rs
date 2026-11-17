//! TypeText Skill
//! 
//! Type text into input fields with human-like simulation and capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::capability::{BrowserAction, BrowserCapability};

/// TypeText skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeTextConfig {
    /// CSS selector or XPath for the input element
    pub selector: String,
    /// Text to type
    pub text: String,
    /// Type options
    #[serde(default)]
    pub options: TypeOptions,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Clear field before typing
    #[serde(default = "default_clear")]
    pub clear_first: bool,
    /// Press Enter after typing
    #[serde(default)]
    pub press_enter: bool,
}

fn default_timeout() -> u64 {
    10000
}

fn default_clear() -> bool {
    true
}

/// Type options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeOptions {
    /// Delay between keystrokes in ms (human-like)
    #[serde(default = "default_delay")]
    pub delay_ms: u64,
    /// Simulate human-like typing with random delays
    #[serde(default)]
    pub human_like: bool,
    /// Don't wait for actionability checks
    #[serde(default)]
    pub no_wait_after: bool,
}

fn default_delay() -> u64 {
    50
}

/// TypeText skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeTextResult {
    /// Whether typing was successful
    pub success: bool,
    /// Number of characters typed
    pub chars_typed: usize,
    /// Element info
    pub element_info: Option<ElementInfo>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
}

/// Element information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInfo {
    pub tag_name: String,
    pub id: Option<String>,
    pub class_name: Option<String>,
    pub value: Option<String>,
    pub input_type: Option<String>,
}

/// TypeText skill
pub struct TypeTextSkill;

impl TypeTextSkill {
    /// Execute type text
    pub async fn execute(
        page: &Page,
        config: TypeTextConfig,
    ) -> Result<TypeTextResult> {
        let start = std::time::Instant::now();
        
        info!("Typing into element: {}", config.selector);
        
        // Wait for element
        let element = page
            .wait_for_selector(&config.selector)
            .await
            .with_context(|| format!("Element not found: {}", config.selector))?;
        
        // Get element info before typing
        let element_info = Self::get_element_info(&element).await;
        
        // Clear field if requested
        if config.clear_first {
            element.click().await?;
            page.keyboard().press("Control+A").await?;
            page.keyboard().press("Delete").await?;
        }
        
        // Type text
        let chars_typed = if config.options.human_like {
            Self::human_type(page, &element, &config.text, config.options.delay_ms).await?
        } else {
            let type_options = playwright::api::TypeOptions {
                delay: config.options.delay_ms as f64,
                no_wait_after: config.options.no_wait_after,
            };
            element.type_text(&config.text, type_options).await?;
            config.text.chars().count()
        };
        
        // Press Enter if requested
        if config.press_enter {
            page.keyboard().press("Enter").await?;
        }
        
        let duration = start.elapsed().as_millis() as u64;
        
        debug!("Type completed: {} chars in {}ms", chars_typed, duration);
        
        Ok(TypeTextResult {
            success: true,
            chars_typed,
            element_info,
            error: None,
            duration_ms: duration,
        })
    }
    
    /// Human-like typing with random delays
    async fn human_type(
        page: &Page,
        element: &playwright::api::ElementHandle,
        text: &str,
        base_delay: u64,
    ) -> Result<usize> {
        element.click().await?;
        
        let mut chars_typed = 0;
        for ch in text.chars() {
            page.keyboard().type_text(&ch.to_string()).await?;
            chars_typed += 1;
            
            // Random delay between 0.5x and 2x base delay
            let delay = base_delay as f64 * (0.5 + rand::random::<f64>() * 1.5);
            tokio::time::sleep(tokio::time::Duration::from_millis(delay as u64)).await;
        }
        
        Ok(chars_typed)
    }
    
    /// Get element information
    async fn get_element_info(element: &playwright::api::ElementHandle) -> Option<ElementInfo> {
        let tag_name = element.evaluate("el => el.tagName.toLowerCase()").await.ok()?;
        let id = element.get_attribute("id").await.ok().flatten();
        let class_name = element.get_attribute("class").await.ok().flatten();
        let value = element.get_attribute("value").await.ok().flatten();
        let input_type = element.get_attribute("type").await.ok().flatten();
        
        Some(ElementInfo {
            tag_name: tag_name.as_str().unwrap_or("unknown").to_string(),
            id,
            class_name,
            value,
            input_type,
        })
    }
    
    /// Type into element by coordinates (for canvas, etc.)
    pub async fn type_at_position(
        page: &Page,
        x: f64,
        y: f64,
        text: &str,
        options: TypeOptions,
    ) -> Result<TypeTextResult> {
        let start = std::time::Instant::now();
        
        page.mouse().move_(x, y).await?;
        page.mouse().click(x, y).await?;
        
        let mut chars_typed = 0;
        for ch in text.chars() {
            page.keyboard().type_text(&ch.to_string()).await?;
            chars_typed += 1;
            
            let delay = options.delay_ms as f64 * (0.5 + rand::random::<f64>() * 1.5);
            tokio::time::sleep(tokio::time::Duration::from_millis(delay as u64)).await;
        }
        
        Ok(TypeTextResult {
            success: true,
            chars_typed,
            element_info: None,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
    
    /// Press a key combination
    pub async fn press_key(
        page: &Page,
        key: &str,
        modifiers: Vec<&str>,
    ) -> Result<()> {
        let key_with_modifiers = if modifiers.is_empty() {
            key.to_string()
        } else {
            format!("{}+{}", modifiers.join("+"), key)
        };
        
        page.keyboard().press(&key_with_modifiers).await?;
        Ok(())
    }
    
    /// Fill form fields from a map
    pub async fn fill_form(
        page: &Page,
        fields: std::collections::HashMap<String, String>,
        default_options: TypeOptions,
    ) -> Result<Vec<TypeTextResult>> {
        let mut results = Vec::new();
        
        for (selector, text) in fields {
            let config = TypeTextConfig {
                selector,
                text,
                options: default_options.clone(),
                timeout_ms: 10000,
                clear_first: true,
                press_enter: false,
            };
            
            let result = Self::execute(page, config).await?;
            results.push(result);
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_type_text_config_defaults() {
        let config = TypeTextConfig {
            selector: "#input".to_string(),
            text: "hello".to_string(),
            options: TypeOptions::default(),
            timeout_ms: 10000,
            clear_first: true,
            press_enter: false,
        };
        
        assert_eq!(config.options.delay_ms, 50);
        assert!(!config.options.human_like);
        assert!(config.clear_first);
    }
}
