//! AnyModelBot 方案 B：amb-bridge 審批狀態機與持久授權
//!
//! 解耦 Agent Runtime 與確認通知通道，提供帶有 10 分鐘 TTL 的審批佇列，
//! 並維護持久化授權記憶，避免重複彈窗干擾主理人。

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::risk_tier::RiskTier;

/// 審批狀態列舉
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::Pending => write!(f, "等待審批 (Pending)"),
            ApprovalStatus::Approved => write!(f, "已批准 (Approved)"),
            ApprovalStatus::Rejected => write!(f, "已駁回 (Rejected)"),
            ApprovalStatus::Expired => write!(f, "超時失效 (Expired)"),
        }
    }
}

/// 審批請求實體
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalRequest {
    pub id: String,
    pub agent_id: String,
    pub risk_tier: RiskTier,
    pub action_signature: String,
    pub description: String,
    pub requested_at: u64,
    pub expires_at: u64,
    pub status: ApprovalStatus,
}

/// 持久化授權記錄
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistentGrant {
    pub agent_id: String,
    pub action_signature: String,
    pub granted_at: u64,
    pub expires_at: u64,
}

/// 審批橋接狀態機
#[derive(Debug, Default, Clone)]
pub struct ApprovalBridge {
    default_ttl_seconds: u64,
    requests: HashMap<String, ApprovalRequest>,
    persistent_grants: HashMap<String, PersistentGrant>,
}

impl ApprovalBridge {
    pub fn new() -> Self {
        Self {
            default_ttl_seconds: 600, // 預設 10 分鐘 TTL
            requests: HashMap::new(),
            persistent_grants: HashMap::new(),
        }
    }

    /// 建立新的審批請求
    pub fn create_request(
        &mut self,
        request_id: &str,
        agent_id: &str,
        risk_tier: RiskTier,
        action_signature: &str,
        description: &str,
        now: u64,
    ) -> ApprovalRequest {
        let req = ApprovalRequest {
            id: request_id.to_string(),
            agent_id: agent_id.to_string(),
            risk_tier,
            action_signature: action_signature.to_string(),
            description: description.to_string(),
            requested_at: now,
            expires_at: now + self.default_ttl_seconds,
            status: ApprovalStatus::Pending,
        };

        self.requests.insert(request_id.to_string(), req.clone());
        req
    }

    /// 批准請求 (可選持久記住授權)
    pub fn approve(&mut self, request_id: &str, remember_duration_secs: Option<u64>, now: u64) -> Result<(), String> {
        let req = self.requests.get_mut(request_id).ok_or_else(|| "審批單號不存在".to_string())?;

        if now > req.expires_at {
            req.status = ApprovalStatus::Expired;
            return Err("審批請求已過期 (TTL 10min 超時)".to_string());
        }

        req.status = ApprovalStatus::Approved;

        // 若啟用持久授權記憶
        if let Some(duration) = remember_duration_secs {
            let grant_key = format!("{}:{}", req.agent_id, req.action_signature);
            self.persistent_grants.insert(
                grant_key,
                PersistentGrant {
                    agent_id: req.agent_id.clone(),
                    action_signature: req.action_signature.clone(),
                    granted_at: now,
                    expires_at: now + duration,
                },
            );
        }

        Ok(())
    }

    /// 駁回請求
    pub fn reject(&mut self, request_id: &str, now: u64) -> Result<(), String> {
        let req = self.requests.get_mut(request_id).ok_or_else(|| "審批單號不存在".to_string())?;
        if now > req.expires_at {
            req.status = ApprovalStatus::Expired;
            return Err("審批請求已過期".to_string());
        }

        req.status = ApprovalStatus::Rejected;
        Ok(())
    }

    /// 檢查是否已經具備有效授權 (透過持久記憶或即時審批)
    pub fn is_authorized(&self, agent_id: &str, action_signature: &str, now: u64) -> bool {
        let grant_key = format!("{}:{}", agent_id, action_signature);
        if let Some(grant) = self.persistent_grants.get(&grant_key) {
            if now <= grant.expires_at {
                return true;
            }
        }
        false
    }

    /// 撤銷指定特工的特定動作授權
    pub fn revoke_authorization(&mut self, agent_id: &str, action_signature: &str) -> bool {
        let grant_key = format!("{}:{}", agent_id, action_signature);
        self.persistent_grants.remove(&grant_key).is_some()
    }

    /// 清理所有過期請求與授權
    pub fn clean_expired(&mut self, now: u64) {
        for req in self.requests.values_mut() {
            if req.status == ApprovalStatus::Pending && now > req.expires_at {
                req.status = ApprovalStatus::Expired;
            }
        }

        self.persistent_grants.retain(|_, grant| now <= grant.expires_at);
    }

    /// 查詢當前有效持久授權清單
    pub fn list_active_grants(&self, now: u64) -> Vec<&PersistentGrant> {
        self.persistent_grants
            .values()
            .filter(|g| now <= g.expires_at)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_bridge_lifecycle_and_timeout() {
        let mut bridge = ApprovalBridge::new();
        let now = 1794120000;

        // 1. 建立審批請求
        let req = bridge.create_request(
            "AP-101",
            "Pollen",
            RiskTier::High,
            "cargo:update",
            "更新專案依賴套件",
            now,
        );
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert_eq!(req.expires_at, now + 600);

        // 2. 測試 10 分鐘內批准
        let res = bridge.approve("AP-101", None, now + 100);
        assert!(res.is_ok());
        assert_eq!(bridge.requests.get("AP-101").unwrap().status, ApprovalStatus::Approved);

        // 3. 建立第二筆並測試逾時過期
        bridge.create_request(
            "AP-102",
            "Pollen",
            RiskTier::High,
            "git:push-force",
            "強制推播分支",
            now,
        );
        let expired_res = bridge.approve("AP-102", None, now + 601);
        assert!(expired_res.is_err());
        assert_eq!(bridge.requests.get("AP-102").unwrap().status, ApprovalStatus::Expired);
    }

    #[test]
    fn test_persistent_authorization_grant_and_revoke() {
        let mut bridge = ApprovalBridge::new();
        let now = 1794120000;

        // 尚未授權
        assert!(!bridge.is_authorized("Honey2", "network:external_fetch", now));

        // 建立並批准，記憶 3600 秒 (1小時)
        bridge.create_request(
            "AP-201",
            "Honey2",
            RiskTier::Medium,
            "network:external_fetch",
            "查詢外部 API 數據",
            now,
        );
        bridge.approve("AP-201", Some(3600), now).unwrap();

        // 檢查已獲授權
        assert!(bridge.is_authorized("Honey2", "network:external_fetch", now + 1800));
        assert_eq!(bridge.list_active_grants(now).len(), 1);

        // 手動撤銷授權
        assert!(bridge.revoke_authorization("Honey2", "network:external_fetch"));
        assert!(!bridge.is_authorized("Honey2", "network:external_fetch", now + 1800));
    }
}

