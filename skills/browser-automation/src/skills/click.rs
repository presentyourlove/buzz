//! Click Skill
//! 
//! Click elements with various options and capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::capability::{BrowserAction, BrowserCapability};

/// Click skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickConfig {
    /// CSS selector or XPath
    pub selector: String,
    /// Click options
    #[serde(default)]
    pub options: ClickOptions,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Wait for navigation after click
    #[serde(default)]
    pub wait_for_navigation: bool,
    /// Navigation wait until condition
    #[serde(default = "default_wait_until")]
    pub wait_until: super::navigate::WaitUntil,
}

fn default_timeout() -> u64 {
    10000
}

fn default_wait_until() -> super::navigate::WaitUntil {
    super::navigate::WaitUntil::NetworkIdle
}

/// Click options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClickOptions {
    /// Button to click (left, right, middle)
    #[serde(default = "default_button")]
    pub button: ClickButton,
    /// Click count (1 for single, 2 for double)
    #[serde(default = "default_click_count")]
    pub click_count: u32,
    /// Delay between clicks in ms
    #[serde(default)]
    pub delay_ms: u64,
    /// Force click even if element is not visible
    #[serde(default)]
    pub force: bool,
    /// Don't wait for actionability checks
    #[serde(default)]
    pub no_wait_after: bool,
    /// Position offset
    pub position: Option<ClickPosition>,
    /// Modifiers (Shift, Control, Alt, Meta)
    #[serde(default)]
    pub modifiers: Vec<ModifierKey>,
}

fn default_button() -> ClickButton {
    ClickButton::Left
}

fn default_click_count() -> u32 {
    1
}

/// Click button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClickButton {
    Left,
    Right,
    Middle,
}

/// Click position offset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickPosition {
    pub x: f64,
    pub y: f64,
}

/// Modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModifierKey {
    Shift,
    Control,
    Alt,
    Meta,
}

/// Click skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickResult {
    /// Whether click was successful
    pub success: bool,
    /// Element info if found
    pub element_info: Option<ElementInfo>,
    /// Navigation occurred
    pub navigated: bool,
    /// Final URL if navigated
    pub final_url: Option<String>,
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
    pub text_content: Option<String>,
    pub bounding_box: Option<BoundingBox>,
}

/// Bounding box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Click skill
pub struct ClickSkill;

impl ClickSkill {
    /// Execute click
    pub async fn execute(
        page: &Page,
        config: ClickConfig,
    ) -> Result<ClickResult> {
        let start = std::time::Instant::now();
        
        info!("Clicking element: {}", config.selector);
        
        // Wait for element
        let element = page
            .wait_for_selector(&config.selector)
            .await
            .with_context(|| format!("Element not found: {}", config.selector))?;
        
        // Get element info before click
        let element_info = Self::get_element_info(&element).await;
        
        // Build click options
        let mut click_options = playwright::api::ClickOptions::default();
        click_options.button = match config.options.button {
            ClickButton::Left => playwright::api::MouseButton::Left,
            ClickButton::Right => playwright::api::MouseButton::Right,
            ClickButton::Middle => playwright::api::MouseButton::Middle,
        };
        click_options.click_count = config.options.click_count as usize;
        click_options.delay = config.options.delay_ms as f64;
        click_options.force = config.options.force;
        click_options.no_wait_after = config.options.no_wait_after;
        
        if let Some(pos) = config.options.position {
            click_options.position = Some(playwright::api::Position { x: pos.x, y: pos.y });
        }
        
        if !config.options.modifiers.is_empty() {
            click_options.modifiers = config.options.modifiers.iter().map(|m| match m {
                ModifierKey::Shift => playwright::api::KeyboardModifier::Shift,
                ModifierKey::Control => playwright::api::KeyboardModifier::Control,
                ModifierKey::Alt => playwright::api::KeyboardModifier::Alt,
                ModifierKey::Meta => playwright::api::KeyboardModifier::Meta,
            }).collect();
        }
        
        // Perform click
        let navigation_result = if config.wait_for_navigation {
            let nav_future = page.wait_for_navigation();
            element.click(click_options).await?;
            nav_future.await.ok()
        } else {
            element.click(click_options).await?;
            None
        };
        
        let duration = start.elapsed().as_millis() as u64;
        let navigated = navigation_result.is_some();
        let final_url = navigation_result.as_ref().map(|_| page.url());
        
        if navigated {
            page.wait_for_load_state(config.wait_until.as_str()).await.ok();
        }
        
        debug!("Click completed in {}ms, navigated: {}", duration, navigated);
        
        Ok(ClickResult {
            success: true,
            element_info,
            navigated,
            final_url,
            error: None,
            duration_ms: duration,
        })
    }
    
    /// Get element information
    async fn get_element_info(element: &playwright::api::ElementHandle) -> Option<ElementInfo> {
        let tag_name = element.evaluate("el => el.tagName.toLowerCase()").await.ok()?;
        let id = element.get_attribute("id").await.ok().flatten();
        let class_name = element.get_attribute("class").await.ok().flatten();
        let text_content = element.text_content().await.ok();
        let bounding_box = element.bounding_box().await.ok().map(|b| BoundingBox {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        });
        
        Some(ElementInfo {
            tag_name: tag_name.as_str().unwrap_or("unknown").to_string(),
            id,
            class_name,
            text_content,
            bounding_box,
        })
    }
    
    /// Click by coordinates (for canvas, etc.)
    pub async fn click_coordinates(
        page: &Page,
        x: f64,
        y: f64,
        options: ClickOptions,
    ) -> Result<ClickResult> {
        let start = std::time::Instant::now();
        
        page.mouse().move_(x, y).await?;
        
        let mut click_options = playwright::api::ClickOptions::default();
        click_options.button = match options.button {
            ClickButton::Left => playwright::api::MouseButton::Left,
            ClickButton::Right => playwright::api::MouseButton::Right,
            ClickButton::Middle => playwright::api::MouseButton::Middle,
        };
        click_options.click_count = options.click_count as usize;
        click_options.delay = options.delay_ms as f64;
        
        page.mouse().down(click_options).await?;
        page.mouse().up(click_options).await?;
        
        Ok(ClickResult {
            success: true,
            element_info: None,
            navigated: false,
            final_url: None,
            execution_time_ms: start.elapsed().as_millis() as u64,
        })
    }
}
