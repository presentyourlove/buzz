---
name: proactive-trigger-orchestrator
description: "主動觸發器協調層技能，提供觸發器全生命週期 CRUD REST API、CircuitBreaker 熔斷機制、健康檢查與手動觸發介面，整合 TriggerRuntime 背景服務"
argument-hint: "[--port <port>] [--storage-dir <path>] [--config <path>]"
user-invocable: true
allowed-tools:
  - read_file
  - write_file
  - edit_file
  - shell
  - ask_user
model: "nvidia/nemotron-3-ultra-550b-a55b"
author: "Pollen"
version: "1.0.0"
tags: [orchestrator, triggers, circuit-breaker, api, phase4, standard, ect-align]
---

# 🎯 proactive-trigger-orchestrator 技能規格書

## 1. 角色定位

本技能屬於 **Phase 4**：主動觸發器協調層 (`proactive-trigger-orchestrator`)，負責提供觸發器的完整 REST API 管理介面、熔斷器機制、健康檢查與手動觸發功能。它是 Phase 3 `proactive-trigger-runtime` 的上層協調介面，採用「無狀態協調、有狀態持久化」架構。

## 2. 核心職責

### 2.1 觸發器 CRUD REST API
- **POST `/api/triggers`** - 建立觸發器（支援 Cron、Interval、Event 三類型）
- **GET `/api/triggers`** - 列出觸發器（支援分頁、過濾、排序）
- **GET `/api/triggers/:id`** - 取得單一觸發器詳細資訊
- **PUT `/api/triggers/:id`** - 更新觸發器（熱重載排程）
- **DELETE `/api/triggers/:id`** - 刪除觸發器（含排程清理）

### 2.2 手動觸發與條件求值
- **POST `/api/triggers/:id/trigger`** - 手動觸發（略過排程，直接求值條件→執行動作）
- 完整復用 `ConditionEvaluator`、`ActionExecutor`、`TriggerRegistry`、`EventBus`

### 2.3 熔斷器機制
- **GET `/api/triggers/:id/health`** - 觸發器級健康檢查（含熔斷狀態）
- 完全復用 `health_analyzer.rs` 的 `CircuitBreakerFile`、`CircuitBreakerStatus`
- 每個觸發器獨立熔斷，互不影響
- 支援配置閾值、自動/手動重置

### 2.4 服務健康檢查
- **GET `/health`** - 服務級健康檢查（Runtime 狀態、註冊數、版本）

### 2.5 生命週期管理
- `TriggerOrchestrator` 協調類別：啟動/停止 Runtime、優雅關機、配置熱重載
- 整合 `AppState`：`TriggerRuntime`、`TriggerRegistry`、`TriggerCircuitBreakerManager`、`EventBus`

## 3. 技術架構

```
┌─────────────────────────────────────────────────────────────┐
│                    Orchestrator Layer                        │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │   REST API  │  │ CircuitBreaker│  │  TriggerOrchestrator│  │
│  │  (Axum)     │  │   Manager     │  │  (Lifecycle)      │   │
│  └──────┬──────┘  └──────┬───────┘  └────────┬─────────┘   │
│         │                │                   │              │
│         └────────────────┼───────────────────┘              │
│                          ▼                                   │
│                  ┌───────────────┐                          │
│                  │   AppState    │                          │
│                  │  (Arc<...>)   │                          │
│                  └───────┬───────┘                          │
│                          │                                   │
├──────────────────────────┼──────────────────────────────────┤
│                    Runtime Layer (Phase 3)                   │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ TriggerRuntime│  │TriggerRegistry│ │  EventBus        │   │
│  │ (Scheduler)   │  │ (Persistence) │ │  (Pub/Sub)       │   │
│  └─────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## 4. 資料模型

### 4.1 Trigger 定義
```rust
pub struct Trigger {
    pub id: String,                    // UUID v4
    pub name: String,                  // 唯一名稱
    pub description: Option<String>,
    pub trigger_type: TriggerType,     // Schedule(Cron/Interval) | Event
    pub conditions: Vec<Condition>,    // 條件陣列（AND 邏輯）
    pub actions: Vec<Action>,          // 動作陣列（循序執行）
    pub enabled: bool,                 // 是否啟用
    pub status: TriggerStatus,         // Active/Inactive/Error
    pub deduplication: DeduplicationConfig,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_fired_at: Option<i64>,
    pub fire_count: u64,
    pub next_fire: Option<i64>,
}
```

### 4.2 CircuitBreaker 狀態
```rust
pub enum TriggerCircuitBreakerState {
    Closed,    // 正常
    HalfOpen,  // 嘗試恢復
    Open,      // 熔斷中
}
```

## 5. 介面契約

### 5.1 請求/回應格式
- **Content-Type**: `application/json`
- **錯誤格式**: `{ "error": "錯誤代碼", "message": "人類可讀訊息", "details": {} }`
- **分頁格式**: `{ "items": [...], "total": N, "page": 1, "page_size": 20 }`

### 5.2 環境變數
| 變數 | 預設值 | 說明 |
|------|--------|------|
| `ORCHESTRATOR_PORT` | `8080` | REST API 監聽埠 |
| `ORCHESTRATOR_STORAGE_DIR` | `./data/triggers` | 觸發器與熔斷器持久化目錄 |
| `ORCHESTRATOR_CONFIG` | `None` | 選配 TOML 配置檔路徑 |

## 6. 執行流程

### 6.1 啟動流程
1. 解析 CLI 參數與環境變數
2. 建立 `TriggerRegistry`（載入持久化觸發器）
3. 建立 `TriggerCircuitBreakerManager`（載入持久化熔斷狀態）
4. 建立 `EventBus`
5. 建立 `AppState` 整合上述組件
6. 建立 `TriggerRuntime` 並設定至 `AppState`
7. 啟動 `TriggerRuntime` 背景服務
8. 啟動 Axum HTTP Server（綁定 `create_router`）
9. 註冊優雅關機信號（SIGTERM/SIGINT）→ 停止 Runtime → 關閉 Server

### 6.2 觸發器建立流程
1. 驗證請求體（名稱唯一、Cron 格式、條件/動作結構）
2. `TriggerRegistry::register` 持久化
3. 若啟用 → `Scheduler::add_trigger` 註冊排程
4. 回傳 `TriggerResponse`

### 6.3 手動觸發流程
1. 從 Registry 取得觸發器
2. 檢查熔斷器狀態（Open 時拒絕，回傳 503）
3. 建立 `EvaluationContext`（含當前系統事實）
4. `ConditionEvaluator::evaluate_all` 條件求值
5. 若通過，依序執行 `actions`（透過 `ActionExecutor`）
6. 記錄 `TriggerHistoryEntry` 至 Registry
7. 更新 `last_fired_at`、`fire_count`、`next_fire`
8. 發送 `TriggerFiredEvent` 至 `EventBus`
9. 記錄成功/失敗到熔斷器

## 7. 錯誤代碼表

| 代碼 | HTTP | 說明 |
|------|------|------|
| `TRIGGER_NOT_FOUND` | 404 | 觸發器不存在 |
| `TRIGGER_NAME_EXISTS` | 409 | 名稱衝突 |
| `INVALID_CRON` | 400 | Cron 表達式無效 |
| `INVALID_CONDITION` | 400 | 條件結構錯誤 |
| `INVALID_ACTION` | 400 | 動作結構錯誤 |
| `CIRCUIT_OPEN` | 503 | 熔斷器開啟，拒絕觸發 |
| `RUNTIME_NOT_READY` | 503 | Runtime 未啟動 |
| `INTERNAL_ERROR` | 500 | 內部錯誤 |

## 8. 測試策略

### 8.1 單元測試
- `CircuitBreaker` 狀態機轉換（Closed→HalfOpen→Open→Closed）
- 多觸發器獨立熔斷隔離
- 熔斷器持久化載入/儲存

### 8.2 整合測試
- 完整 CRUD 流程（建立→讀取→更新→刪除）
- 手動觸發：條件通過→執行動作→記錄歷史
- 手動觸發：條件不通過→不執行動作
- 熔斷器開啟時拒絕觸發
- 更新觸發器熱重載排程

### 8.3 端到端測試
- 啟動 Orchestrator → 發送 HTTP 請求 → 驗證回應與持久化
- 優雅關機：收到 SIGTERM → 停止接受請求 → 完成進行中觸發 → 關閉

## 9. 觀測性

### 9.1 結構化日誌
- `tracing` crate，JSON 格式輸出
- 關鍵 Span：`http_request`、`trigger_evaluation`、`action_execution`、`circuit_breaker_transition`

### 9.2 指標
- 觸發器總數、啟用數、錯誤數
- 熔斷器狀態分佈
- 平均觸發延遲、P99
- Runtime uptime

## 10. 部署檢查清單

- [ ] `ORCHESTRATOR_PORT` 端口可用
- [ ] `ORCHESTRATOR_STORAGE_DIR` 可讀寫
- [ ] 依賴 `TriggerRuntime` (Phase 3) 已部署並正常
- [ ] 單元測試通過：`cargo test -p buzz-agent test_orchestrator`
- [ ] 整合測試通過
- [ ] 健康檢查端點 `/health` 回傳 `healthy`
- [ ] 優雅關機驗證（SIGTERM 10秒內完成）

## 11. 相依性

- **Phase 3 必須完成**：`proactive-trigger-runtime`（`TriggerRuntime`、`TriggerRegistry`、`ConditionEvaluator`、`ActionExecutor`、`EventBus`）
- **內部 Crate**：`buzz-agent`（`triggers` 模組）、`buzz-core`（事件類型）
- **外部依賴**：`axum`、`tokio`、`tower-http`、`tracing`、`serde`、`uuid`、`chrono`、`thiserror`、`tempfile`（測試）

## 12. 版本歷程

| 版本 | 日期 | 變更 |
|------|------|------|
| 1.0.0 | 2026-11-19 | Phase 4 初版：CRUD API、CircuitBreaker、Orchestrator 生命週期 |
