//! Click Skill
//! 
//! Click elements with various options and capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use playwright::protocol::click::{ClickOptions as PwClickOptions, KeyboardModifier, MouseButton};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClickButton {
    #[default]
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
        
        // Wait for element and get info
        let locator = page.locator(&config.selector);
        let element_info = Self::get_element_info(&locator).await;
        
        // Build click options using playwright ClickOptionsBuilder
        let mut click_options_builder = PwClickOptions::builder();
        click_options_builder = click_options_builder.button(match config.options.button {
            ClickButton::Left => MouseButton::Left,
            ClickButton::Right => MouseButton::Right,
            ClickButton::Middle => MouseButton::Middle,
        });

        if config.options.click_count > 1 {
            click_options_builder = click_options_builder.click_count(config.options.click_count as u32);
        }

        if config.options.delay_ms > 0 {
            click_options_builder = click_options_builder.delay(config.options.delay_ms as f64);
        }

        if config.options.force {
            click_options_builder = click_options_builder.force(true);
        }

        if config.options.no_wait_after {
            click_options_builder = click_options_builder.no_wait_after(true);
        }

        if let Some(pos) = &config.options.position {
            click_options_builder = click_options_builder.position(playwright::Position { x: pos.x, y: pos.y });
        }

        if !config.options.modifiers.is_empty() {
            let modifiers: Vec<KeyboardModifier> = config.options.modifiers.iter().map(|m| match m {
                ModifierKey::Shift => KeyboardModifier::Shift,
                ModifierKey::Control => KeyboardModifier::Control,
                ModifierKey::Alt => KeyboardModifier::Alt,
                ModifierKey::Meta => KeyboardModifier::Meta,
            }).collect();
            click_options_builder = click_options_builder.modifiers(modifiers);
        }

        let click_options = click_options_builder.build();

        // Perform click
        locator.click(Some(click_options)).await?;
        
        let duration = start.elapsed().as_millis() as u64;
        let final_url = Some(page.url().to_string());
        
        debug!("Click completed in {}ms", duration);
        
        Ok(ClickResult {
            success: true,
            element_info,
            navigated: false,
            final_url,
            error: None,
            duration_ms: duration,
        })
    }
    
    /// Get element information
    async fn get_element_info(locator: &playwright::Locator) -> Option<ElementInfo> {
        let tag_name: Option<String> = locator.evaluate("el => el.tagName.toLowerCase()", None::<&()>).await.ok();
        let id = locator.get_attribute("id").await.ok().flatten();
        let class_name = locator.get_attribute("class").await.ok().flatten();
        let text_content = locator.text_content().await.ok().flatten();
        let bounding_box = locator.bounding_box().await.ok().flatten().map(|b| BoundingBox {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        });
        
        Some(ElementInfo {
            tag_name: tag_name.unwrap_or_else(|| "unknown".to_string()),
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
        
        let mouse_button = match options.button {
            ClickButton::Left => MouseButton::Left,
            ClickButton::Right => MouseButton::Right,
            ClickButton::Middle => MouseButton::Middle,
        };
        
        let mouse_options = playwright::protocol::MouseOptions::builder()
            .button(mouse_button)
            .click_count(options.click_count)
            .delay(options.delay_ms as f64)
            .build();
        
        page.mouse().click(x, y, Some(mouse_options)).await?;
        
        Ok(ClickResult {
            success: true,
            element_info: None,
            navigated: false,
            final_url: None,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
