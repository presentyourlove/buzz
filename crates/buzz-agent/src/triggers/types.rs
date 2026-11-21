//! 核心觸發器類型定義
//!
//! 依據 PLANS/PROACTIVE_TRIGGERS_PLAN.md 設計，參考 skills/agent-skill/src/task_store.rs 與 skills/antigravity/src/types.rs

use crate::triggers::condition::{AtomicCondition, ComparisonOperator, Condition, ConditionNode, FactProvider};
use crate::triggers::event_bus::EventBusConfig;
use crate::triggers::scheduler::SchedulerConfig;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// 觸發器主結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub trigger_type: TriggerType,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub deduplication: DeduplicationConfig,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub fire_count: u64,
    pub next_fire: Option<DateTime<Utc>>,
}

/// 觸發類型枚舉
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerType {
    /// 定時觸發
    Schedule {
        #[serde(default)]
        cron: Option<String>,
        #[serde(default)]
        interval_secs: Option<u64>,
        #[serde(default = "default_timezone")]
        timezone: String,
        #[serde(default)]
        jitter_secs: u64,
    },
    /// Nostr 事件觸發
    NostrEvent {
        kinds: Vec<u16>,
        #[serde(default)]
        authors: Vec<String>,
        #[serde(default)]
        tags: Vec<Vec<String>>,
        #[serde(default = "default_relay_urls")]
        relay_urls: Vec<String>,
    },
    /// 檔案系統觸發
    FileSystem {
        paths: Vec<String>,
        #[serde(default = "default_fs_events")]
        events: Vec<FsEventType>,
        #[serde(default)]
        recursive: bool,
        #[serde(default)]
        debounce_ms: u64,
    },
    /// Webhook 觸發
    Webhook {
        #[serde(default = "default_webhook_path")]
        path: String,
        #[serde(default)]
        secret: Option<String>,
        #[serde(default)]
        allowed_ips: Vec<String>,
    },
    /// 系統狀態閾值觸發
    SystemMetric {
        metric: SystemMetricType,
        operator: ComparisonOperator,
        threshold: f64,
        #[serde(default)]
        duration_secs: u64,
    },
}

/// 檔案系統事件類型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum FsEventType {
    Create,
    Modify,
    Delete,
    Rename,
}

/// 系統指標類型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemMetricType {
    CpuUsage,
    MemoryUsage,
    DiskUsage,
    NetworkLatency,
    QueueDepth,
    ErrorRate,
}

/// 動作定義
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    /// 提交任務到 agent-skill
    SubmitTask {
        task_type: String,
        agent_name: String,
        description: String,
        #[serde(default)]
        input_template: serde_json::Value,
        #[serde(default)]
        retry_policy: RetryPolicy,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// 發送 Nostr 事件
    PublishNostrEvent {
        kind: u16,
        content_template: String,
        #[serde(default)]
        tags_template: Vec<Vec<String>>,
        #[serde(default = "default_relay_urls")]
        relay_urls: Vec<String>,
        #[serde(default)]
        retry_policy: RetryPolicy,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// 發送通知到頻道
    SendNotification {
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        recipients: Vec<String>,
        template: String,
        #[serde(default)]
        retry_policy: RetryPolicy,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// 執行命令
    RunCommand {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default = "default_working_dir")]
        working_dir: String,
        #[serde(default)]
        retry_policy: RetryPolicy,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// 發出內部事件
    EmitEvent {
        event_type: String,
        #[serde(default)]
        payload: serde_json::Value,
    },
    /// HTTP 請求
    HttpRequest {
        method: HttpMethod,
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        body_template: Option<String>,
        #[serde(default)]
        retry_policy: RetryPolicy,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
}

/// HTTP 方法
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// 重試策略
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetryPolicy {
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub base_delay_secs: u64,
    #[serde(default = "default_max_delay")]
    pub max_delay_secs: u64,
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
}

/// 去重配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeduplicationConfig {
    pub key_template: String,
    pub window_secs: u64,
    pub max_executions_per_window: u32,
}

/// 觸發器引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_triggers: usize,
    #[serde(default = "default_event_buffer")]
    pub event_buffer_size: usize,
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
}

/// 事件總線訊息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub id: Uuid,
    pub source: EventSource,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// 事件來源
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Scheduler,
    NostrEvent,
    FileSystem,
    Webhook,
    SystemMetric,
    Manual,
}

/// 排程器下一次觸發時間
#[derive(Debug, Clone)]
pub struct NextFireTime {
    pub trigger_id: Uuid,
    pub next_fire: DateTime<Utc>,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_relay_urls() -> Vec<String> {
    vec!["wss://presentyourlove.communities.buzz.xyz".to_string()]
}

fn default_fs_events() -> Vec<FsEventType> {
    vec![FsEventType::Create, FsEventType::Modify]
}

fn default_webhook_path() -> String {
    "/webhook/triggers".to_string()
}

fn default_timeout() -> u64 {
    60
}

fn default_working_dir() -> String {
    ".".to_string()
}

fn default_max_delay() -> u64 {
    3600
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_check_interval() -> u64 {
    30
}

fn default_max_concurrent() -> usize {
    10
}

fn default_event_buffer() -> usize {
    1000
}

fn default_true() -> bool {
    true
}

/// TriggerRuntime 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRuntimeConfig {
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_triggers: usize,
    #[serde(default = "default_event_buffer")]
    pub event_buffer_size: usize,
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    #[serde(default)]
    pub scheduler_config: SchedulerConfig,
    #[serde(default)]
    pub event_bus_config: EventBusConfig,
}

impl Default for TriggerRuntimeConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 30,
            max_concurrent_triggers: 10,
            event_buffer_size: 1000,
            enable_metrics: true,
            scheduler_config: SchedulerConfig::default(),
            event_bus_config: EventBusConfig::default(),
        }
    }
}

/// TriggerRuntime 健康狀態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRuntimeHealth {
    pub is_running: bool,
    #[serde(default)]
    pub running: bool,
    pub registered_triggers: usize,
    pub active_triggers: usize,
    #[serde(default)]
    pub trigger_count: usize,
    pub last_check: DateTime<Utc>,
    pub events_processed: u64,
    pub errors_count: u64,
    pub uptime_secs: u64,
}
/// TriggerRuntime 錯誤類型
#[derive(Debug, Clone, thiserror::Error)]
pub enum TriggerRuntimeError {
    #[error("Trigger already running")]
    AlreadyRunning,
    #[error("Trigger not running")]
    NotRunning,
    #[error("Invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("Trigger not found: {0}")]
    TriggerNotFound(String),
    #[error("Scheduler error: {0}")]
    Scheduler(String),
    #[error("Condition evaluation error: {0}")]
    Condition(String),
    #[error("Event bus error: {0}")]
    EventBus(String),
    #[error("Fact provider error: {0}")]
    FactProvider(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Invalid trigger: {0}")]
    InvalidTrigger(String),
}

/// 觸發器觸發事件

impl Trigger {
    /// 建立新觸發器
    pub fn new(name: String, description: String, trigger_type: TriggerType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            enabled: true,
            trigger_type,
            conditions: Vec::new(),
            actions: Vec::new(),
            deduplication: DeduplicationConfig {
                key_template: "{{trigger.id}}".to_string(),
                window_secs: 3600,
                max_executions_per_window: 1,
            },
            created_at: now,
            updated_at: now,
            last_fired_at: None,
            fire_count: 0,
            next_fire: None,
        }
    }
}

/// 觸發器狀態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TriggerStatus {
    /// 正常運行中
    Active,
    /// 已禁用
    Disabled,
    /// 等待觸發
    Pending,
    /// 執行中
    Running,
    /// 錯誤狀態
    Error(String),
    /// 已完成（一次性觸發器）
    Completed,
}

/// 觸發器觸發事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerFiredEvent {
    pub trigger_id: Uuid,
    pub trigger_name: String,
    pub fired_at: DateTime<Utc>,
    pub condition_result: Option<bool>,
    pub actions_executed: Vec<String>,
    pub error: Option<String>,
}
