//! Daily Digest Skill
//! 核心功能：將對話內容轉換為結構化每日摘要，提取決策、洞察與重要信息

mod digest_generator;
mod markdown_renderer;
mod vault_sync;
mod noise_filter;

use digest_generator::DigestGenerator;
use markdown_renderer::MarkdownRenderer;
use vault_sync::{VaultSync, SyncConfig};
use noise_filter::NoiseFilter;

/// Daily Digest 技能主類
/// 整合多個子模組，實現完整的對話摘要生成與處理流程
pub struct DailyDigestSkill {
    /// 摘要生成器
    generator: DigestGenerator,
    /// Markdown 渲染器
    renderer: MarkdownRenderer,
    /// Vault 同步器
    vault_sync: Option<VaultSync>,
    /// 噪音過濾器
    noise_filter: NoiseFilter,
    /// 技能配置
    config: SkillConfig,
}

/// 技能配置結構
pub struct SkillConfig {
    /// 是否啟用 Vault 同步
    pub enable_vault_sync: bool,
    /// Vault 配置
    pub vault_config: Option<SyncConfig>,
    /// 是否自動過濾
    pub enable_auto_filter: bool,
    /// 每日日期字符串
    pub daily_date: String,
}

impl DailyDigestSkill {
    /// 創建新的每日摘要技能實例
    pub fn new(config: SkillConfig) -> Self {
        let mut vault_sync = None;
        if config.enable_vault_sync {
            if let Some(vault_config) = &config.vault_config {
                vault_sync = Some(VaultSync::new(vault_config.clone()));
            }
        }
        
        Self {
            generator: DigestGenerator::new(),
            renderer: MarkdownRenderer::new(),
            vault_sync,
            noise_filter: NoiseFilter::new(),
            config,
        }
    }

    /// 處理輸入的對話內容
    pub fn process_content(&mut self, content: &str) -> ProcessResult {
        // 1. 噪音過濾
        let filter_result = self.noise_filter.filter_content(content);
        
        let mut result = ProcessResult {
            success: false,
            processed_content: String::new(),
            quality_score: filter_result.quality_score,
            decisions: Vec::new(),
            errors: Vec::new(),
        };
        
        if !filter_result.is_signal {
            result.errors.push("Content failed noise filtering".to_string());
            return result;
        }
        
        // 2. Markdown 渲染與結構化
        let structured_text = self.renderer.to_structured_text(filter_result.filtered_content.as_str());
        result.processed_content = structured_text;
        
        // 3. 決策提取
        let decisions = self.generator.extract_decisions();
        result.decisions = decisions;
        
        result.success = true;
        result
    }

    /// 生成每日摘要
    pub fn generate_daily_digest(&mut self, thread_data: Vec<(String, String)>) -> DigestResult {
        // 設置摘要生成器中的對話數據
        for (key, value) in thread_data {
            self.generator.set_thread_data(key, value);
        }
        
        // 驗證提取過程
        if let Err(e) = self.generator.validate_extraction() {
            return DigestResult {
                success: false,
                digest: String::new(),
                errors: vec![e],
                complexity_score: 0.0,
            };
        }
        
        // 生成摘要 JSON
        let digest_json = self.generator.to_json();
        
        // 同步到 Vault（如果啟用）
        let mut vault_result = VaultSyncResult {
            success: false,
            file_path: String::new(),
            action: String::new(),
        };
        
        if let Some(vault) = &mut self.vault_sync {
            // 構建摘要內容
            let content = format!("# Daily Digest - {}\n\n{}", self.config.daily_date, digest_json);
            
            match vault.sync_to_vault(&self.config.daily_date, &content) {
                Ok(result) => {
                    vault_result.success = true;
                    vault_result.file_path = result.file_path;
                    vault_result.action = result.action;
                }
                Err(e) => {
                    vault_result.errors.push(e);
                }
            }
        }
        
        DigestResult {
            success: true,
            digest: digest_json,
            errors: vec![],
            complexity_score: self.generator.analyze_complexity(),
        }
    }

    /// 獲取品質統計信息
    pub fn get_quality_stats(&self) -> QualityStats {
        self.noise_filter.get_quality_stats()
    }

    /// 獲取 Vault 同步狀態
    pub fn get_vault_status(&self) -> Option<vault_sync::SyncStatus> {
        self.vault_sync.as_ref().map(|v| v.get_sync_status())
    }
}

/// 處理結果結構
pub struct ProcessResult {
    /// 處理是否成功
    pub success: bool,
    /// 處理後的內容
    pub processed_content: String,
    /// 品質分數
    pub quality_score: f64,
    /// 提取的決策列表
    pub decisions: Vec<String>,
    /// 錯誤列表
    pub errors: Vec<String>,
}

/// 摘要結果結構
pub struct DigestResult {
    /// 生成是否成功
    pub success: bool,
    /// 生成的摘要內容
    pub digest: String,
    /// 錯誤列表
    pub errors: Vec<String>,
    /// 複雜度分數
    pub complexity_score: f64,
}

/// Vault 同步結果結構
pub struct VaultSyncResult {
    /// 同步是否成功
    pub success: bool,
    /// 文件路徑
    pub file_path: String,
    /// 執行操作
    pub action: String,
    /// 錯誤列表
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_sync::SyncConfig;

    #[test]
    fn test_daily_digest_skill_creation() {
        let config = SkillConfig {
            enable_vault_sync: false,
            vault_config: None,
            enable_auto_filter: true,
            daily_date: "2024-01-01".to_string(),
        };
        
        let skill = DailyDigestSkill::new(config);
        assert_eq!(skill.config.daily_date, "2024-01-01");
        assert!(skill.config.enable_vault_sync == false);
    }

    #[test]
    fn test_daily_digest_skill_content_processing() {
        let config = SkillConfig {
            enable_vault_sync: false,
            vault_config: None,
            enable_auto_filter: true,
            daily_date: "2024-01-01".to_string(),
        };
        
        let mut skill = DailyDigestSkill::new(config);
        let content = "⭐ 完成核心功能實作";
        
        let result = skill.process_content(content);
        assert!(result.success);
        assert!(result.quality_score > 0.7);
        assert_eq!(result.decisions.len(), 1);
    }

    #[test]
    fn test_daily_digest_skill_generation() {
        let config = SkillConfig {
            enable_vault_sync: false,
            vault_config: None,
            enable_auto_filter: true,
            daily_date: "2024-01-01".to_string(),
        };
        
        let mut skill = DailyDigestSkill::new(config);
        let thread_data = vec![
            ("decision1".to_string(), "⭐ 建立 Rust 技能目錄".to_string()),
            ("decision2".to_string(), "⭐ 實作核心模組".to_string()),
        ];
        
        let result = skill.generate_daily_digest(thread_data);
        assert!(result.success);
        assert!(result.complexity_score > 0.0);
        assert!(!result.digest.is_empty());
    }

    #[test]
    fn test_daily_digest_skill_with_vault() {
        let vault_config = SyncConfig {
            vault_path: "/tmp/test".to_string(),
            daily_note_folder: "daily".to_string(),
            tags: vec!["test".to_string()],
            enable_bidirectional: false,
        };
        
        let config = SkillConfig {
            enable_vault_sync: true,
            vault_config: Some(vault_config),
            enable_auto_filter: true,
            daily_date: "2024-01-01".to_string(),
        };
        
        let skill = DailyDigestSkill::new(config);
        assert!(skill.vault_sync.is_some());
        
        let status = skill.get_vault_status();
        assert!(status.is_some());
    }
}
