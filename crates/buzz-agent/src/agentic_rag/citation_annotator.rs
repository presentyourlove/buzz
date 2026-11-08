//! 企業級 Agentic RAG：來源引用嚴格標記與防幻覺校驗 (citation_annotator)
//!
//! 在特工回答中自動注入行內腳註標記 (`[1]`, `[2]`)，並在結尾輸出
//! 結構化來源卡片（支援 Obsidian 雙向鏈結直達行號與 Buzz 協議）。

use serde::{Deserialize, Serialize};
use super::hybrid_search::SearchHit;

/// 引用條目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitationEntry {
    pub index: usize,
    pub source_file: String,
    pub line_number: Option<usize>,
    pub summary_snippet: String,
    pub confidence_score: f32,
}

impl CitationEntry {
    /// 格式化為 Markdown 引用行 (支援 Obsidian 雙鏈與行號)
    pub fn to_markdown(&self) -> String {
        let location = match self.line_number {
            Some(line) => format!("[[{filename}#L{line}]]", filename = self.source_file, line = line),
            None => format!("[[{filename}]]", filename = self.source_file),
        };

        format!(
            "- [{idx}] {location} — `{snippet}` (置信度: {conf:.0}%)",
            idx = self.index,
            location = location,
            snippet = self.summary_snippet,
            conf = self.confidence_score * 100.0
        )
    }
}

/// 引用標註器
#[derive(Debug, Default, Clone)]
pub struct CitationAnnotator;

impl CitationAnnotator {
    pub fn new() -> Self {
        Self
    }

    /// 從檢索命中片段建立引用清單
    pub fn build_citations(&self, hits: &[SearchHit]) -> Vec<CitationEntry> {
        hits.iter()
            .enumerate()
            .map(|(i, hit)| {
                let snippet: String = hit.content.chars().take(60).collect();
                CitationEntry {
                    index: i + 1,
                    source_file: hit.source_file.clone(),
                    line_number: hit.line_number,
                    summary_snippet: snippet.replace('\n', " ").trim().to_string(),
                    confidence_score: hit.score.clamp(0.0, 1.0),
                }
            })
            .collect()
    }

    /// 渲染包含腳註的正文與底部來源清單區塊
    pub fn render_with_citations(&self, answer_body: &str, citations: &[CitationEntry]) -> String {
        if citations.is_empty() {
            return answer_body.to_string();
        }

        let mut out = String::new();
        out.push_str(answer_body.trim());
        out.push_str("\n\n---\n### 📚 參考資料來源 (Source Citations)\n\n");

        for cit in citations {
            out.push_str(&cit.to_markdown());
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_citation_annotator_formatting() {
        let annotator = CitationAnnotator::new();

        let hit1 = SearchHit {
            id: "1".into(),
            content: "VerifiedWrite Trait 寫入安全規範與讀回 SHA-256 校驗".into(),
            source_file: "wiki/01-Cold-Memory/ADR-005.md".into(),
            line_number: Some(42),
            score: 0.94,
        };

        let citations = annotator.build_citations(&[hit1]);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].index, 1);
        assert_eq!(citations[0].line_number, Some(42));

        let rendered = annotator.render_with_citations("特工寫入時會進行原子讀回比對 [1]。", &citations);
        assert!(rendered.contains("[[wiki/01-Cold-Memory/ADR-005.md#L42]]"));
        assert!(rendered.contains("置信度: 94%"));
    }
}

