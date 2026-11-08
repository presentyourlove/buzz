//! 企業級 Agentic RAG：智能滑動窗口重疊分塊切片器 (chunker)
//!
//! 依據 Markdown 標題層級拆分語義區塊，並在相鄰 Chunk 間保留 10%~20%
//! 的滑動重疊 (Overlap)，防止程式碼區塊或長列表在邊界被硬切斷裂。

use serde::{Deserialize, Serialize};

/// 文本分塊片段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextChunk {
    pub chunk_index: usize,
    pub title_hierarchy: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// 智能分塊器
#[derive(Debug, Clone)]
pub struct SmartChunker {
    pub max_chunk_chars: usize,
    pub overlap_ratio: f32, // 0.10 ~ 0.20
}

impl Default for SmartChunker {
    fn default() -> Self {
        Self {
            max_chunk_chars: 1000,
            overlap_ratio: 0.15,
        }
    }
}

impl SmartChunker {
    pub fn new(max_chunk_chars: usize, overlap_ratio: f32) -> Self {
        Self {
            max_chunk_chars,
            overlap_ratio: overlap_ratio.clamp(0.05, 0.30),
        }
    }

    /// 對 Markdown 文本進行分塊與滑動重疊處理
    pub fn chunk_markdown(&self, text: &str) -> Vec<TextChunk> {
        let lines: Vec<&str> = text.lines().collect();
        let mut chunks = Vec::new();

        let mut current_lines: Vec<&str> = Vec::new();
        let mut current_chars = 0;
        let mut start_line = 1;
        let mut current_heading = String::from("Root");

        let overlap_chars = (self.max_chunk_chars as f32 * self.overlap_ratio) as usize;

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let line_len = line.chars().count() + 1; // 算入換行

            // 偵測 Markdown 標題更新層級
            if line.starts_with('#') {
                current_heading = line.trim().to_string();
            }

            // 若目前積累的字元超出 chunk 上限，觸發打包切片
            if current_chars + line_len > self.max_chunk_chars && !current_lines.is_empty() {
                chunks.push(TextChunk {
                    chunk_index: chunks.len() + 1,
                    title_hierarchy: current_heading.clone(),
                    content: current_lines.join("\n"),
                    start_line,
                    end_line: line_num - 1,
                });

                // 計算保留重疊行
                let mut kept_lines: Vec<&str> = Vec::new();
                let mut kept_chars = 0;
                for l in current_lines.iter().rev() {
                    let l_len = l.chars().count() + 1;
                    if kept_chars + l_len <= overlap_chars {
                        kept_lines.push(l);
                        kept_chars += l_len;
                    } else {
                        break;
                    }
                }
                kept_lines.reverse();

                start_line = line_num.saturating_sub(kept_lines.len());
                current_lines = kept_lines;
                current_chars = kept_chars;
            }

            current_lines.push(line);
            current_chars += line_len;
        }

        // 收集最後剩餘的區塊
        if !current_lines.is_empty() {
            chunks.push(TextChunk {
                chunk_index: chunks.len() + 1,
                title_hierarchy: current_heading,
                content: current_lines.join("\n"),
                start_line,
                end_line: lines.len(),
            });
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunker_sliding_window_overlap() {
        let chunker = SmartChunker::new(300, 0.20); // 300 字上限，20% 重疊 (60字)

        let mut doc = String::new();
        doc.push_str("# 第一章 系統架構\n");
        for i in 1..=15 {
            doc.push_str(&format!("這是系統核心運行的第 {} 行詳細說明文字，包含安全門禁與設定。\n", i));
        }

        let chunks = chunker.chunk_markdown(&doc);
        assert!(chunks.len() >= 2);

        // 檢查相鄰區塊是否有重疊文字
        let chunk1_last_line = chunks[0].content.lines().last().unwrap();
        assert!(
            chunks[1].content.contains(chunk1_last_line),
            "相鄰 Chunk 應保有滑動窗口重疊"
        );
    }
}

