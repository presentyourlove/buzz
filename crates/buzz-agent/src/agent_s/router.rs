//! Agent-S 特工動態意圖路由決策器 (AgentRouter)
//!
//! 根據任務文字描述提取特徵與意圖，對照專家特工領域矩陣計算匹配度，
//! 輸出推薦特工與置信度理由。

use serde::{Deserialize, Serialize};

/// 專家特工角色列舉
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialistRole {
    Pollen,  // 代碼工程、編譯除錯、架構實作
    Sentry,  // 安全審核、權限門禁、防破壞與漂移檢查
    Wax,     // 長期記憶、Obsidian 雙腦同步、對話摘要
    Honey2,  // 架構企劃、需求拆解、NIP 協議定義
    Fizz,    // 通用助理、日常問答、定時提醒
}

impl SpecialistRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecialistRole::Pollen => "Pollen",
            SpecialistRole::Sentry => "Sentry",
            SpecialistRole::Wax => "Wax",
            SpecialistRole::Honey2 => "Honey2",
            SpecialistRole::Fizz => "Fizz",
        }
    }
}

/// 路由決策結果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingDecision {
    pub recommended_agent: String,
    pub confidence: f32,
    pub matched_intent: String,
    pub reasoning: String,
    pub fallback_agent: String,
}

/// 特工動態路由器
#[derive(Debug, Default, Clone)]
pub struct AgentRouter;

impl AgentRouter {
    pub fn new() -> Self {
        Self
    }

    /// 根據任務描述進行特工動態路由
    pub fn route_task(&self, task_description: &str) -> RoutingDecision {
        let task_lower = task_description.to_lowercase();

        // 領域特徵關鍵字庫
        let code_keywords = [
            "代碼", "程式", "rust", "python", "node", "實作", "編譯", "debug",
            "compile", "test", "單元測試", "crate", "修復", "bug", "函數",
        ];
        let security_keywords = [
            "安全", "審核", "門禁", "權限", "漏洞", "阻斷", "防護", "漂移",
            "三檔門禁", "security", "guard", "audit", "verify",
        ];
        let memory_keywords = [
            "記憶", "摘要", "obsidian", "雙腦", "歸檔", "digest", "同步",
            "知識庫", "wiki", "歷史記錄", "log", "冷記憶",
        ];
        let planning_keywords = [
            "企劃", "計畫", "規劃", "拆解", "nip", "協議", "藍圖", "todo",
            "roadmap", "架構設計", "proposal",
        ];

        // 計算各專家分數
        let mut pollen_score = 0.0f32;
        let mut sentry_score = 0.0f32;
        let mut wax_score = 0.0f32;
        let mut honey2_score = 0.0f32;

        for kw in &code_keywords {
            if task_lower.contains(kw) {
                pollen_score += 1.0;
            }
        }

        for kw in &security_keywords {
            if task_lower.contains(kw) {
                sentry_score += 1.2; // 安全權重略高
            }
        }

        for kw in &memory_keywords {
            if task_lower.contains(kw) {
                wax_score += 1.0;
            }
        }

        for kw in &planning_keywords {
            if task_lower.contains(kw) {
                honey2_score += 1.0;
            }
        }

        // 決策排名
        let mut scores = vec![
            (SpecialistRole::Pollen, pollen_score, "軟體工程與代碼實作"),
            (SpecialistRole::Sentry, sentry_score, "安全架構與門禁審核"),
            (SpecialistRole::Wax, wax_score, "知識記憶與雙腦同步"),
            (SpecialistRole::Honey2, honey2_score, "系統企劃與協議規劃"),
        ];

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (top_role, top_score, intent_desc) = scores[0];

        if top_score > 0.0 {
            let confidence = (top_score / (top_score + 1.5)).clamp(0.65, 0.98);
            RoutingDecision {
                recommended_agent: top_role.as_str().to_string(),
                confidence,
                matched_intent: intent_desc.to_string(),
                reasoning: format!(
                    "依據任務特徵高度符合 {} 領域（匹配權重: {:.1}）",
                    top_role.as_str(),
                    top_score
                ),
                fallback_agent: scores[1].0.as_str().to_string(),
            }
        } else {
            // 無專門技術領域關鍵字，預設路由給通用助理 Fizz
            RoutingDecision {
                recommended_agent: SpecialistRole::Fizz.as_str().to_string(),
                confidence: 0.60,
                matched_intent: "日常通用助理與對話查詢".to_string(),
                reasoning: "未偵測到深度專業領域關鍵字，由通用特工 Fizz 提供接待與引導".to_string(),
                fallback_agent: SpecialistRole::Honey2.as_str().to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_router_intent_matching() {
        let router = AgentRouter::new();

        // 1. 測試工程實作意圖 -> Pollen
        let d1 = router.route_task("請幫我撰寫 Rust 代碼並跑單元測試修復 bug");
        assert_eq!(d1.recommended_agent, "Pollen");
        assert!(d1.confidence >= 0.7);

        // 2. 測試安全審核意圖 -> Sentry
        let d2 = router.route_task("審核這段 PR 是否有越權路徑安全漏洞與架構漂移");
        assert_eq!(d2.recommended_agent, "Sentry");
        assert!(d2.confidence >= 0.7);

        // 3. 測試記憶歸檔意圖 -> Wax
        let d3 = router.route_task("將昨天的對話摘要同步並歸檔至 Obsidian 第二大腦 wiki");
        assert_eq!(d3.recommended_agent, "Wax");
        assert!(d3.confidence >= 0.7);

        // 4. 測試企劃拆解意圖 -> Honey2
        let d4 = router.route_task("請規劃下一階段的 NIP 協議擴充架構藍圖與 todo 計畫");
        assert_eq!(d4.recommended_agent, "Honey2");
        assert!(d4.confidence >= 0.7);

        // 5. 測試通用閒聊/非技術查詢 -> Fizz
        let d5 = router.route_task("你好，今天天氣如何？");
        assert_eq!(d5.recommended_agent, "Fizz");
        assert_eq!(d5.fallback_agent, "Honey2");
    }
}

