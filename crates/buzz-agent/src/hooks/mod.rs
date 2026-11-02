// HookRegistry module - Automated validation and security hooks for Buzz Agent
pub mod rules;
pub mod types;

pub use rules::*;
pub use types::*;

use std::collections::HashMap;
use crate::types::ToolResult;

pub struct HookRegistry {
    rules: HashMap<String, RuleDef>,
}

impl HookRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            rules: HashMap::new(),
        };
        
        // 載入預設規則
        registry.load_default_rules();
        
        registry
    }
    
    fn load_default_rules(&mut self) {
        use crate::hooks::rules::*;
        
        let default_rules: Vec<Box<dyn Rule>> = vec![
            Box::new(HardcodedBlockRule),
            Box::new(JsonValidationRule),
            Box::new(DesignDocCheckRule),
            Box::new(BranchProtectionRule),
            Box::new(LargeFileRefuseRule),
            Box::new(EntityRegistryRule),
            Box::new(DataFileJsonRule),
            Box::new(SkillChangeTestRule),
        ];
        
        for rule in default_rules {
            let id = rule.id().to_string();
            let trigger = rule.trigger();
            let rule_def = RuleDef { rule, trigger };
            self.rules.insert(id, rule_def);
        }
    }
    
    // 在 agent.rs run() 循環中調用
    pub async fn run_pre_tool_use(&self, ctx: &HookContext) -> HookResult {
        for (_id, rule_def) in &self.rules {
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
        for (_id, rule_def) in &self.rules {
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
        for (_id, rule_def) in &self.rules {
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
        for (_id, rule_def) in &self.rules {
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
        for (_id, rule_def) in &self.rules {
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
        for (_id, rule_def) in &self.rules {
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
