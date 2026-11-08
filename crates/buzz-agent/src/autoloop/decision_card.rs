//! 決定權分離互動卡片 (Decision Card)
//! 當特工執行高危操作（如刪除、覆蓋非沙箱目錄、自信度低於閾值）時，
//! 暫停自洽迴圈，發送「拍板卡片」至主理人私訊頻道，待主理人授權確認後才放行。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CardRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DecisionCardStatus {
    PendingApproval,
    Approved,
    Rejected,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionCard {
    pub card_id: String,
    pub task_id: String,
    pub agent_name: String,
    pub action_title: String,
    pub action_description: String,
    pub target_paths: Vec<String>,
    pub risk_level: CardRiskLevel,
    pub confidence_score: f32,
    pub status: DecisionCardStatus,
    pub created_at: u64,
}

impl DecisionCard {
    pub fn new(
        task_id: &str,
        agent_name: &str,
        action_title: &str,
        action_description: &str,
        target_paths: Vec<String>,
        risk_level: CardRiskLevel,
        confidence_score: f32,
    ) -> Self {
        let card_id = format!(
            "card_{}_{}",
            task_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        Self {
            card_id,
            task_id: task_id.to_string(),
            agent_name: agent_name.to_string(),
            action_title: action_title.to_string(),
            action_description: action_description.to_string(),
            target_paths,
            risk_level,
            confidence_score,
            status: DecisionCardStatus::PendingApproval,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// 判斷是否需要強制暫停特工迴圈，向主理人尋求拍板
    pub fn requires_escalation(&self) -> bool {
        self.risk_level == CardRiskLevel::High
            || self.risk_level == CardRiskLevel::Critical
            || self.confidence_score < 0.70
    }

    /// 授權批准
    pub fn approve(&mut self) {
        self.status = DecisionCardStatus::Approved;
    }

    /// 駁回授權
    pub fn reject(&mut self) {
        self.status = DecisionCardStatus::Rejected;
    }
}

