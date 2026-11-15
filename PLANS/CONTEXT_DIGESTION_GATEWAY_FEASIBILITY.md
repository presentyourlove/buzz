---
title: "Buzz 上下文消化與語義檢索中介機制 - 可行性分析與實施方案"
tags: [architecture, workflow, agent, embedding, context-window, rag]
status: active
created: 2026-10-24
---

# Buzz 上下文消化與語義檢索中介機制 - 可行性分析與實施方案

## 背景與問題定義

### 核心痛點
- **Token 爆炸**：頻道歷史訊息過長，導致 Agent (Fizz/Pollen/Honey) 接收上下文時超出模型 Token 限制
- **資訊稀釋**：長對話中關鍵決策、背景資訊被大量雜訊淹沒
- **重複計算**：每次 Agent 執行都需重新讀取完整歷史，浪費算力與成本

### 目標
建立一個**「上下文消化與語義檢索中介層」**，在使用者發言 → Agent 接收之間，自動：
1. **消化濃縮** 長對話為關鍵背景摘要
2. **語義檢索** 根據當前任務檢索相關歷史片段
3. **動態注入** 只將必要上下文傳給下游 Agent

---

## 現有架構分析

### 1. 訊息流向 (Relay → Desktop → Agent)

```
用戶發言 (kind:9)
    │
    ▼
Relay 儲存事件 (buzz-db + buzz-search 索引)
    │
    ▼
WorkflowEngine.on_event() ──觸發工作流──▶ 執行步驟 (send_message, call_webhook...)
    │
    ▼
Desktop: observerRelayStore 訂閱 agent telemetry
    │
    ▼
buzz-agent (ACP) 接收 session/prompt ──▶ LLM 推理
```

### 2. 關鍵組件

| 組件 | 位置 | 功能 | 可擴充點 |
|------|------|------|----------|
| **buzz-search** | `crates/buzz-search` | PostgreSQL FTS (tsvector) 全文檢索 | 已有社群級索引、權限隔離 |
| **buzz-workflow** | `crates/buzz-workflow` | YAML 定義、觸發器、步驟執行、ActionSink | `call_webhook` 可呼叫外部 API、`send_message` 回覆頻道 |
| **buzz-agent** | `crates/buzz-agent` | ACP 協議、會話管理、工具調用、Token 追蹤 | `system_prompt` 注入、`session/prompt` 歷史控制 |
| **Desktop Agent 橋接** | `desktop/src/features/agents/observerRelayStore.ts` | 解密 observer 事件、構建 transcript、提供給 UI | `TranscriptState` 可被攔截/修飾 |

### 3. 現有 Token 控制機制
- `buzz-agent` 內建 `should_handoff` 機制：根據 `last_request_input_tokens` 決定是否換人
- `accumulated_input_tokens` 追蹤會話累積 Token
- 但**缺乏**：主動壓縮/摘要/檢索機制

---

## 可行性評估

### 方案對比矩陣

| 維度 | 方案 A：Workflow + call_webhook | 方案 B：自訂 ActionSink 擴充 | 方案 C：Desktop 層攔截 Transcript | 方案 C：Relay 層前置處理 |
|------|--------------------------------|------------------------------|----------------------------------|-------------------------|
| **實作難度** | ⭐⭐ 低 (現有功能組合) | ⭐⭐⭐ 中 (需擴充 ActionSink trait) | ⭐⭐⭐ 中 (React 狀態管理複雜) | ⭐⭐⭐⭐⭐ 高 (核心路徑修改) |
| **延遲** | +1-3 秒 (HTTP 呼叫) | +50-200ms (進程內) | ~0 (本地處理) | +50-100ms (寫入路徑) |
| **彈性** | 高 (YAML 可配置) | 高 (Rust 類型安全) | 中 (受限於前端架構) | 低 (影響所有租戶) |
| **權限/安全** | 需 owner/admin (SEC-006) | 同 Workflow 權限模型 | 僅本地資料 | 需極謹慎設計 |
| **多租戶隔離** | 原生支援 (community_id) | 原生支援 | 原生支援 (per-agent store) | 需額外實作 |
| **維護成本** | 低 (聲明式) | 中 (Rust 代碼) | 中 (前端邏輯) | 高 (核心基礎設施) |
| **推薦度** | ✅ **首選 (MVP)** | ✅ 進階版 | ⚠️ 補充方案 | ❌ 不建議 |

### 結論：**方案 A (Workflow + call_webhook) 為最佳 MVP 路徑**

理由：
1. **零架構變更**：利用現有 `call_webhook` + `send_message` 即可串接外部 Embedding 服務
2. **聲明式配置**：YAML 即可調整觸發條件、處理邏輯、輸出格式
3. **權限模型就緒**：SEC-006 已處理 exfiltration 風險
4. **可觀測性**：Workflow 執行追蹤、失敗重試、審計日誌皆內建

---

## 實施方案：三階段演進

### Phase 1：MVP - Workflow 驅動的「消化器」 (1-2 週)

#### 架構圖
```
用戶發言 (kind:9)
    │
    ▼
WorkflowEngine.on_event() ──匹配 trigger: message_posted
    │
    ├──▶ Step 1: call_webhook → Gemini Embedding API
    │     Input: 最近 N 條訊息 (從 buzz-search 取得)
    │     Output: 摘要 + 關鍵決策點 + 語意向量
    │
    ├──▶ Step 2: call_webhook → 向量資料庫 (可選: pgvector / Pinecone / 本地)
    │     存儲: (event_id, summary, embedding, metadata)
    │
    └──▶ Step 3: send_message → 發布「消化結果」到專用頻道或 Thread
          格式: 📋 **Context Digest** [timestamp]
          • Summary: ...
          • Key Decisions: [...]
          • Relevant Ref: #msg-id-1, #msg-id-2
```

#### Workflow YAML 範例
```yaml
name: "Context Digestion Gateway"
description: "每則訊息觸發，消化歷史並發布摘要供 Agent 參考"
trigger:
  on: message_posted
  filter: 'str_len(trigger_text) > 10'  # 忽略太短訊息
steps:
  - id: fetch_recent
    name: "取得最近 50 條頻道歷史"
    action: call_webhook
    url: "http://localhost:8080/internal/search"  # 內部搜尋服務
    method: "POST"
    headers:
      Content-Type: "application/json"
    body: |
      {
        "community": "{{community.id}}",
        "channel": "{{trigger.channel_id}}",
        "limit": 50,
        "since_hours": 24
      }
    timeout_secs: 10

  - id: digest
    name: "呼叫 Gemini Embedding 摘要化"
    action: call_webhook
    url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:embedContent"
    method: "POST"
    headers:
      Content-Type: "application/json"
      x-goog-api-key: "{{secrets.GEMINI_API_KEY}}"
    body: |
      {
        "content": {
          "parts": [{"text": "請將以下對話濃縮為 200 字內摘要，並列出 3-5 個關鍵決策點：\n{{steps_fetch_recent_output_body}}"}]
        },
        "task_type": "SUMMARIZATION"
      }
    timeout_secs: 30

  - id: store_embedding
    name: "存入向量庫 (可選)"
    if: 'steps_digest_output_status == 200'
    action: call_webhook
    url: "http://localhost:8080/internal/vector-upsert"
    method: "POST"
    body: |
      {
        "channel": "{{trigger.channel_id}}",
        "event_id": "{{trigger.message_id}}",
        "summary": "{{steps_digest_output_body}}",
        "embedding": "{{steps_digest_output_embedding}}"
      }
    timeout_secs: 10

  - id: publish_digest
    name: "發布消化結果到 #context-digest 頻道"
    action: send_message
    channel: "context-digest-uuid"  # 預建立的專用頻道
    text: |
      📋 **Context Digest** `{{trigger.timestamp}}`
      **Channel**: #{{trigger.channel_name}}
      **Trigger**: {{trigger.text | truncate(100)}}
      ---
      {{steps_digest_output_body}}
      ---
      *Refs: {{steps_fetch_recent_output_top_3_ids}}*
    reply_in_thread: false
```

#### 所需基礎設施
| 項目 | 說明 | 實作方式 |
|------|------|----------|
| **內部搜尋 API** | 供 Workflow 取得頻道歷史 | 新增 relay 端點 `/internal/search`，直接用 `buzz-search` |
| **Gemini API Key** | 儲存於 secrets | `buzz-workflow` 已支援 `headers` 模板，可引用 `{{secrets.KEY}}` |
| **向量資料庫** | 可選，Phase 2 再接 | Phase 1 可先跳過，只做摘要發布 |
| **專用頻道** | `#context-digest` | 手動建立或腳本自動建立 |

#### 優點
- **最快上線**：無需修改 Rust 核心代碼
- **可調參數**：YAML 改 `limit`、`since_hours`、摘要 Prompt 即可微調
- **失敗隔離**：Workflow 失敗不影響主頻道訊息流

---

### Phase 2：進階 - 原生 ActionSink 擴充 (3-4 週)

#### 目標
將「消化邏輯」內建到 Workflow 引擎，移除外部 HTTP 依賴，降低延遲。

#### 架構變更

1. **擴充 `ActionDef` 新增 `DigestContext` 動作**
```rust
// crates/buzz-workflow/src/schema.rs
pub enum ActionDef {
    // ... 既有項目
    /// 消化頻道上下文並產出摘要/向量
    DigestContext {
        /// 要消化的訊息數量上限
        limit: usize,
        /// 回溯時間範圍 (秒)
        since_secs: Option<u64>,
        /// 摘要指令模板
        prompt_template: String,
        /// 輸出目標：channel / variable / both
        #[serde(default)]
        output_to: DigestOutputTarget,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestOutputTarget {
    Channel,      // 發送到指定頻道
    Variable,     // 只存入 workflow 變數供下游步驟使用
    Both,
}
```

2. **實作 `ActionSink::digest_context`**
```rust
// crates/buzz-workflow/src/action_sink.rs
pub trait ActionSink: Send + Sync {
    // ... 既有 send_message
    
    /// 執行上下文消化：搜尋歷史 → 呼叫 Embedding → 回傳摘要
    fn digest_context(
        &self,
        community_id: CommunityId,
        channel_id: &str,
        params: DigestParams,
    ) -> Pin<Box<dyn Future<Output = Result<DigestResult, ActionSinkError>> + Send + '_>>;
}

// Relay 端實作：直接用 buzz-search + 內嵌 Embedding client
```

3. **內嵌 Embedding Client** (可選：支援多供應商)
```rust
// crates/buzz-workflow/src/embedding.rs
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    async fn summarize(&self, text: &str, prompt: &str) -> Result<String, EmbeddingError>;
}

pub struct GeminiEmbeddingProvider { api_key: String, http: reqwest::Client }
pub struct LocalEmbeddingProvider { model_path: PathBuf }  // 如 bge-m3, nomic-embed-text
```

#### Workflow YAML 簡化為
```yaml
name: "Context Digestion Gateway v2"
trigger:
  on: message_posted
steps:
  - id: digest
    action: digest_context
    limit: 50
    since_secs: 86400
    prompt_template: "摘要以下對話，保留關鍵決策與行動項：\n{{history}}"
    output_to: both
    timeout_secs: 15
```

#### 優點
- **延遲降低 10x**：無 HTTP 迴路，進程內直接呼叫
- **類型安全**：Rust 編譯期檢查參數
- **交易一致性**：可與 DB 寫入同一事務

---

### Phase 3：智能化 - RAG 增強 Agent 上下文 (4-6 週)

#### 目標
Agent 執行前，**自動檢索**相關歷史片段注入 `system_prompt`，而非依賴人工查看 `#context-digest` 頻道。

#### 架構
```
Agent 接收任務 (session/prompt)
    │
    ▼
Desktop: useAgentContextEnrichment Hook (新增)
    │
    ├──▶ 解析任務意圖 (關鍵詞 / Embedding)
    │
    ├──▶ 查詢向量庫 (pgvector / 專用)
    │     WHERE channel = current AND embedding
