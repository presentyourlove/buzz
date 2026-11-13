// unslop 代碼美學與去 AI 味審查門禁單元測試
use buzz_agent::hooks::rules::unslop_guard::UnslopGuardRule;
use buzz_agent::hooks::rules::Rule;
use buzz_agent::hooks::types::{HookContext, HookResult, HookTrigger};
use serde_json::json;
use std::path::PathBuf;

fn create_test_ctx(tool: &str, args: serde_json::Value) -> HookContext {
    HookContext {
        tool_name: tool.to_string(),
        tool_arguments: args,
        session_id: "test-unslop-session".to_string(),
        working_dir: PathBuf::from("."),
        trigger_event: HookTrigger::PreToolUse,
    }
}

#[tokio::test]
async fn test_clean_code_passes() {
    let rule = UnslopGuardRule;

    let clean_code = r#"
        export function calculateTotal(items: CartItem[]): number {
            return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
        }
    "#;

    let ctx = create_test_ctx("write_file", json!({
        "path": "src/cart.ts",
        "content": clean_code
    }));

    let res = rule.check(&ctx).await.unwrap();
    assert!(matches!(res, HookResult::Continue), "乾淨業務代碼應順利放行");
}

#[tokio::test]
async fn test_severe_slop_todo_blocked() {
    let rule = UnslopGuardRule;

    let slop_code = r#"
        export function processPayment(paymentInfo: any) {
            // TODO: implement later
            return true;
        }
    "#;

    let ctx = create_test_ctx("write_file", json!({
        "path": "src/payment.ts",
        "content": slop_code
    }));

    let res = rule.check(&ctx).await.unwrap();
    match res {
        HookResult::Block { reason } => {
            assert!(reason.contains("Unslop 代碼清潔門禁攔截"));
            assert!(reason.contains("TODO: implement later"));
        }
        _ => panic!("Expected Block for severe TODO slop, got {:?}", res),
    }
}

#[tokio::test]
async fn test_severe_slop_placeholder_blocked() {
    let rule = UnslopGuardRule;

    let slop_code = r#"
        export const fetchUserData = async (userId: string) => {
            /* Placeholder for actual logic */
            return { id: userId, name: "Mock User" };
        };
    "#;

    let ctx = create_test_ctx("edit_file", json!({
        "path": "src/api/user.ts",
        "replacement": slop_code
    }));

    let res = rule.check(&ctx).await.unwrap();
    match res {
        HookResult::Block { reason } => {
            assert!(reason.contains("Unslop 代碼清潔門禁攔截"));
            assert!(reason.contains("虛假邏輯佔位符"));
        }
        _ => panic!("Expected Block for placeholder slop, got {:?}", res),
    }
}

#[tokio::test]
async fn test_aesthetic_slop_gradient_warned() {
    let rule = UnslopGuardRule;

    let slop_ui = r#"
        <div className="bg-gradient-to-r from-purple-500 to-indigo-600 shadow-purple-500 p-4">
            <h1 className="text-white text-xl">Vibecoded Dashboard</h1>
        </div>
    "#;

    let ctx = create_test_ctx("write_file", json!({
        "path": "components/Header.tsx",
        "content": slop_ui
    }));

    let res = rule.check(&ctx).await.unwrap();
    match res {
        HookResult::Warn { message } => {
            assert!(message.contains("Unslop 代碼美學建議"));
            assert!(message.contains("典型 AI 劣質紫色霓虹漸層"));
        }
        _ => panic!("Expected Warn for aesthetic slop, got {:?}", res),
    }
}

#[tokio::test]
async fn test_non_write_tool_ignored() {
    let rule = UnslopGuardRule;

    // Shell 指令中即使有 TODO 字樣也不被 UnslopGuardRule 阻擋
    let ctx_shell = create_test_ctx("shell", json!({
        "command": "git grep 'TODO: implement later'"
    }));

    let res = rule.check(&ctx_shell).await.unwrap();
    assert!(matches!(res, HookResult::Continue));
}

