//! 摘要生成器模組
//! 
//! 負責提取執行緒中的核心決策、行動項目和代碼片段，生成結構化摘要
//! 
//! 主要功能包括：
//! 1. 對訊息進行行級過濾和分析
//! 2. 提取決策、行動項目和代碼片段
//! 3. 生成結構化的 ThreadDigest 和 DailyDigest
//! 4. 整合 Vault 同步功能

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use chrono::{NaiveDate, DateTime, Utc};
use regex::Regex;

use crate::daily_digest::{noise_filter::NoiseFilter, vault_sync::VaultSync};

/// 摘要配置
/// 
/// 包含每日摘要生成所需的配置參數
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DigestConfig {
    /// Obsidian Vault 路徑，可選
    pub vault_path: Option<PathBuf>,
    /// 每日摘要儲存目錄名稱
    pub daily_folder: String,
    /// 摘要標籤列表
    pub tags: Vec<String>,
    /// 每個摘要的最大執行緒數量
    pub max_threads_per_digest: usize,
    /// 是否啟用嚴格模式
    pub strict_mode: bool,
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            vault_path: None,
            daily_folder: "daily".to_string(),
            tags: vec!["digest".to_string(), "daily".to_string()],
            max_threads_per_digest: 20,
            strict_mode: true,
        }
    }
}

/// 執行緒摘要
/// 
/// 表示單個執行緒的結構化摘要信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThreadDigest {
    /// 執行緒 ID
    pub thread_id: String,
    /// 根事件 ID
    pub root_event_id: String,
    /// 參與者列表
    pub participants: Vec<String>,
    /// 訊息數量
    pub message_count: usize,
    /// 決策列表
    pub decisions: Vec<String>,
    /// 行動項目列表
    pub action_items: Vec<ActionItem>,
    /// 關鍵代碼片段列表
    pub key_code_snippets: Vec<String>,
    /// 執行緒摘要
    pub summary: String,
}

/// 行動項目
/// 
/// 表示執行緒中需要採取的行動
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionItem {
    /// 行動項目描述
    pub description: String,
    /// 指派人
    pub assignee: String,
    /// 是否已完成
    pub completed: bool,
    /// 截止日期（可選）
    pub due_date: Option<NaiveDate>,
}

/// 每日摘要
/// 
/// 表示整天的所有執行緒的聚合摘要
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailyDigest {
    /// 日期
    pub date: NaiveDate,
    /// 所有執行緒的摘要
    pub threads: Vec<ThreadDigest>,
    /// 總訊息數量
    pub total_messages: usize,
    /// 所有決策（聚合）
    pub all_decisions: Vec<String>,
    /// 所有行動項目（聚合）
    pub all_action_items: Vec<ActionItem>,
    /// 所有關鍵代碼片段（聚合）
    pub all_key_code_snippets: Vec<String>,
    /// 生成時間
    pub generated_at: DateTime<Utc>,
}

/// 摘要生成器
/// 
/// 負責從訊息中提取有意義的內容，生成結構化摘要
pub struct DigestGenerator {
    /// 行級雜訊過濾器
    noise_filter: NoiseFilter,
    /// Vault 同步器（可選）
    vault_sync: Option<VaultSync>,
    /// 生成配置
    config: DigestConfig,
}

impl DigestGenerator {
    /// 創建新的摘要生成器實例
    /// 
    /// # 參數
    /// 
    /// * `noise_filter` - 用於過濾訊息的噪音過濾器
    /// * `config` - 生成配置
    /// 
    /// # 返回值
    /// 
    /// 返回新的 DigestGenerator 實例
    pub fn new(noise_filter: NoiseFilter, config: DigestConfig) -> Self {
        let vault_sync = match &config.vault_path {
            Some(path) => Some(VaultSync::new(path.clone(), config.daily_folder.clone())),
            None => None,
        };
        
        Self {
            noise_filter,
            vault_sync,
            config,
        }
    }
    
    /// 為指定日期和頻道生成每日摘要
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
    /// # 錯誤
    /// 
    /// 如果生成過程中遇到錯誤，將返回相應的錯誤信息
    pub async fn generate_daily_digest(
        &self,
        date: NaiveDate,
        channel_id: &str,
    ) -> Result<DailyDigest, DigestError> {
        // 這裡將模擬從頻道中提取訊息
        // 在實際實現中，將調用 buzz 的 API 來獲取當天的訊息
        let messages = self.extract_channel_messages(date, channel_id).await?;
        
        // 提取執行緒
        let threads = self.extract_threads(&messages);
        
        // 聚合摘要信息
        let digest = self.aggregate_digest_info(&threads, date);
        
        // 執行 Vault 同步（如果已配置）
        if let Some(vault_sync) = &self.vault_sync {
            let markdown = crate::daily_digest::markdown_renderer::render_daily_digest(
                date,
                &digest,
                &self.config,
            );
            
            vault_sync.sync(date, &markdown).map_err(DigestError::VaultSyncError)?;
        }
        
        Ok(digest)
    }
    
    /// 提取指定日期和頻道的所有訊息
    /// 
    /// # 參數
    /// 
    /// * `date` - 訊息日期
    /// * `channel_id` - 頻道 ID
    /// 
    /// # 返回值
    /// 
    /// 返回包含訊息列表的 Result 型
    /// 
    /// # 注意
    /// 
    /// 此方法需要接入 Buzz 的 API 來獲取實際訊息數據
    async fn extract_channel_messages(&self, date: NaiveDate, channel_id: &str) -> Result<Vec<Message>, DigestError> {
        // TODO: 接入 Buzz 的 API 來獲取訊息
        // 這是模擬實現
        let mut messages = Vec::new();
        
        // 模擬生成一些測試訊息
        let test_messages = vec![
            Message {
                id: "msg1".to_string(),
                thread_id: "thread1".to_string(),
                author: "alice".to_string(),
                content: "\u{1F44D} 已完成 PR 並且レビュー者已批准。\n\n@Pollen 請實作模組 X\n\n決議採用 Rust 實作\n\n```rust\nfn main() {\n    println!("Hello, world!");\n}\n```",
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
            Message {
                id: "msg2".to_string(),
                thread_id: "thread1".to_string(),
                author: "bob".to_string(),
                content: "OK，我會處理。\n\n需要更新文檔。\n\n@Honey2 請撰寫文檔。\n\n任務：\n1. 撰寫 API 文檔\n2. 撰寫使用範例",
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
            Message {
                id: "msg3".to_string(),
                thread_id: "thread2".to_string(),
                author: "charlie".to_string(),
                content: "Error: 無法連接到數據庫\n\n連線失敗：Connection refused\n\n需要排查服務器問題。\n\n排查步驟：\n1. 檢查服務器狀態\n2. 確認端口配置\n3. 重啟數據庫服務",
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
            Message {
                id: "msg4".to_string(),
                thread_id: "thread2".to_string(),n                author: "david".to_string(),
                content: "心跳: OK\n\nkeepalive tick\n\n狀態: 正常",
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
            Message {
                id: "msg5".to_string(),
                thread_id: "thread3".to_string(),
                author: "eve".to_string(),
                content: "這是一個純表情符號回應: \u{1F60A}\n\n好的",
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
        ];
        
        messages.extend(test_messages);
        
        Ok(messages)
    }
    
    /// 從訊息列表中提取執行緒
    /// 
    /// # 參數
    /// 
    /// * `messages` - 訊息列表
    /// 
    /// # 返回值
    /// 
    /// 返回包含執行緒摘要列表的 Vec<ThreadDigest> 型
    fn extract_threads(&self, messages: &[Message]) -> Vec<ThreadDigest> {
        let mut thread_map: HashMap<String, ThreadData> = HashMap::new();
        
        for message in messages {
            let thread_data = thread_map.entry(message.thread_id.clone()).or_insert(ThreadData::new(&message.thread_id));
            thread_data.add_message(message);
        }
        
        thread_data.into_values()
            .map(|thread_data| self.create_thread_digest(thread_data))
            .collect()
    }
    
    /// 聚合摘要信息
    /// 
    /// # 參數
    /// 
    /// * `threads` - 執行緒摘要列表
    /// * `date` - 日期
    /// 
    /// # 返回值
    /// 
    /// 返回包含聚合信息的 DailyDigest 型
    fn aggregate_digest_info(&self, threads: &[ThreadDigest], date: NaiveDate) -> DailyDigest {
        let total_messages: usize = threads.iter().map(|t| t.message_count).sum();
        let all_decisions: Vec<String> = threads.iter()
            .flat_map(|t| t.decisions.clone())
            .collect();
        let all_action_items: Vec<ActionItem> = threads.iter()
            .flat_map(|t| t.action_items.clone())
            .collect();
        let all_
