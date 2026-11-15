//! 整合測試：驗證四層上下文壓縮（Tiered 80% 門檻、D6 守衛提示、Legacy 回退）與階段 E 跨進程 AES-256-GCM 加密 Checkpoint 續作。

mod common;

use std::time::Duration;

use common::{openai_text, spawn_capturing_llm, Harness};
use serde_json::{json, Value};
use tempfile::tempdir;

fn openai_text_with_usage(content: &str, prompt_tokens: u64) -> Value {
    json!({
        "id": "cc-usage",
        "object": "chat.completion",
        "model": "fake-model",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop",
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": 10,
            "total_tokens": prompt_tokens + 10,
        },
    })
}

async fn init_session_temp_cwd(h: &mut Harness) -> String {
    h.send(
        "initialize",
        json!({"protocolVersion": 1, "clientCapabilities": {}}),
    )
    .await;
    let _ = h.recv().await;
    let cwd = std::env::temp_dir().to_string_lossy().into_owned();
    h.send("session/new", json!({"cwd": cwd, "mcpServers": []}))
        .await;
    let r = h
        .recv_until(|v| v.get("result").is_some() || v.get("error").is_some())
        .await;
    r["result"]["sessionId"]
        .as_str()
        .unwrap_or_else(|| panic!("session/new failed: {r}; stderr={}", h.stderr_text()))
        .to_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tiered_handoff_at_80_percent_updates_continuation_note() {
    let llm = spawn_capturing_llm(vec![
        openai_text_with_usage("round-1-done", 820),
        openai_text("rolling-summary-alpha"),
        openai_text_with_usage("round-2-done", 20),
    ])
    .await;
    let cp_dir = tempdir().unwrap();
    let cp_dir_s = cp_dir.path().to_string_lossy().into_owned();

    let mut h = Harness::spawn_with_env(
        &llm.url,
        &[
            ("BUZZ_AGENT_COMPACTION_MODE", "tiered"),
            ("BUZZ_AGENT_MAX_CONTEXT_TOKENS", "1000"),
            ("BUZZ_AGENT_HANDOFF_THRESHOLD_PERCENT", "80"),
            ("BUZZ_AGENT_MAX_HANDOFFS", "2"),
            ("BUZZ_AGENT_CHECKPOINT_DIR", &cp_dir_s),
        ],
    )
    .await;

    let sid = init_session_temp_cwd(&mut h).await;

    let p1 = h
        .send(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "step 1"}]}),
        )
        .await;
    let r1 = h.recv_until(|v| v["id"] == json!(p1)).await;
    assert!(r1.get("result").is_some(), "p1 failed: {r1}");

    let p2 = h
        .send(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "step 2"}]}),
        )
        .await;
    let r2 = h.recv_until(|v| v["id"] == json!(p2)).await;
    assert!(r2.get("result").is_some(), "p2 failed: {r2}");

    let stderr = h
        .wait_for_stderr("handoff #1", Duration::from_secs(5))
        .await;
    assert!(stderr.contains("handoff #1"));

    let captured = llm.captured.lock().await;
    assert_eq!(captured.len(), 3, "expected turn1 + summarize + turn2");
    let post_handoff_msgs = captured[2]["messages"].as_array().unwrap();
    let joined = serde_json::to_string(post_handoff_msgs).unwrap();
    assert!(joined.contains("rolling-summary-alpha"));
    assert!(
        joined.contains("continue executing the next step from the summary"),
        "expected D6 continuation instruction, got: {joined}"
    );
    assert!(
        !joined.contains("Conclude your turn promptly."),
        "D6 prompt must not instruct early stop: {joined}"
    );

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_mode_keeps_90_percent_threshold_unchanged() {
    let llm = spawn_capturing_llm(vec![
        openai_text_with_usage("round-1-done", 820),
        openai_text_with_usage("round-2-done", 20),
    ])
    .await;

    let mut h = Harness::spawn_with_env(
        &llm.url,
        &[
            ("BUZZ_AGENT_COMPACTION_MODE", "legacy"),
            ("BUZZ_AGENT_MAX_CONTEXT_TOKENS", "1000"),
            ("BUZZ_AGENT_MAX_HANDOFFS", "2"),
        ],
    )
    .await;

    let sid = init_session_temp_cwd(&mut h).await;
    let p1 = h
        .send(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "step 1"}]}),
        )
        .await;
    let _ = h.recv_until(|v| v["id"] == json!(p1)).await;

    let p2 = h
        .send(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "step 2"}]}),
        )
        .await;
    let _ = h.recv_until(|v| v["id"] == json!(p2)).await;

    let captured = llm.captured.lock().await;
    assert_eq!(
        captured.len(),
        2,
        "820/1000 must NOT handoff in legacy 90% mode"
    );
    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_process_checkpoint_restores_context_after_agent_restart() {
    let cp_dir = tempdir().unwrap();
    let cp_dir_s = cp_dir.path().to_string_lossy().into_owned();
    let thread_hex = "1".repeat(64);
    let ctx_header = format!(
        "<context>\nChannel: ops (#5ca019f6-1a27-592a-b015-4baf0eb33572)\nThread root: {thread_hex}\n</context>\n"
    );

    // 子進程 1：完成第一回合任務後結束進程並留下 AES-256-GCM 加密存檔
    let llm1 = spawn_capturing_llm(vec![openai_text("PHASE_1_FINDING_ALPHA")]).await;
    {
        let mut h1 = Harness::spawn_with_env(
            &llm1.url,
            &[
                ("BUZZ_AGENT_CHECKPOINT_ENABLED", "true"),
                ("BUZZ_AGENT_CHECKPOINT_DIR", &cp_dir_s),
                ("BUZZ_AGENT_CHECKPOINT_KEY", "integration-test-key"),
            ],
        )
        .await;
        let sid1 = init_session_temp_cwd(&mut h1).await;
        let p1 = h1
            .send(
                "session/prompt",
                json!({
                    "sessionId": sid1,
                    "prompt": [{"type": "text", "text": format!("{ctx_header}開始階段一調查")}]
                }),
            )
            .await;
        let r1 = h1.recv_until(|v| v["id"] == json!(p1)).await;
        assert!(r1.get("result").is_some(), "child 1 failed: {r1}");
        h1.shutdown().await;
    }

    // 確認磁碟已寫入密文 .chk，且明文不含敏感回答字樣
    let chk_files: Vec<_> = std::fs::read_dir(cp_dir.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("chk"))
        .collect();
    assert_eq!(chk_files.len(), 1, "expected 1 encrypted checkpoint file");
    let cipher_bytes = std::fs::read(&chk_files[0]).unwrap();
    assert!(cipher_bytes.starts_with(b"BZCP01"));
    assert!(!String::from_utf8_lossy(&cipher_bytes).contains("PHASE_1_FINDING_ALPHA"));

    // 子進程 2：全新進程 + 全新 session/new，應自動還原先前上下文
    let llm2 = spawn_capturing_llm(vec![openai_text("PHASE_2_CONTINUED_OK")]).await;
    let mut h2 = Harness::spawn_with_env(
        &llm2.url,
        &[
            ("BUZZ_AGENT_CHECKPOINT_ENABLED", "true"),
            ("BUZZ_AGENT_CHECKPOINT_DIR", &cp_dir_s),
            ("BUZZ_AGENT_CHECKPOINT_KEY", "integration-test-key"),
        ],
    )
    .await;
    let sid2 = init_session_temp_cwd(&mut h2).await;
    let p2 = h2
        .send(
            "session/prompt",
            json!({
                "sessionId": sid2,
                "prompt": [{"type": "text", "text": format!("{ctx_header}繼續第二階段")}]
            }),
        )
        .await;
    let r2 = h2.recv_until(|v| v["id"] == json!(p2)).await;
    assert!(r2.get("result").is_some(), "child 2 failed: {r2}");

    let _ = h2
        .wait_for_stderr(
            "restored encrypted session checkpoint from disk",
            Duration::from_secs(5),
        )
        .await;

    let captured2 = llm2.captured.lock().await;
    let serialized = serde_json::to_string(&captured2[0]["messages"]).unwrap();
    assert!(
        serialized.contains("PHASE_1_FINDING_ALPHA"),
        "restarted session must see restored phase 1 context, got: {serialized}"
    );
    h2.shutdown().await;
}
