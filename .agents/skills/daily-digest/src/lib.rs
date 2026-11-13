//! Daily Digest Skill - Rust 內建技能實現
//! 
//! 本技能負責生成每日對話摘要、決策複盤與 Obsidian Vault 雙向同步
//! 
//! 核心功能模組：
//! 1. noise_filter.rs - 行級雜訊過濾器 (Noise Filter v2: Q&A/閒聊識別與啟發式評分)
//! 2. digest_generator.rs - 摘要生成器 (整合 SpecValidator 規範預檢與 ThreadCategory 分類)
//! 3. markdown_renderer.rs - Markdown 渲染器 (分類區塊渲染、DoD 清單、統計概覽)
//! 4. spec_validator.rs - MVP 規範驗證器 (Front-matter 與 DoD 檢查清單檢驗)
//! 5. vault_sync.rs - Vault 同步器 (原子寫入與 INDEX 索引維護)

pub mod noise_filter;
pub mod digest_generator;
pub mod markdown_renderer;
pub mod spec_validator;
pub mod vault_sync;

/// 重新導出常用類型和結構，方便外部與測試調用
pub use noise_filter::{NoiseFilter, NoiseCategory};
pub use digest_generator::{
    DigestGenerator, DigestConfig, Message, ThreadDigest, ThreadCategory,
    ThreadData, ActionItem, DailyDigest, DigestError,
};
pub use markdown_renderer::MarkdownRenderer;
pub use spec_validator::{SpecValidator, ValidationResult, ValidationError, ValidationWarning};
pub use vault_sync::{VaultSync, SyncResult, SyncError};

/// 技能入口函數 - 初始化每日摘要技能
pub fn init_skill() -> DailyDigestSkill {
    DailyDigestSkill::new()
}

/// 載入技能配置
pub fn load_config() -> DigestConfig {
    DigestConfig::default()
}

/// 日記摘要技能主類
pub struct DailyDigestSkill {
    pub noise_filter: NoiseFilter,
    pub generator: DigestGenerator,
    pub renderer: MarkdownRenderer,
}

impl DailyDigestSkill {
    /// 創建新的技能實例
    pub fn new() -> Self {
        let noise_filter = NoiseFilter::new();
        let config = DigestConfig::default();
        let generator = DigestGenerator::new(noise_filter.clone(), config);
        let renderer = MarkdownRenderer;
        
        Self {
            noise_filter,
            generator,
            renderer,
        }
    }
    
    /// 生成每日摘要
    pub async fn generate_daily_digest(
        &self,
        date: chrono::NaiveDate,
        channel_id: &str,
    ) -> Result<DailyDigest, DigestError> {
        self.generator.generate_daily_digest(date, channel_id).await
    }
    
    /// 渲染 Markdown
    pub fn render_markdown(
        &self,
        date: chrono::NaiveDate,
        digest: &DailyDigest,
        config: &DigestConfig,
    ) -> String {
        MarkdownRenderer::render_daily_digest(date, digest, config)
    }
}

impl Default for DailyDigestSkill {
    fn default() -> Self {
        Self::new()
    }
}

/// 創建測試摘要（用於測試）
fn create_test_digest() -> DailyDigest {
    let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    
    let thread1 = ThreadDigest {
        thread_id: "thread1".to_string(),
        root_event_id: "root1".to_string(),
        participants: vec!["alice".to_string(), "bob".to_string()],
        message_count: 3,
        decisions: vec!["決議採用 Rust 實作".to_string(), "選擇這個方案".to_string()],
        action_items: vec![
            ActionItem {
                description: "撰寫 API 文檔".to_string(),
                assignee: "Pollen".to_string(),
                completed: false,
                due_date: None,
            },
            ActionItem {
                description: "修復 bug #123".to_string(),
                assignee: "Honey2".to_string(),
                completed: false,
                due_date: None,
            },
        ],
        key_code_snippets: vec![
            "fn main() { println!(\"Hello\"); }".to_string(),
            "println!(\"Hello\");".to_string(),
        ],
        summary: "執行緒包含 3 條訊息，2 個參與者".to_string(),
        category: ThreadCategory::Engineering,
    };
    
    let thread2 = ThreadDigest {
        thread_id: "thread2".to_string(),
        root_event_id: "root2".to_string(),
        participants: vec!["charlie".to_string()],
        message_count: 2,
        decisions: vec!["建議延後發布".to_string()],
        action_items: vec![],
        key_code_snippets: vec![],
        summary: "執行緒包含 2 條訊息，1 個參與者".to_string(),
        category: ThreadCategory::Consult,
    };
    
    DailyDigest {
        date,
        threads: vec![thread1, thread2],
        total_messages: 5,
        all_decisions: vec![
            "決議採用 Rust 實作".to_string(),
            "選擇這個方案".to_string(),
            "建議延後發布".to_string(),
        ],
        all_action_items: vec![
            ActionItem {
                description: "撰寫 API 文檔".to_string(),
                assignee: "Pollen".to_string(),
                completed: false,
                due_date: None,
            },
            ActionItem {
                description: "修復 bug #123".to_string(),
                assignee: "Honey2".to_string(),
                completed: false,
                due_date: None,
            },
        ],
        all_key_code_snippets: vec![
            "fn main() { println!(\"Hello\"); }".to_string(),
            "println!(\"Hello\");".to_string(),
        ],
        generated_at: chrono::Utc::now(),
    }
}

/// 便利函數 - 生成測試摘要
pub fn create_test_digest_for_rendering() -> DailyDigest {
    create_test_digest()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_skill_creation() {
        let skill = DailyDigestSkill::new();
        assert!(!skill.noise_filter.noise_patterns.is_empty());
        assert!(skill.generator.config.daily_folder == "daily");
    }
    
    #[tokio::test]
    async fn test_skill_generate_daily_digest() {
        let skill = DailyDigestSkill::new();
        let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let channel_id = "test_channel".to_string();
        
        let digest = skill.generate_daily_digest(date, &channel_id).await.unwrap();
        
        assert_eq!(digest.date, date);
        assert!(digest.total_messages > 0);
        assert!(!digest.all_decisions.is_empty() || !digest.all_action_items.is_empty() || !digest.all_key_code_snippets.is_empty());
    }
    
    #[tokio::test]
    async fn test_skill_render_markdown() {
        let skill = DailyDigestSkill::new();
        let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let digest = create_test_digest();
        let config = load_config();
        let md = skill.render_markdown(date, &digest, &config);
        assert!(md.contains("2024-01-15"));
        assert!(md.contains("⚙️ 工程類"));
        assert!(md.contains("💬 諮詢類"));
        assert!(md.contains("## DoD 檢查清單"));
    }
}
