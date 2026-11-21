//! `buzz pack` subcommands — local persona pack operations.
//!
//! These commands operate on local pack directories. No relay connection needed.

use std::path::Path;
use crate::error::CliError;

/// Run `buzz pack validate <path>`.
///
/// Calls `validate_pack()` from the persona crate, prints diagnostics,
/// and exits with the appropriate code:
/// - 0: valid (may have warnings)
/// - 1: errors found
pub fn cmd_validate(path: &str) -> Result<(), CliError> {
    let pack_dir = Path::new(path);
    if !pack_dir.exists() {
        return Err(CliError::Usage(format!("path does not exist: {path}")));
    }
    if !pack_dir.is_dir() {
        return Err(CliError::Usage(format!("not a directory: {path}")));
    }

    let report = buzz_persona::validate::validate_pack(pack_dir);

    for diag in &report.diagnostics {
        match diag {
            buzz_persona::validate::ValidationDiagnostic::Error(msg) => {
                eprintln!("  ERROR: {msg}");
            }
            buzz_persona::validate::ValidationDiagnostic::Warning(msg) => {
                eprintln!("  WARN:  {msg}");
            }
        }
    }

    if report.has_errors() {
        return Err(CliError::Usage("Validation failed.".into()));
    } else if report.has_warnings() {
        println!("Valid (with warnings).");
    } else {
        println!("Valid.");
    }

    Ok(())
}

/// Run `buzz pack inspect <path>`.
///
/// Loads and resolves a pack, then pretty-prints a summary of each persona's
/// effective configuration.
pub fn cmd_inspect(path: &str) -> Result<(), CliError> {
    let pack_dir = Path::new(path);
    if !pack_dir.exists() {
        return Err(CliError::Usage(format!("path does not exist: {path}")));
    }
    if !pack_dir.is_dir() {
        return Err(CliError::Usage(format!("not a directory: {path}")));
    }

    // Resolve the pack — shows fully effective config (post-merge, post-split).
    let pack = buzz_persona::resolve::resolve_pack(pack_dir)
        .map_err(|e| CliError::Other(format!("failed to resolve pack: {e}")))?;

    // Header
    println!("Pack: {} ({})", pack.name, pack.id);
    println!("Version: {}", pack.version);
    println!("Personas: {}", pack.personas.len());
    println!();

    // Per-persona summary (fully resolved effective config)
    for persona in &pack.personas {
        println!("  {}", persona.name);
        println!("    Display: {}", persona.display_name);
        println!("    Description: {}", persona.description);

        if let Some(ref llm_provider) = persona.llm_provider {
            if let Some(ref model) = persona.model {
                println!("    Model: {llm_provider}:{model}");
            } else {
                println!("    Provider: {llm_provider}");
            }
        } else if let Some(ref model) = persona.model {
            println!("    Model: {model}");
        }
        if let Some(temp) = persona.temperature {
            println!("    Temperature: {temp}");
        }
        if let Some(ctx) = persona.max_context_tokens {
            println!("    Max context tokens: {ctx}");
        }

        if !persona.subscribe.is_empty() {
            println!("    Subscribe: {}", persona.subscribe.join(", "));
        }

        let rt = &persona.triggers;
        let mut parts = Vec::new();
        if rt.mentions {
            parts.push("mentions".to_string());
        }
        if !rt.keywords.is_empty() {
            parts.push(format!("keywords {:?}", rt.keywords));
        }
        if rt.all_messages {
            parts.push("all_messages".to_string());
        }
        if !parts.is_empty() {
            println!("    Triggers: {}", parts.join(" + "));
        }

        println!("    Thread replies: {}", persona.thread_replies);
        println!("    Broadcast replies: {}", persona.broadcast_replies);

        if !persona.mcp_servers.is_empty() {
            println!("    MCP servers: {}", persona.mcp_servers.len());
        }

        if !persona.skills.is_empty() {
            println!("    Skills: {}", persona.skills.join(", "));
        }

        if let Some(ref avatar) = persona.avatar {
            println!("    Avatar: {avatar}");
        }

        let prompt_preview = if persona.system_prompt.chars().count() > 80 {
            let truncated: String = persona.system_prompt.chars().take(77).collect();
            format!("{truncated}...")
        } else {
            persona.system_prompt.clone()
        };
        println!(
            "    System prompt: {} chars ({})",
            persona.system_prompt.len(),
            prompt_preview.replace('\n', " ")
        );

        if !persona.runtime_env_vars.is_empty() {
            let env_str: Vec<String> = persona
                .runtime_env_vars
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            println!("    Env vars: {}", env_str.join(", "));
        }
        println!();
    }

    Ok(())
}

/// Run `buzz pack syntax-gate <args>` - Syntax Gate 前置語法稽核門禁
///
/// 依據 DESIGN_SPEC_B_TRIPLE_GATE.md §Syntax Gate 規格實作。
/// 驗證指令參數的語意與語法合法性，不向 Relay 廣播，純本地靜態檢查。
pub fn cmd_syntax_gate(args: &[String]) -> Result<(), CliError> {
    if args.is_empty() {
        return Err(CliError::Usage("syntax-gate requires command arguments to validate".to_string()));
    }

    println!("🔍 執行 Syntax Gate 語法靜態稽核: {:?}", args);

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--event" || arg == "--event-id" {
            if i + 1 < args.len() {
                let id = &args[i + 1];
                if !is_valid_event_id(id) {
                    return Err(CliError::Usage(format!("無效的 Event ID: '{}' (必須為 64 位元 Hex 字串)", id)));
                }
                i += 1;
            }
        } else if arg == "--author" || arg == "--pubkey" || arg == "--mention" || arg == "--assignee" {
            if i + 1 < args.len() {
                let pk = &args[i + 1];
                if !is_valid_pubkey(pk) {
                    return Err(CliError::Usage(format!("無效的公鑰 '{}' (必須為 64 位元 Hex 或 npub1...)", pk)));
                }
                i += 1;
            }
        } else if arg == "--channel" {
            if i + 1 < args.len() {
                let ch = &args[i + 1];
                if !is_valid_uuid(ch) {
                    return Err(CliError::Usage(format!("無效的 Channel UUID: '{}'", ch)));
                }
                i += 1;
            }
        } else if arg == "--content" || arg == "--json" {
            if i + 1 < args.len() {
                let content = &args[i + 1];
                if content.trim().starts_with('{') || content.trim().starts_with('[') {
                    if let Err(e) = serde_json::from_str::<serde_json::Value>(content) {
                        return Err(CliError::Usage(format!("JSON 內容語法錯誤: {e}")));
                    }
                }
                i += 1;
            }
        }
        i += 1;
    }

    println!("✅ Syntax Gate 驗證通過: 語意與參數合法");
    Ok(())
}

/// Run `buzz pack triple-gate` — Triple Gate (Syntax, Semantic, Fingerprint) 統一驗收與 Circuit Breaker 熔斷
///
/// 依據 DESIGN_SPEC_B_TRIPLE_GATE.md 與 Phase 56 後台架構進化規格實作。
/// 在 60 秒內完成三大閘門整合驗證，任一 Gate 失敗即刻觸發 Circuit Breaker 阻斷並可自動回滾工作樹。
pub async fn cmd_triple_gate(
    path: &str,
    timeout_secs: u64,
    auto_rollback: bool,
    expected_fingerprint: Option<&str>,
) -> Result<(), CliError> {
    println!("🛡️ 啟動 Triple Gate 三重驗收閘門管線...");
    println!("   目標路徑: {}", path);
    println!("   超時限制: {}s | 自動回滾: {}", timeout_secs, auto_rollback);

    let config = buzz_agent::gates::TripleGateConfig {
        repo_root: std::path::PathBuf::from(path),
        timeout_seconds: timeout_secs,
        auto_rollback,
        expected_fingerprint: expected_fingerprint.map(ToString::to_string),
    };

    let result = buzz_agent::gates::TripleGatePipeline::run(config).await;

    println!("\n📋 閘門檢驗報告 (總耗時: {}ms):", result.total_duration_ms);
    for step in &result.steps {
        let mark = if step.passed { "✅ PASS" } else { "❌ FAIL" };
        println!("   [{}] {} ({}ms)", mark, step.gate_name, step.duration_ms);
        for detail in &step.details {
            println!("       • {}", detail);
        }
    }

    if result.passed {
        println!("\n✨ 【TRIPLE GATE PASSED】三大驗收閘門全數通過，允許合併 (Merge Authorized)！");
        Ok(())
    } else {
        eprintln!("\n🚨 【TRIPLE GATE REJECTED】驗收未通過！Circuit Breaker 熔斷器已觸發！");
        if result.rollback_performed {
            eprintln!("⚠️  工作樹污染已自動回滾 (git checkout .) 避免污染主線！");
        }
        if let Some(fix) = result.fix_suggestion {
            eprintln!("\n🛠️  修復建議：\n{}", fix);
            // 嘗試將修復建議寫入 WORK_LOGS
            let log_dir = std::path::Path::new(".buzz/WORK_LOGS");
            let _ = std::fs::create_dir_all(log_dir);
            let _ = std::fs::write(log_dir.join("FIX_SUGGESTION.patch"), fix);
        }
        Err(CliError::Usage("Triple Gate validation failed. Git merge aborted.".to_string()))
    }
}

fn is_valid_event_id(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_valid_pubkey(s: &str) -> bool {
    if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if s.starts_with("npub1") && s.len() >= 60 {
        return true;
    }
    false
}

fn is_valid_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_event_id() {
        assert!(is_valid_event_id("a".repeat(64).as_str()));
        assert!(is_valid_event_id("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"));
        assert!(!is_valid_event_id("short"));
        assert!(!is_valid_event_id("g".repeat(64).as_str())); // not hex
        assert!(!is_valid_event_id("")); // empty
        assert!(!is_valid_event_id("a".repeat(63).as_str())); // too short
        assert!(!is_valid_event_id("a".repeat(65).as_str())); // too long
    }

    #[test]
    fn test_is_valid_pubkey() {
        // 64-char hex
        assert!(is_valid_pubkey("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"));
        assert!(!is_valid_pubkey("short"));
        assert!(!is_valid_pubkey("g".repeat(64).as_str()));
        
        // npub1... bech32 (known valid npub from test fixtures)
        assert!(is_valid_pubkey("npub1g53mukx3qqv5q4z08e5kpnf0qgxw3u5d8nw7x3j0k8z5v0q5q5q5q5q5q"));
        assert!(!is_valid_pubkey("npub1short"));
        assert!(!is_valid_pubkey("npub0invalid")); // wrong prefix
    }

    #[test]
    fn test_is_valid_uuid() {
        assert!(is_valid_uuid("5ca019f6-1a27-592a-b015-4baf0eb33572"));
        assert!(is_valid_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(!is_valid_uuid("invalid-uuid"));
        assert!(!is_valid_uuid(""));
        assert!(!is_valid_uuid("5ca019f6-1a27-592a-b015")); // incomplete
    }
}
