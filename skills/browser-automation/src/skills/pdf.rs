//! PDF Skill
//! 
//! Generate PDFs from pages with various options and capability validation.

use anyhow::{Context, Result};
use playwright::Page;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

use crate::capability::{BrowserAction, BrowserCapability};

/// PDF skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfConfig {
    /// Output file path (optional, returns bytes if not provided)
    pub path: Option<PathBuf>,
    /// PDF options
    #[serde(default)]
    pub options: PdfOptions,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30000
}

/// PDF options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PdfOptions {
    /// Paper format
    #[serde(default = "default_format")]
    pub format: PaperFormat,
    /// Custom paper width (e.g., "8.5in", "210mm")
    pub width: Option<String>,
    /// Custom paper height (e.g., "11in", "297mm")
    pub height: Option<String>,
    /// Print background graphics
    #[serde(default)]
    pub print_background: bool,
    /// Landscape orientation
    #[serde(default)]
    pub landscape: bool,
    /// Margin settings
    #[serde(default)]
    pub margin: Margin,
    /// Scale factor
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// Page ranges (e.g., "1-2, 4")
    pub page_ranges: Option<String>,
    /// Prefer CSS page size
    #[serde(default)]
    pub prefer_css_page_size: bool,
    /// Generate tagged PDF
    #[serde(default)]
    pub tagged: bool,
    /// Generate outline (bookmarks)
    #[serde(default)]
    pub outline: bool,
}

fn default_format() -> PaperFormat {
    PaperFormat::A4
}

fn default_scale() -> f64 {
    1.0
}

/// Paper format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PaperFormat {
    Letter,
    Legal,
    Tabloid,
    Ledger,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
}

/// Margin settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Margin {
    pub top: Option<String>,
    pub right: Option<String>,
    pub bottom: Option<String>,
    pub left: Option<String>,
}

/// PDF skill result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfResult {
    /// Whether PDF generation was successful
    pub success: bool,
    /// PDF bytes (if path not provided)
    pub pdf_bytes: Option<Vec<u8>>,
    /// Output file path (if provided)
    pub path: Option<PathBuf>,
    /// Number of pages
    pub page_count: Option<u32>,
    /// File size in bytes
    pub file_size: Option<u64>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
}

/// PDF skill
pub struct PdfSkill;

impl PdfSkill {
    /// Execute PDF generation
    pub async fn execute(
        page: &Page,
        config: PdfConfig,
    ) -> Result<PdfResult> {
        let start = std::time::Instant::now();
        
        info!("Generating PDF: format={:?}, landscape={}", config.options.format, config.options.landscape);
        
        // Build PDF options
        let mut options = playwright::api::PdfOptions::default();
        options.format = Some(match config.options.format {
            PaperFormat::Letter => playwright::api::PaperFormat::Letter,
            PaperFormat::Legal => playwright::api::PaperFormat::Legal,
            PaperFormat::Tabloid => playwright::api::PaperFormat::Tabloid,
            PaperFormat::Ledger => playwright::api::PaperFormat::Ledger,
            PaperFormat::A0 => playwright::api::PaperFormat::A0,
            PaperFormat::A1 => playwright::api::PaperFormat::A1,
            PaperFormat::A2 => playwright::api::PaperFormat::A2,
            PaperFormat::A3 => playwright::api::PaperFormat::A3,
            PaperFormat::A4 => playwright::api::PaperFormat::A4,
            PaperFormat::A5 => playwright::api::PaperFormat::A5,
            PaperFormat::A6 => playwright::api::PaperFormat::A6,
        });
        
        if let Some(width) = config.options.width {
            options.width = Some(width);
        }
        if let Some(height) = config.options.height {
            options.height = Some(height);
        }
        options.print_background = config.options.print_background;
        options.landscape = config.options.landscape;
        options.margin = Some(playwright::api::Margin {
            top: config.options.margin.top,
            right: config.options.margin.right,
            bottom: config.options.margin.bottom,
            left: config.options.margin.left,
        });
        options.scale = Some(config.options.scale);
        options.page_ranges = config.options.page_ranges;
        options.prefer_css_page_size = config.options.prefer_css_page_size;
        options.tagged = config.options.tagged;
        options.outline = config.options.outline;
        
        // Wait for network idle before generating PDF
        page.wait_for_load_state("networkidle").await?;
        
        // Generate PDF
        let pdf_bytes = page.pdf(options).await
            .context("Failed to generate PDF")?;
        
        let duration = start.elapsed().as_millis() as u64;
        let file_size = pdf_bytes.len() as u64;
        
        // Save to file if path provided
        let path = if let Some(path) = config.path {
            tokio::fs::write(&path, &pdf_bytes).await
                .with_context(|| format!("Failed to write PDF to {:?}", path))?;
            Some(path)
        } else {
            None
        };
        
        // Estimate page count (PDF parsing would be needed for exact count)
        let page_count = Self::estimate_page_count(&pdf_bytes);
        
        debug!("PDF generated: {} bytes, ~{} pages in {}ms", file_size, page_count.unwrap_or(0), duration);
        
        Ok(PdfResult {
            success: true,
            pdf_bytes: if config.path.is_none() { Some(pdf_bytes) } else { None },
            path,
            page_count,
            file_size: Some(file_size),
            error: None,
            duration_ms: duration,
        })
    }
    
    /// Estimate page count from PDF bytes (simplified)
    fn estimate_page_count(bytes: &[u8]) -> Option<u32> {
        // Look for /Count in PDF trailer
        let content = String::from_utf8_lossy(bytes);
        if let Some(pos) = content.rfind("/Count") {
            let after = &content[pos + 6..];
            if let Some(end) = after.find(|c: char| !c.is_ascii_digit()) {
                let count_str = &after[..end];
                if let Ok(count) = count_str.parse::<u32>() {
                    return Some(count);
                }
            }
        }
        None
    }
    
    /// Generate PDF with custom CSS for printing
    pub async fn execute_with_print_css(
        page: &Page,
        config: PdfConfig,
        print_css: &str,
    ) -> Result<PdfResult> {
        // Inject print CSS
        page.add_style_tag(playwright::api::AddStyleTagOptions {
            content: Some(print_css.to_string()),
            ..Default::default()
        }).await?;
        
        // Emulate print media
        page.emulate_media("print").await?;
        
        // Generate PDF
        let result = Self::execute(page, config).await?;
        
        // Reset media emulation
        page.emulate_media("screen").await?;
        
        Ok(result)
    }
    
    /// Generate PDF from specific element
    pub async fn generate_from_element(
        page: &Page,
        selector: &str,
        config: PdfConfig,
    ) -> Result<PdfResult> {
        // Get element dimensions
        let element = page.wait_for_selector(selector).await
            .context("Element not found")?;
        
        let bounding_box = element.bounding_box().await
            .context("Failed to get element bounding box")?;
        
        // Set custom paper size to match element
        let mut custom_config = config;
        custom_config.options.width = Some(format!("{}px", bounding_box.width));
        custom_config.options.height = Some(format!("{}px", bounding_box.height));
        
        let start = std::time::Instant::now();
        let pdf_bytes = page.pdf(custom_config.options.clone()).await
            .context("PDF generation failed")?;
        
        Ok(PdfResult {
            pdf_bytes,
            page_count: 1,
            execution_time_ms: start.elapsed().as_millis() as u64,
        })
    }
}
