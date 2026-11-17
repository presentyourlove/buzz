//! Audit Logging System
//! 
//! Provides comprehensive audit logging for browser automation activities,
//! including action tracking, credential usage, and security events.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Audit event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// Browser session started
    SessionStart,
    /// Browser session ended
    SessionEnd,
    /// Navigation action
    Navigate,
    /// Click action
    Click,
    /// Type text action
    TypeText,
    /// Screenshot taken
    Screenshot,
    /// PDF generated
    PdfGenerated,
    /// JavaScript evaluated
    EvalJS,
    /// Wait for condition
    WaitFor,
    /// File downloaded
    Download,
    /// File uploaded
    Upload,
    /// Credential accessed
    CredentialAccess,
    /// Credential used for authentication
    CredentialUsed,
    /// Cookie accessed
    CookieAccess,
    /// Network request intercepted
    NetworkRequest,
    /// Security violation attempt
    SecurityViolation,
    /// Error occurred
    Error,
    /// Capability check failed
    CapabilityDenied,
}

/// Audit event severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Audit event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: Uuid,
    /// Event type
    pub event_type: AuditEventType,
    /// Event severity
    pub severity: AuditSeverity,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Session ID
    pub session_id: Uuid,
    /// Context ID
    pub context_id: Option<Uuid>,
    /// Capability ID
    pub capability_id: Option<Uuid>,
    /// User/agent identifier
    pub actor: String,
    /// Action description
    pub action: String,
    /// Target (URL, selector, etc.)
    pub target: Option<String>,
    /// Result (success, failure, error message)
    pub result: AuditResult,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

/// Audit result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuditResult {
    Success { details: Option<String> },
    Failure { error: String },
    Denied { reason: String },
}

/// Audit logger configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Log file path
    pub log_path: PathBuf,
    /// Maximum log file size in MB
    pub max_file_size_mb: u64,
    /// Maximum number of rotated files
    pub max_files: usize,
    /// Enable console output
    pub console_output: bool,
    /// Minimum severity to log
    pub min_severity: AuditSeverity,
    /// Enable JSON format
    pub json_format: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_path: PathBuf::from("logs/browser_audit.log"),
            max_file_size_mb: 100,
            max_files: 10,
            console_output: true,
            min_severity: AuditSeverity::Info,
            json_format: true,
        }
    }
}

/// Audit logger
#[derive(Debug)]
pub struct AuditLogger {
    config: AuditConfig,
    writer: Arc<Mutex<BufWriter<std::fs::File>>>,
    current_size: Arc<Mutex<u64>>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub async fn new(config: AuditConfig) -> Result<Self> {
        // Ensure log directory exists
        if let Some(parent) = config.log_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_path)?;
        
        let metadata = file.metadata()?;
        let current_size = metadata.len();
        
        let writer = Arc::new(Mutex::new(BufWriter::new(file)));
        let current_size = Arc::new(Mutex::new(current_size));
        
        Ok(Self {
            config,
            writer,
            current_size,
        })
    }
    
    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) -> Result<()> {
        // Check severity filter
        if (event.severity as u8) < self.config.min_severity as u8 {
            return Ok(());
        }
        
        let line = if self.config.json_format {
            serde_json::to_string(&event)?
        } else {
            format!(
                "{} [{}] {} - {} - {}",
                event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                event.severity as u8,
                event.event_type as u8,
                event.action,
                event.result
            )
        };
        
        // Write to file
        {
            let mut writer = self.writer.lock().await;
            writeln!(writer, "{}", line)?;
            writer.flush()?;
        }
        
        // Update size and rotate if needed
        let mut size = self.current_size.lock().await;
        *size += line.len() as u64 + 1; // +1 for newline
        
        if *size > self.config.max_file_size_mb * 1024 * 1024 {
            drop(size);
            self.rotate().await?;
        }
        
        // Console output
        if self.config.console_output {
            match event.severity {
                AuditSeverity::Info => info!("AUDIT: {}", event.action),
                AuditSeverity::Warning => warn!("AUDIT: {}", event.action),
                AuditSeverity::Error | AuditSeverity::Critical => {
                    tracing::error!("AUDIT: {} - {:?}", event.action, event.result)
                }
            }
        }
        
        Ok(())
    }
    
    /// Log a simple event
    pub async fn log_event(
        &self,
        event_type: AuditEventType,
        severity: AuditSeverity,
        session_id: Uuid,
        actor: &str,
        action: &str,
        result: AuditResult,
    ) -> Result<()> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            event_type,
            severity,
            timestamp: Utc::now(),
            session_id,
            context_id: None,
            capability_id: None,
            actor: actor.to_string(),
            action: action.to_string(),
            target: None,
            result,
            metadata: HashMap::new(),
            duration_ms: None,
        };
        
        self.log(event).await
    }
    
    /// Rotate log files
    async fn rotate(&self) -> Result<()> {
        // Close current writer
        {
            let mut writer = self.writer.lock().await;
            writer.flush()?;
        }
        
        // Rotate files
        for i in (1..self.config.max_files).rev() {
            let from = self.config.log_path.with_extension(format!("log.{}", i));
            let to = self.config.log_path.with_extension(format!("log.{}", i + 1));
            if from.exists() {
                if i + 1 >= self.config.max_files {
                    tokio::fs::remove_file(&from).await?;
                } else {
                    tokio::fs::rename(&from, &to).await?;
                }
            }
        }
        
        // Move current to .1
        let rotated = self.config.log_path.with_extension("log.1");
        tokio::fs::rename(&self.config.log_path, &rotated).await?;
        
        // Create new file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_path)?;
        
        {
            let mut writer = self.writer.lock().await;
            *writer = BufWriter::new(file);
        }
        
        {
            let mut size = self.current_size.lock().await;
            *size = 0;
        }
        
        info!("Audit log rotated");
    }
}
