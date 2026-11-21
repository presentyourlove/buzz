# 設計規格書 D：smolagents 技術儲備

## 1. 概覽與目標
本規格書定義 smolagents 框架在 Buzz 生態系中的技術儲備與整合方案。smolagents 是 Hugging Face 推出的輕量級 Agent 框架，特點為：代碼即 Agent（Code Agents）、原生支援工具調用、極簡 API 設計、強類型結構化輸出。

### 1.1 設計目標
- **技術儲備**：建立 smolagents 在本地環境的可運行 POC，驗證其與 Buzz Agent (buzz-agent) 的互操作性
- **整合評估**：評估 smolagents 的 Code Agent 模式能否補強 Buzz 現有 Agent 的結構化輸出與工具鏈能力
- **遷移路徑**：若評估通過，定義從 buzz-agent 向 smolagents 混合模式演進的介面契約與資料遷移策略

### 1.2 非目標 (Out of Scope)
- 本階段**不**替換現有 buzz-agent 核心運行時
- 本階段**不**引入 Hugging Face Hub 模型下載依賴（使用本地/開源模型）
- 本階段**不**處理多模態 Agent 能力（純文字/代碼工具鏈為主）

## 2. POC 範圍
### 2.1 核心驗證場景
| 場景 ID | 描述 | 成功標準 |
|---------|------|----------|
| D-POC-01 | Code Agent 基礎執行 | 能以 Python 代碼形式定義並執行多步推理任務，工具調用正確 |
| D-POC-02 | 結構化輸出 | 使用  類別與  定義工具，輸出符合 Schema 驗證 |
| D-POC-02 | 結構化輸出 | 使用 Tool 類別與 JsonSchema 定義工具，輸出符合 Schema 驗證 |
| D-POC-03 | 本地模型推理 | 支援 llama.cpp / ollama 本地模型，無外部 API 依賴 |
| D-POC-04 | Buzz 事件橋接 | smolagents 執行結果能轉換為 Nostr 事件 (Kind 40002/30300) 發布至 Buzz 頻道 |
| D-POC-05 | 工具鏈互操作 | smolagents 定義的工具能直接調用 buzz-dev-mcp (shell, file, web_search 等) |

### 2.2 POC 交付物
- 可運行的 POC 代碼：
- 可運行的 POC 代碼：C:/私人/ai agent/buzz/.scratch/smolagents_poc/
- 技術評估報告：（含效能、易用性、整合難度評分）
- 技術評估報告：SMOLAGENTS_EVALUATION_REPORT.md（含效能、易用性、整合難度評分）
- 整合介面規格書：（若評估通過）
- 整合介面規格書：SMOLAGENTS_INTEGRATION_SPEC.md（若評估通過）

## 3. 技術選型
| 層級 | 選型 | 版本/來源 | 理由 |
|------|------|-----------|------|
| Agent 框架 | smolagents | GitHub: huggingface/smolagents (main branch) | 官方源碼、極簡 API、Code Agent 原生支援 |
| 本地推理引擎 | llama.cpp | b4600+ (GGUF 支援) | 純 C++、無 Python 依賴、CPU/GPU 統一、支援 GGUF 量化模型 |
| 替代推理引擎 | ollama | 0.3+ | Go 實現、REST API、模型管理友好、作為 llama.cpp 失效時備選 |
| 模型選擇 | Qwen2.5-Coder-7B-Instruct-GGUF (Q4_K_M) | Hugging Face: Qwen/Qwen2.5-Coder-7B-Instruct-GGUF | 代碼能力強、中文友好、7B 參數適合本地推理、Apache 2.0 授權 |
| Python 環境 | uv + Python 3.11+ | 官方 | 快速依賴管理、虛擬環境隔離、與 buzz-dev-mcp 兼容 |
| 事件橋接 | buzz-sdk (Rust) + Python FFI / 或 REST Webhook | crates/buzz-sdk | 複用現有 Nostr 事件建構與簽名邏輯，避免重複造輪子 |

## 4. 整合介面規格
### 4.1 事件橋接架構
```
smolagents POC (Python)          Buzz Relay (Rust)
+-------------------+           +-------------------+
| CodeAgent.run() |  ----->   |  HTTP/Webhook     |
| (工具鏈執行)  |           |  /hooks/smolagent |
+-------------------+           +-------------------+
       |                              |
       v                              v
+-------------------+           +-------------------+
| buzz-dev-mcp    |           |  Nostr 事件       |
| (shell, file,   |           |  Kind: 40002,     |
|  web_search)    |           |  30300, 40092     |
+-------------------+           +-------------------+
```

### 4.2 Webhook 契約 (POST /hooks/smolagent)
```json
{
  "agent_id": "smolagent-001",
  "task_id": "task-abc123",
  "channel_id": "5ca019f6-1a27-592a-b015-4baf0eb33572",
  "event_kind": 40002,
  "content": "任務執行完成：...",
  "tools_used": ["shell", "web_search"],
  "token_usage": {"input": 1250, "output": 380},
  "status": "success",
  "timestamp": "2026-11-19T10:30:00Z"
}
```

### 4.3 工具橋接介面
| smolagents Tool | buzz-dev-mcp 方法 | 參數映射 |
|-----------------|-------------------|----------|
| ShellTool       | shell             | command -> command, timeout -> timeout_ms |
| FileReadTool    | read_file         | path -> path, offset -> offset, limit -> limit |
| FileWriteTool   | str_replace       | path -> path, old_str -> old_str, new_str -> new_str |
| WebSearchTool   | web_search        | query -> query |
| WebFetchTool    | web_fetch         | url -> url |
| FactCheckTool   | fact_check        | claim -> claim, sources -> sources |

## 5. 測試環境與驗收
### 5.1 環境需求
- 硬體：CPU 推理支援 (AVX2)、16GB+ 記憶體、可選 GPU (CUDA/ROCm)
- 軟體：Windows 11 / Linux、Python 3.11+、uv、llama.cpp (b4600+)、可選 ollama
- 模型檔案：Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf (~4.5GB)
- 網路：本地回環 (127.0.0.1) 即可，無外網依賴

### 5.2 驗收測試用例
| 用例 ID | 描述 | 輸入 | 預期輸出 | 通過標準 |
|---------|------|------|----------|----------|
| D-TC-01 | 單輪代碼生成 | 自然語言需求 -> Python 函數 | 可執行代碼、單元測試通過 | 代碼語法正確、功能符合需求 |
| D-TC-02 | 多輪工具鏈 | 問題 -> 搜尋 -> 讀檔 -> 寫檔 -> 驗證 | 完整工作流執行、結果正確 | 每步工具調用成功、最終結果正確 |
| D-TC-03 | 結構化輸出驗證 | 定義 JSON Schema 的工具 | 符合 Schema 的輸出 | jsonschema 驗證通過 |
| D-TC-04 | Buzz 事件發布 | 任務完成觸發 Webhook | Buzz 頻道收到 Kind:40002 事件 | 事件簽名有效、內容正確、可在客戶端顯示 |
