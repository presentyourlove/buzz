---
title: "proactive-trigger-runtime"
version: "1.0.0"
description: "主動式觸發器運行時 — 整合 Scheduler、BuzzSchedule、ConditionEvaluator、EventBus 的背景服務"
author: "Pollen"
license: "MIT"
tags: ["trigger", "runtime", "scheduler", "condition", "event-bus"]
homepage: "https://github.com/block/buzz"
repository: "https://github.com/block/buzz"
---

# proactive-trigger-runtime

主動式觸發器運行時技能，提供完整的觸發器生命週期管理與執行引擎。

## 架構設計

```
┌─────────────────────────────────────────────────────────────┐
│                    TriggerRuntime                           │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Scheduler   │  │ BuzzSchedule │  │ ConditionEvaluator │  │
│  │  (cron/      │  │  (IANA tz,   │  │  (AST + Provider  │  │
│  │   interval)  │  │   DST safe)  │  │   Registry)      │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                 │                   │             │
│         └─────────────────┼───────────────────┘             │
│                           ▼                                 │
│              ┌───────────────────────┐                      │
│              │      EventBus         │                      │
│              │ (Pub/Sub + Backpressure)                      │
│              └───────────┬───────────┘                      │
│                          ▼                                   │
│              ┌───────────────────────┐                      │
│              │   TriggerExecutor     │                      │
│              │ (Action Pipeline)     │                      │
│              └───────────────────────┘                      │
└─────────────────────────────────────────────────────────────┘
```

## 核心組件

### 1. TriggerRuntime (主入口)
- 管理觸發器的完整生命週期：註冊、排程、評估、執行
- 提供啟動/停止/重載控制介面
- 內建優雅關閉與健康檢查

### 2. Scheduler (排程器)
- 基於 `buzz_cron::BuzzSchedule` 的 6 欄位 cron 解析
- 支援 IANA 時區與 DST 自動處理
- 支援固定間隔 + 抖動
- 使用二元堆優先級佇列管理下次觸發時間

### 3. ConditionEvaluator (條件評估器)
- AST 為基礎的條件 DSL（And/Or/Not/Atomic）
- 12 種比較運算子
- FactProvider trait 註冊表（系統指標、時間、配置、自訂）
- TTL 快取與短路求值優化

### 4. EventBus (事件總線)
- 發布/訂閱模式
- 背壓策略：DropOldest / Block / ReturnError
- 事件去重（可配置時間窗口）
- 多來源路由

## API 介面

### TriggerRuntime
```rust
pub struct TriggerRuntime {
    pub async fn new(config: TriggerRuntimeConfig) -> Result<Self>
    pub async fn start(&mut self) -> Result<()>
    pub async fn stop(&mut self) -> Result<()>
    pub async fn register_trigger(&self, trigger: Trigger) -> Result<()>
    pub async fn unregister_trigger(&self, id: Uuid) -> Result<()>
    pub async fn enable_trigger(&self, id: Uuid) -> Result<()>
    pub async fn disable_trigger(&self, id: Uuid) -> Result<()>
    pub async fn get_trigger_status(&self, id: Uuid) -> Result<TriggerStatus>
    pub async fn list_triggers(&self) -> Result<Vec<TriggerSummary>>
    pub async fn health_check(&self) -> HealthStatus
}
```

### TriggerRuntimeConfig
```rust
pub struct TriggerRuntimeConfig {
    pub scheduler: SchedulerConfig,
    pub event_bus: EventBusConfig,
    pub evaluator: EvaluatorConfig,
    pub executor: ExecutorConfig,
}
```

## 事件幂等性與去重

- 每個觸發器執行生成唯一 `execution_id` (UUID v7)
- 事件負載包含 `deduplication_key`（支援模板渲染）
- EventBus 內建滑動窗口去重（預設 1 小時）

## 錯誤處理與熔斷

- 整合 `system-health-guardian` 的 `CircuitBreaker`
- 執行失敗自動記錄連續失敗計數
- 達到閾值自動熔斷該觸發器
- 支援手動/自動重置

## 監控指標

- `triggers_registered_total` - 註冊觸發器總數
- `triggers_fired_total` - 觸發次數
- `triggers_failed_total` - 失敗次數
- `trigger_evaluation_duration_seconds` - 條件評估延遲
- `trigger_execution_duration_seconds` - 執行延遲
- `event_bus_queue_depth` - 事件佇列深度
- `scheduler_next_fire_timestamp` - 下次觸發時間戳

## 配置範例

```yaml
trigger_runtime:
  scheduler:
    check_interval_secs: 1
    max_concurrent: 10
    enable_jitter: true
    default_jitter_secs: 5
  event_bus:
    buffer_size: 1000
    backpressure_strategy: "DropOldest"
    dedup_window_secs: 3600
  evaluator:
    cache_ttl_secs: 60
    enable_short_circuit: true
  executor:
    max_concurrent_executions: 10
    execution_timeout_secs: 300
    retry_policy:
      max_retries: 3
      backoff_secs: 5
```

## 依賴關係

| Crate | 用途 |
|-------|------|
| `buzz_cron` | Cron 解析與時區處理 |
| `buzz_agent::triggers::condition` | 條件 AST 與評估器 |
| `buzz_agent::triggers::scheduler` | 排程器 |
| `buzz_agent::triggers::event_bus` | 事件總線 |
| `buzz_agent::triggers::types` | 核心類型定義 |
| `buzz_agent::health_guardian` | CircuitBreaker 復用 |

## 測試策略

1. **單元測試** - 每個組件獨立測試（已完成 65+ 測試）
2. **整合測試** - 端到端觸發器生命週期
3. **混沌測試** - 時區變更、時鐘漂移、背壓場景
4. **負載測試** - 1000+ 並發觸發器

## 部署考量

- 設計為長運行背景服務
- 支援熱重載觸發器定義（無需重啟）
- 優雅關閉：等待進行中執行完成或超時強制終止
- 支援多實例部署（需外部分佈式鎖協調）

## 相容性

- Rust 1.75+
- 支援 Linux/macOS/Windows
- 無外部服務依賴（純庫模式）

## 許可證

MIT
