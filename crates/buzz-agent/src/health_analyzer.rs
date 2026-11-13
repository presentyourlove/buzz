//! System Health Guardian — Analyzer
//! 掃描失敗日誌、解析錯誤堆疊、檢測熔斷狀態

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::types::AgentError;

/// 失敗日誌條目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureLogEntry {
    pub timestamp: String,
    pub component: String,
    pub error_type: String,
    pub stack_trace: Option<String>,
    pub severity: Severity,
}

/// 錯誤嚴重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// 熔斷器狀態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    pub tripped: bool,
    pub tripped_at: Option<String>,
    pub reason: Option<String>,
    pub failure_count: u32,
    pub last_failure: Option<String>,
}

/// 熔斷器狀態檔案結構 (對應 WORK_LOGS/circuit_breaker.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerFile {
    pub tripped: bool,
    pub tripped_at: Option<String>,
    pub reason: Option<String>,
    pub failure_count: u32,
    pub last_failure: Option<String>,
    pub component: Option<String>,
}

/// 分析結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub failures: Vec<FailureLogEntry>,
    pub circuit_breakers: Vec<CircuitBreakerStatus>,
    pub timestamp: String,
}

/// 掃描 WORK_LOGS 目錄下的失敗日誌
pub async fn scan_failure_logs(work_logs_path: &Path) -> Result<Vec<FailureLogEntry>, AgentError> {
    let mut failures = Vec::new();
    
    if !work_logs_path.exists() {
        return Ok(failures);
    }
    
    let entries = fs::read_dir(work_logs_path)
        .map_err(|e| AgentError::Other(format!("Failed to read WORK_LOGS: {e}")))?;
    
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let one_hour_ago = now.saturating_sub(3600);
    
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("log") {
            continue;
        }
        
        // 限制單一日誌分析大小（避免超大型日誌造成 OOM DoS）
        const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > MAX_LOG_BYTES {
                continue;
            }
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        let component = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let lines: Vec<&str> = content.lines().collect();

        // 解析日誌行，尋找 ERROR/CRITICAL
        for (line_num, &line) in lines.iter().enumerate() {
            let severity = parse_severity(line);
            if severity >= Severity::Error {
                let timestamp = extract_timestamp(line).unwrap_or_else(|| now_timestamp());
                let error_type = extract_error_type(line);
                let stack_trace = extract_stack_trace_from_lines(&lines, line_num);
                
                failures.push(FailureLogEntry {
                    timestamp,
                    component: component.clone(),
                    error_type,
                    stack_trace,
                    severity,
                });
            }
        }
    }
    
    // 按時間排序，最新的在前
    failures.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    Ok(failures)
}

/// 解析嚴重程度
fn parse_severity(line: &str) -> Severity {
    let line_upper = line.to_uppercase();
    if line_upper.contains("CRITICAL") || line_upper.contains("FATAL") {
        Severity::Critical
    } else if line_upper.contains("ERROR") || line_upper.contains("ERRO[") {
        Severity::Error
    } else if line_upper.contains("WARN") {
        Severity::Warning
    } else {
        Severity::Info
    }
}

/// 提取時間戳
fn extract_timestamp(line: &str) -> Option<String> {
    // 簡單解析常見格式：尋找類似 2026-11-02T15:30:45Z 或 2026-11-02 15:30:45 的模式
    // 不使用 regex，改用簡單的字串搜尋
    for prefix in ["[", ""] {
        for sep in ["T", " "] {
            let search = format!("{}{}{}", prefix, sep, ":");
            // 這裡簡化：尋找包含日期時間模式的子字串
        }
    }
    
    // 簡單版本：尋找第一個看起來像時間戳的部分
    let words: Vec<&str> = line.split_whitespace().collect();
    for word in words {
        // 檢查是否符合基本時間戳格式
        let clean = word.trim_matches(['[', ']']);
        if clean.len() >= 19 && 
           clean.chars().nth(4) == Some('-') && 
           clean.chars().nth(7) == Some('-') &&
           (clean.chars().nth(10) == Some('T') || clean.chars().nth(10) == Some(' ')) &&
           clean.chars().nth(13) == Some(':') &&
           clean.chars().nth(16) == Some(':') {
            return Some(clean.to_string());
        }
    }
    None
}

/// 提取錯誤類型（使用 ASCII 大小寫不敏感搜尋，避免 Unicode 轉大寫改變 Byte 長度造成切片 Panic）
fn extract_error_type(line: &str) -> String {
    let line_ascii_upper = line.to_ascii_uppercase();
    // 優先匹配行首或獨立嚴重度前綴（例如 "CRITICAL "、"ERROR: "），否則退回子字串匹配
    for (kw, default_err) in [("CRITICAL", "CriticalError"), ("ERROR", "UnknownError")] {
        let pos_opt = if line_ascii_upper.starts_with(kw) {
            Some(0)
        } else {
            line_ascii_upper.find(&format!("{} ", kw))
                .or_else(|| line_ascii_upper.find(&format!("{}:", kw)))
                .or_else(|| line_ascii_upper.find(kw))
        };
        if let Some(pos) = pos_opt {
            let start = pos + kw.len();
            if let Some(after_raw) = line.get(start..) {
                let after = after_raw.trim_start_matches([':', ' ', '\t', '-']);
                if let Some(end) = after.find([':', ' ', '\t', '[']) {
                    if let Some(extracted) = after.get(..end) {
                        if !extracted.is_empty() {
                            return extracted.to_string();
                        }
                    }
                }
                let word = after.split_whitespace().next().unwrap_or(default_err);
                return word.trim_end_matches(':').to_string();
            }
        }
    }
    "UnknownError".to_string()
}

/// 提取堆疊追蹤
fn extract_stack_trace(content: &str, error_line: usize) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    extract_stack_trace_from_lines(&lines, error_line)
}

fn extract_stack_trace_from_lines(lines: &[&str], error_line: usize) -> Option<String> {
    let mut trace = Vec::new();
    
    // 往後找最多 50 行作為堆疊
    for i in (error_line + 1)..lines.len().min(error_line + 50) {
        let line = lines[i].trim();
        if line.is_empty() {
            break;
        }
        // 堆疊追蹤通常以 "at " 或 "   at " 開頭，或包含文件路徑:行號
        if line.starts_with("at ") || line.contains(".rs:") || line.contains(".js:") || line.contains(".py:") {
            trace.push(line);
        } else if trace.is_empty() {
            // 第一行可能不是標準格式
            trace.push(line);
        } else {
            break;
        }
    }
    
    if trace.is_empty() {
        None
    } else {
        Some(trace.join("\n"))
    }
}

/// 讀取熔斷器狀態
pub async fn read_circuit_breaker(path: &Path) -> Result<CircuitBreakerStatus, AgentError> {
    if !path.exists() {
        // 檔案不存在視為未熔斷
        return Ok(CircuitBreakerStatus {
            tripped: false,
            tripped_at: None,
            reason: None,
            failure_count: 0,
            last_failure: None,
        });
    }
    
    let content = fs::read_to_string(path)
        .map_err(|e| AgentError::Other(format!("Failed to read circuit breaker: {e}")))?;
    
    let file: CircuitBreakerFile = serde_json::from_str(&content)
        .map_err(|e| AgentError::Other(format!("Failed to parse circuit breaker: {e}")))?;
    
    Ok(CircuitBreakerStatus {
        tripped: file.tripped,
        tripped_at: file.tripped_at,
        reason: file.reason,
        failure_count: file.failure_count,
        last_failure: file.last_failure,
    })
}

/// 寫入熔斷器狀態
pub async fn write_circuit_breaker(path: &Path, status: &CircuitBreakerFile) -> Result<(), AgentError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AgentError::Other(format!("Failed to create directory: {e}")))?;
    }
    
    let content = serde_json::to_string_pretty(status)
        .map_err(|e| AgentError::Other(format!("Failed to serialize circuit breaker: {e}")))?;
    
    fs::write(path, content)
        .map_err(|e| AgentError::Other(format!("Failed to write circuit breaker: {e}")))?;
    
    Ok(())
}

/// 執行完整分析
pub async fn run_analysis(config: &Config) -> Result<AnalysisResult, AgentError> {
    let work_logs_path = config.health_guardian.as_ref()
        .map(|h| h.work_logs_path.clone())
        .unwrap_or_else(|| PathBuf::from(".buzz/WORK_LOGS"));
    
    let circuit_breaker_path = config.health_guardian.as_ref()
        .map(|h| h.circuit_breaker_path.clone())
        .unwrap_or_else(|| PathBuf::from(".buzz/WORK_LOGS/circuit_breaker.json"));
    
    let failures = scan_failure_logs(&work_logs_path).await?;
    let circuit_breaker = read_circuit_breaker(&circuit_breaker_path).await?;
    
    Ok(AnalysisResult {
        failures,
        circuit_breakers: vec![circuit_breaker],
        timestamp: now_timestamp(),
    })
}

/// 取得當前時間戳
fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_scan_failure_logs_empty_dir() {
        let temp = TempDir::new().unwrap();
        let result = scan_failure_logs(temp.path()).await.unwrap();
        assert!(result.is_empty());
    }
    
    #[tokio::test]
    async fn test_scan_failure_logs_with_errors() {
        let temp = TempDir::new().unwrap();
        let log_file = temp.path().join("test_component.log");
        fs::write(&log_file, "2026-01-01T12:00:00Z ERROR TestError: something failed\n   at test.rs:10\n   at main.rs:5\n").unwrap();
        
        let result = scan_failure_logs(temp.path()).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].component, "test_component");
        assert_eq!(result[0].error_type, "TestError");
        assert_eq!(result[0].severity, Severity::Error);
    }
    
    #[tokio::test]
    async fn test_read_write_circuit_breaker() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("circuit_breaker.json");
        
        // Read non-existent
        let cb = read_circuit_breaker(&path).await.unwrap();
        assert!(!cb.tripped);
        assert_eq!(cb.failure_count, 0);
        
        // Write and read back
        let file = CircuitBreakerFile {
            tripped: true,
            tripped_at: Some("1234567890".into()),
            reason: Some("test".into()),
            failure_count: 5,
            last_failure: Some("1234567890".into()),
            component: Some("test".into()),
        };
        write_circuit_breaker(&path, &file).await.unwrap();
        
        let cb = read_circuit_breaker(&path).await.unwrap();
        assert!(cb.tripped);
        assert_eq!(cb.failure_count, 5);
        assert_eq!(cb.reason, Some("test".into()));
    }
    
    #[test]
    fn test_parse_severity() {
        assert_eq!(parse_severity("CRITICAL: something"), Severity::Critical);
        assert_eq!(parse_severity("FATAL error"), Severity::Critical);
        assert_eq!(parse_severity("ERROR: failed"), Severity::Error);
        assert_eq!(parse_severity("ERRO[0000] msg"), Severity::Error);
        assert_eq!(parse_severity("WARN: warning"), Severity::Warning);
        assert_eq!(parse_severity("INFO: info"), Severity::Info);
    }
    
    #[test]
    fn test_extract_error_type() {
        assert_eq!(extract_error_type("ERROR ConnectionError: failed"), "ConnectionError");
        assert_eq!(extract_error_type("CRITICAL TimeoutError at"), "TimeoutError");
    }
}
   
