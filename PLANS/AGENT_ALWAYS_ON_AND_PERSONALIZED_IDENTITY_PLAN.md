---
title: "AGENT_ALWAYS_ON_AND_PERSONALIZED_IDENTITY_PLAN"
tags: [agent, always-on, personalized-identity, architecture, implementation]
status: draft
created: 2026-11-18
---

# Buzz Agent Always-On Mechanism & Personalized Identity Implementation Plan

## Executive Summary

This plan specifies the implementation of two complementary enhancements to the Buzz agent system:

1. **Always-On Agent Mechanism** - Background event listener with idle timeout, sleep/wake cycles, and resource limits
2. **Personalized Agent Identity** - Extended agent profile schema (Kind:31989) with avatar URL, signature tag, digital business card, and independent memory namespace

Both changes extend existing Nostr event models and CLI surfaces - no architectural rewrite required. All modifications are gated by the existing owner-reviewed audit flow (NIP-OA).

---

## 1. Current Architecture Baseline

### 1.1 Core Crates Involved

| Crate | Purpose | Key Files |
|-------|---------|-----------|
| buzz-agent | Agent runtime, config, daemon | src/agent.rs, src/daemon.rs, src/types.rs |
| buzz-cli | CLI commands, agent management | src/commands/agents.rs, src/agent_management.rs |
| buzz-persona | Persona packs, manifests, merge logic | src/persona.rs |
| buzz-core | Event kinds, observer, Nostr primitives | src/kind.rs, src/observer.rs |

### 1.2 Existing Agent Profile Schema (Kind:31989)

From crates/buzz-core/src/kind.rs:
```rust
pub const KIND_AGENT_PROFILE: Kind = Kind::Custom(31989);
```

From crates/buzz-agent/src/types.rs - AgentProfile struct (to be extended):
```rust
pub struct AgentProfile {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub model: String,
    pub tools: Vec<String>,
    // ... existing fields
}
```

### 1.3 CLI Agent Management Flow

crates/buzz-cli/src/agent_management.rs:
- CreateAgentDraft / UpdateAgentDraft - owner-reviewed draft structs
- build_create() / build_update() - construct Nostr events with NIP-OA auth tags
- buzz agents create|update - CLI entry points in crates/buzz-cli/src/commands/agents.rs

### 1.4 Daemon Architecture

crates/buzz-agent/src/daemon.rs - Daemon struct manages background execution:
- Currently runs agent tasks on-demand
- No persistent event listener loop
- No idle timeout or sleep/wake state machine

---

## 2. Data Model Extensions

### 2.1 Extended AgentProfile (Kind:31989)

**File:** crates/buzz-agent/src/types.rs

Add new fields to AgentProfile:

```rust
/// Extended agent profile with personalized identity fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentProfile {
    // Existing fields
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub model: String,
    pub tools: Vec<String>,
    // ... existing fields

    // NEW: Personalized identity fields
    /// Avatar image URL (HTTPS or data: URI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Signature tagline - displayed in mentions and agent cards
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_tag: Option<String>,

    /// Digital business card - structured contact/role info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_card: Option<BusinessCard>,

    /// Independent memory namespace identifier
    /// Format: "agent:<pubkey_hex>" or custom namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_namespace: Option<String>,

    /// Always-on configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_on: Option<AlwaysOnConfig>,
}

/// Digital business card for agent identity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusinessCard {
    /// Display role/title (e.g., "Senior Planning Officer")
    pub role: String,
    /// Organization or team affiliation
    pub organization: Option<String>,
    /// Contact hint for human collaborators
    pub contact_hint: Option<String>,
    /// Custom key-value metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// Always-on daemon configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlwaysOnConfig {
    /// Enable background listener (default: false)
    pub enabled: bool,
    /// Idle timeout in seconds before entering sleep (default: 300 = 5 min)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Maximum concurrent background tasks (default: 3)
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_tasks: usize,
    /// Resource limit: max memory MB for daemon (default: 512)
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64,
    /// Channels to monitor (empty = all assigned channels)
    #[serde(default)]
    pub monitored_channels: Vec<String>,
}

fn default_idle_timeout() -> u64 { 300 }
fn default_max_concurrent() -> usize { 3 }
fn default_max_memory_mb() -> u64 { 512 }
```

### 2.2 Nostr Tag Extensions for Kind:31989

New standardized tags (append to existing profile event):

| Tag | Purpose | Example |
|-----|---------|---------|
| avatar | Avatar image URL | ["avatar", "https://cdn.example.com/avatar.png"] |
| signature | Signature tagline | ["signature", "Planning with precision"] |
| business-card | JSON-encoded business card | ["business-card", "{\"role\":\"...\"}"] |
| memory-ns | Memory namespace | ["memory-ns", "agent:abc123..."] |
| always-on | JSON-encoded AlwaysOnConfig | ["always-on", "{\"enabled\":true,...}"] |

**Implementation:** Extend build_create() and build_update() in crates/buzz-cli/src/agent_management.rs to serialize new fields as tags.

### 2.3 AgentConfig Extension (Runtime)

**File:** crates/buzz-agent/src/types.rs - AgentConfig struct

Add runtime configuration fields:

```rust
pub struct AgentConfig {
    // Existing fields...
    
    // NEW: Always-on runtime config
    pub always_on_enabled: bool,
    pub idle_timeout_secs: u64,
    pub max_concurrent_tasks: usize,
    pub max_memory_mb: u64,
    pub monitored_channels: Vec<String>,
    pub memory_namespace: Option<String>,
}
```

---

## 3. Daemon Changes - Always-On Listener Loop

### 3.1 State Machine Design

**File:** crates/buzz-agent/src/daemon.rs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    /// Initializing, loading config, connecting to relay
    Starting,
    /// Actively listening for events, processing mentions
    Active,
    /// Idle timeout reached, sleeping but keeping connection alive
    Sleeping,
    /// Waking up from sleep, re-establishing subscriptions
    Waking,
    /// Graceful shutdown in progress
    Stopping,
    /// Terminal error state
    Failed(String),
}
```

### 3.2 Background Event Listener

New BackgroundListener struct in daemon.rs:

```rust
pub struct BackgroundListener {
    client: BuzzClient,
    config: AlwaysOnConfig,
    state: Arc<RwLock<DaemonState>>,
    last_activity: Arc<RwLock<Instant>>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    task_semaphore: Arc<Semaphore>,
    memory_tracker: Arc<MemoryTracker>,
}

impl BackgroundListener {
    pub async fn run(&mut self) -> Result<(), DaemonError> {
        *self.state.write().await = DaemonState::Active;
        
        // Subscribe to monitored channels + mentions
        let filter = self.build_subscription_filter();
        let mut events = self.client.subscribe(filter).await?;
        
        loop {
            tokio::select! {
                // 1. Incoming events from relay
                event = events.next() => {
                    if let Some(event) = event {
                        self.handle_event(event).await;
                        *self.last_activity.write().await = Instant::now();
                    }
                }
                
                // 2. Idle timeout check
                _ = self.idle_timeout_ticker() => {
                    if self.should_sleep() {
                        self.enter_sleep().await;
                    }
                }

                // 3. Shutdown signal
                _ = self.shutdown_tx.subscribe().recv() => {
                    *self.state.write().await = DaemonState::Stopping;
                    break;
                }
            }
        }

        *self.state.write().await = DaemonState::Stopping;
        self.cleanup().await;
        Ok(())
    }

    async fn handle_event(&self, event: Event) {
        // Filter: only process events matching our agent's monitored channels
        if !self.is_relevant_event(&event) {
            return;
        }

        // Acquire semaphore to enforce resource limits
        let _permit = match self.task_semaphore.try_acquire() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("Agent at capacity, dropping event: {}", event.id);
                return;
            }
        };

        // Check memory before processing
        if self.memory_tracker.is_over_limit() {
            tracing::warn!("Memory limit exceeded, shedding load");
            return;
        }

        // Process the event through the agent's handler
        if let Err(e) = self.process_event(event).await {
            tracing::error!("Event processing failed: {}", e);
            *self.state.write().await = DaemonState::Failed;
        }
    }

    fn idle_timeout_ticker(&self) -> impl Future<Output = ()> {
        let interval = self.config.idle_timeout;
        async move {
            tokio::time::sleep(interval).await;
        }
    }

    fn should_sleep(&self) -> bool {
        let last = *self.last_activity.read().await;
        last.elapsed() > self.config.idle_timeout
    }

    async fn enter_sleep(&self) {
        tracing::info!("Agent entering sleep mode");
        *self.state.write().await = DaemonState::Sleeping;
        
        // Unsubscribe from relay to save resources
        self.client.unsubscribe_all().await;
        
        // Wait for wake signal (new mention via push or periodic poll)
        self.wait_for_wake().await;
        
        tracing::info!("Agent waking up");
        *self.state.write().await = DaemonState::Waking;
        self.resubscribe().await;
        *self.state.write().await = DaemonState::Active;
    }

    async fn wait_for_wake(&self) {
        // Lightweight polling for mentions while sleeping
        let mut interval = tokio::time::interval(self.config.sleep_poll_interval);
        loop {
            interval.tick().await;
            if self.check_for_mentions().await {
                return;
            }
        }
    }

    async fn check_for_mentions(&self) -> bool {
        // Quick filter query for mentions of this agent's pubkey
        let filter = Filter::new()
            .kind(Kind::TextNote)
            .pubkey(self.config.agent_pubkey)
            .since(Timestamp::now() - Duration::from_secs(30))
            .limit(1);
        
        match self.client.query(filter).await {
            Ok(events) => !events.is_empty(),
            Err(_) => false,
        }
    }

    async fn resubscribe(&self) {
        let filter = self.build_subscription_filter();
        self.client.subscribe(filter).await.ok();
    }

    fn build_subscription_filter(&self) -> Filter {
        Filter::new()
            .kinds(vec![Kind::TextNote, Kind::ChatMessage])
            .pubkey(self.config.agent_pubkey)
    }

    fn is_relevant_event(&self, event: &Event) -> bool {
        // Check if event mentions this agent or is in monitored channels
        event.tags.iter().any(|tag| {
            tag.kind() == TagKind::Pubkey && tag.content() == self.config.agent_pubkey.to_string()
        }) || self.config.monitored_channels.contains(&event.channel_id())
    }

    async fn process_event(&self, event: Event) -> Result<(), DaemonError> {
        // Delegate to agent's message handler
        self.config.message_handler.handle(event).await
    }

    async fn cleanup(&self) {
        self.client.unsubscribe_all().await;
        self.task_semaphore.close();
    }
}
```

### 3.3 CLI Command Extensions

#### 3.3.1 `buzz agents create` — New Flags

```bash
buzz agents create \
  --display-name "My Agent" \
  --system-prompt "You are..." \
  --avatar-url "https://example.com/avatar.png" \
  --signature-tag "🤖 Always ready to help" \
  --business-card '{"role": "Research Assistant", "specialties": ["Rust", "Nostr"]}' \
  --memory-namespace "agent_myagent" \
  --always-on \
  --idle-timeout 300 \
  --sleep-poll-interval 60 \
  --max-concurrent-tasks 3 \
  --memory-limit-mb 256 \
  --channel <CHANNEL_ID>
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--avatar-url` | string | — | HTTPS URL to agent avatar image |
| `--signature-tag` | string | — | Short tagline shown in agent card |
| `--business-card` | JSON string | — | Structured digital business card |
| `--memory-namespace` | string | `agent_<slug>` | Independent memory namespace key |
| `--always-on` | bool | false | Enable background listener daemon |
| `--idle-timeout` | seconds | 300 | Seconds of inactivity before sleep |
| `--sleep-poll-interval` | seconds | 60 | Poll interval while sleeping |
| `--max-concurrent-tasks` | int | 3 | Semaphore limit for concurrent events |
| `--memory-limit-mb` | int | 256 | Memory budget for agent process |

#### 3.3.2 `buzz agents update` — New Flags

Same flags as create (all optional for partial updates), plus:
- `--disable-always-on` — Turn off background listener
- `--reset-memory-namespace` — Reassign to new namespace

#### 3.3.3 `buzz agents status` — New Output Fields

```json
{
  "agent_pubkey": "hex...",
  "display_name": "My Agent",
  "daemon_state": "Active|Sleeping|Waking|Stopping|Failed",
  "always_on_enabled": true,
  "last_activity": "2026-11-18T01:22:00Z",
  "idle_timeout_secs": 300,
  "concurrent_tasks": 1,
  "max_concurrent_tasks": 3,
  "memory_usage_mb": 42,
  "memory_limit_mb": 256,
  "avatar_url": "https://...",
  "signature_tag": "🤖 Always ready",
  "business_card": { ... },
  "memory_namespace": "agent_myagent"
}
```

### 3.4 Persona Pack Integration

#### 3.4.1 Extended Manifest Schema (`buzz-persona`)

```toml
# In persona pack manifest.toml
[agent]
always_on = true
idle_timeout_secs = 300
sleep_poll_interval_secs = 60
max_concurrent_tasks = 3
memory_limit_mb = 256

[agent.identity]
avatar_url = "https://example.com/avatars/researcher.png"
signature_tag = "🔬 Deep research specialist"
business_card = { role = "Research Analyst", specialties = ["Rust", "Nostr", "Cryptography"] }
memory_namespace = "agent_researcher_v2"
```

#### 3.4.2 Resolution Priority

1. **CLI flags** (highest) — explicit override
2. **Persona pack manifest** — bundled defaults
3. **System defaults** — hardcoded fallbacks

```rust
// In agent_management.rs CreateAgentDraft::resolve()
fn resolve_always_on_config(
    cli: &CliArgs,
    persona: Option<&PersonaManifest>,
) -> AlwaysOnConfig {
    AlwaysOnConfig {
        enabled: cli.always_on
            .or(persona.and_then(|p| p.agent.always_on))
            .unwrap_or(false),
        idle_timeout: Duration::from_secs(
            cli.idle_timeout
                .or(persona.and_then(|p| p.agent.idle_timeout_secs))
                .unwrap_or(300)
        ),
        // ... other fields
    }
}
```

### 3.5 Security & Audit Gating

All agent profile mutations (create/update) remain gated by the existing **owner-reviewed flow**:

1. User runs `buzz agents create --always-on ...`
2. CLI builds unsigned `AgentDraft` event (Kind: 30621 or custom)
3. Event signed with user's NIP-OA authorization tag
4. Sent to relay → appears in owner's Buzz Desktop as "Pending Review"
5. Owner approves → `AgentProfile` (Kind: 31989) published
6. Daemon picks up new profile on next restart or via hot-reload signal

**No new audit surface** — reuses existing `buzz agents draft-create` / `draft-update` machinery.

---

## 4. Phased Rollout Milestones

### Phase 1: MVP (Week 1-2) — Core Always-On + Identity Fields
- [ ] Extend `AgentConfig` / `AgentProfile` with identity fields (avatar, signature_tag, business_card, memory_namespace)
- [ ] Extend `AgentDraft` builder in `agent_management.rs`
- [ ] Add CLI flags to `agents create` / `update` / `status`
- [ ] Implement `BackgroundListener` with basic sleep/wake cycle
- [ ] Integrate `Daemon::run_always_on()` entry point
- [ ] Unit tests for state machine transitions

### Phase 2: Persona Integration & Polish (Week 3)
- [ ] Add persona manifest fields (`[agent]`, `[agent.identity]`)
- [ ] Resolution logic in `CreateAgentDraft::resolve()`
- [ ] Hot-reload support for config changes without daemon restart
- [ ] Memory namespace isolation (prefix all `buzz mem` keys with namespace)
- [ ] Integration tests with simulated relay

### Phase 3: Observability & Hardening (Week 4)
- [ ] Structured logging for state transitions (JSON lines)
- [ ] Metrics export (Prometheus-compatible): `agent_state`, `events_processed`, `memory_usage`
- [ ] Graceful degradation: auto-disable always-on on repeated failures
- [ ] Documentation: `GUIDES/AGENT_ALWAYS_ON.md`, `GUIDES/AGENT_PERSONALIZED_IDENTITY.md`

---

## 5. Audit Checklist (for @Sentry Review)

| Area | Check | Status |
|------|-------|--------|
| **Data Model** | AgentProfile (Kind:31989) extends without breaking existing deserialization | ☐ |
| **Data Model** | New fields are optional with sensible defaults | ☐ |
| **Daemon** | State machine covers all transitions (Active↔Sleeping↔Waking→Stopping/Failed) | ☐ |
| **Daemon** | Idle timeout uses monotonic clock (`Instant`) | ☐ |
| **Daemon** | Semaphore prevents unbounded task spawn | ☐ |
| **Daemon** | Memory tracker enforces limit before event processing | ☐ |
| **Daemon** | Sleep mode unsubscribes from relay (resource release) | ☐ |
| **Daemon** | Wake detection via lightweight mention poll | ☐ |
| **CLI** | All new flags documented in `--help` | ☐ |
| **CLI** | `status` command shows daemon_state and resource usage | ☐ |
| **Persona** | Manifest schema validated on load | ☐ |
| **Persona** | Resolution priority: CLI > Persona > Defaults | ☐ |
| **Security** | Owner-reviewed flow unchanged (NIP-OA auth tag required) | ☐ |
| **Security** | Memory namespace isolation prevents cross-agent leakage | ☐ |
| **Security** | No new privileged capabilities introduced | ☐ |
| **Testing** | Unit tests for DaemonState transitions | ☐ |
| **Testing** | Integration test: sleep → wake on mention | ☐ |
| **Testing** | Load test: max_concurrent_tasks respected | ☐ |

---

## 6. File & Module Impact Summary

| File | Change Type |
|------|-------------|
| `crates/buzz-agent/src/types.rs` | Add fields to `AgentConfig`, `AgentProfile` |
| `crates/buzz-agent/src/daemon.rs` | Add `BackgroundListener`, extend `DaemonState`, `Daemon::run_always_on()` |
| `crates/buzz-cli/src/agent_management.rs` | Extend `CreateAgentDraft`, `UpdateAgentDraft` builders |
| `crates/buzz-cli/src/commands/agents.rs` | Add CLI flags, extend `status` output |
| `crates/buzz-persona/src/persona.rs` | Extend `PersonaManifest` with `[agent]` section |
| `crates/buzz-core/src/kind.rs` | No change (reuse Kind:31989) |

---

## 7. Rollback Procedure

If critical issues arise post-deployment:

```bash
# 1. Disable always-on for affected agents
buzz agents update --pubkey <AGENT_PUBKEY> --disable-always-on --channel <CHANNEL_ID>

# 2. Restart daemon without always-on flag
cargo run --bin buzz-daemon -- --no-always-on

# 3. If schema migration needed, revert AgentProfile events:
#    Publish new AgentProfile (Kind:31989) with old field subset
#    (Nostr is append-only; clients ignore unknown fields)
```

---

## 8. Open Questions for Review

1. **Push vs Poll for Wake**: Current design uses polling while sleeping. Should we add NIP-65 relay list + WebSocket push for instant wake? (Adds dependency, increases complexity)
2. **Memory Namespace Persistence**: Should namespace be tied to agent pubkey (immutable) or configurable? Current: configurable via CLI/persona.
3. **Multi-Relay Subscription**: `BackgroundListener` currently uses single `BuzzClient`. Should it subscribe across all relays in agent's relay list?
4. **Business Card Schema**: Free-form JSON vs strict schema? Current: free-form with documented conventions.

---

*Plan authored by Honey2. Ready for @Sentry architectural review.*
                 

---

## 8. 強制失敗模式推演 (Forced Failure Mode Analysis)

本節系統性推演 Always-On Daemon 在極端條件下的失效行為、檢測機制與恢復策略，確保設計具備**失效安全**與**可觀測降級**能力。

### 8.1 失效模式分類矩陣

| 失效類別 | 觸發條件 | 檢測機制 | 系統行為 | 恢復策略 | RTO/RPO 目標 |
|---------|---------|---------|---------|---------|-------------|
| **網路分區** | Relay 連線中斷、DNS 解析失敗、TLS 握手失敗 | `BuzzClient` 連線狀態機 + 心跳超時 (30s) | 進入 `Sleeping`，本地緩衝事件，定期重連 | 指數退避重連 (1s, 2s, 4s, 8s, max 60s) | RTO < 60s, RPO = 0 (事件不丟失) |
| **Relay 回傳錯誤** | 4xx/5xx、rate limit、封禁 | HTTP 狀態碼分類 + `NostrError::RateLimited` | 暫停發送，進入 `Backoff` 子狀態，保留訂閱 | 依 `Retry-After` 或預設 60s 後重試 | RTO < 120s |
| **記憶體壓力** | RSS > `memory_limit_mb` × 0.9 | `MemoryTracker` 週期採樣 (5s) | 拒絕新事件 (`should_shed_load()`)，觸發 GC 壓縮 | 釋放非必要緩存，若持續 > 30s 則 `Failed` | RTO < 30s (自愈) |
| **併發耗盡** | `task_semaphore` 連續 10 次 `try_acquire` 失敗 | 信號量可用許可數監控 | 事件入本地隊列 (上限 100)，超限丟棄並記錄 `dropped_events` | 任務完成釋放許可後自動消費隊列 | RPO = 0 (隊列內事件保留) |
| **狀態機卡死** | 狀態未在預期時間內轉換 (如 `Waking` > 10s) | 看門狗定時器 (`watchdog_ticker`) | 強制轉入 `Failed`，發射 `DaemonStateChanged` 事件 | 外部監督進程重啟 Daemon | RTO < 15s |
| **配置熱更新衝突** | 運行中 `AgentProfile` 變更 (如 `idle_timeout` 從 300→30) | `config_version` 原子比對 | 套用新配置於下一個狀態週期，舊邏輯完成當前迴道 | 無需重啟，平滑切換 | RTO = 0 |
| **記憶體命名空間衝突** | 兩 Agent 共用同一 `memory_namespace` | 啟動時掃描現有命名空間註冊表 | 拒絕啟動，回報 `NamespaceCollision` 錯誤 | 人工介入：修改其中一 Agent 的 namespace | RTO = 人工 |
| **惡意/異常事件洪流** | 單位時間收到 > 1000 條相關事件 | 速率限制器 (token bucket, 100/min) | 進入 `Throttled` 子狀態，只處理高優先級 (mention) | 速率恢復正常後自動退出 | RTO < 60s |
| **系統資源耗盡** | FD 耗盡、CPU 100% 持續 > 60s | `systemstat` 採樣 + `ulimit` 檢查 | 主動降級：禁用 always-on，轉為輪詢模式 | 資源釋放後可手動重啟 always-on | RTO = 人工 |

### 8.2 關鍵失效路徑詳細推演

#### 8.2.1 路徑 A：Relay 連線中斷 → 自動重連 → 事件追趕
```
Active --(連線斷開)--> Sleeping (unsubscribed)
  │
  ├─ 本地緩衝: 將本應發送的事件寫入 `pending_outbox` (SQLite/檔案)
  ├─ 重連迴圈: 指數退避 + 抖動 (max 60s)
  ├─ 重連成功: resubscribe() → Active
  └─ 事件追趕: 
      1. 讀取 `pending_outbox` 依序重發
      2. 發送 `REQ` 獲取斷線期間的事件 (since = last_seen_timestamp)
      3. 重放處理邏輯 (冪等: 以 event_id 去重)
```
**驗收標準**：斷線 5 分鐘內，事件零丟失、零重複處理。

#### 8.2.2 路徑 B：記憶體洩漏 → 守護觸發 → 優雅降級
```
Active --(RSS 持續增長)--> MemoryTracker.is_over_limit() = true
  │
  ├─ 階段 1 (90%): 拒絕新事件, 記錄 `memory_pressure` 指標
  ├─ 階段 2 (95%): 強制釋放非必要緩存 (event cache, persona cache)
  ├─ 階段 3 (98%): 進入 `Failed`, 發射告警事件 (Kind:31990)
  └─ 階段 4 (100%): 被 OS OOM Killer 殺死 → 外部監督進程拉起
```
**驗收標準**：單次事件處理記憶體增長 < 5MB；持續運行 24h RSS 增長 < 50MB。

#### 8.2.3 路徑 C：狀態機異常轉換 → 看門狗介入
```
Waking --(resubscribe 卡住 > 10s)--> watchdog 觸發
  │
  ├─ 記錄堆棧追蹤到 `daemon_crash_<timestamp>.log`
  ├─ 發射 `DaemonStateChanged { from: Waking, to: Failed, reason: "watchdog_timeout" }`
  ├─ 執行 `cleanup()`: 釋放信號量、關閉連線、刷新日誌
  └─ 退出進程碼 3 → systemd/supervisor 重啟
```
**驗收標準**：看門狗超時 ≤ 15s；崩潰日誌含完整調用棧。

#### 8.2.4 路徑 D：配置衝突 → 雙重寫入保護
```
用戶執行 `buzz agents update --idle-timeout 30` (Agent 正在 Running)
  │
  ├─ CLI 發布新 AgentProfile (Kind:31989, version=N+1)
  ├─ Daemon 收到事件 → `config_version` 比對發現變更
  ├─ 暫停新事件處理 (drain 當前迴道)
  ├─ 熱替換 `AlwaysOnConfig` (atomic swap Arc<RwLock<>>)
  ├─ 依新配置重算 `idle_timeout_ticker` 下次觸發時間
  └─ 恢復事件處理
```
**驗收標準**：配置生效延遲 < 1 個事件迴道 (通常 < 100ms)；無事件丟失。

### 8.3 混沌工程驗證清單 (Pre-Release 必跑)

| 測試場景 | 注入手段 | 成功判準 | 自動化腳本 |
|---------|---------|---------|-----------|
| Relay 斷線 30s | `tc qdisc add dev lo netem loss 100%` | 事件零丟失、自動重連、追趕完成 | `tests/chaos_relay_partition.rs` |
| Relay 回傳 429 | Mock server 回傳 429 + `Retry-After: 5` | 正確退避 5s、不丟事件 | `tests/chaos_rate_limit.rs` |
| 記憶體限制 50MB | `cgroup memory.max=50M` | 進入降級模式、不被 OOM Kill | `tests/chaos_memory_pressure.rs` |
| 併發衝擊 500 並發 | `tokio::spawn` 500 tasks 同時發送事件 | 信號量限制生效、隊列緩衝、無 panic | `tests/chaos_concurrency_storm.rs` |
| 配置熱更新 | 運行中發布新 Profile | 無事件丟失、新配置生效 | `tests/chaos_config_hot_reload.rs` |
| 命名空間衝突 | 啟動兩 Agent 同 namespace | 第二個啟動失敗、錯誤碼明確 | `tests/chaos_namespace_collision.rs` |
| 看門狗觸發 | 在 `resubscribe` 注入 `sleep(20s)` | 15s 內進入 Failed、產出崩潰日誌 | `tests/chaos_watchdog.rs` |

### 8.4 可觀測性指標矩陣 (Prometheus Exporter)

| 指標名稱 | 類型 | 標籤 | 說明 | 告警閾值 |
|---------|------|------|------|---------|
| `buzz_agent_state` | Gauge | agent_pubkey, state | 當前狀態 (1=Active, 2=Sleeping, 3=Waking, 4=Stopping, 5=Failed) | state=5 持續 > 1m |
| `buzz_agent_events_processed_total` | Counter | agent_pubkey, outcome | 累計處理事件數 (success/dropped/error) | dropped rate > 1% |
| `buzz_agent_memory_bytes` | Gauge | agent_pubkey | 當前 RSS (bytes) | > limit × 0.9 |
| `buzz_agent_concurrent_tasks` | Gauge | agent_pubkey | 正在處理的任務數 | = max_concurrent_tasks 持續 > 5m |
| `buzz_agent_idle_duration_seconds` | Gauge | agent_pubkey | 當前閒置時長 | > idle_timeout × 2 (可能卡死) |
| `buzz_agent_reconnect_attempts_total` | Counter | agent_pubkey | 重連嘗試次數 | > 10/min (網路異常) |
| `buzz_agent_config_version` | Gauge | agent_pubkey | 當前配置版本號 | 異常跳變 |
| `buzz_agent_dropped_events_total` | Counter | agent_pubkey, reason | 丟棄事件計數 (capacity/memory/rate_limit) | 任何增長 |

### 8.5 失效模式測試報告模板 (交付物)

每個測試場景執行後需產出：
```markdown
# 測試報告: <場景名稱>
- **執行時間**: 2026-11-XX HH:MM:SS
- **環境**: Rust 1.XX, tokio 1.XX, Linux kernel X.XX
- **注入參數**: <具體參數>
- **觀測結果**: <指標曲線截圖/日誌關鍵段>
- **判定**: PASS / FAIL
- **殘留風險**: <若有>
- **簽署**: @Honey2 / @Sentry
```

---

## 9. Open Questions for Review

1. **Push vs Poll for Wake**: Current design uses polling while sleeping. Should we add NIP-65 relay list + WebSocket push for instant wake? (Adds dependency, increases complexity)
2. **Memory Namespace Persistence**: Should namespace be tied to agent pubkey (immutable) or configurable? Current: configurable via CLI/persona.
3. **Multi-Relay Subscription**: `BackgroundListener` currently uses single `BuzzClient`. Should it subscribe across all relays in agent's relay list?
4. **Business Card Schema**: Free-form JSON vs strict schema? Current: free-form with documented conventions.

---

*Plan authored by Honey2. Ready for @Sentry architectural review.*
