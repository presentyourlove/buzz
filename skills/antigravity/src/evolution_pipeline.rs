//! 演化管線協調器

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, warn, instrument};
use uuid::Uuid;

use crate::types::*;
use crate::config::AntigravityConfig;
use crate::storage::AntigravityStore;
use crate::nostr_bridge::NostrBridge;
use crate::discipline_guard::DisciplineGuard;
use crate::consensus_ingest::ConsensusIngest;
use crate::bottleneck_rca::BottleneckRca;
use crate::code_health::CodeHealthCheck;
use crate::task_sync::TaskSync;
use crate::knowledge_sync::KnowledgeSync;
use crate::cicd_heal::CicdHeal;

/// 演化管線
pub struct EvolutionPipeline {
    config: AntigravityConfig,
    pipeline_config: PipelineConfig,
    store: Arc<AntigravityStore>,
    nostr_bridge: Option<Arc<NostrBridge>>,
    
    // 階段處理器
    discipline_guard: DisciplineGuard,
    consensus_ingest: ConsensusIngest,
    bottleneck_rca: BottleneckRca,
    code_health: CodeHealthCheck,
    task_sync: TaskSync,
    knowledge_sync: KnowledgeSync,
    cicd_heal: CicdHeal,
}

impl EvolutionPipeline {
    pub async fn new(config: AntigravityConfig, pipeline_config: PipelineConfig) -> Result<Self> {
        let store = Arc::new(AntigravityStore::new(
            &config.storage.db_path,
            config.storage.wal_mode,
        )?);
        
        // Nostr 橋接器（可選，如果配置了私鑰）
        let nostr_bridge = if std::env::var(&config.nostr.private_key_env).is_ok() {
            Some(Arc::new(NostrBridge::new(config.clone()).await?))
        } else {
            warn!("未配置 Nostr 私鑰，跳過事件發布");
            None
        };
        
        Ok(Self {
            config: config.clone(),
            pipeline_config,
            store,
            nostr_bridge,
            discipline_guard: DisciplineGuard::new(),
            consensus_ingest: ConsensusIngest::new(),
            bottleneck_rca: BottleneckRca::new(),
            code_health: CodeHealthCheck::new(),
            task_sync: TaskSync::new(),
            knowledge_sync: KnowledgeSync::new(),
            cicd_heal: CicdHeal::new(config.clone(), store.clone()),
        })
    }
    
    #[instrument(skip(self))]
    pub async fn execute(&mut self) -> Result<EvolutionReport> {
        let start_time = Utc::now();
        let date = self.pipeline_config.date;
        let stages = self.pipeline_config.stages_to_run();
        
        info!("啟動演化管線 - 日期: {}, 階段: {:?}", date, stages.iter().map(|s| s.code()).collect::<Vec<_>>());
        
        let mut stage_results = Vec::new();
        let mut summary = EvolutionSummary::default();
        let mut all_success = true;
        
        for stage in &stages {
            let mut stage_result = StageResult::new(*stage);
            let stage_start = Utc::now();
            
            info!("執行階段: {} ({})", stage.name(), stage.code());
            
            let result = match stage {
                PipelineStage::DisciplineGuard => self.run_discipline_guard(&mut stage_result).await,
                PipelineStage::ConsensusIngest => self.run_consensus_ingest(&mut stage_result, &mut summary).await,
                PipelineStage::BottleneckRCA => self.run_bottleneck_rca(&mut stage_result, &mut summary).await,
                PipelineStage::CodeHealthCheck => self.run_code_health(&mut stage_result, &mut summary).await,
                PipelineStage::TaskSync => self.run_task_sync(&mut stage_result, &mut summary).await,
                PipelineStage::KnowledgeSync => self.run_knowledge_sync(&mut stage_result, &mut summary).await,
                PipelineStage::CicdSelfHeal => self.run_cicd_heal(&mut stage_result, &mut summary).await,
            };
            
            if let Err(e) = result {
                error!("階段 {} 執行失敗: {}", stage.code(), e);
                stage_result.complete(false, None, Some(e.to_string()));
                all_success = false;
            } else {
                stage_result.complete(true, None, None);
            }
            
            stage_results.push(stage_result);
        }
        
        let duration = (Utc::now() - start_time).num_milliseconds() as f64 / 1000.0;
        let stages_completed = stage_results.iter().filter(|s| s.success).count();
        
        // 生成報告元數據
        let metadata = ReportMetadata {
            generated_at: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_commit: Self::get_git_commit(),
            rust_version: Self::get_rust_version(),
        };
        
        let report = EvolutionReport {
            id: Uuid::now_v7(),
            date,
            success: all_success,
            stages_total: stages.len(),
            stages_completed,
            duration_seconds: duration,
            stage_results,
            summary,
            metadata,
        };
        
        // 儲存報告
        if !self.pipeline_config.dry_run {
            self.store.save_report(&report).await?;
            
            // 發布 Nostr 事件
            if let Some(bridge) = &self.nostr_bridge {
                if let Err(e) = bridge.publish_evolution_report(&report).await {
                    error!("發布 Nostr 事件失敗: {}", e);
                }
            }
        }
        
        // 生成輸出檔案
        self.write_output_files(&report).await?;
        
        info!("演化管線完成 - 成功: {}, 耗時: {:.2}s", all_success, duration);
        
        Ok(report)
    }
    
    async fn run_discipline_guard(&self, stage_result: &mut StageResult) -> Result<()> {
        let result = self.discipline_guard.check(&self.config).await?;
        stage_result.output = Some(serde_json::to_value(&result)?);
        stage_result.metrics.insert("violations".to_string(), serde_json::to_value(result.violation_count)?);
        Ok(())
    }
    
    async fn run_consensus_ingest(&self, stage_result: &mut StageResult, summary: &mut EvolutionSummary) -> Result<()> {
        let items = self.consensus_ingest.extract().await?;
        summary.consensus_items = items.len();
        
        if !self.pipeline_config.dry_run && !items.is_empty() {
            self.store.save_consensus(&items, self.pipeline_config.date).await?;
            
            if let Some(bridge) = &self.nostr_bridge {
                bridge.publish_consensus(&items).await?;
            }
        }
        
        stage_result.output = Some(serde_json::to_value(&items)?);
        stage_result.metrics.insert("items_count".to_string(), serde_json::to_value(items.len())?);
        Ok(())
    }
    
    async fn run_bottleneck_rca(&self, stage_result: &mut StageResult, summary: &mut EvolutionSummary) -> Result<()> {
        let findings = self.bottleneck_rca.analyze().await?;
        summary.rca_findings = findings.len();
        
        if !self.pipeline_config.dry_run && !findings.is_empty() {
            self.store.save_rca_findings(&findings, self.pipeline_config.date).await?;
            
            if let Some(bridge) = &self.nostr_bridge {
                bridge.publish_rca_findings(&findings).await?;
            }
        }
        
        stage_result.output = Some(serde_json::to_value(&findings)?);
        stage_result.metrics.insert("findings_count".to_string(), serde_json::to_value(findings.len())?);
        Ok(())
    }
    
    async fn run_code_health(&self, stage_result: &mut StageResult, summary: &mut EvolutionSummary) -> Result<()> {
        let issues = self.code_health.check().await?;
        summary.health_issues = issues.len();
        
        if !self.pipeline_config.dry_run && !issues.is_empty() {
            self.store.save_health_issues(&issues, self.pipeline_config.date).await?;
            
            if let Some(bridge) = &self.nostr_bridge {
                bridge.publish_health_issues(&issues).await?;
            }
        }
        
        stage_result.output = Some(serde_json::to_value(&issues)?);
        stage_result.metrics.insert("issues_count".to_string(), serde_json::to_value(issues.len())?);
        Ok(())
    }
    
    async fn run_task_sync(&self, stage_result: &mut StageResult, summary: &mut EvolutionSummary) -> Result<()> {
        let tasks = self.task_sync.sync().await?;
        summary.tasks_synced = tasks.len();

        if !self.pipeline_config.dry_run && !tasks.is_empty() {
            for task in &tasks {
                self.store.save_task(task).await?;
            }

            if let Some(bridge) = &self.nostr_bridge {
                for task in &tasks {
                    bridge.publish_task_update(task).await?;
                }
            }
        }

        stage_result.output = Some(serde_json::to_value(&tasks)?);
        stage_result.metrics.insert("tasks_synced".to_string(), serde_json::to_value(tasks.len())?);
        Ok(())
    }

    async fn run_knowledge_sync(&self, stage_result: &mut StageResult, summary: &mut EvolutionSummary) -> Result<()> {
        let knowledge = self.knowledge_sync.sync().await?;
        summary.knowledge_synced = knowledge.len();

        if !self.pipeline_config.dry_run && !knowledge.is_empty() {
            for item in &knowledge {
                self.store.save_knowledge(item).await?;
            }

            if let Some(bridge) = &self.nostr_bridge {
                for item in &knowledge {
                    bridge.publish_knowledge(item).await?;
                }
            }
        }

        stage_result.output = Some(serde_json::to_value(&knowledge)?);
        stage_result.metrics.insert("knowledge_synced".to_string(), serde_json::to_value(knowledge.len())?);
        Ok(())
    }

    async fn run_cicd_heal(&self, stage_result: &mut StageResult, summary: &mut EvolutionSummary) -> Result<()> {
        let actions = self.cicd_heal.heal().await?;
        summary.cicd_healed = actions.len();

        if !self.pipeline_config.dry_run && !actions.is_empty() {
            for action in &actions {
                self.store.save_heal_action(action).await?;
            }
        }

        stage_result.output = Some(serde_json::to_value(&actions)?);
        stage_result.metrics.insert("actions_executed".to_string(), serde_json::to_value(actions.len())?);
        Ok(())
    }

    fn get_git_commit() -> String {
        "local".to_string()
    }

    fn get_rust_version() -> String {
        "rustc".to_string()
    }

    /// 落盤詳細報告、極簡速讀版與 JSON 數據（支援專案集中路徑）
    async fn write_output_files(&self, report: &EvolutionReport) -> Result<()> {
        let reports_dir = Path::new("C:/私人/ai agent/buzz/REPORTS");
        let outbox_dir = Path::new("C:/私人/ai agent/buzz/OUTBOX");
        crate::report_renderer::ReportRenderer::write_outputs(report, reports_dir, outbox_dir)?;
        Ok(())
    }
}
