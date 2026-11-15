//! Antigravity - 架構演進官核心庫
//! 
//! 負責每日系統進化機制的自動化分析、架構決策萃取、
//! 技術債識別與自癒腳本調度。

pub mod evolution_pipeline;
pub mod discipline_guard;
pub mod consensus_ingest;
pub mod bottleneck_rca;
pub mod code_health;
pub mod task_sync;
pub mod knowledge_sync;
pub mod cicd_heal;
pub mod types;
pub mod config;
pub mod storage;
pub mod nostr_bridge;
pub mod report_renderer;

pub use crate::evolution_pipeline::EvolutionPipeline;
pub use crate::types::*;
pub use crate::config::AntigravityConfig;
pub use crate::storage::AntigravityStore;
pub use crate::nostr_bridge::NostrBridge;
pub use crate::report_renderer::ReportRenderer;

use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use tracing::{info, error};

/// CLI 參數定義
#[derive(Parser, Debug)]
#[command(name = "antigravity", version, about = "架構演進官 - 每日系統進化機制")]
pub struct Cli {
    /// 指定日期 YYYYMMDD
    #[arg(short, long)]
    pub date: Option<String>,
    
    /// 演練模式（不寫入資料庫、不發送事件）
    #[arg(long)]
    pub dry_run: bool,
    
    /// 跳過特定模組
    #[arg(long, num_args = 1..)]
    pub skip: Vec<String>,
    
    /// 僅執行特定模組
    #[arg(long, num_args = 1..)]
    pub only: Vec<String>,
    
    /// 純 JSON 輸出
    #[arg(long)]
    pub json_only: bool,
}

/// 主入口點
pub async fn run(cli: Cli) -> Result<EvolutionReport> {
    let config = AntigravityConfig::load()?;
    let date = cli.date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y%m%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    
    let pipeline_config = PipelineConfig {
        date,
        dry_run: cli.dry_run,
        skip_stages: cli.skip,
        only_stages: cli.only,
        json_only: cli.json_only,
    };
    
    let mut pipeline = EvolutionPipeline::new(config, pipeline_config).await?;
    let report = pipeline.execute().await?;
    
    if cli.json_only {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    
    Ok(report)
}

fn print_report(report: &EvolutionReport) {
    println!("{}", "=".repeat(60));
    println!("🏗️  Antigravity 每日演化行動報告 - {}", report.date);
    println!("{}", "=".repeat(60));
    println!("{}", ReportRenderer::render_brief(report));
    println!();
    println!("📄 詳細報告: C:/私人/ai agent/buzz/REPORTS/DAILY_EVOLUTION_REPORT_{}.md", report.date.format("%Y%m%d"));
    println!("📄 極簡簡報: C:/私人/ai agent/buzz/REPORTS/DAILY_EVOLUTION_BRIEF_{}.md", report.date.format("%Y%m%d"));
    println!("📄 JSON數據: C:/私人/ai agent/buzz/OUTBOX/DAILY_EVOLUTION_REPORT_{}.json", report.date.format("%Y%m%d"));
}
