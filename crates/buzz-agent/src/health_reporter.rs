//! System Health Guardian — Reporter
//! 產出結構化健全性報告 (Markdown + JSON)

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use hex;
use rand::{Rng, RngExt};

use crate::health_analyzer::{AnalysisResult, FailureLogEntry, CircuitBreakerStatus, Severity};
use crate::health_diagnoser::{HealthCheckResult, OverallStatus, ProcessStatus};
use crate::health_guardian::RemediationAction;

/// 完整健全性報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub check_id: String,
    pub timestamp: String,
    pub overall_status: OverallStatus,
    pub checks: HealthCheckResult,
    pub failures: Vec<FailureLogEntry>,
    pub circuit_breakers: Vec<CircuitBreakerStatus>,
    pub remediations: Vec<RemediationAction>,
    pub recommendations: Vec<String>,
}

impl std::fmt::Display for HealthReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", render_markdown(self))
    }
}

/// 產出 Markdown 格式報告
pub fn render_markdown(report: &HealthReport) -> String {
    let mut md = String::new();
    
    md.push_str(&format!("# System Health Report\n\n"));
    md.push_str(&format!("**Check ID**: {}\n", report.check_id));
    md.push_str(&format!("**Timestamp**: {}\n", format_timestamp(&report.timestamp)));
    md.push_str(&format!("**Overall Status**: {}\n\n", status_badge(&report.overall_status)));
    
    // CLI 版本
    md.push_str("## CLI Version\n\n");
    md.push_str(&format!("```\n{}\n```\n\n", report.checks.cli_version));
    
    // 環境變數
    md.push_str("## Environment Variables\n\n");
    md.push_str("| Variable | Present | Valid Format | Value |\n");
    md.push_str("|----------|---------|--------------|-------|\n");
    for var in [
        ("BUZZ_RELAY_URL", &report.checks.env_vars.buzz_relay_url),
        ("BUZZ_PRIVATE_KEY", &report.checks.env_vars.buzz_private_key),
        ("BUZZ_AUTH_TAG", &report.checks.env_vars.buzz_auth_tag),
        ("OPENROUTER_API_KEY", &report.checks.env_vars.openrouter_key),
        ("GITHUB_TOKEN", &report.checks.env_vars.github_token),
    ] {
        let status = if var.1.present { "✅" } else { "❌" };
        let format = if var.1.valid_format { "✅" } else { "❌" };
        let masked = var.1.masked_value.as_deref().unwrap_or("—");
        md.push_str(&format!("| {} | {} | {} | {} |\n", var.0, status, format, masked));
    }
    md.push_str("\n");
    
    // 背景行程
    md.push_str("## Background Processes\n\n");
    md.push_str("| Process | Alive | PID | Port | Listening | Details |\n");
    md.push_str("|---------|-------|-----|------|-----------|---------|\n");
    for proc in &report.checks.background_processes {
        let alive = if proc.alive { "✅" } else { "❌" };
        let pid = proc.pid.map(|p| p.to_string()).unwrap_or_else(|| "—".into());
        let port = proc.expected_port.map(|p| p.to_string()).unwrap_or_else(|| "—".into());
        let listening = if proc.port_listening { "✅" } else { "❌" };
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} |\n", 
            proc.name, alive, pid, port, listening, proc.details));
    }
    md.push_str("\n");
    
    // 失敗日誌
    if !report.failures.is_empty() {
        md.push_str("## Recent Failures (Last Hour)\n\n");
        md.push_str("| Time | Component | Severity | Error Type | Stack Trace |\n");
        md.push_str("|------|-----------|----------|------------|-------------|\n");
        for failure in &report.failures {
            let stack = failure.stack_trace.as_deref().unwrap_or("—");
            let short_stack = if stack.len() > 50 { &stack[..50] } else { stack };
            md.push_str(&format!("| {} | {} | {} | {} | {} |\n",
                format_timestamp(&failure.timestamp),
                failure.component,
                severity_badge(&failure.severity),
                failure.error_type,
                short_stack.replace('\n', " ")));
        }
        md.push_str("\n");
    } else {
        md.push_str("## Recent Failures\n\n✅ No failures in the last hour.\n\n");
    }
    
    // 熔斷器
    md.push_str("## Circuit Breakers\n\n");
    for cb in &report.circuit_breakers {
        let status = if cb.tripped { "🔴 TRIPPED" } else { "🟢 CLOSED" };
        md.push_str(&format!("**{}**\n\n", status));
        md.push_str(&format!("- Tripped: {}\n", if cb.tripped { "Yes" } else { "No" }));
        if let Some(at) = &cb.tripped_at {
            md.push_str(&format!("- Tripped At: {}\n", format_timestamp(at)));
        }
        if let Some(reason) = &cb.reason {
            md.push_str(&format!("- Reason: {}\n", reason));
        }
        md.push_str(&format!("- Failure Count: {}\n", cb.failure_count));
        if let Some(last) = &cb.last_failure {
            md.push_str(&format!("- Last Failure: {}\n", format_timestamp(last)));
        }
        md.push_str("\n");
    }
    
    // 修復動作
    if !report.remediations.is_empty() {
        md.push_str("## Remediation Actions\n\n");
        md.push_str("| Action | Target | Reason | Success | Output |\n");
        md.push_str("|--------|--------|--------|---------|--------|\n");
        for r in &report.remediations {
            let success = if r.success { "✅" } else { "❌" };
            let output = r.output.as_deref().unwrap_or("—");
            let short_output = if output.len() > 50 { &output[..50] } else { output };
            md.push_str(&format!("| {:?} | {} | {} | {} | {} |\n",
                r.action_type, r.target, r.reason, success, short_output));
        }
        md.push_str("\n");
    }
    
    // 建議
    if !report.recommendations.is_empty() {
        md.push_str("## Recommendations\n\n");
        for rec in &report.recommendations {
            md.push_str(&format!("- {}\n", rec));
        }
        md.push_str("\n");
    }
    
    md.push_str("---\n*Generated by System Health Guardian*");
    
    md
}

/// 產出 JSON 格式報告
pub fn render_json(report: &HealthReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

/// 狀態徽章
fn status_badge(status: &OverallStatus) -> String {
    match status {
        OverallStatus::Healthy => "🟢 Healthy",
        OverallStatus::Degraded => "🟡 Degraded",
        OverallStatus::Critical => "🔴 Critical",
    }.to_string()
}

/// 嚴重程度徽章
fn severity_badge(severity: &crate::health_analyzer::Severity) -> String {
    match severity {
        crate::health_analyzer::Severity::Critical => "🔴 Critical",
        crate::health_analyzer::Severity::Error => "🟠 Error",
        crate::health_analyzer::Severity::Warning => "🟡 Warning",
        crate::health_analyzer::Severity::Info => "🔵 Info",
    }.to_string()
}

/// 格式化時間戳
fn format_timestamp(ts: &str) -> String {
    ts.parse::<u64>()
        .ok()
        .and_then(|ts| {
            let dt = chrono::DateTime::from_timestamp(ts as i64, 0)?;
            Some(dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        })
        .unwrap_or_else(|| ts.to_string())
}

/// 產生檢查 ID
pub fn generate_check_id() -> String {
    let mut bytes = [0u8; 8];
    rand::rng().fill(&mut bytes);
    format!("hc_{}", hex::encode(bytes))
}

/// 取得當前時間戳
pub fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

/// 建立完整報告
pub fn build_report(
    checks: HealthCheckResult,
    analysis: AnalysisResult,
    remediations: Vec<RemediationAction>,
) -> HealthReport {
    let overall = determine_overall_status(&checks, &analysis);
    let recommendations = generate_recommendations(&checks, &analysis);
    
    HealthReport {
        check_id: generate_check_id(),
        timestamp: now_timestamp(),
        overall_status: overall,
        checks,
        failures: analysis.failures,
        circuit_breakers: analysis.circuit_breakers,
        remediations,
        recommendations,
    }
}

/// 判斷整體狀態
fn determine_overall_status(checks: &HealthCheckResult, analysis: &AnalysisResult) -> OverallStatus {
    let mut critical = false;
    let mut degraded = false;
    
    // 檢查環境變數
    for var in [
        &checks.env_vars.buzz_relay_url,
        &checks.env_vars.buzz_private_key,
    ] {
        if !var.present || !var.valid_format {
            critical = true;
        }
    }
    
    // 檢查關鍵背景行程
    for proc in &checks.background_processes {
        if !proc.alive {
            // 關鍵行程離線 = Critical
            if proc.name == "openrouter-gateway" || proc.name == "buzz-proxy" {
                critical = true;
            } else {
                degraded = true;
            }
        }
        if proc.expected_port.is_some() && !proc.port_listening {
            degraded = true;
        }
    }
    
    // 檢查熔斷器
    for cb in &analysis.circuit_breakers {
        if cb.tripped {
            critical = true;
        }
        if cb.failure_count > 5 {
            degraded = true;
        }
    }
    
    // 檢查失敗日誌
    let critical_failures = analysis.failures.iter()
        .filter(|f| f.severity == Severity::Critical)
        .count();
    let error_failures = analysis.failures.iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    
    if critical_failures > 0 {
        critical = true;
    } else if error_failures > 3 {
        degraded = true;
    }
    
    if critical {
        OverallStatus::Critical
    } else if degraded {
        OverallStatus::Degraded
    } else {
        OverallStatus::Healthy
    }
}

/// 產生建議事項
fn generate_recommendations(checks: &HealthCheckResult, analysis: &AnalysisResult) -> Vec<String> {
    let mut recs = Vec::new();
    
    // 環境變數建議
    for var in [
        ("BUZZ_RELAY_URL", &checks.env_vars.buzz_relay_url),
        ("BUZZ_PRIVATE_KEY", &checks.env_vars.buzz_private_key),
        ("BUZZ_AUTH_TAG", &checks.env_vars.buzz_auth_tag),
        ("OPENROUTER_API_KEY", &checks.env_vars.openrouter_key),
        ("GITHUB_TOKEN", &checks.env_vars.github_token),
    ] {
        if !var.1.present {
            recs.push(format!("Set missing environment variable: {}", var.0));
        } else if !var.1.valid_format {
            recs.push(format!("Fix invalid format for environment variable: {}", var.0));
        }
    }
    
    // 背景行程建議
    for proc in &checks.background_processes {
        if !proc.alive {
            recs.push(format!("Restart process '{}' (PID file: {:?})", proc.name, proc.pid_file));
        }
        if proc.expected_port.is_some() && !proc.port_listening {
            recs.push(format!("Check port {} for process '{}' - not listening", proc.expected_port.unwrap(), proc.name));
        }
    }
    
    // 熔斷器建議
    for cb in &analysis.circuit_breakers {
        if cb.tripped {
            recs.push(format!("Circuit breaker tripped: {} - investigate root cause", cb.reason.as_deref().unwrap_or("unknown")));
        }
        if cb.failure_count > 3 {
            recs.push(format!("High failure count ({}) for circuit breaker - review recent errors", cb.failure_count));
        }
    }
    
    // 失敗日誌建議
    if !analysis.failures.is_empty() {
        recs.push(format!("Review {} recent failure(s) in WORK_LOGS", analysis.failures.len()));
    }
    
    if recs.is_empty() {
        recs.push("All systems operational".to_string());
    }
    
    recs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health_analyzer::{FailureLogEntry, Severity, CircuitBreakerStatus};
    use crate::health_diagnoser::{HealthCheckResult, EnvVarStatus, VarCheck, ProcessStatus, OverallStatus};
    use crate::health_guardian::RemediationAction;
    
    fn sample_report(status: OverallStatus) -> HealthReport {
        HealthReport {
            check_id: "test".into(),
            timestamp: "0".into(),
            overall_status: status,
            checks: HealthCheckResult {
                cli_version: "test".into(),
                env_vars: EnvVarStatus {
                    buzz_relay_url: VarCheck { name: "BUZZ_RELAY_URL".into(), present: true, valid_format: true, masked_value: Some("wss://***".into()) },
                    buzz_private_key: VarCheck { name: "BUZZ_PRIVATE_KEY".into(), present: true, valid_format: true, masked_value: Some("abcd***efgh".into()) },
                    buzz_auth_tag: VarCheck { name: "BUZZ_AUTH_TAG".into(), present: true, valid_format: true, masked_value: Some("tag".into()) },
                    openrouter_key: VarCheck { name: "OPENROUTER_API_KEY".into(), present: true, valid_format: true, masked_value: Some("sk-or-***".into()) },
                    github_token: VarCheck { name: "GITHUB_TOKEN".into(), present: true, valid_format: true, masked_value: Some("ghp_***".into()) },
                },
                background_processes: vec![],
                timestamp: "0".into(),
            },
            failures: vec![],
            circuit_breakers: vec![],
            remediations: vec![],
            recommendations: vec![],
        }
    }
    
    #[test]
    fn test_determine_overall_status_healthy() {
        let checks = HealthCheckResult {
            cli_version: "test".into(),
            env_vars: EnvVarStatus {
                buzz_relay_url: VarCheck { name: "BUZZ_RELAY_URL".into(), present: true, valid_format: true, masked_value: None },
                buzz_private_key: VarCheck { name: "BUZZ_PRIVATE_KEY".into(), present: true, valid_format: true, masked_value: None },
                buzz_auth_tag: VarCheck { name: "BUZZ_AUTH_TAG".into(), present: true, valid_format: true, masked_value: None },
                openrouter_key: VarCheck { name: "OPENROUTER_API_KEY".into(), present: true, valid_format: true, masked_value: None },
                github_token: VarCheck { name: "GITHUB_TOKEN".into(), present: true, valid_format: true, masked_value: None },
            },
            background_processes: vec![
                ProcessStatus { name: "openrouter-gateway".into(), expected_port: Some(8080), pid_file: None, alive: true, pid: Some(123), port_listening: true, last_sync_ago_secs: None, details: "".into() },
            ],
            timestamp: "0".into(),
        };
        let analysis = AnalysisResult {
            failures: vec![],
            circuit_breakers: vec![CircuitBreakerStatus { tripped: false, tripped_at: None, reason: None, failure_count: 0, last_failure: None }],
            timestamp: "0".into(),
        };
        assert_eq!(determine_overall_status(&checks, &analysis), OverallStatus::Healthy);
    }
    
    #[test]
    fn test_determine_overall_status_critical_missing_env() {
        let checks = HealthCheckResult {
            cli_version: "test".into(),
            env_vars: EnvVarStatus {
                buzz_relay_url: VarCheck { name: "BUZZ_RELAY_URL".into(), present: false, valid_format: false, masked_value: None },
                buzz_private_key: VarCheck { name: "BUZZ_PRIVATE_KEY".into(), present: true, valid_format: true, masked_value: None },
                buzz_auth_tag: VarCheck { name: "BUZZ_AUTH_TAG".into(), present: true, valid_format: true, masked_value: None },
                openrouter_key: VarCheck { name: "OPENROUTER_API_KEY".into(), present: true, valid_format: true, masked_value: None },
                github_token: VarCheck { name: "GITHUB_TOKEN".into(), present: true, valid_format: true, masked_value: None },
            },
            background_processes: vec![],
            timestamp: "0".into(),
        };
        let analysis = AnalysisResult {
            failures: vec![],
            circuit_breakers: vec![CircuitBreakerStatus { tripped: false, tripped_at: None, reason: None, failure_count: 0, last_failure: None }],
            timestamp: "0".into(),
        };
        assert_eq!(determine_overall_status(&checks, &analysis), OverallStatus::Critical);
    }
    
    #[test]
    fn test_generate_recommendations() {
        let checks = HealthCheckResult {
            cli_version: "test".into(),
            env_vars: EnvVarStatus {
                buzz_relay_url: VarCheck { name: "BUZZ_RELAY_URL".into(), present: false, valid_format: false, masked_value: None },
                buzz_private_key: VarCheck { name: "BUZZ_PRIVATE_KEY".into(), present: true, valid_format: true, masked_value: None },
                buzz_auth_tag: VarCheck { name: "BUZZ_AUTH_TAG".into(), present: true, valid_format: true, masked_value: None },
                openrouter_key: VarCheck { name: "OPENROUTER_API_KEY".into(), present: true, valid_format: true, masked_value: None },
                github_token: VarCheck { name: "GITHUB_TOKEN".into(), present: true, valid_format: true, masked_value: None },
            },
            background_processes: vec![
                ProcessStatus { name: "openrouter-gateway".into(), expected_port: Some(8080), pid_file: None, alive: false, pid: None, port_listening: false, last_sync_ago_secs: None, details: "".into() },
            ],
            timestamp: "0".into(),
        };
        let analysis = AnalysisResult {
            failures: vec![FailureLogEntry { timestamp: "0".into(), component: "test".into(), error_type: "TestError".into(), stack_trace: None, severity: Severity::Error }],
            circuit_breakers: vec![CircuitBreakerStatus { tripped: true, tripped_at: Some("0".into()), reason: Some("test".into()), failure_count: 5, last_failure: Some("0".into()) }],
            timestamp: "0".into(),
        };
        let recs = generate_recommendations(&checks, &analysis);
        assert!(recs.iter().any(|r| r.contains("BUZZ_RELAY_URL")));
        assert!(recs.iter().any(|r| r.contains("Restart process 'openrouter-gateway'")));
        assert!(recs.iter().any(|r| r.contains("Circuit breaker tripped")));
    }
} 
