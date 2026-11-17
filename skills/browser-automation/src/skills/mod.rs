//! Skills Module
//!
//! Re-exports all browser automation skills including the 7 built-in skills
//! and higher-order business skills (Form Fill, Structured Data Scrape).

pub mod click;
pub mod eval_js;
pub mod form_fill;
pub mod navigate;
pub mod pdf;
pub mod screenshot;
pub mod structured_scrape;
pub mod trait_module;
pub mod type_text;
pub mod wait_for; // BrowserSkill trait & registry

pub use crate::capability;
pub use crate::permission_guard;
pub use crate::permission_guard::PermissionGuard;
pub use click::{ClickConfig, ClickResult, ClickSkill};
pub use eval_js::{ConsoleLog, EvalJSConfig, EvalJSResult, EvalJSSkill};
pub use form_fill::{FormFillConfig, FormFillResult, FormFillSkill};
pub use navigate::{NavigateConfig, NavigateResult, NavigateSkill, WaitUntil};
pub use pdf::{Margin, PaperFormat, PdfConfig, PdfOptions, PdfResult, PdfSkill};
pub use screenshot::{
    AnimationHandling, ClipRegion, ImageFormat, ScreenshotConfig, ScreenshotResult, ScreenshotSkill,
};
pub use structured_scrape::{
    StructuredScrapeConfig, StructuredScrapeResult, StructuredScrapeSkill,
};
pub use trait_module::{init_registry, registry, BrowserSkill, SkillRegistry};
pub use type_text::{TypeTextConfig, TypeTextResult, TypeTextSkill};
pub use wait_for::{
    NetworkWaitKind, WaitCondition, WaitForConfig, WaitForResult, WaitForSelectorState,
    WaitForSkill,
};

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
