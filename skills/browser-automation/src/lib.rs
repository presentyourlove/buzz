//! Browser Automation Skill
//! 
//! 基於 Playwright Rust 的安全瀏覽器自動化技能，整合 Antigravity Encrypted Vault
//! 提供 7 大內建技能：Navigate, Click, TypeText, Screenshot, PDF, EvalJS, WaitFor

pub mod browser_pool;
pub mod context_manager;
pub mod capability;
pub mod skills;
pub mod audit;
pub mod stealth;
pub mod vault;
pub mod permission_guard;

pub use browser_pool::{BrowserPool, PoolConfig, PoolStats, IsolatedContext, BrowserTypeConfig};
pub use context_manager::ContextManager;
pub use capability::{
    BrowserCapability, BrowserAction, NetworkEgressPolicy, CapabilityBuilder, CapabilitySet
};
pub use skills::{
    NavigateSkill, ClickSkill, TypeTextSkill, ScreenshotSkill, PdfSkill, EvalJSSkill, WaitForSkill,
    AVAILABLE_SKILLS, SkillContext,
};
pub use audit::{AuditLogger, AuditConfig, AuditEvent, AuditEventType, AuditSeverity, AuditResult, AuditSession};
pub use stealth::{StealthConfig, apply_stealth, apply_max_stealth, StealthUtils};
pub use vault::{VaultClient, BrowserCredential, CredentialSelector};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the browser automation skill
pub async fn initialize(config: PoolConfig) -> anyhow::Result<BrowserPool> {
    BrowserPool::new(config).await
}
