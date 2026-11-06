// Hardcoded security rule - detects real hardcoded secrets in tool parameters
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct HardcodedBlockRule;

#[async_trait]
impl Rule for HardcodedBlockRule {
    fn id(&self) -> &'static str {
        "hardcoded-block"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        if !ctx.matches_tool(&["write_file", "write", "edit_file", "edit", "str_replace", "Write", "Edit"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(content_str) = ctx.get_str_arg(&["content", "replacement", "Content"]) {
            // 掃描高危真實 API Key / Private Key 簽名模式
            let secret_signatures = [
                ("sk-proj-", "OpenAI Project API Key"),
                ("sk-or-v1-", "OpenRouter API Key"),
                ("AIzaSy", "Google Gemini API Key"),
                ("ghp_", "GitHub Personal Access Token"),
                ("nsec1", "Nostr Private Key (NIP-19)"),
                ("-----BEGIN OPENSSH PRIVATE KEY-----", "SSH Private Key"),
                ("-----BEGIN RSA PRIVATE KEY-----", "RSA Private Key"),
                ("-----BEGIN PRIVATE KEY-----", "PKCS8 Private Key"),
                ("-----BEGIN EC PRIVATE KEY-----", "EC Private Key"),
            ];

            for (sig, desc) in &secret_signatures {
                if content_str.contains(sig) {
                    return Ok(HookResult::Block {
                        reason: format!(
                            "【PreToolUse 安全門禁攔截】代碼中檢測到明文硬編碼敏感金鑰 ({}: 匹配到 `{}`)。請改由 .env 或環境變數載入，禁止直接寫入原始碼！",
                            desc, sig
                        ),
                    });
                }
            }
        }

        Ok(HookResult::Continue)
    }
}
