---
title: "Phase 3 Mode B Agent Daemon 完整實作計畫 (DM 通知模式)"
tags: [phase3, reminders, agent-daemon, mode-b, dm-notification, nip-er]
status: active
created: 2026-10-31
---

# Phase 3：Mode B Agent 代理提醒完整實作計畫

## 核心結論與需求變更

**老闆授權啟動 Phase 3**，核心需求變更：
> 「到期推播機制規範為 Direct Message：當時間到達時，一律由 Fizz 透過私聊私訊 (Direct Message) 推送提醒給我，不要發在公開頻道。」

**架構影響**：
- 原計畫 `send_channel_message` (kind:1 + `#c` tag + `@mention`) → **改為 `send_dm_message` (kind:4, 加密給目標用戶)**
- 目標用戶 pubkey 從 reminder payload 的 `target.authorPubkey` 或委託時記錄取得
- Fizz (Agent) 使用自身金鑰簽署 kind:4 事件，NIP-44 加密給目標用戶

---

## 現狀盤點 (2026-10-31)

### 已存在檔案 (部分實作)
| 檔案 | 狀態 | 備註 |
|------|------|------|
| `crates/buzz-agent/src/daemon.rs` | ✅ 存在 | 生命週期管理、啟動/停止/重啟、分散式鎖整合 |
| `crates/buzz-agent/src/daemon_ws.rs` | ✅ 存在 | WebSocket 客戶端、自動重連、NIP-42 認證、訂閱 |
| `crates/buzz-agent/src/daemon_sub.rs` | ✅ 存在 | 訂閱管理、kind:30300 過濾器 |
| `crates/buzz-agent/src/daemon_dispatch.rs` | ❌ **缺失** | Due Signal 解析、NIP-44 解密、觸發 DM 發送 |
| `crates/buzz-agent/src/daemon_lock.rs` | ❌ **缺失** | Redis 分散式單例鎖 |

### 待修改檔案
| 檔案 | 任務 |
|------|------|
| `crates/buzz-agent/src/config.rs` | 新增 `agent_keys: Option<Arc<nostr::Keys>>`，從 `BUZZ_AGENT_KEYS` 環境變數載入 |
| `crates/buzz-agent/src/agent.rs` | `RunCtx` 擴充 `agent_keys` 欄位 |
| `crates/buzz-agent/src/builtin.rs` | 新增 3 個內建工具：`create_reminder`、`sign_and_send_event`、`send_dm_message` |
| `crates/buzz-agent/src/lib.rs` | 整合 Daemon 到 App 啟動/關閉鉤子，注入 agent_keys |

---

## 詳細實作規格

### 1. Config 擴充 (`config.rs`)

```rust
// 在 Config 結構體中新增
pub agent_keys: Option<Arc<nostr::Keys>>,
```

載入邏輯（於 `Config::from_env()` 中）：
```rust
// 從 BUZZ_AGENT_KEYS 環境變數載入 (格式: nsec1... 或十六進位私鑰)
let agent_keys = std::env::var("BUZZ_AGENT_KEYS").ok().and_then(|s| {
    Keys::parse(&s).ok().map(Arc::new)
});
```

### 2. RunCtx 擴充 (`agent.rs:143-238`)

```rust
pub struct RunCtx<'a> {
    // ... 既有欄位 ...
    
    /// Agent 自身的 Nostr Keys（含 privkey），僅在 Mode B Daemon 模式下注入
    /// 由 App 啟動時從配置載入，透過 Arc 共享
    /// **安全性**：僅在 Daemon 子系統與內建工具中可見，不暴露給 LLM 提示詞
    pub agent_keys: Option<Arc<nostr::Keys>>,
}
```

### 3. 內建工具 (`builtin.rs`)

#### 3.1 `create_reminder` (供 LLM 調用)
```rust
pub fn create_reminder_def() -> ToolDef {
    ToolDef {
        name: "create_reminder".into(),
        description: "委託後台排程器建立定時提醒事件。到期時 Agent 會透過私訊 (DM) 通知目標用戶。".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "提醒具體內容"},
                "at": {"type": "string", "description": "何時提醒：相對時間 (如 '30m', '2h') 或 ISO8601"},
                "target_user_pubkey": {"type": "string", "description": "接收提醒的用戶 pubkey (hex)"},
                "channel_id": {"type": "string", "description": "關聯頻道 UUID (選填，供上下文參考)"},
                "event_id": {"type": "string", "description": "關聯訊息事件 ID (選填)"},
                "preview": {"type": "string", "description": "關聯訊息預覽文字 (選填)"}
            },
            "required": ["content", "at", "target_user_pubkey"]
        }),
    }
}

async fn execute_create_reminder(
    args: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref()
        .ok_or_else(|| AgentError::InvalidParams("Agent keys not available".into()))?;
    
    // 解析參數
    let content = args["content"].as_str().ok_or(...)?;
    let at = args["at"].as_str().ok_or(...)?;
    let target_user_pubkey = args["target_user_pubkey"].as_str().ok_or(...)?;
    let channel_id = args["channel_id"].as_str();
    let event_id = args["event_id"].as_str();
    let preview = args["preview"].as_str();
    
    // 解析時間
    let not_before = parse_timestamp_or_duration(at)?;
    validate_not_before(not_before)?;
    
    // 驗證 pubkey
    let target_pubkey = PublicKey::from_hex(target_user_pubkey)
        .map_err(|_| AgentError::InvalidParams("invalid target_user_pubkey".into()))?;
    
    // 生成 d-tag (128-bit entropy)
    let d_tag = generate_d_tag();
    
    // 建構提醒內容 JSON (NIP-ER 規範)
    let mut target = json!({});
    if let Some(ch) = channel_id { target["channelId"] = json!(ch); }
    if let Some(eid) = event_id { target["eventId"] = json!(eid); }
    if let Some(p) = preview { target["preview"] = json!(p); }
    target["authorPubkey"] = json!(target_user_pubkey);
    
    let reminder_content = json!({
        "target": target,
        "status": "pending",
        "note": content,
        // Mode B 專用：記錄目標用戶 pubkey 供 DM 發送使用
        "dm_recipient_pubkey": target_user_pubkey,
    });
    
    // NIP-44 自加密 (Agent 自己解密)
    let ciphertext = nip44_encrypt_to_self(keys, &reminder_content.to_string())?;
    
    // 建構事件
    let mut builder = EventBuilder::new(Kind::Custom(30300), ciphertext)
        .tag(Tag::parse(["d", &d_tag])?)
        .tag(Tag::parse(["not_before", &not_before.to_string()])?)
        .tag(Tag::parse(["alt", "Encrypted reminder"])?);
    
    let event = builder.sign_with_keys(keys.as_ref())?;
    
    // 發送到 Relay (復用既有 WebSocket 或 HTTP)
    send_event_via_relay(&event, ctx).await?;
    
    Ok(ToolResult {
        content: vec![ToolResultContent::Text(format!(
            "Reminder created: d-tag={}, triggers at {}", d_tag, not_before
        ))],
        is_error: false,
        provider_id: String::new(),
    })
}
```

#### 3.2 `sign_and_send_event` (Mode B 專用，供 Daemon/工具內部調用)
```rust
pub fn sign_and_send_event_def() -> ToolDef {
    ToolDef {
        name: "sign_and_send_event".into(),
        description: "以 Agent 金鑰簽署並發送 Nostr 事件（Mode B 專用）".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "kind": {"type": "integer", "description": "事件 kind"},
                "content": {"type": "string", "description": "事件內容"},
                "tags": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}}
            },
            "required": ["kind", "content", "tags"]
        }),
    }
}

async fn execute_sign_and_send_event(
    args: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref().ok_or(...)?;
    let kind: u16 = args["kind"].as_u64().ok_or(...)?.try_into()?;
    let content = args["content"].as_str().ok_or(...)?;
    let tags: Vec<Vec<String>> = serde_json::from_value(args["tags"].clone())?;
    
    let mut builder = EventBuilder::new(Kind::Custom(kind), content);
    for tag in tags {
        builder = builder.tag(Tag::parse(tag)?);
    }
    let event = builder.sign_with_keys(keys.as_ref())?;
    
    send_event_via_relay(&event, ctx).await
}
```

#### 3.3 `send_dm_message` (核心：發送 DM 給目標用戶)
```rust
pub fn send_dm_message_def() -> ToolDef {
    ToolDef {
        name: "send_dm_message".into(),
        description: "以 Agent 身份發送加密私訊 (kind:4) 給指定用戶 (Mode B 到期喚醒後調用)".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "recipient_pubkey": {"type": "string", "description": "接收者 pubkey (hex)"},
                "content": {"type": "string", "description": "私訊內容"},
                "reply_to": {"type": "string", "description": "可選：回覆的事件 ID"}
            },
            "required": ["recipient_pubkey", "content"]
        }),
    }
}

async fn execute_send_dm_message(
    args: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref().ok_or(...)?;
    let recipient_pubkey = args["recipient_pubkey"].as_str().ok_or(...)?;
    let content = args["content"].as_str().ok_or(...)?;
    let reply_to = args["reply_to"].as_str().and_then(|s| EventId::from_hex(s).ok());
    
    let recipient = PublicKey::from_hex(recipient_pubkey)
        .map_err(|_| AgentError::InvalidParams("invalid recipient_pubkey".into()))?;
    
    // NIP-44 加密給接收者
    let ciphertext = nip44_encrypt(keys, &recipient, content)?;
    
    // 建構 kind:4 加密私訊
    let mut builder = EventBuilder::new(Kind::EncryptedDirectMessage, ciphertext)
        .tag(Tag::parse(["p", recipient_pubkey])?);
    
    if let Some(rt) = reply_to {
        builder = builder.tag(Tag::parse(["e", &rt.to_hex(), "", "reply"])?)?;
    }
    
    let event = builder.sign_with_keys(keys.as_ref())?;
    send_event_via_relay(&event, ctx).await
}
```

---

## 11. 失敗模式矩陣與降級策略 (Failure Mode Matrix)

此章節推演 Phase 3 (Mode B Agent Daemon) 在極端情況下的行為，定義具體的檢測、降級與恢復機制。

| 編號 | 失敗場景 | 觸發條件 | 檢測機制 | 即時降級行為 | 恢復策略 | 風險等級 |
|------|----------|----------|----------|--------------|----------|----------|
| **FM-01** | **Redis 完全不可用** | 連線超時、拒絕連線、認證失敗 | `daemon_lock.rs` 獲取鎖時返回 `RedisError::ConnectionRefused` | 1. Daemon **拒絕啟動**，回傳明確錯誤 `DaemonError::RedisUnavailable`<br>2. Mode A (頻道內提醒) **完全不受影響**<br>3. 記錄結構化錯誤日誌含 `redis_url` 供排查 | 1. Redis 恢復後，下次 App 重啟自動嘗試啟動 Daemon<br>2. 支援熱重載：`SIGHUP` 觸發配置重讀與 Daemon 重啟 | 🔴 **Critical** |
| **FM-02** | **Redis 單節點故障切換** | Sentinel/Cluster 故障轉移期間短暫不可用 (1-5s) | 鎖心跳續期失敗 (`lock_heartbeat` 返回 `Err`) | 1. 心跳 task 捕獲錯誤，**主動釋放鎖** (`drop` 觸發 Lua 安全釋放)<br>2. 進入 **指數退避重試** (1s, 2s, 4s, max 30s)<br>3. 重試成功自動重新獲取鎖並恢復訂閱 | 無需人工介入，自動恢復；鎖 TTL 10s 確保故障節點最多佔用 10s | 🟡 **High** |
| **FM-03** | **Relay 拒收事件** | Relay 返回 `CLOSED`、`AUTH_REQUIRED`、速率限制、禁用 kind:4/30300 | `send_event_via_relay` 返回 `RelayError::Rejected` / `RateLimited` | 1. **幂等重試**：相同事件最多重試 3 次，間隔 2s/5s/10s<br>2. 速率限制時：**本地緩衝隊列** (內存最多 100 條)，背壓通知上層<br>3. 永久拒絕 (如 kind 禁用)：**標記事件失敗**，寫入死信隊列供人工排查 | 1. Relay 恢復/配置修正後，死信隊列可重放<br>2. 建議配置多 Relay 輪詢 (Phase 3 範圍外) | 🟠 **Medium** |
| **FM-04** | **私鑰輪換 (Key Rotation)** | `BUZZ_AGENT_KEYS` 環境變數更新、Agent 密鑰洩漏撤銷 | App 啟動時 `Config::from_env()` 解析出新 Keys；運行期不支援熱更新 | 1. **僅支援冷啟動輪換**：需重啟 App 才能載入新私鑰<br>2. 舊提醒事件 (kind:30300) 使用舊私鑰 NIP-44 自加密，**無法被新私鑰解密**<br>3. 策略：輪換前**停止建立新提醒**，等待舊提醒全部到期/處理完畢再重啟 | 1. 提供 `buzz-agent key-rotate` CLI 輔助：匯出舊提醒 d-tag 清單、驗證新私鑰可用性<br>2. 文檔化輪換 SOP：停寫 → 等待排空 → 重啟 | 🟡 **High** |
| **FM-05** | **網路分區** | Agent 與 Relay/Rredis 之間丟包、高延遲、DNS 解析失敗 | WebSocket `ping/pong` 超時 (預設 30s)、Redis 命令超時 | 1. **WebSocket**：觸發 `daemon_ws.rs` 重連邏輯 (指數退避 1s-60s)<br>2. **Redis**：鎖心跳失敗 → 主動釋放鎖 → 退避重試<br>3. **事件發送**：本地緩衝，網路恢復後批量重放<br>4. **關鍵**：Daemon **不退出**，持續嘗試恢復連線 | 1. 網路恢復自動重連、重訂閱、續期鎖<br>2. 建議部署層面配置多可用區 Relay/Rredis | 🟠 **Medium** |
| **FM-06** | **時鐘漂移** | 系統時間被 NTP 向前/向後調整 > 1s | `not_before` 時間戳與 `SystemTime::now()` 比對偏差 | 1. **向前調整**：提醒**提前觸發** (可接受，最多提前漂移量)<br>2. **向後調整**：提醒**延遲觸發** (風險：可能錯過不應觸發的時間窗)<br>3. 緩解：Daemon 啟動時記錄 `startup_monotonic = Instant::now()`，內部排程**改用 `Instant` 而非 `SystemTime`** 計算相對等待 | 1. 強制要求部署環境運行 `chrony`/`ntpd`<br>2. 代碼層面：`daemon_dispatch.rs` 排程器使用 `tokio::time::sleep_until(Instant)` | 🟢 **Low** |
| **FM-07** | **NIP-44 解密失敗** | 提醒事件內容被竄改、錯誤私鑰、協議版本不匹配 | `nip44_decrypt` 返回 `Error::InvalidCiphertext` / `Error::DecryptionFailed` | 1. **靜默丟棄**單條失敗事件，**不中斷**處理循環<br>2. 記錄 `warn` 級日誌：`event_id`, `d_tag`, 錯誤類型<br>3. 計數器 `decrypt_failures_total` 暴露給 metrics | 1. 人工排查：檢查對應 d-tag 的提醒是否為惡意/損壞<br>2. 若大量失敗 → 檢查 `BUZZ_AGENT_KEYS` 是否正確 | 🟢 **Low** |
| **FM-08** | **DM 發送目標用戶離線/封鎖** | 目標用戶未連接 Relay、已封鎖 Agent、Relay 拒絕投遞 kind:4 | Relay 不返回 ACK、或返回 `NOTICE` 錯誤 | 1. **Fire-and-forget**：幂等發送，**不等待 ACK**<br>2. 依賴 Nostr 協議特性：用戶上線後從 Relay 歷史拉取<br>3. 可選：配置 `dm_delivery_timeout` 觸發二次發送 (Phase 3 範圍外) | 無需主動恢復，Nostr 協議保證最終一致性 | 🟢 **Low** |
| **FM-09** | **Daemon 重複啟動 (Split-brain)** | 多實例同時獲取 Redis 鎖 (時鐘不同步、Redis 主從延遲) | `daemon_lock.rs` Lua 腳本 `SET NX EX` 原子性保證 | 1. **只有 1 個實例能獲取鎖** (Redis 單線程保證)<br>2. 失敗實例立即退出，記錄 `info` 日誌 "another instance holds the lock"<br>3. TTL 10s + heartbeat 3.3s 確保鎖快速釋放 | 自動修復：鎖持有者心跳停止 → TTL 過期 → 其他實例搶佔 | 🟡 **High** |
| **FM-10** | **記憶體洩漏/緩衝區爆滿** | 長期運行下事件緩衝隊列無限增長、WebSocket 訊息堆積 | 內存監控：緩衝隊列長度 > 1000、RSS > 500MB | 1. **硬性上限**：緩衝隊列 `VecDeque` 固定容量 1000，滿時 **丟棄最舊事件** (LRU)<br>2. 記錄 `error` 日誌並暴露 `buffer_dropped_total` metrics<br>3. WebSocket 讀寫分離 task 設定 `bounded` channel (容量 100) | 1. 監控告警觸發人工介入<br>2. 重啟 Daemon 釋放記憶體 | 🟠 **Medium** |

---

### 降級策略總覽 (Graceful Degradation Summary)

| 功能層級 | 正常模式 | Redis 故障 | Relay 故障 | 網路分區 | 私鑰輪換 |
|----------|----------|------------|------------|----------|----------|
| **Mode A (頻道提醒)** | ✅ 完整 | ✅ 完整 | ✅ 完整 | ✅ 完整 | ✅ 完整 |
| **Mode B (DM 提醒)** | ✅ 完整 | ❌ **停用** (拒啟動) | ⏳ 本地緩衝+重試 | ⏳ 本地緩衝+重連 | ❌ 需重啟 |
| **建立提醒 (LLM 工具)** | ✅ 完整 | ✅ 可用 (寫入 Relay) | ⏳ 重試/緩衝 | ⏳ 重試/緩衝 | ❌ 需新私鑰 |

> **設計原則**：Mode B 是 **增強功能**，任何基礎設施故障**絕不拖垮 Mode A 核心業務**。Daemon 採用「寧缺毋濫」啟動策略：缺 Redis/私鑰 → 直接不啟動，而非降級運行。

---

### 驗收測試建議 (Failure Mode Testing)

| 測試案例 | 執行方式 | 通過標準 |
|----------|----------|----------|
| **T-FM-01** Redis 斷線 | `docker stop redis` → 觀察 Daemon 日誌 | Daemon 記錄錯誤並退出，Mode A 正常 |
| **T-FM-02** Redis 故障轉移 | Redis Sentinel 故障切換 → 觀察心跳 | 鎖自動釋放，切換完成後自動重獲鎖恢復 |
| **T-FM-03** Relay 拒收 kind:4 | Relay 配置禁用 kind:4 → 建立提醒 | 事件進入死信隊列，不阻塞主循環 |
| **T-FM-04** 私鑰輪換 SOP | 修改 `BUZZ_AGENT_KEYS` → 重啟 App | 新提醒用新鑰加密，舊提醒標記不可解密 |
| **T-FM-05** 時鐘向後調 10s | `date -s "@$(($(date +%s)-10))"` | 提醒延遲 ≤ 10s，無重複觸發 |
| **T-FM-06** 損壞提醒事件 | 手工發送無效 ciphertext kind:30300 | 單條解密失敗被丟棄，後續事件正常處理 |
| **T-FM-07** 並發啟動 3 實例 | 同時啟動 3 個 Agent 進程 | 僅 1 個持有鎖，其他 2 個優雅退出 |

---

### 監控指標建議 (Prometheus Metrics)

```prometheus
# Daemon 生命週期
buzz_daemon_status{mode="b"} 1/0
buzz_daemon_uptime_seconds

# Redis 鎖
buzz_daemon_lock_acquired_total
buzz_daemon_lock_heartbeat_failures_total
buzz_daemon_lock_lost_total

# Relay 通訊
buzz_daemon_events_sent_total{kind="30300|4",result="ok|retry|failed"}
buzz_daemon_relay_reconnect_total
buzz_daemon_buffer_dropped_total

# NIP-44
buzz_daemon_decrypt_failures_total
buzz_daemon_encrypt_failures_total

# 提醒處理
buzz_daemon_reminders_due_total
buzz_daemon_reminders_delivered_total
buzz_daemon_reminders_failed_total
```
  
