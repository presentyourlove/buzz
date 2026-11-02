// Rule trait, definitions, and re-exports for the hooks system
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use async_trait::async_trait;

#[async_trait]
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn trigger(&self) -> HookTrigger;
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>>;
}

pub struct RuleDef {
    pub rule: Box<dyn Rule>,
    pub trigger: HookTrigger,
}

pub mod hardcoded_block;
pub mod json_validation;
pub mod design_doc_check;
pub mod branch_protection;
pub mod large_file_refuse;
pub mod entity_registry;
pub mod data_file_json;
pub mod skill_change_test;

pub use hardcoded_block::HardcodedBlockRule;
pub use json_validation::JsonValidationRule;
pub use design_doc_check::DesignDocCheckRule;
pub use branch_protection::BranchProtectionRule;
pub use large_file_refuse::LargeFileRefuseRule;
pub use entity_registry::EntityRegistryRule;
pub use data_file_json::DataFileJsonRule;
pub use skill_change_test::SkillChangeTestRule;
