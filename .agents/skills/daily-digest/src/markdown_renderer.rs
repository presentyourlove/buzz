//! Markdown 渲染器模組
//! 
//! 負責將每日摘要渲染為符合 Obsidian 規格的 Frontmatter + Markdown 格式的文件
//! 支援分類聚合（工程類、硬體諮詢類、諮詢類、維運類、其他類）與 DoD 檢查清單。

use chrono::{NaiveDate, Utc, Local};

use crate::digest_generator::{DailyDigest, ThreadDigest, ThreadCategory, DigestConfig};

/// 日記摘要渲染器
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// 渲染每日摘要（包含統計概覽、分類執行緒、決策提煉、行動項目、DoD 清單與雙向連結）
    pub fn render_daily_digest(
        date: NaiveDate,
        digest: &DailyDigest,
        config: &DigestConfig,
    ) -> String {
        Self::render_categorized_digest(date, digest, config)
    }

    /// 分類渲染每日摘要
    pub fn render_categorized_digest(
        date: NaiveDate,
        digest: &DailyDigest,
        config: &DigestConfig,
    ) -> String {
        let mut content = String::new();
        
        // 1. 生成 Frontmatter
        let frontmatter = create_frontmatter(date, digest, config);
        content.push_str(&frontmatter);
        
        // 2. 生成標題和元數據
        content.push_str(&format!("# {} 每日對話摘要與決策複盤\n\n", date.format("%Y-%m-%d")));
        content.push_str(&format!("> **生成時間**：{}  \n", Local::now().format("%Y-%m-%d %H:%M:%S")));
        content.push_str("> **執行代理人**：Daily Digest Skill  \n\n");
        
        // 3. 統計概覽
        content.push_str("## 📊 統計概覽\n");
        content.push_str(&format!(
            "- 執行緒數：{} | 訊息數：{} | 決策數：{} | 行動項目：{}\n\n",
            digest.threads.len(),
            digest.total_messages,
            digest.all_decisions.len(),
            digest.all_action_items.len(),
        ));

        // 4. 分類渲染各類執行緒
        // 順序：工程類 → 硬體諮詢類 → 諮詢類 → 維運類 → 其他類
        let categories = [
            ThreadCategory::Engineering,
            ThreadCategory::Hardware,
            ThreadCategory::Consult,
            ThreadCategory::Ops,
            ThreadCategory::Other,
        ];

        for cat in &categories {
            let cat_threads: Vec<&ThreadDigest> = digest.threads.iter()
                .filter(|t| t.category == *cat)
                .collect();

            if !cat_threads.is_empty() {
                content.push_str(&format!("## {}\n\n", cat.display_name()));
                for thread in cat_threads {
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
                            let status = if action_item.completed { "[x]" } else { "[ ]" };
                            content.push_str(&format!("  - {} **{}** (指派：{})  \n", 
                                status,
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
        }

        // 5. 核心決策提煉部分
        if !digest.all_decisions.is_empty() {
            content.push_str("## 核心決策提煉\n\n");
            for decision in &digest.all_decisions {
                content.push_str(&format!("- **{}**\n", decision));
            }
            content.push_str("\n");
        }
        
        // 6. 行動項目追蹤部分
        if !digest.all_action_items.is_empty() {
            content.push_str("## 行動項目追蹤\n\n");
            for action_item in &digest.all_action_items {
                let status = if action_item.completed { "[x]" } else { "[ ]" };
                content.push_str(&format!("- {} **{}** (指派：{})  \n", 
                    status,
                    escape_markdown(&action_item.description), 
                    escape_markdown(&action_item.assignee)
                ));
            }
            content.push_str("\n");
        }

        // 7. DoD 檢查清單
        content.push_str("## DoD 檢查清單\n");
        content.push_str("- [ ] 核心決策與行動項目已提取\n");
        content.push_str("- [ ] 執行緒分類歸檔正確\n");
        content.push_str("- [ ] 雙向連結已建立\n\n");
        
        // 8. 雙向連結部分
        content.push_str("## 📎 雙向連結\n\n");
        content.push_str("- [[core-memory]]\n");
        content.push_str("- [[wiki/02-Conversation-Digest/INDEX|對話索引]]\n");
        
        content
    }
}

/// 創建 Frontmatter
fn create_frontmatter(
    date: NaiveDate,
    digest: &DailyDigest,
    config: &DigestConfig,
) -> String {
    let tags = config.tags.iter()
        .map(|t| format!("\"{}\"", t))
        .collect::<Vec<String>>()
        .join(", ");
    
    format!(r#"---
title: "{} 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "{}"
threads_count: {}
messages_count: {}
synced_at: "{}"
tags: [{}]
---
"#,
        date.format("%Y-%m-%d"),
        date.format("%Y-%m-%d"),
        digest.threads.len(),
        digest.total_messages,
        Utc::now().to_rfc3339(),
        tags
    )
}

/// 轉義 Markdown 特殊字符
fn escape_markdown(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
}
