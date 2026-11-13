// HookRegistry module - Automated validation and security hooks for Buzz Agent
pub mod rules;
pub mod types;

pub use rules::*;
pub use types::*;

use crate::types::ToolResult;

pub struct HookRegistry {
    rules: Vec<RuleDef>,
}

impl HookRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            rules: Vec::new(),
        };
        
        // 載入預設規則（維持決定性執行順序）
        registry.load_default_rules();
        
        registry
    }
    
    fn load_default_rules(&mut self) {
        use crate::hooks::rules::*;
        
        let default_rules: Vec<Box<dyn Rule>> = vec![
            Box::new(ThreeTierGateRule),
            Box::new(DangerCommandBlockRule),
            Box::new(PathTraversalGuardRule),
            Box::new(LargeFileGuardRule),
            Box::new(SyntaxIntegrityCheckRule),
            Box::new(HardcodedBlockRule),
            Box::new(JsonValidationRule),
            Box::new(DesignDocCheckRule),
            Box::new(BranchProtectionRule),
            Box::new(LargeFileRefuseRule),
            Box::new(EntityRegistryRule),
            Box::new(DataFileJsonRule),
            Box::new(SkillChangeTestRule),
            Box::new(UnslopGuardRule),
        ];
        
        for rule in default_rules {
            let trigger = rule.trigger();
            let rule_def = RuleDef { rule, trigger };
            self.rules.push(rule_def);
        }
    }
    
    // 在 agent.rs run() 循環中調用
    pub async fn run_pre_tool_use(&self, ctx: &HookContext) -> HookResult {
        for rule_def in &self.rules {
            if rule_def.trigger == HookTrigger::PreToolUse {
                match rule_def.rule.check(ctx).await {
                    Ok(HookResult::Continue) => continue,
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }
        }
        HookResult::Continue
    }
    
    pub async fn run_post_tool_use(&self, ctx: &HookContext, _result: &ToolResult) -> HookResult {
        for rule_def in &self.rules {
            if rule_def.trigger == HookTrigger::PostToolUse {
                match rule_def.rule.check(ctx).await {
                    Ok(HookResult::Continue) => continue,
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }
        }
        HookResult::Continue
    }
    
    pub async fn run_session_start(&self, ctx: &HookContext) -> HookResult {
        for rule_def in &self.rules {
            if rule_def.trigger == HookTrigger::SessionStart {
                match rule_def.rule.check(ctx).await {
                    Ok(HookResult::Continue) => continue,
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }
        }
        HookResult::Continue
    }
    
    pub async fn run_session_end(&self, ctx: &HookContext) -> HookResult {
        for rule_def in &self.rules {
            if rule_def.trigger == HookTrigger::SessionEnd {
                match rule_def.rule.check(ctx).await {
                    Ok(HookResult::Continue) => continue,
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }
        }
        HookResult::Continue
    }
    
    pub async fn run_subagent_start(&self, ctx: &HookContext) -> HookResult {
        for rule_def in &self.rules {
            if rule_def.trigger == HookTrigger::SubagentStart {
                match rule_def.rule.check(ctx).await {
                    Ok(HookResult::Continue) => continue,
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }
        }
        HookResult::Continue
    }
    
    pub async fn run_subagent_end(&self, ctx: &HookContext) -> HookResult {
        for rule_def in &self.rules {
            if rule_def.trigger == HookTrigger::SubagentEnd {
                match rule_def.rule.check(ctx).await {
                    Ok(HookResult::Continue) => continue,
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }
        }
        HookResult::Continue
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_ctx(tool: &str, args: serde_json::Value) -> HookContext {
        HookContext {
            tool_name: tool.into(),
            tool_arguments: args,
            session_id: "test-session".into(),
            working_dir: PathBuf::from("."),
            trigger_event: HookTrigger::PreToolUse,
        }
    }

    #[tokio::test]
    async fn test_hook_registry_blocks_path_traversal() {
        let reg = HookRegistry::new();
        let ctx = make_ctx("write_file", serde_json::json!({"path": "../../etc/passwd"}));
        let res = reg.run_pre_tool_use(&ctx).await;
        assert!(matches!(res, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_registry_blocks_danger_command() {
        let reg = HookRegistry::new();
        let ctx = make_ctx("bash", serde_json::json!({"command": "rm -rf /"}));
        let res = reg.run_pre_tool_use(&ctx).await;
        assert!(matches!(res, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_registry_blocks_hardcoded_secret() {
        let reg = HookRegistry::new();
        let ctx = make_ctx(
            "write_file",
            serde_json::json!({"path": "src/main.rs", "content": "let k = \"sk-or-v1-1234567890abcdef\";"}),
        );
        let res = reg.run_pre_tool_use(&ctx).await;
        assert!(matches!(res, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_registry_blocks_branch_force_push() {
        let reg = HookRegistry::new();
        let ctx = make_ctx("run_command", serde_json::json!({"CommandLine": "git push origin main"}));
        let res = reg.run_pre_tool_use(&ctx).await;
        assert!(matches!(res, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_registry_allows_safe_operations() {
        let reg = HookRegistry::new();
        let ctx = make_ctx(
            "write_file",
            serde_json::json!({"path": "src/safe.rs", "content": "fn main() {}"}),
        );
        let res = reg.run_pre_tool_use(&ctx).await;
        assert!(matches!(res, HookResult::Continue));
    }
}
