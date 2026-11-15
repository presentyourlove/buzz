//! 核心類型定義

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 演化管線階段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    #[serde(rename = "01")]
    DisciplineGuard,
    #[serde(rename = "02")]
    ConsensusIngest,
    #[serde(rename = "03")]
    BottleneckRCA,
    #[serde(rename = "04")]
    CodeHealthCheck,
    #[serde(rename = "05")]
    TaskSync,
    #[serde(rename = "06")]
    KnowledgeSync,
    #[serde(rename = "07")]
    CicdSelfHeal,
}

impl PipelineStage {
    pub fn all() -> Vec<PipelineStage> {
        vec![
            PipelineStage::DisciplineGuard,
            PipelineStage::ConsensusIngest,
            PipelineStage::BottleneckRCA,
            PipelineStage::CodeHealthCheck,
            PipelineStage::TaskSync,
            PipelineStage::KnowledgeSync,
            PipelineStage::CicdSelfHeal,
        ]
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "01" => Some(PipelineStage::DisciplineGuard),
            "02" => Some(PipelineStage::ConsensusIngest),
            "03" => Some(PipelineStage::BottleneckRCA),
            "04" => Some(PipelineStage::CodeHealthCheck),
            "05" => Some(PipelineStage::TaskSync),
            "06" => Some(PipelineStage::KnowledgeSync),
            "07" => Some(PipelineStage::CicdSelfHeal),
            _ => None,
        }
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            PipelineStage::DisciplineGuard => "DisciplineGuard",
            PipelineStage::ConsensusIngest => "ConsensusIngest",
            PipelineStage::BottleneckRCA => "BottleneckRCA",
            PipelineStage::CodeHealthCheck => "CodeHealthCheck",
            PipelineStage::TaskSync => "TaskSync",
            PipelineStage::KnowledgeSync => "KnowledgeSync",
            PipelineStage::CicdSelfHeal => "CicdSelfHeal",
        }
    }
    
    pub fn code(&self) -> &'static str {
        match self {
            PipelineStage::DisciplineGuard => "01",
            PipelineStage::ConsensusIngest => "02",
            PipelineStage::BottleneckRCA => "03",
            PipelineStage::CodeHealthCheck => "04",
            PipelineStage::TaskSync => "05",
            PipelineStage::KnowledgeSync => "06",
            PipelineStage::CicdSelfHeal => "07",
        }
    }
}

/// 階段執行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage: PipelineStage,
    pub name: String,
    pub success: bool,
    pub duration_seconds: f64,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub metrics: HashMap<String, serde_json::Value>,
}

impl StageResult {
    pub fn new(stage: PipelineStage) -> Self {
        let now = Utc::now();
        Self {
            stage,
            name: stage.name().to_string(),
            success: false,
            duration_seconds: 0.0,
            started_at: now,
            completed_at: now,
            output: None,
            error: None,
            metrics: HashMap::new(),
        }
    }
    
    pub fn complete(&mut self, success: bool, output: Option<serde_json::Value>, error: Option<String>) {
        self.completed_at = Utc::now();
        self.duration_seconds = (self.completed_at - self.started_at).num_milliseconds() as f64 / 1000.0;
        self.success = success;
        self.output = output;
        self.error = error;
    }
}

/// 管線配置
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub date: NaiveDate,
    pub dry_run: bool,
    pub skip_stages: Vec<String>,
    pub only_stages: Vec<String>,
    pub json_only: bool,
}

impl PipelineConfig {
    pub fn should_run(&self, stage: PipelineStage) -> bool {
        let code = stage.code();
        if !self.only_stages.is_empty() {
            return self.only_stages.iter().any(|s| s == code);
        }
        !self.skip_stages.iter().any(|s| s == code)
    }
    
    pub fn stages_to_run(&self) -> Vec<PipelineStage> {
        PipelineStage::all()
            .into_iter()
            .filter(|s| self.should_run(*s))
            .collect()
    }
}

/// Nostr 事件載荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrEventPayload {
    pub kind: u16,
    pub content: String,
    pub tags: Vec<Vec<String>>,
    pub created_at: u64,
    pub pubkey: String,
}

/// 任務定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub assignee: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub due_date: Option<NaiveDate>,
    pub source: TaskSource,
}

/// 任務狀態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    InReview,
    Done,
    Blocked,
    Cancelled,
}

/// 任務優先級
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// 任務來源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskSource {
    DailyEvolution,
    Manual,
    Issue,
    PR,
}

/// 共識項目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusItem {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub category: ConsensusCategory,
    pub source_message_id: Option<String>,
    pub source_channel: Option<String>,
    pub agreed_by: Vec<String>,
    pub decided_at: DateTime<Utc>,
    pub status: ConsensusStatus,
}

/// 共識類別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusCategory {
    Architecture,
    Process,
    TechnicalDebt,
    Security,
    Deployment,
    Testing,
    Documentation,
}

/// 共識狀態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusStatus {
    Proposed,
    Agreed,
    Implemented,
    Superseded,
    Rejected,
}

/// RCA 發現
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaFinding {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub root_cause: String,
    pub impact: String,
    pub severity: RcaSeverity,
    pub category: RcaCategory,
    pub affected_components: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub discovered_at: DateTime<Utc>,
    pub status: RcaStatus,
}

/// RCA 嚴重度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RcaSeverity {
    Info = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// RCA 類別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RcaCategory {
    Performance,
    Reliability,
    Security,
    Maintainability,
    Scalability,
    Observability,
}

/// RCA 狀態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RcaStatus {
    Open,
    Investigating,
    Mitigated,
    Resolved,
    WontFix,
}

/// 代碼健康問題
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub category: HealthCategory,
    pub severity: HealthSeverity,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub rule_id: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub status: HealthStatus,
}

/// 健康類別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCategory {
    CompilationWarning,
    DependencyVulnerability,
    TestCoverage,
    CodeComplexity,
    DeadCode,
    UnusedDependency,
    SecurityAudit,
    PerformanceRegression,
}

/// 健康嚴重度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HealthSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Critical = 3,
}

/// 健康狀態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Open,
    InProgress,
    Fixed,
    Ignored,
}

/// 每日進化報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionReport {
    pub id: Uuid,
    pub date: NaiveDate,
    pub success: bool,
    pub stages_total: usize,
    pub stages_completed: usize,
    pub duration_seconds: f64,
    pub stage_results: Vec<StageResult>,
    pub summary: EvolutionSummary,
    pub metadata: ReportMetadata,
}

/// 演化統計摘要
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvolutionSummary {
    pub consensus_items: usize,
    pub rca_findings: usize,
    pub health_issues: usize,
    pub tasks_synced: usize,
    pub knowledge_synced: usize,
    pub cicd_healed: usize,
    pub high_risk_actions: Vec<HighRiskAction>,
    pub autonomous_actions: Vec<AutonomousAction>,
}

/// 報告元數據
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub generated_at: DateTime<Utc>,
    pub version: String,
    pub git_commit: String,
    pub rust_version: String,
}

/// 🛡️ 高風險 / 建議後台架構操作項（由主理人 / Antigravity 外部執行）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighRiskAction {
    pub id: String,
    pub title: String,
    pub target: String,
    pub reason: String,
    pub why_backend_only: String,
    pub suggested_steps: Vec<String>,
}

/// ⚡ 低風險 / 特工自主優化項（由 Buzz / Pollen / Honey2 執行）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousAction {
    pub id: String,
    pub title: String,
    pub assignee: String,
    pub target_file: String,
    pub goal: String,
    pub acceptance_criteria: String,
}

/// 通用行動項
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub id: String,
    pub source_stage: String,
    pub title: String,
    pub priority: String,
    pub assignee: String,
    pub due_date: String,
    pub acceptance_criteria: String,
} 
