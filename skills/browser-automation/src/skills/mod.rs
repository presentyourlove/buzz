//! Skills Module
//! 
//! Re-exports all browser automation skills including the 7 built-in skills
//! and higher-order business skills (Form Fill, Structured Data Scrape).

pub mod navigate;
pub mod click;
pub mod type_text;
pub mod screenshot;
pub mod pdf;
pub mod eval_js;
pub mod wait_for;
pub mod form_fill;
pub mod structured_scrape;
pub mod trait_module;  // BrowserSkill trait & registry

pub use navigate::{NavigateSkill, NavigateConfig, NavigateResult, WaitUntil};
pub use click::{ClickSkill, ClickConfig, ClickResult};
pub use type_text::{TypeTextSkill, TypeTextConfig, TypeTextResult};
pub use screenshot::{ScreenshotSkill, ScreenshotConfig, ScreenshotResult, ImageFormat, AnimationHandling, ClipRegion};
pub use pdf::{PdfSkill, PdfConfig, PdfResult, PdfOptions, PaperFormat, Margin};
pub use eval_js::{EvalJSSkill, EvalJSConfig, EvalJSResult, ConsoleLog};
pub use wait_for::{WaitForSkill, WaitForConfig, WaitForResult, WaitCondition, WaitForSelectorState, NetworkWaitKind};
pub use form_fill::{FormFillSkill, FormFillConfig, FormFillResult};
pub use structured_scrape::{StructuredScrapeSkill, StructuredScrapeConfig, StructuredScrapeResult};
pub use trait_module::{BrowserSkill, SkillRegistry, registry, init_registry};
pub use crate::permission_guard;
pub use crate::permission_guard::PermissionGuard;
pub use crate::capability;

// All available skills (extended with higher-order skills)
pub const AVAILABLE_SKILLS: &[&str] = &[
    "navigate",
    "click",
    "type_text",
    "screenshot",
    "pdf",
    "eval_js",
    "wait_for",
    "form_fill",
    "structured_scrape",
];

/// Skill execution context
#[derive(Debug, Clone)]
pub struct SkillContext {
    /// Capability for this skill execution
    pub capability: crate::capability::BrowserCapability,
    /// Browser pool reference
    pub pool: Option<crate::browser_pool::BrowserPool>,
    /// Audit logger
    pub audit: Option<std::sync::Arc<crate::audit::AuditLogger>>,
}

impl SkillContext {
    pub fn new(capability: crate::capability::BrowserCapability) -> Self {
        Self {
            capability,
            pool: None,
            audit: None,
        }
    }
    
    pub fn with_pool(mut self, pool: crate::browser_pool::BrowserPool) -> Self {
        self.pool = Some(pool);
        self
    }
    
    pub fn with_audit(mut self, audit: std::sync::Arc<crate::audit::AuditLogger>) -> Self {
        self.audit = Some(audit);
        self
    }
}
