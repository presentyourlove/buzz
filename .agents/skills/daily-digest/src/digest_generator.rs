//! 摘要生成器模組
//! 
//! 負責提取執行緒中的核心決策、行動項目和代碼片段，生成結構化摘要
//! 整合 MVP 規範驗證器 (SpecValidator) 與分類機制 (ThreadCategory)

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use chrono::{NaiveDate, DateTime, Utc};
use regex::Regex;

use crate::noise_filter::NoiseFilter;
use crate::vault_sync::{VaultSync, SyncError};
use crate::spec_validator::SpecValidator;

/// 執行緒分類枚舉
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ThreadCategory {
    /// 工程類（代碼、架構、缺陷、功能開發）
    Engineering,
    /// 硬體諮詢類（規格、伺服器、設備硬體）
    Hardware,
    /// 諮詢類（一般諮詢、建議、方案推薦）
    Consult,
    /// 維運類（部署、日誌、監控、服務維護）
    Ops,
    /// 其他類
    Other,
}

impl ThreadCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            ThreadCategory::Engineering => "⚙️ 工程類",
            ThreadCategory::Hardware => "🔧 硬體諮詢類",
            ThreadCategory::Consult => "💬 諮詢類",
            ThreadCategory::Ops => "🚀 維運類",
            ThreadCategory::Other => "📝 其他類",
        }
    }
}

/// 摘要生成錯誤枚舉
#[derive(Debug)]
pub enum DigestError {
    VaultSyncError(SyncError),
    ValidationError(String),
    MessageExtractionError(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DigestError::VaultSyncError(e) => write!(f, "Vault 同步錯誤: {}", e),
            DigestError::ValidationError(e) => write!(f, "MVP 規範驗證失敗: {}", e),
            DigestError::MessageExtractionError(e) => write!(f, "訊息提取錯誤: {}", e),
            DigestError::IoError(e) => write!(f, "IO 錯誤: {}", e),
        }
    }
}

impl std::error::Error for DigestError {}

/// 頻道訊息結構
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub author: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub is_system: bool,
    pub reactions: Vec<String>,
}

/// 執行緒數據
#[derive(Debug, Clone)]
pub struct ThreadData {
    pub thread_id: String,
    pub root_event_id: String,
    pub messages: Vec<Message>,
}

impl ThreadData {
    pub fn new(thread_id: &str) -> Self {
        Self {
            thread_id: thread_id.to_string(),
            root_event_id: String::new(),
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, message: &Message) {
        if self.messages.is_empty() {
            self.root_event_id = message.id.clone();
        }
        self.messages.push(message.clone());
    }
}

/// 摘要配置
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

/// 行動項目
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionItem {
    pub description: String,
    pub assignee: String,
    pub completed: bool,
    pub due_date: Option<NaiveDate>,
}

/// 執行緒摘要
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThreadDigest {
    pub thread_id: String,
    pub root_event_id: String,
    pub participants: Vec<String>,
    pub message_count: usize,
    pub decisions: Vec<String>,
    pub action_items: Vec<ActionItem>,
    pub key_code_snippets: Vec<String>,
    pub summary: String,
    pub category: ThreadCategory,
}

/// 每日摘要聚合
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailyDigest {
    pub date: NaiveDate,
    pub threads: Vec<ThreadDigest>,
    pub total_messages: usize,
    pub all_decisions: Vec<String>,
    pub all_action_items: Vec<ActionItem>,
    pub all_key_code_snippets: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

/// 摘要生成器
pub struct DigestGenerator {
    pub noise_filter: NoiseFilter,
    pub vault_sync: Option<VaultSync>,
    pub config: DigestConfig,
}

impl DigestGenerator {
    /// 創建新的摘要生成器實例
    pub fn new(noise_filter: NoiseFilter, config: DigestConfig) -> Self {
        let vault_sync = config.vault_path.as_ref().map(|path| {
            VaultSync::new(path.clone(), config.daily_folder.clone())
        });

        Self {
            noise_filter,
            vault_sync,
            config,
        }
    }

    /// 為指定日期和頻道生成每日摘要
    pub async fn generate_daily_digest(
        &self,
        date: NaiveDate,
        channel_id: &str,
    ) -> Result<DailyDigest, DigestError> {
        let messages = self.extract_channel_messages(date, channel_id).await?;
        let threads = self.extract_threads(&messages);
        let digest = self.aggregate_digest_info(&threads, date);

        // 渲染 Markdown
        let markdown = crate::markdown_renderer::MarkdownRenderer::render_daily_digest(
            date,
            &digest,
            &self.config,
        );

        // SpecValidator 規範預檢
        let filename = format!("{}.md", date.format("%Y-%m-%d"));
        let validator = SpecValidator::new(self.config.strict_mode);
        let val_result = validator.validate(&markdown, &filename);

        if !val_result.valid {
            let error_str = val_result.errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(DigestError::ValidationError(error_str));
        }

        // Vault 雙向同步
        if let Some(vault_sync) = &self.vault_sync {
            vault_sync.sync(date, &markdown).map_err(DigestError::VaultSyncError)?;
        }

        Ok(digest)
    }

    /// 從訊息列表中提取執行緒
    pub fn extract_threads(&self, messages: &[Message]) -> Vec<ThreadDigest> {
        let mut thread_map: HashMap<String, ThreadData> = HashMap::new();

        for message in messages {
            let thread_data = thread_map
                .entry(message.thread_id.clone())
                .or_insert_with(|| ThreadData::new(&message.thread_id));
            thread_data.add_message(message);
        }

        thread_map
            .into_values()
            .map(|td| self.create_thread_digest(td))
            .collect()
    }

    /// 創建單一執行緒摘要
    pub fn create_thread_digest(&self, thread_data: ThreadData) -> ThreadDigest {
        let decisions = self.extract_decisions(&thread_data.messages);
        let action_items = self.extract_action_items(&thread_data.messages);
        let key_code_snippets = self.extract_code_snippets(&thread_data.messages);

        let mut participants: Vec<String> = thread_data
            .messages
            .iter()
            .map(|m| m.author.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        participants.sort();

        let summary = if let Some(d) = decisions.first() {
            d.clone()
        } else if let Some(first_msg) = thread_data.messages.first() {
            first_msg.content.lines().next().unwrap_or("執行緒討論").chars().take(40).collect()
        } else {
            "無訊息執行緒".to_string()
        };

        let category = Self::categorize_thread(&summary, &decisions, &action_items);

        ThreadDigest {
            thread_id: thread_data.thread_id,
            root_event_id: thread_data.root_event_id,
            participants,
            message_count: thread_data.messages.len(),
            decisions,
            action_items,
            key_code_snippets,
            summary,
            category,
        }
    }

    /// 啟發式分類執行緒
    pub fn categorize_thread(summary: &str, decisions: &[String], action_items: &[ActionItem]) -> ThreadCategory {
        let mut full_text = summary.to_string();
        full_text.push(' ');
        full_text.push_str(&decisions.join(" "));
        full_text.push(' ');
        full_text.push_str(&action_items.iter().map(|a| a.description.as_str()).collect::<Vec<_>>().join(" "));
        
        let lower = full_text.to_lowercase();

        if lower.contains("硬體") || lower.contains("hardware") || lower.contains("設備") || lower.contains("裝置") || lower.contains("epyc") || lower.contains("cpu") || lower.contains("gpu") {
            ThreadCategory::Hardware
        } else if lower.contains("諮詢") || lower.contains("consult") || lower.contains("建議") || lower.contains("推薦") || lower.contains("發布計劃") {
            ThreadCategory::Consult
        } else if lower.contains("部署") || lower.contains("deploy") || lower.contains("運維") || lower.contains("監控") || lower.contains("維護") || lower.contains("服務器") || lower.contains("排查") || lower.contains("重啟") {
            ThreadCategory::Ops
        } else {
            ThreadCategory::Engineering
        }
    }

    /// 提取決策
    pub fn extract_decisions(&self, messages: &[Message]) -> Vec<String> {
        let mut decisions = Vec::new();
        let decision_regex = Regex::new(r"(決策|決議|選擇|採用|決定|確認|建議)[:：]?\s*(.+)").unwrap();

        for msg in messages {
            let filtered_lines = self.noise_filter.filter_lines(&msg.content);
            for line in filtered_lines {
                let trimmed = line.trim();
                if let Some(caps) = decision_regex.captures(trimmed) {
                    if let Some(matched) = caps.get(0) {
                        let text = matched.as_str().trim_end_matches(['。', '.', '；', ';']).to_string();
                        if !decisions.contains(&text) {
                            decisions.push(text);
                        }
                    }
                } else if trimmed.starts_with("決議") || trimmed.starts_with("決策") || trimmed.starts_with("選擇") {
                    let text = trimmed.trim_end_matches(['。', '.', '；', ';']).to_string();
                    if !decisions.contains(&text) {
                        decisions.push(text);
                    }
                }
            }
        }
        decisions
    }

    /// 提取行動項目
    pub fn extract_action_items(&self, messages: &[Message]) -> Vec<ActionItem> {
        let mut items = Vec::new();
        let mention_regex = Regex::new(r"@([a-zA-Z0-9_\-\u4e00-\u9fa5]+)").unwrap();

        for msg in messages {
            let lines = msg.content.lines();
            for line in lines {
                let trimmed = line.trim();
                let is_action = trimmed.contains("task")
                    || trimmed.contains("TODO")
                    || trimmed.contains("FIXME")
                    || trimmed.contains("行動項目")
                    || trimmed.contains("指派");

                if is_action {
                    let assignee = mention_regex.captures(trimmed)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| msg.author.clone());

                    items.push(ActionItem {
                        description: trimmed.to_string(),
                        assignee,
                        completed: trimmed.contains("[x]") || trimmed.contains("已完成"),
                        due_date: None,
                    });
                }
            }
        }
        items
    }

    /// 提取代碼片段
    pub fn extract_code_snippets(&self, messages: &[Message]) -> Vec<String> {
        let mut snippets = Vec::new();
        let mut in_block = false;
        let mut current_block = Vec::new();

        for msg in messages {
            for line in msg.content.lines() {
                if line.trim().starts_with("```") {
                    if in_block {
                        snippets.push(current_block.join("\n"));
                        current_block.clear();
                        in_block = false;
                    } else {
                        in_block = true;
                    }
                } else if in_block {
                    current_block.push(line.to_string());
                } else if line.contains("Error:") || line.contains("Exception:") {
                    snippets.push(line.trim().to_string());
                }
            }
        }
        snippets
    }

    /// 聚合摘要信息
    pub fn aggregate_digest_info(&self, threads: &[ThreadDigest], date: NaiveDate) -> DailyDigest {
        let total_messages: usize = threads.iter().map(|t| t.message_count).sum();
        let all_decisions: Vec<String> = threads.iter()
            .flat_map(|t| t.decisions.clone())
            .collect();
        let all_action_items: Vec<ActionItem> = threads.iter()
            .flat_map(|t| t.action_items.clone())
            .collect();
        let all_key_code_snippets: Vec<String> = threads.iter()
            .flat_map(|t| t.key_code_snippets.clone())
            .collect();

        DailyDigest {
            date,
            threads: threads.to_vec(),
            total_messages,
            all_decisions,
            all_action_items,
            all_key_code_snippets,
            generated_at: Utc::now(),
        }
    }

    /// 提取模擬頻道訊息
    async fn extract_channel_messages(&self, _date: NaiveDate, _channel_id: &str) -> Result<Vec<Message>, DigestError> {
        let test_messages = vec![
            Message {
                id: "msg1".to_string(),
                thread_id: "thread1".to_string(),
                author: "alice".to_string(),
                content: "已完成 PR\n\n@Pollen 請實作模組 X\n\n決議採用 Rust 實作\n\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```".to_string(),
                timestamp: Utc::now(),
                is_system: false,
                reactions: vec![],
            },
            Message {
                id: "msg2".to_string(),
                thread_id: "thread1".to_string(),
                author: "bob".to_string(),
                content: "OK，我會處理。\n\n需要更新文檔。\n\n@Honey2 請撰寫文檔。\n\n任務：\n1. 撰寫 API 文檔\n2. 撰寫使用範例".to_string(),
                timestamp: Utc::now(),
                is_system: false,
                reactions: vec![],
            },
            Message {
                id: "msg3".to_string(),
                thread_id: "thread2".to_string(),
                author: "charlie".to_string(),
                content: "Error: 無法連接到數據庫\n\n連線失敗：Connection refused\n\n需要排查服務器問題。\n\n排查步驟：\n1. 檢查服務器狀態\n2. 確認端口配置\n3. 重啟數據庫服務".to_string(),
                timestamp: Utc::now(),
                is_system: false,
                reactions: vec![],
            },
        ];

        Ok(test_messages)
    }
}
