//! Markdown 渲染器模組
//! 
//! 負責將每日摘要渲染為符合 Obsidian 規格的 Frontmatter + Markdown 格式的文件
//! 
//! 主要功能包括：
//! 1. 生成 YAML Frontmatter
//! 2. 渲染每日摘要的 Markdown 內容
//! 3. 支持 UTF-8 LF 編碼
//! 4. 支援 Obsidian 專用超連結

use std::fmt::{Display, Formatter};
use chrono::{NaiveDate, DateTime, Utc, Local};
use serde::Serialize;

use crate::daily_digest::{digest_generator::{DailyDigest, ThreadDigest, ActionItem}};

/// 日记摘要渲染器
/// 
/// 負責將每日摘要渲染為 Obsidian 相容的 Markdown 文件
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// 渲染每日摘要
    /// 
    /// # 參數
    /// 
    /// * `date` - 日期
    /// * `digest` - 日日摘要
    /// * `config` - 摘要配置
    /// 
    /// # 返回值
    /// 
    /// 返回包含渲染後 Markdown 的 String 型
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use chrono::NaiveDate;
    /// use daily_digest::{digest_generator::DailyDigest, markdown_renderer::MarkdownRenderer};
    /// use serde_json::json;
    /// 
    /// fn main() {
    ///     let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    ///     let digest = create_test_digest();
    ///     let renderer = MarkdownRenderer;
    ///     
    ///     let markdown = renderer.render_daily_digest(date, &digest);
    ///     println!("{}", markdown);
    /// }
    /// ```
    pub fn render_daily_digest(
        date: NaiveDate,
        digest: &DailyDigest,
        config: &crate::daily_digest::digest_generator::DigestConfig,
    ) -> String {
        let mut content = String::new();
        
        // 生成 Frontmatter
        let frontmatter = create_frontmatter(date, digest, config);
        content.push_str(&frontmatter);
        
        // 生成標題和元數據
        content.push_str(&format!("# {} 每日對話摘要與決策複盤\n\n", date.format("%Y-%m-%d")));
        content.push_str(&format!("> **生成時間**：{}  \n", Local::now().format("%Y-%m-%d %H:%M:%S")));
        content.push_str("> **執行代理人**：Daily Digest Skill  \n\n");
        
        // 生成核心決策提煉部分
        if !digest.all_decisions.is_empty() {
            content.push_str("## 核心決策提煉\n\n");
            for decision in &digest.all_decisions {
                content.push_str(&format!("- **{}**\n", decision));
            }
            content.push_str("\n");
        }
        
        // 生成行動項目追蹤部分
        if !digest.all_action_items.is_empty() {
            content.push_str("## 行動項目追蹤\n\n");
            for action_item in &digest.all_action_items {
                content.push_str(&format!("- [ ] **{}** (指派：{})  \n", 
                    escape_markdown(&action_item.description), 
                    escape_markdown(&action_item.assignee)
                ));
            }
            content.push_str("\n");
        }
        
        // 生成執行緒詳情部分
        if !digest.threads.is_empty() {
            content.push_str("## 執行緒詳情\n\n");
            for thread in &digest.threads {
                content.push_str(&format!("### {} ({} 訊息)\n", thread.summary, thread.message_count));
                content.push_str(&format!("- 參與者：{}  \n", thread.participants.join(", ")));
                content.push_str(&format!("- 執行緒 ID：`{}`  \n", thread.thread_id));
                
                if !thread.decisions.is_empty() {
                    content.push_str("- 決策：\n");
                    for decision in &thread.decisions {
                        content.push_str(&format!("  - {}\n", decision));
                    }
                }
                
                if !thread.action_items.is_empty() {
                    content.push_str("- 行動項目：\n");
                    for action_item in &thread.action_items {
                        content.push_str(&format!("  - [ ] **{}** (指派：{})  \n", 
                            escape_markdown(&action_item.description), 
                            escape_markdown(&action_item.assignee)
                        ));
                    }
                }
                
                if !thread.key_code_snippets.is_empty() {
                    content.push_str("- 關鍵代碼片段：\n");
                    for snippet in &thread.key_code_snippets {
                        content.push_str(&format!("  - `{}`\n", escape_markdown(snippet)));
                    }
                }
                
                content.push_str("\n");
            }
        }
        
        // 生成雙向連結部分
        content.push_str("## 雙向連結\n\n");
        content.push_str("- [[core-memory]]\n");
        content.push_str("- [[02-Conversation-Digest/INDEX|對話索引]]\n");
        
        content
    }
}

/// 創建 Frontmatter
/// 
/// # 參數
/// 
/// * `date` - 日期
/// * `digest` - 日日摘要
/// * `config` - 摘要配置
/// 
/// # 返回值
/// 
/// 返回包含 Frontmatter 的 String 型
fn create_frontmatter(
    date: NaiveDate,
    digest: &DailyDigest,
    config: &crate::daily_digest::digest_generator::DigestConfig,
) -> String {
    let tags = config.tags.iter()
        .map(|t| format!("\"{}\", t))
        .collect::<Vec<String>>()
        .join(", ");
    
    format!(r"""---
:title: "{} 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "{}"
threads_count: {}
messages_count: {}
synced_at: "{}"
tags: [{}]
---
""",
        date.format("%Y-%m-%d"),
        date.format("%Y-%m-%d"),
        digest.threads.len(),
        digest.total_messages,
        Utc::now().to_rfc3339(),
        tags
    )
}

/// 轉義 Markdown 特殊字符
/// 
/// # 參數
/// 
/// * `text` - 要轉義的文字
/// 
/// # 返回值
/// 
/// 返回轉義後的文字
fn escape_markdown(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace("`", "\`")
        .replace("*", "\*")
        .replace("_", "\_")
        .replace("{", "\{")
        .replace("}", "\}")
        .replace("[", "\[")
        .replace("]", "\]")
        .replace("(", "\(")
        .replace(")", "\)")
        .replace("#", "\#")
        .replace("+", "\+")
        .replace(\"-\", \"\-\")
        .replace(\".\", \"\.\")
        .replace(">", \"\>\")
}

/// 創建測試摘要（用於測試）
/// 
/// # 返回值
/// 
/// 返回一個包含測試數據的 DailyDigest 型
fn create_test_digest() -> DailyDigest {
    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    
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
        summary: "決策和行動項目的討論".to_string(),
    };
    
    let thread2 = ThreadDigest {
        thread_id: "thread2".to_string(),
        root_event_id: "root2".to_string(),
        participants: vec!["charlie".to_string()],
        message_count: 2,
        decisions: vec!["建議延後發布".to_string()],
        action_items: vec![],
        key_code_snippets: vec![],
        summary: "發布計劃的討論".to_string(),
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
            "println
