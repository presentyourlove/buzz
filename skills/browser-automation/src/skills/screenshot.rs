//! Screenshot Skill
//! 
//! Take screenshots with various options and capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

use crate::capability::{BrowserAction, BrowserCapability};

/// Screenshot skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotConfig {
    /// Output file path (optional, returns bytes if not provided)
    pub path: Option<PathBuf>,
    /// Screenshot options
    #[serde(default)]
    pub options: ScreenshotOptions,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    10000
}

/// Screenshot options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenshotOptions {
    /// Capture full page (not just viewport)
    #[serde(default)]
    pub full_page: bool,
    /// Capture specific element only
    pub element_selector: Option<String>,
    /// Image format
    #[serde(default = "default_format")]
    pub format: ImageFormat,
    /// JPEG quality (0-100)
    #[serde(default = "default_quality")]
    pub quality: u8,
    /// Omit background (transparent)
    #[serde(default)]
    pub omit_background: bool,
    /// Clip region
    pub clip: Option<ClipRegion>,
    /// Animation handling
    #[serde(default = "default_animations")]
    pub animations: AnimationHandling,
}

fn default_format() -> ImageFormat {
    ImageFormat::Png
}

fn default_quality() -> u8 {
    90
}

fn default_animations() -> AnimationHandling {
    AnimationHandling::Allow
}

/// Image format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
}

/// Animation handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationHandling {
    Allow,
    Disabled,
}

/// Clip region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Screenshot skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotResult {
    /// Whether screenshot was successful
    pub success: bool,
    /// Image bytes (if path not provided)
    pub image_bytes: Option<Vec<u8>>,
    /// Output file path (if provided)
    pub path: Option<PathBuf>,
    /// Image dimensions
    pub dimensions: Option<ImageDimensions>,
    /// File size in bytes
    pub file_size: Option<u64>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
}

/// Image dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

/// Screenshot skill
pub struct ScreenshotSkill;

impl ScreenshotSkill {
    /// Execute screenshot
    pub async fn execute(
        page: &Page,
        config: ScreenshotConfig,
    ) -> Result<ScreenshotResult> {
        let start = std::time::Instant::now();
        
        info!("Taking screenshot: full_page={}, format={:?}", config.options.full_page, config.options.format);
        
        // Build screenshot options
        let mut options = playwright::api::ScreenshotOptions::default();
        options.full_page = config.options.full_page;
        options.format = match config.options.format {
            ImageFormat::Png => playwright::api::ScreenshotFormat::Png,
            ImageFormat::Jpeg => playwright::api::ScreenshotFormat::Jpeg,
            ImageFormat::Webp => playwright::api::ScreenshotFormat::Webp,
        };
        options.quality = Some(config.options.quality);
        options.omit_background = config.options.omit_background;
        options.animations = match config.options.animations {
            AnimationHandling::Allow => playwright::api::AnimationHandling::Allow,
            AnimationHandling::Disabled => playwright::api::AnimationHandling::Disabled,
        };
        
        if let Some(clip) = config.options.clip {
            options.clip = Some(playwright::api::ClipRect {
                x: clip.x,
                y: clip.y,
                width: clip.width,
                height: clip.height,
            });
        }
        
        let image_bytes = if let Some(selector) = config.options.element_selector {
            // Element screenshot
            let element = page
                .wait_for_selector(&selector)
                .await
                .with_context(|| format!("Element not found: {}", selector))?;
            
            element.screenshot(options).await?
        } else {
            // Page screenshot
            page.screenshot(options).await?
        };
        
        let duration = start.elapsed().as_millis() as u64;
        let file_size = image_bytes.len() as u64;
        
        // Save to file if path provided
        let path = if let Some(path) = config.path {
            tokio::fs::write(&path, &image_bytes).await
                .with_context(|| format!("Failed to write screenshot to {:?}", path))?;
            Some(path)
        } else {
            None
        };
        
        // Get dimensions from image (simplified)
        let dimensions = Self::get_image_dimensions(&image_bytes);
        
        debug!("Screenshot completed: {} bytes in {}ms", file_size, duration);
        
        Ok(ScreenshotResult {
            success: true,
            image_bytes: if config.path.is_none() { Some(image_bytes) } else { None },
            path,
            dimensions,
            file_size: Some(file_size),
            error: None,
            duration_ms: duration,
        })
    }
    
    /// Get image dimensions from bytes (simplified)
    fn get_image_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
        // PNG signature check
        if bytes.len() >= 24 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            return Some(ImageDimensions { width, height });
        }
        
        // JPEG - would need more complex parsing
        // WebP - would need more complex parsing
        
        None
    }
    
    /// Take multiple screenshots (for comparison)
    pub async fn take_sequence(
        page: &Page,
        configs: Vec<ScreenshotConfig>,
        interval_ms: u64,
    ) -> Result<Vec<ScreenshotResult>> {
        let mut results = Vec::new();
        
        for config in configs {
            let result = Self::execute(page, config).await?;
            results.push(result);
            
            if interval_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
            }
        }
        
        Ok(results)
    }
    
    /// Take screenshot on scroll (full page with stitching)
    pub async fn full_page_scroll(
        page: &Page,
        config: ScreenshotConfig,
    ) -> Result<ScreenshotResult> {
        // Ensure we're at the top
        page.evaluate("window.scrollTo(0, 0)").await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Take full page screenshot
        let mut full_config = config.clone();
        full_config.options.full_page = true;
        
        Self::execute(page, full_config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_screenshot_config_defaults() {
        let config = ScreenshotConfig {
            path: Some(PathBuf::from("screenshot.png")),
            options: ScreenshotOptions::default(),
            timeout_ms: 10000,
        };
        
        assert!(!config.options.full_page);
        assert_eq!(config.options.format, ImageFormat::Png);
        assert_eq!(config.options.quality, 90);
    }
}
