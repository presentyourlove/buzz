//! AnyModelBot 方案 B：Human-in-the-loop 四級漸進式權限確認與審批橋接
//!
//! 包含四大核心防護：
//! 1. `risk_tier`：四級漸進式風險模型 (Level 1 Low 至 Level 4 Critical)
//! 2. `delete_gate`：amb-delete-gate 遞歸刪除防破壞門禁 (非沙箱路徑嚴格禁止)
//! 3. `computer_use_gate`：amb-computer-use-gate 桌面鍵鼠操作前置門禁 (攔截高危按鍵)
//! 4. `approval_bridge`：amb-bridge 審批狀態機與持久授權 (TTL 10min)

pub mod risk_tier;
pub mod delete_gate;
pub mod computer_use_gate;
pub mod approval_bridge;

pub use risk_tier::{RiskEvaluator, RiskTier};
pub use delete_gate::{DeleteGate, DeleteGateResult};
pub use computer_use_gate::{ComputerUseGate, ComputerUseGateResult};
pub use approval_bridge::{ApprovalBridge, ApprovalRequest, ApprovalStatus, PersistentGrant};

