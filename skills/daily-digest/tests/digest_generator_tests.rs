//! 摘要生成器單元與整合測試
//! 
//! 測試 Daily Digest Skill 的核心功能，包括：
//! 1. 行級雜訊過濾器功能 (Noise Filter v2: Q&A、閒聊、啟發式評分、閾值邊界)
//! 2. 摘要生成器核心邏輯與 ThreadCategory 分類
//! 3. 每日摘要生成流程與 SpecValidator MVP 規範驗證
//! 4. Markdown 分類渲染功能
//! 5. Vault 同步功能

use daily_digest::{
    noise_filter::NoiseFilter,
    digest_generator::{
        DigestGenerator, DigestConfig, Message, ThreadData, ThreadDigest,
        ThreadCategory, ActionItem, DailyDigest,
    },
    markdown_renderer::MarkdownRenderer,
    spec_validator::{SpecValidator, ValidationError},
};

/// 測試行級雜訊過濾器基本功能
#[test]
fn test_noise_filter_creation() {
    let filter = NoiseFilter::new();
    assert!(!filter.noise_patterns.is_empty());
    assert!(!filter.signal_patterns.is_empty());
    assert_eq!(filter.signal_threshold, 0);
}

/// 測試有意義訊息識別
#[test]
fn test_is_signal_line_with_signal() {
    let filter = NoiseFilter::new();
    assert!(filter.is_signal_line("@Pollen 請實作模組 X"));
    assert!(filter.is_signal_line("決議採用 Rust 實作"));
    assert!(filter.is_signal_line("Error: connection refused"));
    assert!(filter.is_signal_line("完成代碼審查"));
    assert!(filter.is_signal_line("fn main() { /* 代碼 */ }"));
    assert!(filter.is_signal_line("\u{1F600} 好的"));
}

/// 測試雜訊訊息識別
#[test]
fn test_is_signal_line_with_noise() {
    let filter = NoiseFilter::new();
    assert!(!filter.is_signal_line("已完成"));
    assert!(!filter.is_signal_line("OK"));
    assert!(!filter.is_signal_line("收到"));
    assert!(!filter.is_signal_line("tracing::debug!(\"llm keepalive tick\")"));
}

/// 測試邊界情況處理
#[test]
fn test_is_signal_line_edge_cases() {
    let filter = NoiseFilter::new();
    assert!(!filter.is_signal_line(""));
    assert!(!filter.is_signal_line("   "));
    assert!(!filter.is_signal_line("OK"));
    assert!(!filter.is_signal_line("good"));
    assert!(!filter.is_signal_line("done"));
    assert!(!filter.is_signal_line("heartbeat tick"));
    assert!(filter.is_signal_line("這是一行有意義的文字"));
}

/// 測試完整的行級過濾流程
#[test]
fn test_filter_lines() {
    let filter = NoiseFilter::new();
    let content = "已完成 task #123\n@Pollen 請實作模組 X\nOK\n決議採用 Rust 實作\nError: connection refused\n完成代碼審查\ntracing::debug!(\"llm keepalive tick\")";
    
    let filtered = filter.filter_lines(content);
    
    assert_eq!(filtered.len(), 4);
    assert!(filtered.contains(&"@Pollen 請實作模組 X".to_string()));
    assert!(filtered.contains(&"決議採用 Rust 實作".to_string()));
    assert!(filtered.contains(&"Error: connection refused".to_string()));
    assert!(filtered.contains(&"完成代碼審查".to_string()));
}

/// 測試空訊息處理
#[test]
fn test_filter_lines_empty() {
    let filter = NoiseFilter::new();
    let content = "已完成\nOK\nheartbeat tick";
    
    let filtered = filter.filter_lines(content);
    assert!(filtered.is_empty());
}

/// 測試 Evolution 1: Q&A 與閒聊過濾及啟發式評分
#[test]
fn test_noise_filter_v2_qa_and_scoring() {
    let filter = NoiseFilter::new();
    assert!(!filter.is_signal_line("請問這個 API 如何調用？"));
    assert!(!filter.is_signal_line("Q: 為什麼測試失敗？"));
    assert!(!filter.is_signal_line("A: 需要配置環境變數"));
    assert!(!filter.is_signal_line("問：何時發布？"));
    assert!(!filter.is_signal_line("答：預計下週一"));
    assert!(!filter.is_signal_line("謝謝"));
    assert!(!filter.is_signal_line("感謝協助"));
    assert!(!filter.is_signal_line("沒問題"));
    assert!(!filter.is_signal_line("沒關係"));
    assert!(!filter.is_signal_line("隨意"));
    assert!(!filter.is_signal_line("無所謂"));

    assert!(filter.score_line("@Pollen 請實作模組 X") >= 2);
    assert!(filter.score_line("決議採用 Rust 實作") >= 2);
    assert!(filter.score_line("請問這個問題如何解決？") < 0);

    let content = "決議採用 Rust 實作\n請問有什麼建議？\n@Pollen 請撰寫測試";
    let scored = filter.filter_lines_with_scores(content);
    assert_eq!(scored.len(), 2);
    assert_eq!(scored[0].0, "決議採用 Rust 實作");
    assert_eq!(scored[1].0, "@Pollen 請撰寫測試");
}

/// 測試 Evolution 1: 閾值極限邊界測試 (signal_threshold = i32::MAX)
#[test]
fn test_noise_filter_extreme_threshold() {
    let filter = NoiseFilter::with_threshold(i32::MAX);
    let content = "決議採用 Rust 實作\n@Pollen 請實作模組 X";
    let filtered = filter.filter_lines(content);
    assert!(filtered.is_empty());
}

/// 測試摘要生成器創建
#[tokio::test]
async fn test_digest_generator_creation() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    assert!(!generator.noise_filter.noise_patterns.is_empty());
    assert!(generator.config.daily_folder == "daily");
}

/// 測試決策提取
#[tokio::test]
async fn test_extract_decisions() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    let messages = vec![
        Message {
            id: "msg1".to_string(),
            thread_id: "thread1".to_string(),
            author: "alice".to_string(),
            content: "決議採用 Rust 實作。\n\n選擇這個方案。".to_string(),
            timestamp: chrono::Utc::now(),
            is_system: false,
            reactions: vec![],
        },
        Message {
            id: "msg2".to_string(),
            thread_id: "thread2".to_string(),
            author: "bob".to_string(),
            content: "心跳: OK\n\nkeepalive tick".to_string(),
            timestamp: chrono::Utc::now(),
            is_system: false,
            reactions: vec![],
        },
    ];
    
    let decisions = generator.extract_decisions(&messages);
    assert_eq!(decisions.len(), 2);
    assert!(decisions.contains(&"決議採用 Rust 實作".to_string()));
    assert!(decisions.contains(&"選擇這個方案".to_string()));
}

/// 測試行動項目提取
#[tokio::test]
async fn test_extract_action_items() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    let messages = vec![
        Message {
            id: "msg1".to_string(),
            thread_id: "thread1".to_string(),
            author: "alice".to_string(),
            content: "需要完成 task #123\n\n@Pollen 請實作模組 X\n\nTODO: 撰寫文檔\nFIXME: 修復 bug".to_string(),
            timestamp: chrono::Utc::now(),
            is_system: false,
            reactions: vec![],
        },
    ];
    
    let action_items = generator.extract_action_items(&messages);
    assert_eq!(action_items.len(), 3);
    assert!(action_items.iter().any(|item| item.description.contains("task")));
    assert!(action_items.iter().any(|item| item.description.contains("TODO")));
    assert!(action_items.iter().any(|item| item.description.contains("FIXME")));
}

/// 測試執行緒提取
#[tokio::test]
async fn test_extract_threads() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    let messages = vec![
        Message {
            id: "msg1".to_string(),
            thread_id: "thread1".to_string(),
            author: "alice".to_string(),
            content: "決議採用 Rust 實作。\n\n選擇這個方案。\n\n@Pollen 請實作模組 X\n\nTODO: 撰寫文檔".to_string(),
            timestamp: chrono::Utc::now(),
            is_system: false,
            reactions: vec![],
        },
        Message {
            id: "msg2".to_string(),
            thread_id: "thread1".to_string(),
            author: "bob".to_string(),
            content: "已完成 task #123\n\n心跳: OK".to_string(),
            timestamp: chrono::Utc::now(),
            is_system: false,
            reactions: vec![],
        },
        Message {
            id: "msg3".to_string(),
            thread_id: "thread2".to_string(),
            author: "charlie".to_string(),
            content: "Error: 無法連接到數據庫\n\n排查服務器問題。\n\n排查步驟：\n1. 檢查服務器狀態\n2. 確認端口配置".to_string(),
            timestamp: chrono::Utc::now(),
            is_system: false,
            reactions: vec![],
        },
    ];
    
    let threads = generator.extract_threads(&messages);
    assert_eq!(threads.len(), 2);
    
    let thread1 = threads.iter().find(|t| t.thread_id == "thread1").unwrap();
    assert_eq!(thread1.thread_id, "thread1");
    assert_eq!(thread1.message_count, 2);
    assert!(thread1.participants.contains(&"alice".to_string()));
    assert!(thread1.participants.contains(&"bob".to_string()));
    assert!(!thread1.decisions.is_empty());
    assert!(!thread1.action_items.is_empty());
    
    let thread2 = threads.iter().find(|t| t.thread_id == "thread2").unwrap();
    assert_eq!(thread2.thread_id, "thread2");
    assert_eq!(thread2.message_count, 1);
    assert!(thread2.participants.contains(&"charlie".to_string()));
}

/// 測試執行緒摘要創建與分類
#[tokio::test]
async fn test_create_thread_digest_and_categorization() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    // 工程類
    let thread_eng = ThreadData {
        thread_id: "eng_thread".to_string(),
        root_event_id: "root_eng".to_string(),
        messages: vec![
            Message {
                id: "msg_eng".to_string(),
                thread_id: "eng_thread".to_string(),
                author: "alice".to_string(),
                content: "決議採用 Rust 實作。\n\n@Pollen 請實作模組 X".to_string(),
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
        ],
    };
    let digest_eng = generator.create_thread_digest(thread_eng);
    assert_eq!(digest_eng.category, ThreadCategory::Engineering);

    // 硬體諮詢類
    let thread_hw = ThreadData {
        thread_id: "hw_thread".to_string(),
        root_event_id: "root_hw".to_string(),
        messages: vec![
            Message {
                id: "msg_hw".to_string(),
                thread_id: "hw_thread".to_string(),
                author: "honey".to_string(),
                content: "建議採用 AMD EPYC 系列伺服器設備硬體。".to_string(),
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
        ],
    };
    let digest_hw = generator.create_thread_digest(thread_hw);
    assert_eq!(digest_hw.category, ThreadCategory::Hardware);

    // 維運類
    let thread_ops = ThreadData {
        thread_id: "ops_thread".to_string(),
        root_event_id: "root_ops".to_string(),
        messages: vec![
            Message {
                id: "msg_ops".to_string(),
                thread_id: "ops_thread".to_string(),
                author: "sentry".to_string(),
                content: "部署監控運維服務器重啟排查。".to_string(),
                timestamp: chrono::Utc::now(),
                is_system: false,
                reactions: vec![],
            },
        ],
    };
    let digest_ops = generator.create_thread_digest(thread_ops);
    assert_eq!(digest_ops.category, ThreadCategory::Ops);
}

/// 測試 Evolution 2: SpecValidator MVP 規範驗證
#[test]
fn test_spec_validator_rules() {
    let validator_strict = SpecValidator::new(true);

    // 合法規格
    let valid_md = r#"---
title: "2026-11-06 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "2026-11-06"
threads_count: 3
messages_count: 10
tags: ["digest", "daily"]
---

# 2026-11-06 每日對話摘要與決策複盤

## DoD 檢查清單
- [ ] 核心決策已提煉
"#;
    let res = validator_strict.validate(valid_md, "2026-11-06.md");
    assert!(res.valid);
    assert!(res.errors.is_empty());

    // 缺少 Front-matter
    let invalid_md_no_fm = "# 純標題無 Frontmatter";
    let res_no_fm = validator_strict.validate(invalid_md_no_fm, "2026-11-06.md");
    assert!(!res_no_fm.valid);
    assert!(res_no_fm.errors.contains(&ValidationError::MissingFrontmatter));

    // 日期不匹配（嚴格模式報錯，非嚴格模式通過並警告）
    let res_mismatch = validator_strict.validate(valid_md, "2026-11-07.md");
    assert!(!res_mismatch.valid);

    let validator_non_strict = SpecValidator::new(false);
    let res_mismatch_warn = validator_non_strict.validate(valid_md, "2026-11-07.md");
    assert!(res_mismatch_warn.valid);
}

/// 測試 Evolution 3: 分類渲染輸出結構
#[test]
fn test_categorized_markdown_renderer() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
    let thread_eng = ThreadDigest {
        thread_id: "thread1".to_string(),
        root_event_id: "root1".to_string(),
        participants: vec!["alice".to_string()],
        message_count: 3,
        decisions: vec!["決議採用 Rust 實作".to_string()],
        action_items: vec![ActionItem {
            description: "撰寫 API 文檔".to_string(),
            assignee: "Pollen".to_string(),
            completed: false,
            due_date: None,
        }],
        key_code_snippets: vec![],
        summary: "工程模組討論".to_string(),
        category: ThreadCategory::Engineering,
    };

    let thread_hw = ThreadDigest {
        thread_id: "thread2".to_string(),
        root_event_id: "root2".to_string(),
        participants: vec!["honey".to_string()],
        message_count: 2,
        decisions: vec!["建議採用 AMD EPYC 系列".to_string()],
        action_items: vec![],
        key_code_snippets: vec![],
        summary: "硬體選型討論".to_string(),
        category: ThreadCategory::Hardware,
    };

    let digest = DailyDigest {
        date,
        threads: vec![thread_eng, thread_hw],
        total_messages: 5,
        all_decisions: vec!["決議採用 Rust 實作".to_string(), "建議採用 AMD EPYC 系列".to_string()],
        all_action_items: vec![ActionItem {
            description: "撰寫 API 文檔".to_string(),
            assignee: "Pollen".to_string(),
            completed: false,
            due_date: None,
        }],
        all_key_code_snippets: vec![],
        generated_at: chrono::Utc::now(),
    };

    let config = DigestConfig::default();
    let rendered = MarkdownRenderer::render_daily_digest(date, &digest, &config);

    assert!(rendered.contains("title: \"2026-11-06 每日對話摘要與決策複盤\""));
    assert!(rendered.contains("## 📊 統計概覽"));
    assert!(rendered.contains("## ⚙️ 工程類"));
    assert!(rendered.contains("## 🔧 硬體諮詢類"));
    assert!(rendered.contains("## DoD 檢查清單"));
    assert!(rendered.contains("- [[core-memory]]"));

    // 驗證渲染出的 Markdown 通過 SpecValidator 嚴格驗證
    let validator = SpecValidator::new(true);
    let val_res = validator.validate(&rendered, "2026-11-06.md");
    assert!(val_res.valid, "Rendered Markdown must pass SpecValidator: {:?}", val_res.errors);
}
