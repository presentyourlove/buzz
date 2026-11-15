//! 儲存層 - SQLite 持久化

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::types::*;

/// Antigravity 資料存儲
#[derive(Clone)]
pub struct AntigravityStore {
    conn: Arc<Mutex<Connection>>,
}

impl AntigravityStore {
    pub fn new(db_path: &Path, wal_mode: bool) -> Result<Self> {
        std::fs::create_dir_all(db_path.parent().unwrap_or(Path::new(".")))?;
        
        let conn = Connection::open(db_path)?;
        
        if wal_mode {
            conn.execute("PRAGMA journal_mode=WAL", [])?;
        }
        
        conn.execute("PRAGMA foreign_keys=ON", [])?;
        conn.execute("PRAGMA busy_timeout=30000", [])?;
        
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        
        store.init_schema()?;
        Ok(store)
    }
    
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.blocking_lock();
        
        // 演化報告表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS evolution_reports (
                id TEXT PRIMARY KEY,
                date TEXT NOT NULL,
                success INTEGER NOT NULL,
                stages_total INTEGER NOT NULL,
                stages_completed INTEGER NOT NULL,
                duration_seconds REAL NOT NULL,
                summary_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // 階段結果表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS stage_results (
                id TEXT PRIMARY KEY,
                report_id TEXT NOT NULL,
                stage_code TEXT NOT NULL,
                stage_name TEXT NOT NULL,
                success INTEGER NOT NULL,
                duration_seconds REAL NOT NULL,
                started_at TEXT NOT NULL,
                completed_at TEXT NOT NULL,
                output_json TEXT,
                error TEXT,
                metrics_json TEXT,
                FOREIGN KEY(report_id) REFERENCES evolution_reports(id)
            )
            "#,
            [],
        )?;
        
        // 任務表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL,
                priority INTEGER NOT NULL,
                assignee TEXT,
                tags_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                due_date TEXT,
                source TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // 共識項目表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS consensus_items (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                source_message_id TEXT,
                source_channel TEXT,
                agreed_by_json TEXT NOT NULL,
                decided_at TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // RCA 發現表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS rca_findings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                root_cause TEXT NOT NULL,
                impact TEXT NOT NULL,
                severity INTEGER NOT NULL,
                category TEXT NOT NULL,
                affected_components_json TEXT NOT NULL,
                recommended_actions_json TEXT NOT NULL,
                discovered_at TEXT NOT NULL,
                status TEXT NOT NULL,
                report_date TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // 代碼健康問題表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS health_issues (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                category TEXT NOT NULL,
                severity INTEGER NOT NULL,
                file_path TEXT,
                line_number INTEGER,
                rule_id TEXT,
                detected_at TEXT NOT NULL,
                status TEXT NOT NULL,
                report_date TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // CI/CD 癒合記錄表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS cicd_heal_records (
                id TEXT PRIMARY KEY,
                workflow_name TEXT NOT NULL,
                run_id TEXT NOT NULL,
                failure_reason TEXT NOT NULL,
                heal_action TEXT NOT NULL,
                success INTEGER NOT NULL,
                attempted_at TEXT NOT NULL,
                completed_at TEXT,
                report_date TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // 知識同步記錄表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS knowledge_sync_records (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                source_path TEXT NOT NULL,
                target_path TEXT NOT NULL,
                action TEXT NOT NULL,
                success INTEGER NOT NULL,
                synced_at TEXT NOT NULL,
                error TEXT,
                report_date TEXT NOT NULL
            )
            "#,
            [],
        )?;
        
        // 索引
        conn.execute("CREATE INDEX IF NOT EXISTS idx_evolution_reports_date ON evolution_reports(date)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_stage_results_report ON stage_results(report_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_consensus_status ON consensus_items(status)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_rca_report_date ON rca_findings(report_date)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_health_report_date ON health_issues(report_date)", [])?;
        
        Ok(())
    }
    
    /// 儲存演化報告
    pub async fn save_report(&self, report: &EvolutionReport) -> Result<()> {
        let conn = self.conn.lock().await;
        let id = Uuid::new_v4().to_string();
        
        conn.execute(
            r#"
            INSERT INTO evolution_reports (id, date, success, stages_total, stages_completed, duration_seconds, summary_json, metadata_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                id,
                report.date.to_string(),
                report.success as i32,
                report.stages_total as i64,
                report.stages_completed as i64,
                report.duration_seconds,
                serde_json::to_string(&report.summary)?,
                serde_json::to_string(&report.metadata)?,
                report.metadata.generated_at.to_rfc3339(),
            ],
        )?;
        
        for stage in &report.stage_results {
            let stage_id = Uuid::new_v4().to_string();
            conn.execute(
                r#"
                INSERT INTO stage_results (id, report_id, stage_code, stage_name, success, duration_seconds, started_at, completed_at, output_json, error, metrics_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                "#,
                params![
                    stage_id,
                    id,
                    stage.stage.code(),
                    stage.name,
                    stage.success as i32,
                    stage.duration_seconds,
                    stage.started_at.to_rfc3339(),
                    stage.completed_at.to_rfc3339(),
                    serde_json::to_string(&stage.output)?,
                    stage.error.clone(),
                    serde_json::to_string(&stage.metrics)?,
                ],
            )?;
        }
        
        Ok(())
    }

    pub async fn save_consensus(&self, _items: &[ConsensusItem], _date: NaiveDate) -> Result<()> {
        Ok(())
    }

    pub async fn save_rca_findings(&self, _findings: &[RcaFinding], _date: NaiveDate) -> Result<()> {
        Ok(())
    }

    pub async fn save_health_issues(&self, _issues: &[HealthIssue], _date: NaiveDate) -> Result<()> {
        Ok(())
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<crate::task_sync::SyncedTask>> {
        Ok(Vec::new())
    }

    pub async fn save_task(&self, _task: &crate::task_sync::SyncedTask) -> Result<()> {
        Ok(())
    }

    pub async fn save_knowledge(&self, _knowledge: &crate::knowledge_sync::SyncedKnowledge) -> Result<()> {
        Ok(())
    }

    pub async fn save_heal_action(&self, _action: &crate::cicd_heal::HealAction) -> Result<()> {
        Ok(())
    }
}
