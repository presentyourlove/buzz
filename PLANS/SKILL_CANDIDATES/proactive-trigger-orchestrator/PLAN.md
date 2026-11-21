---
title: "proactive-trigger-orchestrator 實作計畫書"
version: "1.0.0"
phase: "Phase 4"
author: "Honey2"
status: "DRAFT"
depends_on: ["proactive-trigger-runtime (Phase 3)"]
---

# 概述

本計畫書定義 `proactive-trigger-orchestrator` 技能的完整實作規格，包含：
1. **CRUD REST API** —— 觸發器全生命週期管理（建立、讀取、更新、刪除、啟用、停用、手動觸發、查詢歷史）
2. **CircuitBreaker 熔斷機制** —— 復用 Phase 1/3 的 `health_guardian` 熔斷邏輯，提供分級熔斷、自動恢復、手動重置
3. **JS/TS 介面層** —— 供前端/外部系統調用的 TypeScript 客戶端 SDK

**設計原則**：
- 嚴格消費 Phase 3 產出的 `TriggerRuntime`、`TriggerRegistry`、`ConditionEvaluator` 介面，**禁止修改**底層核心庫
- 採用「薄介面層」架構：Rust 核心暴露 Tauri 命令 → TypeScript 封裝 → 對外暴露 REST 風格 API
- 熔斷機制完全復用 `health_guardian.rs` 的 `CircuitBreakerFile`、`CircuitBreakerStatus`、`reset_circuit_breaker` 邏輯

---

# 1. 架構定位

```
┌─────────────────────────────────────────────────────────────────┐
│                    proactive-trigger-orchestrator               │
├─────────────────────────────────────────────────────────────────┤
│  TS/JS SDK (crates/buzz-agent/bindings/trigger-orchestrator/)   │
│  ├── TriggerOrchestratorClient                                  │
│  ├── TriggerCRUD                                                │
│  ├── CircuitBreakerManager                                      │
│  └── types.ts (共享型別)                                         │
├─────────────────────────────────────────────────────────────────┤
│  Tauri Commands (crates/buzz-agent/src/orchestrator/)           │
│  ├── trigger_crud.rs      // create/read/update/delete/enable/  │
│  │                         disable/fire/list/history            │
│  ├── circuit_breaker.rs   // status/list/reset/configure        │
│  └── mod.rs               // 命令註冊                            │
├─────────────────────────────────────────────────────────────────┤
│  核心依賴 (Phase 3 產出，唯讀)                                    │
│  ├── TriggerRuntime                                                   │
│  ├── TriggerRegistry                                                  │
│  ├── ConditionEvaluator                                               │
│  └── health_guardian (CircuitBreaker 邏輯)                          │
└─────────────────────────────────────────────────────────────────┘
```

---

# 2. 核心資料模型（對齊 Phase 3 types.rs）

所有型別直接引用或映射自 `crates/buzz-agent/src/triggers/types.rs`：

```typescript
// packages/trigger-orchestrator/src/types.ts
export interface Trigger {
  id: string;                    // UUID v4
  name: string;                  // 唯一業務名稱
  description?: string;
  enabled: boolean;
  trigger_type: TriggerType;
  conditions: Condition[];       // Condition DSL AST
  actions: Action[];             // 動作清單
  deduplication: DeduplicationConfig;
  created_at: string;            // ISO 8601
  updated_at: string;
  last_fired_at?: string;
  fire_count: number;
  next_fire?: string;
}

export type TriggerType = 
  | { Schedule: { cron_expression: string; timezone?: string } }
  | { FileSystem: { path: string; events: FileSystemEvent[]; recursive: boolean } }
  | { Webhook: { path: string; secret?: string; methods: string[] } }
  | { SystemMetric: { metric: SystemMetricType; threshold: MetricThreshold } }
  | { NostrEvent: { filter: NostrFilter } };

export interface Condition {
  // 遞迴結構，對齊 condition.rs 的 ConditionNode
  type: "and" | "or" | "not" | "atomic";
  conditions?: Condition[];      // and/or 使用
  condition?: Condition;         // not 使用
  fact?: string;                 // atomic 使用
  operator?: ComparisonOperator;
  value?: serde_json::Value;
}

export interface Action {
  type: ActionType;
  config: Record<string, serde_json::Value>;
}

export type ActionType = 
  | "SendNostrEvent" 
  | "RunCommand" 
  | "HttpRequest" 
  | "EmitEvent" 
  | "Log";

export interface DeduplicationConfig {
  key_template: string;
  window_secs: number;
  max_executions_per_window: number;
}

export interface TriggerHistoryEntry {
  id: string;
  trigger_id: string;
  trigger_name: string;
  fired_at: string;
  condition_result: boolean;
  actions_executed: string[];
  error?: string;
  execution_duration_ms: number;
}

export interface TriggerRuntimeHealth {
  is_running: boolean;
  running: boolean;
  registered_triggers: number;
  active_triggers: number;
  trigger_count: number;
  last_check: string;
  events_processed: number;
  errors_count: number;
  uptime_secs: number;
}
```

---

# 3. CRUD API 規格（Tauri Commands）

## 3.1 命令清單

| 命令 | 說明 | 輸入 | 輸出 |
|------|------|------|------|
| `trigger:create` | 建立觸發器 | `CreateTriggerRequest` | `Trigger` |
| `trigger:get` | 取得單一觸發器 | `id: string` | `Trigger` |
| `trigger:list` | 列出觸發器（支援分頁、過濾） | `ListTriggersRequest` | `PaginatedResponse<Trigger>` |
| `trigger:update` | 更新觸發器 | `UpdateTriggerRequest` | `Trigger` |
| `trigger:delete` | 刪除觸發器 | `id: string` | `void` |
| `trigger:enable` | 啟用觸發器 | `id: string` | `Trigger` |
| `trigger:disable` | 停用觸發器 | `id: string` | `Trigger` |
| `trigger:fire` | 手動觸發（略過條件/排程） | `id: string` | `TriggerFiredResult` |
| `trigger:history` | 查詢觸發歷史 | `HistoryQuery` | `PaginatedResponse<TriggerHistoryEntry>` |
| `trigger:runtime-health` | 取得執行環境健康狀態 | 無 | `TriggerRuntimeHealth` |

## 3.2 Request/Response 定義

```rust
// crates/buzz-agent/src/orchestrator/trigger_crud.rs

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTriggerRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: TriggerType,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub deduplication: Option<DeduplicationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTriggerRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub trigger_type: Option<TriggerType>,
    pub conditions: Option<Vec<Condition>>,
    pub actions: Option<Vec<Action>>,
    pub deduplication: Option<DeduplicationConfig>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListTriggersRequest {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub enabled: Option<bool>,
    pub trigger_type: Option<TriggerTypeFilter>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerFiredResult {
    pub success: bool,
    pub condition_result: Option<bool>,
    pub actions_executed: Vec<String>,
    pub error: Option<String>,
    pub execution_duration_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryQuery {
    pub trigger_id: Option<String>,
    pub from: Option<String>,      // ISO 8601
    pub to: Option<String>,
    pub success_only: Option<bool>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
```

## 3.3 實作細節

### `trigger:create`
1. 驗證 `name` 唯一性（`TriggerRegistry::get_by_name`）
2. 驗證 `cron_expression` 合法性（`BuzzSchedule::from_expression`）
3. 驗證 `conditions` AST 可編譯（`ConditionEvaluator::validate`）
4. 建立 `Trigger` 實體，寫入 `TriggerRegistry::insert`
5. 若 `enabled=true` 且 `TriggerRuntime::is_running()`，熱載入至運行中排程器

### `trigger:update`
1. 讀取現有觸發器，合併變更
2. 重新驗證（同 create）
3. `TriggerRegistry::update` 持久化
4. 熱重載：停用舊排程 → 若啟用則註冊新排程

### `trigger:fire`（手動觸發）
1. 從 Registry 取得觸發器
2. 建立 `EvaluationContext`（含當前系統事實）
3. `ConditionEvaluator::evaluate_all` 條件求值
4. 若通過，依序執行 `actions`（透過 `ActionExecutor`）
5. 記錄 `TriggerHistoryEntry` 至 Registry
6. 更新 `last_fired_at`、`fire_count`、`next_fire`
7. 發送 `TriggerFiredEvent` 至 `EventBus`

---

# 4. CircuitBreaker 熔斷機制

## 4.1 設計目標

- **完全復用** `health_guardian.rs` 的 `CircuitBreakerFile`、`CircuitBreakerStatus`、檔案持久化、自動/手動重置邏輯
- 將熔斷狀態**掛載到觸發器層級**：每個觸發器獨立熔斷，互不影響
- 提供 **REST 風格管理介面**：查詢狀態、手動重置、配置閾值

## 4.2 資料模型（映射自 health_analyzer.rs）

```typescript
// packages/trigger-orchestrator/src/circuit-breaker.ts
export interface CircuitBreakerStatus {
  component: string;           // 觸發器 ID 或 "trigger:{id}"
  tripped: boolean;
  tripped_at?: string;
  reason?: string;
  failure_count: number;
  last_failure?: string;
  threshold: number;           // 觸發熔斷的失敗次數
  auto_reset_after_secs?: numb
