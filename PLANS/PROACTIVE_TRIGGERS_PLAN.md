---
title: "Proactive Autonomous Triggers 實作計畫書"
tags: [proactive-triggers, buzz, background-tasks, scheduling, event-driven]
status: draft
created: 2026-11-15
---

# Proactive Autonomous Triggers 實作計畫書

> **分工定位**：由 **Buzz Agent 團隊** 自主落實，屬於業務邏輯與 Agent 行為迴圈範疇，在現有框架內直接落地。

---

## 1. 背景與動機

參考 Cue.im 的「全天候主動背景事件觸發與主動關懷」能力，Buzz 需要讓 Agent 從「被動響應」升級為「主動關懷」：
- 定期巡檢專案依賴更新、GitHub Issue/PR 狀態
- 監控特定檔案/目錄變更並自動觸發處理
- 基於時間排程的例行性任務（每日報告、每周總結）
- 事件驅動的即時響應（Nostr 事件、Webhook、檔系統事件）

**核心原則**：
- ✅ **本地端** 輕量級背景服務，不依賴外部雲端
- ✅ 復用現有 **Nostr 節點** 與 **Task 系統** (agent-skill)
- ✅ 配置化、可觀測、可熱重載

---

## 2. 架構設計

### 2.1 整體分層

```
┌─────────────────────────────────────────────────────────────┐
│                    Buzz Agent 業務層                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │ Dependency      │  │ GitHub Monitor  │  │ File Watch  │  │
│  │ Checker Skill   │  │ Skill           │  │ Skill       │  │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘  │
└───────────┼────────────────────┼───────────────────┼─────────┘
            ▼                    ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│              Proactive Trigger Engine (核心)                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Scheduler (Cron + Interval)                          │  │
│  │  ├─ CronExpression Parser (quartz 相容)               │  │
│  │  ├─ Interval Timer (精確到秒)                          │  │
│  │  └─ Jitter/Randomization (避免雷群效應)                │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Event Bus (Nostr + Local)                            │  │
│  │  ├─ Nostr Subscription Manager (Kind 過濾)            │  │
│  │  ├─ File System Watcher (notify crate)                │  │
│  │  ├─ Webhook Receiver (可選, 本地 HTTP)                │  │
│  │  └─ Event Router → Trigger Matcher                    │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Trigger Registry & Matcher                           │  │
│  │  ├─ Condition Evaluator (CEL/JsonLogic)               │  │
│  │  ├─ Deduplication (幂等鍵 + 時間窗口)                  │  │
│  │  └─ Action Dispatcher → agent-skill Task Store        │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
            ▲                    ▲                   ▲
            │                    │                   │
┌───────────┴────────────────────┴───────────────────┴─────────┐
│              現有基礎設施 (複用)                               │
│  - agent-skill Task Store (SQLite + Nostr Bridge)             │
│  - Buzz Nostr Client (Relay 連線管理)                          │
│  - Agent Runtime (工具調用、上下文管理)                        │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心資料模型

```rust
// 依據 skills/agent-skill/src/task_store.rs:15 參考 SkillTask 設計
// 依據 skills/antigravity/src/types.rs:45 參考 TriggerConfig 設計

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub trigger_type: TriggerType,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub deduplication: DeduplicationConfig,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub fire_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerType {
    /// 定時觸發
    Schedule {
        cron: Option<String>,      // "0 9 * * 1-5" (每工作天 09:00)
        interval_secs: Option<u64>, // 3600 (每小時)
        timezone: String,           // "Asia/Taipei"
        jitter_secs: u64,           // 0-300 秒隨機抖動
    },
    /// Nostr 事件觸發
    NostrEvent {
        kinds: Vec<u16>,            // [1, 30300, 30023]
        authors: Option<Vec<String>>, // pubkey hex
        tags: Option<Vec<TagFilter>>, // [["p", "..."], ["#t", "buzz"]]
        relay_urls: Option<Vec<String>>,
    },
    /// 檔案系統觸發
    FileSystem {
        paths: Vec<PathBuf>,
        events: Vec<FsEventType>,   // Create, Modify, Delete, Rename
        recursive: bool,
        debounce_ms: u64,           // 500ms 去抖
    },
    /// Webhook 觸發 (本地 HTTP endpoint)
    Webhook {
        path: String,               // "/webhook/github"
        secret: Option<String>,     // HMAC 驗證
        allowed_ips: Vec<IpAddr>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,              // "event.content", "task.status"
    pub operator: ConditionOp,      // Eq, Ne, Gt, Lt, Contains, Regex, CEL
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_type: ActionType,
    pub config: Value,              // 動作參數
    pub retry_policy: RetryPolicy,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionType {
    /// 提交任務到 agent-skill
    SubmitTask {
        task_type: String,
        agent_name: String,
        description: String,
        input_template: Value,      // 支援模板變數 {{event.id}}, {{trigger.name}}
    },
    /// 發送 Nostr 事件
    SendNostrEvent {
        kind: u16,
        content_template: String,
        tags_template: Vec<Value>,
    },
    /// 執行本地命令
    RunCommand {
        command: String,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
    },
    /// 發送通知 (DM/頻道)
    SendNotification {
        channel_id: Option<String>,
        recipients: Vec<String>,    // pubkey 或 npub
        template: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    pub key_template: String,       // "{{trigger.id}}-{{event.id}}"
    pub window_secs: u64,           // 3600 秒內不重複觸發
    pub max_executions_per_window: u32, // 1
}
```

### 2.3 配置檔案結構 (`config/triggers.toml`)

```toml
[engine]
check_interval_secs = 30          # 掃描間隔
max_concurrent_triggers = 10      # 並發觸發上限
event_buffer_size = 1000          # 事件緩衝區大小
enable_metrics = true

[[triggers]]
id = "daily-dependency-check"
name = "每日依賴更新檢查"
enabled = true
trigger_type = { Schedule = { cron = "0 8 * * 1-5", timezone = "Asia/Taipei", jitter_secs = 300 } }
conditions = []
actions = [
  { type = "SubmitTask", config = { task_type = "dependency_check", agent_name = "Honey2", description = "檢查專案依賴更新", input_template = {} }, retry_policy = { max_retries = 3, base_delay_secs = 60 }, timeout_secs = 600 }
]
deduplication = { key_template = "{{trigger.id}}-{{date}}", window_secs = 86400, max_executions_per_window = 1 }

[[triggers]]
id = "github-issue-monitor"
name = "GitHub Issue 監控"
enabled = true
trigger_type = { NostrEvent = { kinds = [30300], tags = [["#t", "github"], ["#t", "issue"]], relay_urls = ["wss://relay.example.com"] } }
conditions = [
  { field = "event.content", operator = "Contains", value = "urgent" },
  { field = "event.tags.#priority", operator = "Eq", value = "high" }
]
actions = [
  { type = "SendNotification", config = { channel_id = "general", recipients = [], template = "🚨 緊急 Issue: {{event.content}}" }, retry_policy = { max_retries = 1, base_delay_secs = 10 }, timeout_secs = 30 }
]
deduplication = { key_template = "{{trigger.id}}-{{event.id}}", window_secs = 3600, max_executions_per_window = 1 }

[[triggers]]
id = "config-file-watcher"
name = "配置檔變更監控"
enabled = true
trigger_type = { FileSystem = { paths = ["config/", ".buzz/config/"], events = ["Modify", "Create"], recursive = true, debounce_ms = 1000 } }
conditions = []
actions = [
  { type = "RunCommand", config = { command = "buzz", args = ["config", "validate"], working_dir = "." }, retry_policy = { max_retries = 0, base_delay_secs = 0 }, timeout_secs = 30 }
]
deduplication = { key_template = "{{trigger.id}}-{{file.path}}", window_secs = 60, max_executions_per_window = 1 }
```

---

## 3. 實作階段

### Phase 1: 核心引擎 (Week 1)
