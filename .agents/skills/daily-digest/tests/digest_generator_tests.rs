//! 摘要生成器單元測試
//! 
//! 測試 Daily Digest Skill 的核心功能，包括：
//! 1. 行級雜訊過濾器功能
//! 2. 摘要生成器核心邏輯
//! 3. 每日摘要生成流程
//! 4. Markdown 渲染功能
//! 5. Vault 同步功能
//! 
//! 這些測試確保技能按照設計實現，並能正確處理各種邊界情況和邊緣情況

use std::path::PathBuf;

use crate::daily_digest::{
    noise_filter::NoiseFilter,
    digest_generator::{DigestGenerator, DigestConfig, Message, ThreadData, ThreadDigest, ActionItem, DailyDigest},
};

/// 測試行級雜訊過濾器基本功能
/// 
/// 測試 NoiseFilter 的創建和基本過濾邏輯
#[test]
fn test_noise_filter_creation() {
    let filter = NoiseFilter::new();
    assert!(!filter.noise_patterns.is_empty());
    assert!(!filter.signal_patterns.is_empty());
}

/// 測試有意義訊息識別
/// 
/// 測試 filter_lines 方法正確識別和保留有意義的訊息
#[test]
fn test_is_signal_line_with_signal() {
    let filter = NoiseFilter::new();
    assert!(filter.is_signal_line("@Pollen 請實作模組 X"));
    assert!(filter.is_signal_line("決議採用 Rust 實作"));
    assert!(filter.is_signal_line("Error: connection refused"));
    assert!(filter.is_signal_line("完成代碼審查"));
    assert!(filter.is_signal_line("fn main() { /* 代碼 */ }"));
    assert!(filter.is_signal_line("\u{1F600} 好的")); // 包含表情符號且有字
}

/// 測試雜訊訊息識別
/// 
/// 測試 filter_lines 方法正確識別和過濾掉雜訊訊息
#[test]
fn test_is_signal_line_with_noise() {
    let filter = NoiseFilter::new();
    assert!(!filter.is_signal_line("已完成"));
    assert!(!filter.is_signal_line("OK"));
    assert!(!filter.is_signal_line("收到"));
    assert!(!filter.is_signal_line("tracing::debug!(\"llm keepalive tick\")"));
}

/// 測試邊界情況處理
/// 
/// 測試各種邊界情況，包括空訊息、純表情、單詞等
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
/// 
/// 測試 filter_lines 方法對複雜訊息內容的處理
#[test]
fn test_filter_lines() {
    let filter = NoiseFilter::new();
    let content = "已完成 task #123\n@Pollen 請實作模組 X\nOK\n決議採用 Rust 實作\nError: connection refused\n完成代碼審查\ntracing::debug!(\"llm keepalive tick\")";
    
    let filtered = filter.filter_lines(content);
    
    assert_eq!(filtered.len(), 4); // @Pollen, 決議採用, Error, 完成代碼審查
    assert!(filtered.contains(&"@Pollen 請實作模組 X".to_string()));
    assert!(filtered.contains(&"決議採用 Rust 實作".to_string()));
    assert!(filtered.contains(&"Error: connection refused".to_string()));
    assert!(filtered.contains(&"完成代碼審查".to_string()));
}

/// 測試空訊息處理
/// 
/// 測試當訊息內容為空或全部為雜訊時的處理
#[test]
fn test_filter_lines_empty() {
    let filter = NoiseFilter::new();
    let content = "已完成\nOK\nheart"; // 所有都是雜訊
    
    let filtered = filter.filter_lines(content);
    assert!(filtered.is_empty());
}

/// 測試摘要生成器創建
/// 
/// 測試 DigestGenerator 的正確創建和配置
#[tokio::test]
async fn test_digest_generator_creation() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    assert!(generator.noise_filter.noise_patterns.len() > 0);
    assert!(generator.config.daily_folder == "daily");
}

/// 測試決策提取
/// 
/// 測試 extract_decisions 方法正確提取決策訊息
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
/// 
/// 測試 extract_action_items 方法正確提取行動項目訊息
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
/// 
/// 測試 extract_threads 方法正確構建執行緒數據結構
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
    
    // 檢查 thread1
    let thread1 = threads.iter().find(|t| t.thread_id == "thread1").unwrap();
    assert_eq!(thread1.thread_id, "thread1");
    assert_eq!(thread1.message_count, 2);
    assert!(thread1.participants.contains(&"alice".to_string()));
    assert!(thread1.participants.contains(&"bob".to_string()));
    assert!(thread1.decisions.len() >= 1);
    assert!(thread1.action_items.len() >= 1);
    
    // 檢查 thread2
    let thread2 = threads.iter().find(|t| t.thread_id == "thread2").unwrap();
    assert_eq!(thread2.thread_id, "thread2");
    assert_eq!(thread2.message_count, 1);
    assert!(thread2.participants.contains(&"charlie".to_string()));
    assert!(thread2.decisions.len() >= 1);
}

/// 測試執行緒摘要創建
/// 
/// 測試 create_thread_digest 方法正確創建執行緒摘要
#[tokio::test]
async fn test_create_thread_digest() {
    let noise_filter = NoiseFilter::new();
    let config = DigestConfig::default();
    let generator = DigestGenerator::new(noise_filter, config);
    
    let thread_data = ThreadData {
        thread_id: "test_thread".to_string(),
        root_event_id: "root123".to_string(),
        messages: vec![
            Message {
                id: "msg1".to_string(),
                thread_id: "test_thread".to_string(),
                author: "alice".to_string(),
                content: "決議採用 Rust 實作。\n\n@
