//! System Health Guardian — Main Coordinator & Remediator
//! 串聯診斷 -> 分析 -> 修復 -> 報告管線

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use buzz_cron::{BuzzSchedule, TimezoneConfig, CronResult, CronScheduleConfig as BuzzCronScheduleConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::sleep;

use crate::config::Config;
use crate::health_analyzer::{AnalysisResult, CircuitBreakerStatus, CircuitBreakerFile};
use crate::health_diagnoser::{HealthCheckResult, OverallStatus};
use crate::health_reporter::{build_report, render_json, render_markdown, HealthReport, now_timestamp};
use crate::types::AgentError;

/// 修復動作類型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemediationType {
    ResetCircuitBreaker,
    RestartProcess,
    ManualInterventionRequired,
}

/// 修復動作記錄
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationAction {
    pub action_type: RemediationType,
    pub target: String,
    pub reason: String,
    pub success: bool,
    pub output: Option<String>,
}

/// Phase A CLI 觸發的單次健康檢查
pub async fn run_health_check_once(config: &Config) -> Result<HealthReport, AgentError> {
    // 1. 診斷
    let checks = crate::health_diagnoser::run_health_check(config).await?;
    
    // 2. 分析
    let analysis = crate::health_analyzer::run_analysis(config).await?;
    
    // 3. Phase A 不執行修復，只報告
    let remediations = Vec::new();
    
    // 4. 產出報告
    let report = build_report(checks, analysis, remediations);
    
    Ok(report)
}

/// Phase B: 執行修復動作
pub async fn run_remediation(config: &Config, report: &HealthReport) -> Vec<RemediationAction> {
    let mut actions = Vec::new();
    
    // 1. 檢查熔斷器是否需要重置
    for cb in &report.circuit_breakers {
        if cb.tripped && should_auto_reset_circuit_breaker(config, cb) {
            let action = reset_circuit_breaker(config, cb).await;
            actions.push(action);
        }
    }
    
    // 2. 檢查背景行程是否需要重啟
    for proc in &report.checks.background_processes {
        if !proc.alive && should_restart_process(config, proc) {
            let action = restart_process(config, proc).await;
            actions.push(action);
        }
    }
    
    actions
}

/// 判斷是否應自動重置熔斷器
fn should_auto_reset_circuit_breaker(config: &Config, cb: &CircuitBreakerStatus) -> bool {
    let cfg = config.health_guardian.as_ref();
    if !cfg.map(|h| h.alert_threshold.circuit_breaker_auto_reset).unwrap_or(true) {
        return false;
    }
    
    // 只有在 failure_count 歸零且無新錯誤 10 分鐘後允許重置
    // 這裡簡化：failure_count > 0 時不重置
    cb.failure_count == 0
}

/// 重置熔斷器
async fn reset_circuit_breaker(config: &Config, cb: &CircuitBreakerStatus) -> RemediationAction {
    let path = config.health_guardian.as_ref()
        .map(|h| h.circuit_breaker_path.clone())
        .unwrap_or_else(|| PathBuf::from(".buzz/WORK_LOGS/circuit_breaker.json"));
    
    let new_status = CircuitBreakerFile {
        tripped: false,
        tripped_at: None,
        reason: None,
        failure_count: 0,
        last_failure: None,
        component: None,
    };
    
    match crate::health_analyzer::write_circuit_breaker(&path, &new_status).await {
        Ok(_) => RemediationAction {
            action_type: RemediationType::ResetCircuitBreaker,
            target: "circuit_breaker".into(),
            reason: "Auto-reset after recovery".into(),
            success: true,
            output: Some("Circuit breaker reset to closed state".into()),
        },
        Err(e) => RemediationAction {
            action_type: RemediationType::ResetCircuitBreaker,
            target: "circuit_breaker".into(),
            reason: "Auto-reset failed".into(),
            success: false,
            output: Some(e.to_string()),
        },
    }
}

/// 判斷是否應重啟行程
fn should_restart_process(config: &Config, proc: &crate::health_diagnoser::ProcessStatus) -> bool {
    let cfg = config.health_guardian.as_ref();
    let _cooldown = cfg.map(|h| h.alert_threshold.process_restart_cooldown_secs).unwrap_or(300);
    
    // 簡化：總是嘗試重啟關鍵行程
    // 實際應檢查上次重啟時間
    proc.name == "openrouter-gateway" || proc.name == "buzz-proxy" || proc.name == "sync_obsidian"
}

/// 重啟行程
async fn restart_process(config: &Config, proc: &crate::health_diagnoser::ProcessStatus) -> RemediationAction {
    let (cmd, args) = match proc.name.as_str() {
        "openrouter-gateway" => {
            ("cargo".to_string(), vec!["run".to_string(), "--bin".to_string(), "openrouter-gateway".to_string()])
        }
        "sync_obsidian" => {
            ("cargo".to_string(), vec!["run".to_string(), "--bin".to_string(), "sync_obsidian".to_string()])
        }
        "buzz-proxy" => {
            ("cargo".to_string(), vec!["run".to_string(), "--bin".to_string(), "buzz-proxy".to_string()])
        }
        _ => return RemediationAction {
            action_type: RemediationType::ManualInterventionRequired,
            target: proc.name.clone(),
            reason: "Unknown process type".into(),
            success: false,
            output: None,
        },
    };
    
    let work_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    match Command::new(&cmd)
        .args(&args)
        .current_dir(&work_dir)
        .spawn()
    {
        Ok(mut child) => {
            // 背景守護進程不應無限期阻塞等待退出，僅等待短暫啟動期確認未立即崩潰
            match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
                Ok(Ok(status)) if status.success() => RemediationAction {
                    action_type: RemediationType::RestartProcess,
                    target: proc.name.clone(),
                    reason: "Process was not running".into(),
                    success: true,
                    output: Some("Process restarted and exited cleanly".into()),
                },
                Ok(Ok(status)) => RemediationAction {
                    action_type: RemediationType::RestartProcess,
                    target: proc.name.clone(),
                    reason: "Process was not running".into(),
                    success: false,
                    output: Some(format!("Restart failed immediately with exit code: {}", status.code().unwrap_or(-1))),
                },
                Ok(Err(e)) => RemediationAction {
                    action_type: RemediationType::RestartProcess,
                    target: proc.name.clone(),
                    reason: "Process was not running".into(),
                    success: false,
                    output: Some(format!("Failed to wait for process: {e}")),
                },
                Err(_) => RemediationAction {
                    action_type: RemediationType::RestartProcess,
                    target: proc.name.clone(),
                    reason: "Process was not running".into(),
                    success: true,
                    output: Some(format!("Background daemon spawned and running (PID: {:?})", child.id())),
                },
            }
        }
        Err(e) => RemediationAction {
            action_type: RemediationType::RestartProcess,
            target: proc.name.clone(),
            reason: "Process was not running".into(),
            success: false,
            output: Some(format!("Failed to spawn process: {e}")),
        },
    }
}

/// 發送告警通知 (Nostr Kind:1)
pub async fn send_alert(config: &Config, report: &HealthReport, consecutive_failures: u32) -> Result<(), AgentError> {
    let threshold = config.health_guardian.as_ref()
        .map(|h| h.alert_threshold.max_consecutive_failures)
        .unwrap_or(3);
    
    if consecutive_failures < threshold {
        return Ok(()); // 未達閾值
    }
    
    let channel_id = config.health_guardian.as_ref()
        .map(|h| h.channel_id.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AgentError::Other("Health guardian channel not configured".into()))?;
    
    let channel_id_for_tags = channel_id.clone();
    let markdown = render_markdown(report);
    let _content = format!("🚨 **System Health Alert** (Consecutive Failures: {})\n\n{}", consecutive_failures, markdown);
    
    // 使用 builtin::call_sign_and_send_event 發送
    // 這裡需要在有 agent_keys 的上下文中調用
    // 實際調用由 daemon 或 CLI 入口處理
    
    // 構建 tags
    let _tags = vec![
        vec!["h".to_string(), channel_id_for_tags],
        vec!["t".to_string(), "health-alert".to_string()],
    ];
    
    // 這裡返回構建好的參數，由調用者實際發送
    tracing::info!("Health alert ready to send: channel={}, failures={}", channel_id, consecutive_failures);
    
    Ok(())
}

/// 連續失敗計數器 (儲存在檔案中)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsecutiveFailures {
    pub count: u32,
    pub last_check: String,
    pub last_status: OverallStatus,
}

/// Cron 排程表達式 (Phase 1.2: 遷移至 buzz-cron crate 完整實作)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronScheduleConfig {
    pub expression: String,
    #[serde(default)]
    pub timezone: Option<String>,  // IANA 時區名稱，如 "America/New_York", "Asia/Tokyo", "UTC"
}

impl CronScheduleConfig {
    pub fn new(expression: String) -> Self {
        Self {
            expression,
            timezone: None,
        }
    }

    pub fn with_timezone(mut self, tz: String) -> Self {
        self.timezone = Some(tz);
        self
    }

    fn to_buzz_schedule(&self) -> CronResult<BuzzSchedule> {
        let tz_config = match &self.timezone {
            Some(tz_str) => TimezoneConfig::new(tz_str),
            None => TimezoneConfig::utc(),
        };
        BuzzSchedule::from_str(&self.expression, tz_config)
    }
}

/// 讀取連續失敗計數
pub async fn read_consecutive_failures(path: &std::path::Path) -> Result<ConsecutiveFailures, AgentError> {
    if !path.exists() {
        return Ok(ConsecutiveFailures {
            count: 0,
            last_check: String::new(),
            last_status: OverallStatus::Healthy,
        });
    }
    
    let content = std::fs::read_to_string(path)
        .map_err(|e| AgentError::Other(format!("Failed to read consecutive failures: {e}")))?;
    
    serde_json::from_str(&content)
        .map_err(|e| AgentError::Other(format!("Failed to parse consecutive failures: {e}")))
}

/// 寫入連續失敗計數
pub async fn write_consecutive_failures(path: &std::path::Path, failures: &ConsecutiveFailures) -> Result<(), AgentError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AgentError::Other(format!("Failed to create directory: {e}")))?;
    }
    
    let content = serde_json::to_string_pretty(failures)
        .map_err(|e| AgentError::Other(format!("Failed to serialize consecutive failures: {e}")))?;
    
    std::fs::write(path, content)
        .map_err(|e| AgentError::Other(format!("Failed to write consecutive failures: {e}")))?;
    
    Ok(())
}

/// Phase A 核心入口：單次健康檢查完整流程
/// 返回 HealthReport (實現 Display 以便 CLI 直接印出)
pub async fn phase_a_run_once(config: &Config, output_json: bool) -> Result<HealthReport, AgentError> {
    // 1. 讀取連續失敗計數
    let consecutive_path = config.health_guardian.as_ref()
        .and_then(|h| h.circuit_breaker_path.parent().map(|p| p.join("consecutive_failures.json")))
        .unwrap_or_else(|| std::path::PathBuf::from(".buzz/WORK_LOGS/consecutive_failures.json"));
    
    let mut consecutive = read_consecutive_failures(&consecutive_path).await?;
    
    // 2. 執行健康檢查
    let report = run_health_check_once(config).await?;
    
    // 3. 更新連續失敗計數
    if report.overall_status == OverallStatus::Critical || report.overall_status == OverallStatus::Degraded {
        consecutive.count += 1;
    } else {
        consecutive.count = 0; // 重置
    }
    consecutive.last_check = report.timestamp.clone();
    consecutive.last_status = report.overall_status.clone();
    
    write_consecutive_failures(&consecutive_path, &consecutive).await?;
    
    // 4. 發送告警 (若達閾值)
    send_alert(config, &report, consecutive.count).await?;
    
    // 5. 輸出報告
    if output_json {
        println!("{}", render_json(&report)?);
    } else {
        println!("{}", render_markdown(&report));
    }
    
    Ok(report)
}

/// Phase B: 帶排程的健康檢查守護進程
/// 在 Daemon 中運行，按 cron 排程定期執行健康檢查並執行修復
pub async fn phase_b_run_daemon(config: Arc<Config>, agent_keys: Arc<crate::types::AgentKeys>, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Result<(), AgentError> {
    let hg_config = config.health_guardian.as_ref()
        .ok_or_else(|| AgentError::Other("Health guardian not configured".into()))?;
    
    // 解析排程 - 使用 from_config 支援時區
    let mut schedule = BuzzSchedule::from_config(BuzzCronScheduleConfig {
        expression: hg_config.schedule.clone(),
        timezone: Some(TimezoneConfig::utc()),
        with_seconds: false,
    }).map_err(|e| AgentError::Other(format!("Failed to parse schedule: {}", e)))?;
    let mut daily_deep_schedule = BuzzSchedule::from_config(BuzzCronScheduleConfig {
        expression: hg_config.daily_deep_schedule.clone(),
        timezone: Some(TimezoneConfig::utc()),
        with_seconds: false,
    }).map_err(|e| AgentError::Other(format!("Failed to parse daily_deep_schedule: {}", e)))?;
    
    // 連續失敗計數檔案路徑
    let consecutive_path = hg_config.circuit_breaker_path.parent()
        .map(|p| p.join("consecutive_failures.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".buzz/WORK_LOGS/consecutive_failures.json"));
    
    tracing::info!("Health Guardian Phase B started: schedule={}, daily_deep={}", 
        hg_config.schedule, hg_config.daily_deep_schedule);
    
    loop {
        // 檢查關閉信號
        if *shutdown_rx.borrow() {
            tracing::info!("Health Guardian received shutdown signal");
            break;
        }
        
        let now = SystemTime::now();
        let now_utc: DateTime<Utc> = now.into();
        
        // 檢查是否該執行常規檢查
        let should_run = schedule.matches_utc(now_utc);
        
        // 檢查是否該執行深度檢查 (每日)
        let should_deep = daily_deep_schedule.matches_utc(now_utc);
        
        if should_run || should_deep {
            tracing::info!("Running health check (deep={})", should_deep);
            
            // 讀取連續失敗計數
            let mut consecutive = read_consecutive_failures(&consecutive_path).await.unwrap_or_else(|_| ConsecutiveFailures {
                count: 0,
                last_check: String::new(),
                last_status: OverallStatus::Healthy,
            });
            
            // 執行健康檢查
            let report = match run_health_check_once(&config).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Health check failed: {}", e);
                    // 即使檢查失敗也要更新計數
                    consecutive.count += 1;
                    consecutive.last_check = now_timestamp();
                    consecutive.last_status = OverallStatus::Critical;
                    let _ = write_consecutive_failures(&consecutive_path, &consecutive).await;
                    sleep(Duration::from_secs(60)).await; // 避免緊循環
                    continue;
                }
            };
            
            // 執行修復動作 (Phase B)
            let remediations = run_remediation(&config, &report).await;
            
            // 重新生成報告以包含修復結果
            let checks = crate::health_diagnoser::run_health_check(&config).await.unwrap_or_else(|_| report.checks.clone());
            let analysis = crate::health_analyzer::run_analysis(&config).await.unwrap_or_else(|_| crate::health_analyzer::AnalysisResult {
                failures: report.failures.clone(),
                circuit_breakers: report.circuit_breakers.clone(),
                timestamp: now_timestamp(),
            });
            let final_report = build_report(checks, analysis, remediations);
            
            // 更新連續失敗計數
            if final_report.overall_status == OverallStatus::Critical || final_report.overall_status == OverallStatus::Degraded {
                consecutive.count += 1;
            } else {
                consecutive.count = 0;
            }
            consecutive.last_check = final_report.timestamp.clone();
            consecutive.last_status = final_report.overall_status.clone();
            
            let _ = write_consecutive_failures(&consecutive_path, &consecutive).await;
            
            // 發送告警通知 (實際 Nostr 發送)
            if let Err(e) = send_alert_with_keys(&agent_keys, &config, &final_report, consecutive.count).await {
                tracing::error!("Failed to send health alert: {}", e);
            }
            
            // 輸出報告到日誌
            tracing::info!("Health check completed: status={:?}", final_report.overall_status);
        }
        
        // 計算下次檢查時間，避免忙等待
        let next_check = std::cmp::min(
            schedule.next_after_utc(now_utc).unwrap_or(now_utc + Duration::from_secs(60)),
            daily_deep_schedule.next_after_utc(now_utc).unwrap_or(now_utc + Duration::from_secs(60)),
        );
        
        let sleep_duration = next_check.signed_duration_since(now_utc).to_std().unwrap_or(Duration::from_secs(60));
        let sleep_secs = sleep_duration.as_secs().min(60).max(1); // 最多睡 60 秒，最少 1 秒
        
        // 使用 select 等待關閉或超時
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            _ = sleep(Duration::from_secs(sleep_secs)) => {}
        }
    }
    
    tracing::info!("Health Guardian Phase B stopped");
    Ok(())
}

/// 發送告警通知 (Nostr Kind:1) - 實際發送版本
async fn send_alert_with_keys(
    agent_keys: &crate::types::AgentKeys,
    config: &Config,
    report: &HealthReport,
    consecutive_failures: u32,
) -> Result<(), AgentError> {
    let threshold = config.health_guardian.as_ref()
        .map(|h| h.alert_threshold.max_consecutive_failures)
        .unwrap_or(3);
    
    if consecutive_failures < threshold {
        return Ok(()); // 未達閾值
    }
    
    let channel_id = config.health_guardian.as_ref()
        .map(|h| h.channel_id.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AgentError::Other("Health guardian channel not configured".into()))?;
    
    let markdown = render_markdown(report);
    let content = format!("🚨 **System Health Alert** (Consecutive Failures: {})\n\n{}", consecutive_failures, markdown);
    
    // 構建 tags
    let tags = vec![
        vec!["h".to_string(), channel_id.clone()],
        vec!["t".to_string(), "health-alert".to_string()],
    ];
    
    // 實際發送 Nostr 事件 (Kind 1 - Text Note)
    let event_id = crate::builtin::call_sign_and_send_event_direct(
        agent_keys,
        &config.relay_ws_url,
        1, // Kind 1: Text Note
        content,
        tags,
    ).await?;
    
    tracing::info!("Health alert sent via Nostr: event_id={}, channel={}, failures={}", 
        event_id, channel_id.clone(), consecutive_failures);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    
    #[test]
    fn test_cron_schedule_every_15_min() {
        let cron = BuzzSchedule::from_expression("*/15 * * * *").unwrap();
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        assert!(cron.matches_utc(t));
        
        let t2 = Utc.with_ymd_and_hms(2026, 1, 1, 12, 15, 0).unwrap();
        assert!(cron.matches_utc(t2));
        
        let t3 = Utc.with_ymd_and_hms(2026, 1, 1, 12, 14, 0).unwrap();
        assert!(!cron.matches_utc(t3));
    }
    
    #[test]
    fn test_cron_matches() {
        let cron = BuzzSchedule::from_expression("0 4 * * *").unwrap();
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 0).unwrap();
        assert!(cron.matches_utc(t));
        
        let t2 = Utc.with_ymd_and_hms(2026, 1, 2, 4, 0, 0).unwrap();
        assert!(cron.matches_utc(t2));
        
        let t3 = Utc.with_ymd_and_hms(2026, 1, 1, 5, 0, 0).unwrap();
        assert!(!cron.matches_utc(t3));
    }
    
    #[test]
    fn test_cron_next_after() {
        let cron = BuzzSchedule::from_expression("0 4 * * *").unwrap();
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 3, 30, 0).unwrap();
        let next = cron.next_after(&t).unwrap();
        let expected = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 0).unwrap();
        assert_eq!(next, expected);
    }
}
