//! System Health Guardian — Diagnoser
//! 純 Rust 實作健康檢查：CLI 版本、環境變數驗證、進程掃描

use std::collections::HashMap;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{System, Pid, ProcessRefreshKind, ProcessesToUpdate};

use crate::config::Config;
use crate::types::AgentError;

/// 健康檢查完整結果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthCheckResult {
    pub cli_version: String,
    pub env_vars: EnvVarStatus,
    pub background_processes: Vec<ProcessStatus>,
    pub timestamp: String,
}

/// 環境變數檢查狀態
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnvVarStatus {
    pub buzz_relay_url: VarCheck,
    pub buzz_private_key: VarCheck,
    pub buzz_auth_tag: VarCheck,
    pub openrouter_key: VarCheck,
    pub github_token: VarCheck,
}

/// 單一環境變數檢查結果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VarCheck {
    pub name: String,
    pub present: bool,
    pub valid_format: bool,
    pub masked_value: Option<String>,
}

/// 背景行程狀態
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessStatus {
    pub name: String,
    pub expected_port: Option<u16>,
    pub pid_file: Option<String>,
    pub alive: bool,
    pub pid: Option<u32>,
    pub port_listening: bool,
    pub last_sync_ago_secs: Option<u64>,
    pub details: String,
}

/// 整體健康狀態
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum OverallStatus {
    Healthy,
    Degraded,
    Critical,
}

/// 環境變數驗證規則
fn validate_env_var(name: &str, value: &str) -> (bool, Option<String>) {
    let masked = if value.len() > 8 {
        Some(format!("{}***{}", &value[..4], &value[value.len()-4..]))
    } else {
        Some("****".to_string())
    };
    
    let valid = match name {
        "BUZZ_RELAY_URL" => value.starts_with("http://") || value.starts_with("https://") || value.starts_with("wss://"),
        "BUZZ_PRIVATE_KEY" => value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()),
        "BUZZ_AUTH_TAG" => !value.is_empty(),
        "OPENROUTER_API_KEY" => value.starts_with("sk-or-"),
        "GITHUB_TOKEN" => value.starts_with("ghp_") || value.starts_with("github_pat_"),
        _ => true,
    };
    (valid, masked)
}

/// 檢查端口是否在監聽
fn is_port_listening(port: u16) -> bool {
    use std::net::TcpListener;
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

/// 透過 sysinfo 掃描進程
fn scan_processes_by_name(sys: &System, name: &str) -> Vec<u32> {
    sys.processes()
        .values()
        .filter(|p| p.name().to_string_lossy().to_lowercase().contains(&name.to_lowercase()))
        .map(|p| p.pid().as_u32())
        .collect()
}

/// 讀取 PID 檔案
fn read_pid_file(path: &str) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// 取得 CLI 版本
fn get_cli_version() -> String {
    Command::new("buzz")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string())
}

/// 取得當前時間戳
fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

/// 執行完整健康檢查
pub async fn run_health_check(config: &Config) -> Result<HealthCheckResult, AgentError> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, ProcessRefreshKind::everything());
    
    // 1. CLI 版本檢查
    let cli_version = get_cli_version();
    
    // 2. 環境變數檢查
    let env_vars = check_env_vars();
    
    // 3. 背景行程檢查
    let background_processes = check_background_processes(&sys, config).await;
    
    Ok(HealthCheckResult {
        cli_version,
        env_vars,
        background_processes,
        timestamp: now_timestamp(),
    })
}

/// 檢查環境變數
fn check_env_vars() -> EnvVarStatus {
    let vars = [
        ("BUZZ_RELAY_URL", std::env::var("BUZZ_RELAY_URL").ok()),
        ("BUZZ_PRIVATE_KEY", std::env::var("BUZZ_PRIVATE_KEY").ok()),
        ("BUZZ_AUTH_TAG", std::env::var("BUZZ_AUTH_TAG").ok()),
        ("OPENROUTER_API_KEY", std::env::var("OPENROUTER_API_KEY").ok()),
        ("GITHUB_TOKEN", std::env::var("GITHUB_TOKEN").ok()),
    ];
    
    let mut checks = HashMap::new();
    for (name, value) in vars {
        let (present, valid_format, masked_value) = match value {
            Some(v) => {
                let (valid, masked) = validate_env_var(name, &v);
                (true, valid, masked)
            }
            None => (false, false, None),
        };
        checks.insert(name.to_string(), VarCheck {
            name: name.to_string(),
            present,
            valid_format,
            masked_value,
        });
    }
    
    EnvVarStatus {
        buzz_relay_url: checks.remove("BUZZ_RELAY_URL").unwrap_or(VarCheck {
            name: "BUZZ_RELAY_URL".into(),
            present: false,
            valid_format: false,
            masked_value: None,
        }),
        buzz_private_key: checks.remove("BUZZ_PRIVATE_KEY").unwrap_or(VarCheck {
            name: "BUZZ_PRIVATE_KEY".into(),
            present: false,
            valid_format: false,
            masked_value: None,
        }),
        buzz_auth_tag: checks.remove("BUZZ_AUTH_TAG").unwrap_or(VarCheck {
            name: "BUZZ_AUTH_TAG".into(),
            present: false,
            valid_format: false,
            masked_value: None,
        }),
        openrouter_key: checks.remove("OPENROUTER_API_KEY").unwrap_or(VarCheck {
            name: "OPENROUTER_API_KEY".into(),
            present: false,
            valid_format: false,
            masked_value: None,
        }),
        github_token: checks.remove("GITHUB_TOKEN").unwrap_or(VarCheck {
            name: "GITHUB_TOKEN".into(),
            present: false,
            valid_format: false,
            masked_value: None,
        }),
    }
}

/// 檢查背景行程
async fn check_background_processes(sys: &System, config: &Config) -> Vec<ProcessStatus> {
    let mut results = Vec::new();
    
    // openrouter-gateway (port 8080)
    let or_pids = scan_processes_by_name(sys, "openrouter");
    let or_pid = or_pids.first().copied();
    let or_port = is_port_listening(config.health_guardian.as_ref().map(|h| h.openrouter_port).unwrap_or(8080));
    results.push(ProcessStatus {
        name: "openrouter-gateway".to_string(),
        expected_port: Some(config.health_guardian.as_ref().map(|h| h.openrouter_port).unwrap_or(8080)),
        pid_file: None,
        alive: or_pid.is_some(),
        pid: or_pid,
        port_listening: or_port,
        last_sync_ago_secs: None,
        details: if or_pid.is_some() { "Process found via sysinfo scan".into() } else { "Process not found".into() },
    });
    
    // sync_obsidian (PID file)
    let sync_pid_path = config.health_guardian.as_ref()
        .map(|h| h.sync_pid_file.to_string_lossy().to_string())
        .unwrap_or_else(|| ".buzz/sync_obsidian.pid".to_string());
    let sync_pid = read_pid_file(&sync_pid_path);
    let sync_alive = sync_pid.and_then(|pid| sys.processes().get(&Pid::from_u32(pid))).is_some();
    let sync_pids = scan_processes_by_name(sys, "sync_obsidian");
    let sync_pid_actual = sync_pids.first().copied().or(sync_pid);
    results.push(ProcessStatus {
        name: "sync_obsidian".to_string(),
        expected_port: None,
        pid_file: Some(sync_pid_path),
        alive: sync_alive || sync_pids.first().is_some(),
        pid: sync_pid_actual,
        port_listening: false,
        last_sync_ago_secs: None, // TODO: 從日誌或狀態檔讀取
        details: if sync_alive { "PID file matches running process".into() } 
                 else if sync_pids.first().is_some() { "Process found via name scan, PID file stale".into() }
                 else { "Process not running".into() },
    });
    
    // buzz-proxy (port 3000)
    let proxy_port = config.health_guardian.as_ref().map(|h| h.proxy_port).unwrap_or(3000);
    let proxy_port_listening = is_port_listening(proxy_port);
    let proxy_pids = scan_processes_by_name(sys, "buzz-proxy");
    let proxy_pid = proxy_pids.first().copied();
    results.push(ProcessStatus {
        name: "buzz-proxy".to_string(),
        expected_port: Some(proxy_port),
        pid_file: None,
        alive: proxy_pid.is_some(),
        pid: proxy_pid,
        port_listening: proxy_port_listening,
        last_sync_ago_secs: None,
        details: if proxy_pid.is_some() { "Process found via sysinfo scan".into() } else { "Process not found".into() },
    });
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[test]
    fn test_validate_env_var() {
        assert_eq!(validate_env_var("BUZZ_RELAY_URL", "wss://relay.example.com"), (true, Some("wss://***".into())));
        assert_eq!(validate_env_var("BUZZ_RELAY_URL", "invalid"), (false, Some("inva***lid".into())));
        assert_eq!(validate_env_var("BUZZ_PRIVATE_KEY", "abcd".repeat(16)), (true, Some("abcd***efgh".into())));
        assert_eq!(validate_env_var("BUZZ_PRIVATE_KEY", "short"), (false, Some("****".into())));
        assert_eq!(validate_env_var("OPENROUTER_API_KEY", "sk-or-valid"), (true, Some("sk-o***alid".into())));
        assert_eq!(validate_env_var("OPENROUTER_API_KEY", "invalid"), (false, Some("inva***lid".into())));
        assert_eq!(validate_env_var("GITHUB_TOKEN", "ghp_valid"), (true, Some("ghp_***alid".into())));
        assert_eq!(validate_env_var("GITHUB_TOKEN", "github_pat_valid"), (true, Some("github***alid".into())));
    }
    
    #[test]
    fn test_is_port_listening() {
        // Port 0 is always available (OS assigns)
        assert!(!is_port_listening(0));
    }
    
    #[test]
    fn test_get_cli_version() {
        let version = get_cli_version();
        // Should return something, even if "unavailable"
        assert!(!version.is_empty());
    }
    
    #[test]
    fn test_scan_processes_by_name() {
        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, ProcessRefreshKind::everything());
        // Just verify it doesn't panic
        let _ = scan_processes_by_name(&sys, "nonexistent_process_xyz");
    }
}
