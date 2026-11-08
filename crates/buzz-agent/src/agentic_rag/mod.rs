//! 企業級 Agentic RAG 智能知識庫吸收核心 (Agentic RAG Engine)
//!
//! 包含六大核心支柱：
//! 1. `hybrid_search`：本機輕量混合檢索與 RRF 融合排序 (k=60)
//! 2. `reranker`：本地交叉打分精排與 4KB 上下文配額硬上限截斷
//! 3. `query_enhancer`：HyDE 假想文本生成範式與 Multi-Query 多維度展開
//! 4. `citation_annotator`：行內腳註標記與 Obsidian 雙鏈行號直達格式化
//! 5. `chunker`：Markdown 標題層級滑動窗口重疊分塊切片器 (15% Overlap)
//! 6. `semantic_cache`：本機語意快取

pub mod hybrid_search;
pub mod reranker;
pub mod query_enhancer;
pub mod citation_annotator;
pub mod chunker;
pub mod semantic_cache;

pub use hybrid_search::{HybridSearchEngine, SearchHit};
pub use reranker::{LocalReranker, RerankedOutput};
pub use query_enhancer::{EnhancedQuery, QueryEnhancer};
pub use citation_annotator::{CitationAnnotator, CitationEntry};
pub use chunker::{SmartChunker, TextChunk};
pub use semantic_cache::{CacheEntry, SemanticCache};

