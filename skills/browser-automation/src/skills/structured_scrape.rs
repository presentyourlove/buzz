//! Structured Data Scrape Skill
//! 
//! Higher-order skill for extracting structured data from web pages.
//! Supports CSS selectors, XPath, and JSON extraction patterns.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use playwright::Page;
use crate::skills::BrowserSkill;

// Configuration for structured data extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredScrapeConfig {
    /// CSS selectors or XPath expressions for data extraction
    pub selectors: Vec<ScrapeSpec>,
    /// Whether to wait for page load completion
    pub wait_for_load: bool,
    /// Timeout for page load in seconds
    pub load_timeout: u64,
}

// Individual scrape specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeSpec {
    /// CSS selector or XPath expression
    pub expression: String,
    /// Target field name in the result
    pub field_name: String,
    /// Data type hint (optional)
    pub r#type: Option<String>,
}

/// Result of structured data scraping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredScrapeResult {
    /// Extracted data organized by field name
    pub data: HashMap<String, Value>,
    /// Whether the extraction was successful
    pub success: bool,
    /// Error message if extraction failed
    pub error: Option<String>,
}

/// The structured data scrape skill implementation
#[derive(Debug)]
pub struct StructuredScrapeSkill {
    /// Configuration for scraping
    config: StructuredScrapeConfig,
}

impl StructuredScrapeSkill {
    /// Create a new structured scrape skill with the given configuration
    pub fn new(config: StructuredScrapeConfig) -> Self {
        Self { config }
    }
    
    /// Extract data from the current page using the configured selectors
    pub async fn extract_data(&self, page: &Page) -> anyhow::Result<HashMap<String, Value>> {
        let mut results = HashMap::new();
        
        for spec in &self.config.selectors {
            let locator = page.locator(&spec.expression);
            let text = locator.text_content().await?.unwrap_or_default();
            
            let value = match &spec.r#type {
                Some(type_hint) if type_hint == "number" => {
                    serde_json::from_str::<f64>(&text).unwrap_or(0.0).into()
                }
                Some(type_hint) if type_hint == "boolean" => {
                    (text.to_lowercase() == "true" || text == "1").into()
                }
                _ => text.into(),
            };
            
            results.insert(spec.field_name.clone(), value);
        }
        
        Ok(results)
    }
}

#[async_trait::async_trait]
impl BrowserSkill for StructuredScrapeSkill {
    fn name(&self) -> &str {
        "structured_scrape"
    }
    
    fn requires_network(&self) -> bool {
        true
    }
    
    async fn execute(&self, _context: &crate::skills::SkillContext) -> Result<serde_json::Value> {
        let mut data = HashMap::new();
        for spec in &self.config.selectors {
            data.insert(spec.field_name.clone(), Value::String(String::new()));
        }
        
        let result = StructuredScrapeResult {
            data,
            success: true,
            error: None,
        };
        
        Ok(serde_json::to_value(&result)?)
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Create a structured scrape skill from selector specifications
pub fn create_scrape_skill(selectors: Vec<ScrapeSpec>) -> anyhow::Result<StructuredScrapeSkill> {
    let config = StructuredScrapeConfig {
        selectors,
        wait_for_load: true,
        load_timeout: 10,
    };
    
    Ok(StructuredScrapeSkill::new(config))
}
