---
name: agent-skill
description: "janestreet/agent_skill 整合技能 - 透過 Nostr 事件流 (Kind:30300) 與 Buzz 同步，提供輕量級 Agent 執行環境、任務持久化與 SQLite 去重"
argument-hint: "[--init <storage_dir>] [--submit <task_json>] [--monitor]"
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
tags: ["agent", "nostr", "task-persistence", "skill", "automation", "daily-evolution"]
---

# Agent Skill (janestreet/agent_skill 整合)

## 1. 目的與範圍

本技能實作 janestreet/agent_skill 概念的 Rust 整合，提供：
- **Nostr 事件橋接** (Kind:30300) - 與 Buzz 討論串雙向同步 Agent 任務
- **任務持久化** - SQLite 本機儲存，支援冪等防重放
- **輕量執行環境** - 非阻塞異步任務執行管線
- **每日進化整合** - 可作為每日系統進化機制的任務執行後端

## 2. 架構設計

### 2.1 模組結構

```
skills/agent-skill/
├── Cargo.toml              # 套件配置 (cdylib + rlib)
├── manifest.json           # 技能元數據清單
├── SKILL.md                # 本文件
└── src/
    ├── lib.rs              # 服務管理器與公共 API
    ├── nostr_bridge.rs     # Nostr Kind:30300 事件橋接
    ├── task_store.rs       # SQLite 任務持久化與去重
    └── runtime.rs          # 任務執行運行時
```

### 2.2 核心類型

| 類型 | 檔案 | 職責 |
|------|------|------|
| `AgentSkillService` | lib.rs | 服務生命週期管理 |
| `NostrBridge` | nostr_bridge.rs | 事件發布/訂閱、頻道/討論串綁定 |
| `TaskStore` | task_store.rs | CRUD、冪等鍵檢查、狀態機 |
| `SkillRuntime` | runtime.rs | 任務佇列、執行器、結果通道 |
| `AgentSkillTaskEvent` | nostr_bridge.rs | Kind:30300 事件結構 |

## 3. 關鍵功能規格

### 3.1 Nostr 事件橋接 (Kind:30300)

```rust
pub const KIND_AGENT_SKILL_TASK: Kind = Kind::Custom(30300);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSkillTaskEvent {
    pub task_id: String,
    pub task_type: String,
    pub agent_name: String,
    pub description: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: String,  // pending, running, completed, failed
    pub created_at: i64,
    pub updated_at: i64,
    pub channel_id: Option<String>,
    pub thread_id: Option<String>,
}
```

- **發布**: `NostrBridge::publish_task_event()` - 任務狀態變更時自動發布
- **訂閱**: `NostrBridge::subscribe()` - 監聽指定頻道/討論串的任務事件
- **去重**: 結合 `TaskStore::is_task_processed()` 防止重複執行

### 3.2 任務持久化與冪等

```rust
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

pub struct SkillTask {
    pub task_id: String,        // 冪等鍵，唯一
    pub task_type: String,
    pub agent_name: String,
    pub description: String,
    pub status: TaskStatus,
    pub input_json: String,
    pub output_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- **SQLite Schema**: `tasks` 表含 `task_id` PRIMARY KEY，保證唯一性
- **冪等檢查**: `is_task_processed(task_id)` 於執行前調用
- **狀態機**: Pending → Running → Completed/Failed/Cancelled

### 3.3 執行運行時

```rust
pub trait TaskExecutor: Send + Sync {
    async fn execute(&self, task: &SkillTask) -> Result<ExecutionResult>;
}

pub struct DefaultCommandExecutor;

pub struct SkillRuntime {
    store: Arc<TaskStore>,
    executor: Arc<dyn TaskExecutor>,
    // mpsc 通道接收執行結果
}
```

- **非阻塞**: 使用 `tokio::spawn` 背景執行
- **結果通道**: `mpsc::Receiver<ExecutionResult>` 供調用端輪詢
- **可擴展**: 實作 `TaskExecutor` trait 即可自定義執行邏輯

## 4. 整合點

### 4.1 每日進化系統 (`scripts/daily_evolution/`)

| 模組 | 整合方式 |
|------|----------|
| `01_work_discipline_guard` | 可提交「開工紀律檢查」任務至 AgentSkill |
| `04_code_health_check` | 可提交「程式碼健康檢查」任務 |
| `07_cicd_self_heal` | 可提交「CI/CD 自癒」任務 |
| `run_daily_evolution.py` | 可作為協調器提交每日任務批次 |

### 4.2 System Health Guardian (`crates/buzz-agent/src/health_guardian.rs`)

- 可註冊為 `HealthChecker` 插件
- 任務執行失敗時自動發送 Nostr 告警 (Kind:30300)
- 支援 `HealthAlertDaemon` 整合

### 4.3 Buzz Agent Core (`crates/buzz-agent/`)

- 可作為內建技能載入 (`buzz-agent` 依賴 `agent-skill`)
- 透過 `PreToolUse`/`PostToolUse` 門禁驗證任務提交

## 5. 使用範例

### 5.1 初始化服務

```rust
use agent_skill::AgentSkillService;

let service = AgentSkillService::init("./data/agent_skill")?;
let runtime = service.runtime();
```

### 5.2 提交任務

```rust
use agent_skill::{SkillTask, TaskStatus};
use chrono::Utc;

let task = SkillTask {
    task_id: "daily-evo-20261114-001".to_string(),
    task_type: "daily_evolution".to_string(),
    agent_name: "Honey2".to_string(),
    description: "生成 2026-11-14 進化提案".to_string(),
    status: TaskStatus::Pending,
    input_json: r#"{"date": "20261114"}"#.to_string(),
    output_json: None,
    created_at: Utc::now().timestamp(),
    updated_at: Utc::now().timestamp(),
};

runtime.submit_task(task).await?;
```

### 5.3 監聽結果

```rust
let mut rx = runtime.subscribe_results();
while let Some(result) = rx.recv().await {
    println!("任務 {} 完成: success={}", result.task_id, result.success);
}
```

### 5.4 CLI 使用

```bash
# 初始化並啟動橋接器
cargo run --bin agent-skill -- --init ./data/agent_skill --monitor

# 提交任務
cargo run --bin agent-skill -- --submit '{"task_id":"...", "task_type":"..."}'
```

## 6. 測試驗收

| 測試 | 指令 | 預期結果 |
|------|------|----------|
| 單元測試 - Nostr Bridge | `cargo test -p agent-skill nostr_bridge` | 所有測試通過 |
| 單元測試 - Task Store | `cargo test -p agent-skill task_store` | 所有測試通過 |
| 單元測試 - Runtime | `cargo test -p agent-skill runtime` | 所有測試通過 |
| 整合編譯 | `cargo check -p agent-skill` | 無錯誤，僅警告 |
| 依賴編譯 | `cargo check -p buzz-agent` | 成功 (agent-skill 為依賴) |

## 7. 部署與運維

### 7.1 環境需求

- Rust 1.70+
- SQLite 3 (內嵌於 `sqlx`/`rusqlite`)
- Nostr Relay 連線 (需配置 `BUZZ_RELAY_URL`)

### 7.2 配置檔範例

```toml
# config/agent_skill.toml
[nostr]
relay_url = "wss://relay.example.com"
private_key = "nsec1..."  # 從環境變數讀取

[storage]
path = "./data/agent_skill"
max_concurrent_tasks = 10

[monitor]
check_interval_secs = 60
alert_on_failure = true
```

### 7.3 監控指標

- `tasks_submitted_total` - 累計提交任務數
- `tasks_completed_total` - 累計完成任務數
- `tasks_failed_total` - 累計失敗任務數
- `bridge_connected` - Nostr 橋接器連線狀態 (bool)
- `queue_depth` - 待執行任務佇列長度

## 8. 風險與對策

| 風險 | 可能性 | 影響 | 緩解措施 |
|------|--------|------|----------|
| Nostr Relay 斷線 | 中 | 高 | 實作重連邏輯、本機佇列緩衝 |
| SQLite 鎖爭用 | 低 | 中 | 使用 WAL 模式、連線池 |
| 任務冪等鍵衝突 | 低 | 高 | 使用 UUID v7 (含時間戳) 生成 task_id |
| 執行器恐慌 | 低 | 高 | `catch_unwind` 包裝、自動重試機制 |

## 9. 版本歷程

| 版本 | 日期 | 變更 |
|------|------|------|
| 1.0.0 | 2026-11-14 | 初始版本：完整實作 Nostr Bridge、Task Store、Runtime |

## 10. 簽署與審核

- **企劃**: Honey2
- **審核**: Sentry (架構安全官)
- **實作**: Pollen (核心技術主管)
- **歸檔**: Wax (第二大腦記憶官)

> **Sentry Audit**: PENDING — 待審核
> **主理人授權**: 2026-11-14 同意立項
