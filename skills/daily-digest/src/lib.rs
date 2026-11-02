//! Daily Digest Skill - Rust 內建技能實現
//! 
//! 本技能負責生成每日對話摘要、決策複盤與 Obsidian Vault 雙向同步
//! 
//! 核心功能模組：
//! 1. noise_filter.rs - 行級雜訊過濾器
//! 2. digest_generator.rs - 摘要生成器
//! 3. markdown_renderer.rs - Markdown 渲染器
//! 4. vault_sync.rs - Vault 同步器
//! 
//! 技能整合點：
//! - Session 建立時發現 (buzz-agent/src/hints.rs)
//! - 載入時機 (buzz-agent/src/builtin.rs:52-124)
//! - Hook 整合 (hooks/mod.rs:50-74)

pub mod noise_filter;
pub mod digest_generator;
pub mod markdown_renderer;
pub mod vault_sync;

/// 重新導出常用類型和結構，方便技能實現
pub use noise_filter::NoiseFilter;
pub use digest_generator::{DigestGenerator, DigestConfig, Message, ThreadDigest, ActionItem, DailyDigest};
pub use markdown_renderer::MarkdownRenderer;
pub use vault_sync::{VaultSync, SyncResult, SyncError};

/// 技能入口函數 - 初始化每日摘要技能
/// 
/// # 返回值
/// 
/// 返回配置完成的 DailyDigestSkill 實例
/// 
/// # 示例
/// 
/// ```rust
/// use daily_digest::{init_skill, load_config};
/// 
/// #[tokio::main]
/// async fn main() {
///     let skill = init_skill();
///     let config = load_config();
///     
///     // 使用技能生成每日摘要
///     let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
///     let channel_id = "test_channel".to_string();
    ///     
    ///     let digest = skill.generate_daily_digest(date, &channel_id).await.unwrap();
    ///     println!("摘要已生成：{}", digest.date);
    /// }
    /// ```
pub fn init_skill() -> DailyDigestSkill {
    DailyDigestSkill::new()
}

/// 載入技能配置
/// 
/// # 返回值
/// 
/// 返回默認的 DigestConfig 配置
/// 
/// # 示例
/// 
/// ```rust
/// use daily_digest::load_config;
/// 
/// fn main() {
///     let config = load_config();
///     println!("技能配置：{:?}", config);
/// }
/// ```
pub fn load_config() -> DigestConfig {
    DigestConfig::default()
}

/// 日記摘要技能主類
/// 
/// 用於管理每日摘要技能的整體功能
pub struct DailyDigestSkill {
    /// 噪音過濾器
    noise_filter: NoiseFilter,
    /// 摘要生成器
    generator: DigestGenerator,
    /// Markdown 渲染器
    renderer: MarkdownRenderer,
}

impl DailyDigestSkill {
    /// 創建新的技能實例
    /// 
    /// # 返回值
    /// 
    /// 返回新的 DailyDigestSkill 實例
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::DailyDigestSkill;
    /// 
    /// let skill = DailyDigestSkill::new();
    /// ```
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
    /// 
    /// # 參數
    /// 
    /// * `date` - 要生成的摘要日期
    /// * `channel_id` - 頻道 ID
    /// 
    /// # 返回值
    /// 
    /// 返回包含生成結果的 Result 型
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::DailyDigestSkill;
    /// use chrono::NaiveDate;
    /// 
    /// #[tokio::main]
    /// async fn main() {
    ///     let skill = DailyDigestSkill::new();
    ///     
    ///     let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    ///     let channel_id = "test_channel".to_string();
    ///     
    ///     match skill.generate_daily_digest(date, &channel_id).await {
    ///         Ok(digest) => println!("摘要已生成：{}", digest.date),
    ///         Err(e) => eprintln!("生成摘要失敗：{}", e),
    ///     }
    /// }
    /// ```
    pub async fn generate_daily_digest(
        &self,
        date: chrono::NaiveDate,
        channel_id: &str,
    ) -> Result<DailyDigest, crate::daily_digest::digest_generator::DigestError> {
        self.generator.generate_daily_digest(date, channel_id).await
    }
    
    /// 渲染 Markdown
    /// 
    /// # 參數
    /// 
    /// * `date` - 日期
    /// * `digest` - 日日摘要
    /// * `config` - 摘要配置
    /// 
    /// # 返回值
    /// 
    /// 返回渲染後的 Markdown 字符串
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::DailyDigestSkill;
    /// use chrono::NaiveDate;
    /// 
    /// let skill = DailyDigestSkill::new();
    /// let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    /// let digest = create_test_digest();
    /// let config = load_config();
    /// 
    /// let markdown = skill.render_markdown(date, &digest, &config);
    /// println!("{}", markdown);
    /// ```
    pub fn render_markdown(
        &self,
        date: chrono::NaiveDate,
        digest: &DailyDigest,
        config: &DigestConfig,
    ) -> String {
        self.renderer.render_daily_digest(date, digest, config)
    }
}

/// 創建測試摘要（用於測試）
/// 
/// # 返回值
/// 
/// 返回一個包含測試數據的 DailyDigest 型
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
            "fn main() {".to_string(),
            "println!(\"Hello\");".to_string(),
        ],
        summary: "執行緒包含 3 條訊息，2 個參與者".to_string(),
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
            "fn main() {".to_string(),
            "println!(\"Hello\");".to_string(),
        ],
        generated_at: chrono::Utc::now(),
    }
}

/// 便利函數 - 生成測試摘要
/// 
/// # 返回值
/// 
/// 返回一個包含測試數據的 DailyDigest 型
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
        let c
