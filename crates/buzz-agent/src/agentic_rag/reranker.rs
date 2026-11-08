//! 企業級 Agentic RAG：本地輕量 Reranker 精排與上下文防爆修剪 (reranker)
//!
//! 對初檢候選進行交叉評分精排，並透過硬上限截斷控制最終上下文配額在 2KB~4KB 內，
//! 杜絕 Token 雪球效應。

use super::hybrid_search::SearchHit;

/// 精排結果
#[derive(Debug, Clone, PartialEq)]
pub struct RerankedOutput {
    pub selected_hits: Vec<SearchHit>,
    pub total_chars: usize,
    pub is_budget_truncated: bool,
}

/// 本地輕量重排器
#[derive(Debug, Clone)]
pub struct LocalReranker {
    max_budget_chars: usize,
    max_hits: usize,
}

impl Default for LocalReranker {
    fn default() -> Self {
        Self {
            max_budget_chars: 4000, // 4KB 上限
            max_hits: 5,
        }
    }
}

impl LocalReranker {
    pub fn new(max_budget_chars: usize, max_hits: usize) -> Self {
        Self {
            max_budget_chars,
            max_hits,
        }
    }

    /// 對初檢片段進行精排與上下文預算修剪
    pub fn rerank_and_prune(&self, query: &str, mut candidates: Vec<SearchHit>) -> RerankedOutput {
        let query_lower = query.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

        // 本地交叉打分 (查詢詞密度加成)
        for hit in candidates.iter_mut() {
            let content_lower = hit.content.to_lowercase();
            let mut cross_score = hit.score;

            for token in &query_tokens {
                if content_lower.contains(token) {
                    cross_score += 0.2;
                }
            }

            // 包含精確檔名匹配加成
            if hit.source_file.to_lowercase().contains(&query_lower) {
                cross_score += 0.5;
            }

            hit.score = cross_score;
        }

        // 依新評分排序
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected = Vec::new();
        let mut total_chars = 0;
        let mut is_budget_truncated = false;

        for hit in candidates {
            if selected.len() >= self.max_hits {
                break;
            }

            let hit_len = hit.content.chars().count();
            if total_chars + hit_len <= self.max_budget_chars {
                total_chars += hit_len;
                selected.push(hit);
            } else {
                is_budget_truncated = true;
                // 部分截斷以塞滿剩餘預算
                let remaining = self.max_budget_chars.saturating_sub(total_chars);
                if remaining >= 100 {
                    let truncated_content: String = hit.content.chars().take(remaining).collect();
                    let mut partial_hit = hit;
                    partial_hit.content = format!("{}... [截斷]", truncated_content);
                    total_chars += partial_hit.content.chars().count();
                    selected.push(partial_hit);
                }
                break;
            }
        }

        RerankedOutput {
            selected_hits: selected,
            total_chars,
            is_budget_truncated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reranker_top_k_and_budget_prune() {
        let reranker = LocalReranker::new(500, 3);

        let c1 = SearchHit {
            id: "1".into(),
            content: "a".repeat(200),
            source_file: "test1.md".into(),
            line_number: Some(1),
            score: 0.8,
        };
        let c2 = SearchHit {
            id: "2".into(),
            content: "b".repeat(200),
            source_file: "test2.md".into(),
            line_number: Some(2),
            score: 0.9,
        };
        let c3 = SearchHit {
            id: "3".into(),
            content: "c".repeat(200),
            source_file: "test3.md".into(),
            line_number: Some(3),
            score: 0.7,
        };

        let output = reranker.rerank_and_prune("查詢", vec![c1, c2, c3]);

        // 總預算 500，200 + 200 = 400，第三個 200 會超出被截斷
        assert!(output.total_chars <= 520);
        assert!(output.is_budget_truncated);
        assert_eq!(output.selected_hits.len(), 3);
        assert_eq!(output.selected_hits[0].id, "2"); // 最高分優先
    }
}

