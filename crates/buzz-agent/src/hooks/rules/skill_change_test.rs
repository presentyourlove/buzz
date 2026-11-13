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
        if !ctx.matches_tool(&["write_file", "write", "edit_file", "edit", "str_replace", "Write", "Edit"]) {
            return Ok(HookResult::Continue);
        }

        let Some(path_str) = ctx.get_str_arg(&["path", "file_path", "Path", "target"]) else {
            return Ok(HookResult::Continue);
        };

        let normalized = path_str.replace('\\', "/");
        if normalized.contains("skills/") && (normalized.ends_with(".md") || normalized.ends_with(".rs")) {
            if let Some(content_str) = ctx.get_str_arg(&["content", "Content", "new_str", "Replacement"]) {
                let skill_keywords = ["skill", "Skill", "SKILL", "prompt"];
                if skill_keywords.iter().any(|keyword| content_str.contains(keyword)) {
                    return Ok(HookResult::Warn {
                        message: "技能檔案/代碼修改後，請運行 `cargo test -p buzz-agent` 確保沒有問題".to_string(),
                    });
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
