// Skill change test rule - reminds to run tests after modifying skill-related files
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct SkillChangeTestRule;

#[async_trait]
impl Rule for SkillChangeTestRule {
    fn id(&self) -> &'static str {
        "skill-change-test"
    }
    
    fn trigger(&self) -> HookTrigger {
        HookTrigger::PostToolUse
    }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        let path_str = ctx.working_dir.to_string_lossy();
        
        if path_str.contains("skills/") && 
           (path_str.ends_with(".md") || path_str.ends_with(".rs")) {
            
            if ctx.tool_name == "Write" || ctx.tool_name == "Edit" {
                if let Some(args) = ctx.tool_arguments.get("arguments") {
                    if let Some(content) = args.get("content") {
                        let content_str = content.as_str().unwrap_or(&content.to_string());
                        
                        let skill_keywords = ["skill", "Skill", "SKILL", "prompt"];
                        let has_skill_content = skill_keywords.iter()
                            .any(|keyword| content_str.contains(keyword));
                        
                        if has_skill_content {
                            return Ok(HookResult::Warn {
                                message: "技能檔案/代碼修改後，請運行 `cargo test -p buzz-agent` 確保沒有問題".to_string()
                            });
                        }
                    }
                }
            }
        }
        
        Ok(HookResult::Continue)
    }
}
