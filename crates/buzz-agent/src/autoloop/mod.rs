//! Agent-Autoloop 決定性自洽執行循環引擎 (參考 KKKKhazix/agent-autoloop 架構)
//! 
//! 特性：
//! 1. 純函式狀態機轉移，保證可重現性與零幽靈執行。
//! 2. 決定權分離：日常常規操作特工在背景自行跟進到底；高危不確定操作觸發拍板卡片由主理人拍板。
//! 3. 最大疊代保護，徹底杜絕無窮死迴圈。

use serde::{Deserialize, Serialize};
pub mod decision_card;
pub use decision_card::{CardRiskLevel, DecisionCard, DecisionCardStatus};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LoopState {
    Idle,
    Planning,
    Executing,
    AwaitingDecision,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoloopContext {
    pub task_id: String,
    pub agent_name: String,
    pub max_iterations: usize,
    pub current_iteration: usize,
    pub state: LoopState,
    pub pending_card: Option<DecisionCard>,
    pub logs: Vec<String>,
}

impl AutoloopContext {
    pub fn new(task_id: &str, agent_name: &str, max_iterations: usize) -> Self {
        Self {
            task_id: task_id.to_string(),
            agent_name: agent_name.to_string(),
            max_iterations,
            current_iteration: 0,
            state: LoopState::Idle,
            pending_card: None,
            logs: Vec::new(),
        }
    }

    /// 純函式狀態轉移：推進一輪執行
    pub fn step(&mut self, next_action_risk: CardRiskLevel, confidence: f32) -> LoopState {
        if self.current_iteration >= self.max_iterations {
            self.state = LoopState::Failed;
            self.logs.push(format!(
                "超過最大疊代次數上限 ({})，觸發自動熔斷防護。",
                self.max_iterations
            ));
            return self.state;
        }

        self.current_iteration += 1;

        // 若當前正等待主理人拍板
        if let Some(card) = &self.pending_card {
            match card.status {
                DecisionCardStatus::Approved => {
                    self.logs.push(format!("主理人已批准卡片 {}，恢復執行。", card.card_id));
                    self.pending_card = None;
                    self.state = LoopState::Executing;
                }
                DecisionCardStatus::Rejected => {
                    self.logs.push(format!("主理人駁回卡片 {}，安全終止任務。", card.card_id));
                    self.state = LoopState::Failed;
                    return self.state;
                }
                DecisionCardStatus::PendingApproval => {
                    self.state = LoopState::AwaitingDecision;
                    return self.state;
                }
                DecisionCardStatus::TimedOut => {
                    self.state = LoopState::Failed;
                    return self.state;
                }
            }
        }

        // 檢查下一動作是否需要向上陳報拍板
        let test_card = DecisionCard::new(
            &self.task_id,
            &self.agent_name,
            "檔案系統異動操作",
            "在非沙箱目錄執行寫入/刪除",
            vec![],
            next_action_risk,
            confidence,
        );

        if test_card.requires_escalation() {
            self.logs.push(format!(
                "動作風險等級 {:?} (自信度 {:.2}) 超出閾值，掛起迴圈並生成拍板卡片。",
                next_action_risk, confidence
            ));
            self.pending_card = Some(test_card);
            self.state = LoopState::AwaitingDecision;
        } else {
            self.logs.push(format!(
                "第 {} 輪常規操作已自動在背景推進執行。",
                self.current_iteration
            ));
            self.state = LoopState::Executing;
        }

        self.state
    }

    /// 標記任務正常完成
    pub fn mark_completed(&mut self) {
        self.state = LoopState::Completed;
        self.logs.push("特工長任務已自主推進到底並順利完工。".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autoloop_routine_execution() {
        let mut ctx = AutoloopContext::new("task_123", "Pollen", 5);
        let s = ctx.step(CardRiskLevel::Low, 0.95);
        assert_eq!(s, LoopState::Executing);
        assert_eq!(ctx.current_iteration, 1);
        ctx.mark_completed();
        assert_eq!(ctx.state, LoopState::Completed);
    }

    #[test]
    fn test_autoloop_escalation_card() {
        let mut ctx = AutoloopContext::new("task_456", "Honey2", 5);
        let s = ctx.step(CardRiskLevel::High, 0.90);
        assert_eq!(s, LoopState::AwaitingDecision);
        assert!(ctx.pending_card.is_some());

        // Approve card
        if let Some(card) = &mut ctx.pending_card {
            card.approve();
        }
        let s2 = ctx.step(CardRiskLevel::Low, 0.95);
        assert_eq!(s2, LoopState::Executing);
    }

    #[test]
    fn test_autoloop_max_iterations_guard() {
        let mut ctx = AutoloopContext::new("task_loop", "Sentry", 2);
        ctx.step(CardRiskLevel::Low, 0.95);
        ctx.step(CardRiskLevel::Low, 0.95);
        let s = ctx.step(CardRiskLevel::Low, 0.95);
        assert_eq!(s, LoopState::Failed);
    }
}
