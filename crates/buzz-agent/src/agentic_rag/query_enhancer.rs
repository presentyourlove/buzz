//! 企業級 Agentic RAG：Query 增強工作流 (query_enhancer)
//!
//! 包含 HyDE (Hypothetical Document Embeddings) 假想答案文本生成範式
//! 與 Multi-Query 多維度查詢詞展開。

use serde::{Deserialize, Serialize};

/// Query 增強結果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnhancedQuery {
    pub original_query: String,
    pub hypothetical_document_prompt: String,
    pub multi_queries: Vec<String>,
}

/// 查詢增強器
#[derive(Debug, Default, Clone)]
pub struct QueryEnhancer;

impl QueryEnhancer {
    pub fn new() -> Self {
        Self
    }

    /// 對原始查詢進行 HyDE 與 Multi-Query 增強
    pub fn enhance(&self, query: &str) -> EnhancedQuery {
        let trimmed = query.trim();

        // 1. 生成 HyDE 假設性文件生成 Prompt 指引
        let hyde_prompt = format!(
            "請以資深技術架構師視角，針對以下問題撰寫一段 100~200 字的理想解答範例（包含關鍵實作概念與核心設定），作為語義向量匹配的參考假想文本：\n「{}」",
            trimmed
        );

        // 2. Multi-Query 多角度展開 (3 個維度：架構定義、代碼實作、排錯驗證)
        let multi_queries = vec![
            format!("{} 架構設計與規範定義", trimmed),
            format!("{} 原始碼實作與函數調用", trimmed),
            format!("{} 常見錯誤除錯與測試驗證", trimmed),
        ];

        EnhancedQuery {
            original_query: trimmed.to_string(),
            hypothetical_document_prompt: hyde_prompt,
            multi_queries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_enhancer_multi_query() {
        let enhancer = QueryEnhancer::new();
        let res = enhancer.enhance("NIP-ER 定時提醒機制");

        assert_eq!(res.original_query, "NIP-ER 定時提醒機制");
        assert!(res.hypothetical_document_prompt.contains("資深技術架構師"));
        assert_eq!(res.multi_queries.len(), 3);
        assert!(res.multi_queries[0].contains("架構設計"));
        assert!(res.multi_queries[1].contains("原始碼實作"));
        assert!(res.multi_queries[2].contains("常見錯誤"));
    }
}

