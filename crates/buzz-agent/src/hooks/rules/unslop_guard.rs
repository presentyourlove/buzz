// heydamir/unslop 式代碼美學與去 AI 味審查門禁 (Aesthetic & Unslop Reviewer)
// 偵測並剔除 AI 虛浮修飾、偷懶佔位符與冗餘樣板代碼
use crate::hooks::types::{HookContext, HookResult, HookTrigger};
use super::Rule;
use async_trait::async_trait;

pub struct UnslopGuardRule;

impl UnslopGuardRule {
    /// 取得寫入或修改的文字內容
    pub fn extract_content(ctx: &HookContext) -> Option<String> {
        ctx.get_str_arg(&["content", "Content", "replacement", "Replacement", "text", "code"])
    }

    /// 掃描嚴重打混佔位符（需直接阻斷）
    pub fn check_severe_slop(content: &str) -> Option<&'static str> {
        let lower = content.to_lowercase();
        let severe_patterns = [
            ("todo: implement later", "未完成之延後實作佔位符 (TODO: implement later)"),
            ("placeholder for actual", "虛假邏輯佔位符 (Placeholder for actual logic)"),
            ("mock data for now", "未替換之暫存假資料 (Mock data for now)"),
            ("insert actual implementation here", "未填充之空殼樣板代碼"),
            ("throw new error(\"not implemented\")", "未實作之拋出異常佔位"),
            ("throw new error('not implemented')", "未實作之拋出異常佔位"),
        ];

        for (pattern, desc) in severe_patterns {
            if lower.contains(pattern) {
                return Some(desc);
            }
        }
        None
    }

    /// 掃描 UI 審美疲勞 Slop（需給予警告）
    pub fn check_aesthetic_slop(content: &str) -> Option<&'static str> {
        let lower = content.to_lowercase();
        let aesthetic_patterns = [
            ("from-purple-500 to-indigo-600", "典型 AI 劣質紫色霓虹漸層 (from-purple-500 to-indigo-600)"),
            ("from-purple-600 to-pink-600", "典型 AI 紫粉色過度修飾漸層 (from-purple-600 to-pink-600)"),
            ("shadow-purple-500", "過度紫色發光陰影特效 (shadow-purple-500)"),
            ("glow-purple", "未經設計系統對齊之高光特效 (glow-purple)"),
        ];

        for (pattern, desc) in aesthetic_patterns {
            if lower.contains(pattern) {
                return Some(desc);
            }
        }
        None
    }
}

#[async_trait]
impl Rule for UnslopGuardRule {
    fn id(&self) -> &'static str {
        "unslop-aesthetic-guard"
    }

    fn trigger(&self) -> HookTrigger {
        HookTrigger::PreToolUse
    }

    async fn check(&self, ctx: &HookContext) -> Result<HookResult, Box<dyn std::error::Error>> {
        // 僅針對檔案寫入與內容替換工具進行檢驗
        if !ctx.matches_tool(&["write_file", "edit_file", "modify_file", "replace_file_content", "patch_file"]) {
            return Ok(HookResult::Continue);
        }

        if let Some(content) = Self::extract_content(ctx) {
            // 1. 優先檢查嚴重打混佔位符 ➔ 阻斷
            if let Some(reason) = Self::check_severe_slop(&content) {
                return Ok(HookResult::Block {
                    reason: format!(
                        "【Unslop 代碼清潔門禁攔截】檢測到代碼中包含 {}！\
                        Sentry 門禁已拒絕此項空殼寫入。請特工落實完整業務邏輯，禁止使用偷懶佔位符打混交付！",
                        reason
                    ),
                });
            }

            // 2. 檢查 UI 審美疲勞 Slop ➔ 警告
            if let Some(desc) = Self::check_aesthetic_slop(&content) {
                return Ok(HookResult::Warn {
                    message: format!(
                        "【Unslop 代碼美學建議】檢測到代碼包含 {}。建議遵循專案簡約設計規範，避免 AI 刻板樣板視覺！",
                        desc
                    ),
                });
            }
        }

        Ok(HookResult::Continue)
    }
}

