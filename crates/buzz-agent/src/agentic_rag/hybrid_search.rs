//! 企業級 Agentic RAG：本機輕量混合檢索與 RRF 融合排序 (hybrid_search)
//!
//! 結合關鍵字全文檢索 (FTS) 與語義向量檢索，採用 RRF (Reciprocal Rank Fusion)
//! 演算法進行無量綱融合排序，平滑常數 k=60。

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 檢索命中片段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub content: String,
    pub source_file: String,
    pub line_number: Option<usize>,
    pub score: f32,
}

/// RRF 融合搜尋器
#[derive(Debug, Clone)]
pub struct HybridSearchEngine {
    rrf_k: f32,
}

impl Default for HybridSearchEngine {
    fn default() -> Self {
        Self { rrf_k: 60.0 }
    }
}

impl HybridSearchEngine {
    pub fn new(rrf_k: f32) -> Self {
        Self { rrf_k }
    }

    /// RRF (Reciprocal Rank Fusion) 融合演算法
    ///
    /// RRF_Score(d) = sum_{m \in M} 1 / (k + rank_m(d))
    pub fn reciprocal_rank_fusion(
        &self,
        keyword_hits: &[SearchHit],
        vector_hits: &[SearchHit],
        limit: usize,
    ) -> Vec<SearchHit> {
        let mut rrf_scores: HashMap<String, (f32, SearchHit)> = HashMap::new();

        // 1. 累加關鍵字檢索排名倒數
        for (rank, hit) in keyword_hits.iter().enumerate() {
            let rrf_contribution = 1.0 / (self.rrf_k + (rank + 1) as f32);
            let entry = rrf_scores.entry(hit.id.clone()).or_insert_with(|| (0.0, hit.clone()));
            entry.0 += rrf_contribution;
        }

        // 2. 累加語義向量檢索排名倒數
        for (rank, hit) in vector_hits.iter().enumerate() {
            let rrf_contribution = 1.0 / (self.rrf_k + (rank + 1) as f32);
            let entry = rrf_scores.entry(hit.id.clone()).or_insert_with(|| (0.0, hit.clone()));
            entry.0 += rrf_contribution;
        }

        // 3. 排序輸出
        let mut combined: Vec<SearchHit> = rrf_scores
            .into_values()
            .map(|(score, mut hit)| {
                hit.score = score;
                hit
            })
            .collect();

        combined.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        combined.truncate(limit);
        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_fusion_scoring() {
        let engine = HybridSearchEngine::default();

        let kw1 = SearchHit {
            id: "doc1".into(),
            content: "VerifiedWrite 規格".into(),
            source_file: "ADR-01.md".into(),
            line_number: Some(10),
            score: 0.9,
        };
        let kw2 = SearchHit {
            id: "doc2".into(),
            content: "NIP-ER 協議".into(),
            source_file: "ADR-02.md".into(),
            line_number: Some(20),
            score: 0.8,
        };

        let vec1 = SearchHit {
            id: "doc2".into(),
            content: "定時提醒事件".into(),
            source_file: "ADR-02.md".into(),
            line_number: Some(20),
            score: 0.95,
        };
        let vec2 = SearchHit {
            id: "doc3".into(),
            content: "Agent-S 自演化".into(),
            source_file: "ADR-03.md".into(),
            line_number: Some(30),
            score: 0.85,
        };

        let results = engine.reciprocal_rank_fusion(&[kw1, kw2], &[vec1, vec2], 5);

        assert_eq!(results.len(), 3);
        // doc2 在兩邊皆高排名前列，融合後應居首位
        assert_eq!(results[0].id, "doc2");
        assert!(results[0].score > results[1].score);
    }
}

