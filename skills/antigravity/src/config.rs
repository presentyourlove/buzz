//! 配置管理

use anyhow::Result;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Antigravity 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityConfig {
    pub nostr: NostrConfig,
    pub storage: StorageConfig,
    pub pipeline: PipelineConfig,
    pub monitor: MonitorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrConfig {
    pub relay_url: String,
    pub kind: u16,
    pub private_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub db_path: PathBuf,
    pub wal_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub default_stages: Vec<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub check_interval_secs: u64,
    pub alert_on_failure: bool,
    pub alert_channel: String,
}

impl AntigravityConfig {
    pub fn load() -> Result<Self> {
        // 優先從環境變數或配置檔讀取，否則使用預設值
        Ok(Self::default())
    }
}

impl Default for AntigravityConfig {
    fn default() -> Self {
        Self {
            nostr: NostrConfig {
                relay_url: std::env::var("BUZZ_RELAY_URL")
                    .unwrap_or_else(|_| "wss://presentyourlove.communities.buzz.xyz".to_string()),
                kind: 30300,
                private_key_env: "ANTIGRAVITY_NSEC".to_string(),
            },
            storage: StorageConfig {
                db_path: PathBuf::from("./data/antigravity"),
                wal_mode: true,
            },
            pipeline: PipelineConfig {
                default_stages: vec!["01".to_string(), "02".to_string(), "03".to_string(), 
                                     "04".to_string(), "05".to_string(), "06".to_string(), "07".to_string()],
                timeout_secs: 300,
            },
            monitor: MonitorConfig {
                check_interval_secs: 3600,
                alert_on_failure: true,
                alert_channel: "general".to_string(),
            },
        }
    }
}
