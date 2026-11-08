//! 企業級 Agentic RAG：本機語意快取 (semantic_cache)
//!
//! 針對高頻重複與高度相似之提問，直接命中本地快取返回檢索結果，
//! 降低模型 Token 消耗與端到端延遲。

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::hybrid_search::SearchHit;

/// 快取條目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub cached_at: u64,
    pub ttl_secs: u64,
}

/// 語意快取管理器
#[derive(Debug, Default, Clone)]
pub struct SemanticCache {
    default_ttl: u64,
    entries: HashMap<String, CacheEntry>,
}

impl SemanticCache {
    pub fn new(default_ttl: u64) -> Self {
        Self {
            default_ttl,
            entries: HashMap::new(),
        }
    }

    /// 正規化查詢 key
    fn normalize_key(query: &str) -> String {
        query.trim().to_lowercase().replace(' ', "")
    }

    /// 查詢快取
    pub fn get(&self, query: &str, now: u64) -> Option<&[SearchHit]> {
        let key = Self::normalize_key(query);
        if let Some(entry) = self.entries.get(&key) {
            if now <= entry.cached_at + entry.ttl_secs {
                return Some(&entry.hits);
            }
        }
        None
    }

    /// 寫入快取
    pub fn put(&mut self, query: &str, hits: Vec<SearchHit>, now: u64) {
        let key = Self::normalize_key(query);
        self.entries.insert(
            key,
            CacheEntry {
                query: query.to_string(),
                hits,
                cached_at: now,
                ttl_secs: self.default_ttl,
            },
        );
    }

    /// 清理過期快取
    pub fn clean_expired(&mut self, now: u64) -> usize {
        let initial_len = self.entries.len();
        self.entries.retain(|_, entry| now <= entry.cached_at + entry.ttl_secs);
        initial_len - self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_cache_hit_and_evict() {
        let mut cache = SemanticCache::new(300); // 5 分鐘 TTL
        let now = 1794120000;

        let hit = SearchHit {
            id: "1".into(),
            content: "快取內容".into(),
            source_file: "test.md".into(),
            line_number: Some(1),
            score: 0.9,
        };

        cache.put("Windows 編譯說明", vec![hit], now);

        // 命中測試 (包含大小寫與空白寬容)
        let found = cache.get("  windows 編譯說明 ", now + 100);
        assert!(found.is_some());
        assert_eq!(found.unwrap()[0].id, "1");

        // 逾時測試
        assert!(cache.get("windows 編譯說明", now + 301).is_none());

        // 清理過期測試
        let cleaned = cache.clean_expired(now + 301);
        assert_eq!(cleaned, 1);
    }
}

