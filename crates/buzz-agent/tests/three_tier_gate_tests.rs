// Mika / dsh-Go 三檔安全門禁單元測試
use buzz_agent::hooks::rules::three_tier_gate::{OperationTier, ThreeTierGateRule};
use buzz_agent::hooks::rules::Rule;
use buzz_agent::hooks::types::{HookContext, HookResult, HookTrigger};
use serde_json::json;
use std::path::PathBuf;

fn create_test_ctx(tool: &str, args: serde_json::Value) -> HookContext {
    HookContext {
        tool_name: tool.to_string(),
        tool_arguments: args,
        session_id: "test-session-123".to_string(),
        working_dir: PathBuf::from("."),
        trigger_event: HookTrigger::PreToolUse,
    }
}

#[tokio::test]
async fn test_readonly_tier_classification_and_pass() {
    let rule = ThreeTierGateRule;

    // 1. 純唯讀工具
    let ctx_read = create_test_ctx("read_file", json!({ "path": "src/lib.rs" }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_read);
    assert_eq!(tier, OperationTier::ReadOnly);
    let res = rule.check(&ctx_read).await.unwrap();
    assert!(matches!(res, HookResult::Continue));

    // 2. 目錄清單
    let ctx_list = create_test_ctx("list_dir", json!({ "path": "." }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_list);
    assert_eq!(tier, OperationTier::ReadOnly);

    // 3. 唯讀 Shell 指令
    let ctx_git_status = create_test_ctx("shell", json!({ "command": "git status -s" }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_git_status);
    assert_eq!(tier, OperationTier::ReadOnly);
    let res = rule.check(&ctx_git_status).await.unwrap();
    assert!(matches!(res, HookResult::Continue));
}

#[tokio::test]
async fn test_restricted_tier_classification_and_pass() {
    let rule = ThreeTierGateRule;

    // 常規寫入非敏感代碼
    let ctx_write = create_test_ctx("write_file", json!({ "path": "src/utils.rs", "content": "pub fn test() {}" }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_write);
    assert_eq!(tier, OperationTier::Restricted);
    let res = rule.check(&ctx_write).await.unwrap();
    assert!(matches!(res, HookResult::Continue));

    // 一般 Shell 構建指令
    let ctx_cargo = create_test_ctx("shell", json!({ "command": "cargo check --tests" }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_cargo);
    assert_eq!(tier, OperationTier::Restricted);
    let res = rule.check(&ctx_cargo).await.unwrap();
    assert!(matches!(res, HookResult::Continue));
}

#[tokio::test]
async fn test_elevated_tier_destructive_command_blocked_without_auth() {
    let rule = ThreeTierGateRule;

    // 測試危險刪除指令
    let ctx_rm = create_test_ctx("shell", json!({ "command": "rm -rf /" }));
    let (tier, detail) = ThreeTierGateRule::classify(&ctx_rm);
    assert_eq!(tier, OperationTier::Elevated);
    assert!(detail.is_some());

    let res = rule.check(&ctx_rm).await.unwrap();
    match res {
        HookResult::Block { reason } => {
            assert!(reason.contains("Mika 三檔安全門禁攔截"));
            assert!(reason.contains("Tier 3"));
        }
        _ => panic!("Expected Block for destructive command, got {:?}", res),
    }

    // 測試 git reset --hard
    let ctx_reset = create_test_ctx("powershell", json!({ "command": "git reset --hard HEAD~1" }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_reset);
    assert_eq!(tier, OperationTier::Elevated);
    let res = rule.check(&ctx_reset).await.unwrap();
    assert!(matches!(res, HookResult::Block { .. }));
}

#[tokio::test]
async fn test_elevated_tier_sensitive_file_blocked_without_auth() {
    let rule = ThreeTierGateRule;

    // 嘗試寫入 .env
    let ctx_env = create_test_ctx("write_file", json!({ "path": "c:/app/.env", "content": "SECRET=123" }));
    let (tier, detail) = ThreeTierGateRule::classify(&ctx_env);
    assert_eq!(tier, OperationTier::Elevated);
    assert!(detail.is_some());

    let res = rule.check(&ctx_env).await.unwrap();
    assert!(matches!(res, HookResult::Block { .. }));

    // 嘗試修改 managed-agents.json
    let ctx_agents = create_test_ctx("edit_file", json!({ "path": "config/managed-agents.json" }));
    let (tier, _) = ThreeTierGateRule::classify(&ctx_agents);
    assert_eq!(tier, OperationTier::Elevated);
    let res = rule.check(&ctx_agents).await.unwrap();
    assert!(matches!(res, HookResult::Block { .. }));
}

#[tokio::test]
async fn test_elevated_tier_passes_with_authorization() {
    let rule = ThreeTierGateRule;

    // 帶有 elevated_authorized: true
    let ctx_auth = create_test_ctx(
        "shell",
        json!({
            "command": "git reset --hard",
            "elevated_authorized": true
        }),
    );
    let (tier, _) = ThreeTierGateRule::classify(&ctx_auth);
    assert_eq!(tier, OperationTier::Elevated);

    let res = rule.check(&ctx_auth).await.unwrap();
    match res {
        HookResult::Warn { message } => {
            assert!(message.contains("已確認合法授權"));
        }
        _ => panic!("Expected Warn for authorized elevated action, got {:?}", res),
    }
}

