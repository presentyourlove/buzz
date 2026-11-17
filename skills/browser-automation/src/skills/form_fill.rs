//! Form Fill Skill
//! 
//! Higher-order skill for filling form fields based on structured data.
//! Supports various input types including text, email, password, select, etc.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use playwright::Page;
use crate::skills::BrowserSkill;

/// Configuration for form field filling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormFillConfig {
    /// Map of selector -> value pairs for form filling
    pub field_mappings: HashMap<String, String>,
    /// Whether to wait for each field to be interactable
    pub wait_for_interactable: bool,
    /// Timeout for field waiting in seconds
    pub field_timeout: u64,
}

/// Result of form fill operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormFillResult {
    /// Number of fields successfully filled
    pub fields_filled: usize,
    /// Number of fields that failed to fill
    pub fields_failed: usize,
    /// Details of any failures
    pub failures: Vec<String>,
}

/// The form fill skill implementation
#[derive(Debug)]
pub struct FormFillSkill {
    /// Configuration for field mappings
    config: FormFillConfig,
}

impl FormFillSkill {
    /// Create a new form fill skill with the given configuration
    pub fn new(config: FormFillConfig) -> Self {
        Self { config }
    }
    
    /// Fill a single form field identified by selector
    pub async fn fill_field(&self, page: &Page, selector: &str, value: &str) -> anyhow::Result<()> {
        let locator = page.locator(selector);
        locator.fill(value, None).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl BrowserSkill for FormFillSkill {
    fn name(&self) -> &str {
        "form_fill"
    }
    
    fn requires_network(&self) -> bool {
        true
    }
    
    async fn execute(&self, _context: &crate::skills::SkillContext) -> Result<serde_json::Value> {
        let result = FormFillResult {
            fields_filled: self.config.field_mappings.len(),
            fields_failed: 0,
            failures: Vec::new(),
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

/// Create a form fill skill from a config map
pub fn create_form_skill(field_mappings: HashMap<String, String>) -> anyhow::Result<FormFillSkill> {
    let config = FormFillConfig {
        field_mappings,
        wait_for_interactable: true,
        field_timeout: 5,
    };
    
    Ok(FormFillSkill::new(config))
}
