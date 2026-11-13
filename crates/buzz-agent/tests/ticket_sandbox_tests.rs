// DeepSeek 式工單目錄隔離單元測試
use buzz_agent::ticket_sandbox::TicketSandbox;
use std::fs;

#[test]
fn test_ticket_sandbox_sanitize_and_creation() {
    let temp_root = std::env::temp_dir().join(format!("buzz_test_{}", uuid_simple()));
    let raw_session = "agent:session/123*456?ticket";
    let sandbox = TicketSandbox::new(&temp_root, raw_session);

    assert_eq!(sandbox.session_id(), "agent_session_123_456_ticket");
    assert!(sandbox.sandbox_dir().ends_with("ticket_agent_session_123_456_ticket"));

    let created_dir = sandbox.create().expect("建立沙箱失敗");
    assert!(created_dir.exists());
    assert!(created_dir.is_dir());

    // 清理
    let _ = sandbox.cleanup();
    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_ticket_sandbox_path_traversal_prevention() {
    let temp_root = std::env::temp_dir().join(format!("buzz_test_{}", uuid_simple()));
    let sandbox = TicketSandbox::new(&temp_root, "session-safe-test");
    sandbox.create().unwrap();

    // 正常路徑解析
    let resolved = sandbox.resolve_path("src/foo/bar.rs").unwrap();
    assert!(resolved.starts_with(sandbox.sandbox_dir()));
    assert!(resolved.ends_with("src/foo/bar.rs"));

    // 嘗試路徑穿越 (..)
    let traversal = sandbox.resolve_path("../dangerous.rs");
    assert!(traversal.is_err(), "必須阻斷包含 .. 的路徑穿越");

    // 嘗試絕對路徑
    #[cfg(windows)]
    let absolute = sandbox.resolve_path("C:\\Windows\\System32");
    #[cfg(not(windows))]
    let absolute = sandbox.resolve_path("/etc/passwd");
    assert!(absolute.is_err(), "必須阻斷絕對路徑重定向");

    let _ = sandbox.cleanup();
    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_ticket_sandbox_write_read_and_sync_to_workspace() {
    let temp_root = std::env::temp_dir().join(format!("buzz_test_{}", uuid_simple()));
    let target_workspace = temp_root.join("target_workspace");
    fs::create_dir_all(&target_workspace).unwrap();

    let sandbox = TicketSandbox::new(&temp_root, "session-sync-test");
    sandbox.create().unwrap();

    // 在沙箱寫入檔案
    let content = b"fn main() { println!(\"Hello Sandbox\"); }";
    sandbox.write_file("src/main.rs", content).expect("寫入沙箱失敗");
    sandbox.write_file("docs/README.md", b"# Sandbox Doc").expect("寫入文檔失敗");

    // 讀取驗證
    let read_back = sandbox.read_file("src/main.rs").expect("讀取失敗");
    assert_eq!(read_back, content);

    // 原子合流至工作區
    let synced = sandbox.sync_to_workspace(&target_workspace).expect("合流失敗");
    assert_eq!(synced.len(), 2);
    assert!(target_workspace.join("src/main.rs").exists());
    assert!(target_workspace.join("docs/README.md").exists());
    assert_eq!(fs::read(target_workspace.join("src/main.rs")).unwrap(), content);

    // 清理沙箱，確認工作區檔案不受影響
    sandbox.cleanup().expect("清理失敗");
    assert!(!sandbox.sandbox_dir().exists());
    assert!(target_workspace.join("src/main.rs").exists());

    let _ = fs::remove_dir_all(&temp_root);
}

fn uuid_simple() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

