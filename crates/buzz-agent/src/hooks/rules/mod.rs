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

pub mod danger_command_block;
pub mod path_traversal_guard;
pub mod large_file_guard;
pub mod syntax_integrity_check;
pub mod hardcoded_block;
pub mod json_validation;
pub mod design_doc_check;
pub mod branch_protection;
pub mod large_file_refuse;
pub mod entity_registry;
pub mod data_file_json;
pub mod skill_change_test;
pub mod three_tier_gate;
pub mod unslop_guard;

pub use danger_command_block::DangerCommandBlockRule;
pub use path_traversal_guard::PathTraversalGuardRule;
pub use large_file_guard::LargeFileGuardRule;
pub use syntax_integrity_check::SyntaxIntegrityCheckRule;
pub use hardcoded_block::HardcodedBlockRule;
pub use json_validation::JsonValidationRule;
pub use design_doc_check::DesignDocCheckRule;
pub use branch_protection::BranchProtectionRule;
pub use large_file_refuse::LargeFileRefuseRule;
pub use entity_registry::EntityRegistryRule;
pub use data_file_json::DataFileJsonRule;
pub use skill_change_test::SkillChangeTestRule;
pub use three_tier_gate::{ThreeTierGateRule, OperationTier};
pub use unslop_guard::UnslopGuardRule;
