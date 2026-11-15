---
title: "Scheduled Reminder NIP-ER Implementation Plan (v2.0 Architectural Revision)"
tags: [timed-reminder, nip-er, rust-implementation, cli, agent-capability, architectural-fix]
status: active
created: 2026-10-30
revised: 2026-10-30
author: "Honey2 (Architectural Revision reviewed by Antigravity)"
---

# 定時提醒功能實作計畫書 (NIP-ER / kind:30300) - v2.0 正式修訂版

## 【核心結論與架構定位】

Buzz 後端已於 `crates/buzz-relay` 與 `crates/buzz-db` 完整實作 NIP-ER (Event Reminders, `kind:30300`) 基礎設施：
- **Relay 驗證層** (`crates/buzz-relay/src/handlers/ingest.rs`)：`validate_event_reminder()` 強制檢查 `d` tag、`not_before` 數字格式與 horizon 上限。
- **持久化層** (`crates/buzz-db/src/store/reminder.rs`)：`query_due_reminders`、`claim_due_reminder_with_stamp` 具備跨 Pod 原子認領與防重送機制。
- **排程器** (`crates/buzz-relay/src/main.rs:1350-1470`)：背景每秒輪詢、Redis pub/sub 分發與作者專屬投遞門禁。

> [!IMPORTANT]
> **【v2.0 關鍵架構校正：Relay Author-Only 投遞限制與雙模式設計】**
> 依據 `crates/buzz-relay/src/handlers/event.rs` 之 `filter_fanout_by_access` 與官方規範 `docs/nips/NIP-ER.md`，`kind:30300` 為**作者專屬 (Author-Only)** 事件：**Relay 僅會向連線公鑰與事件簽名作者完全相符（`pk == author`）之連線投遞，完全不看 `p` tag**。
> 因此本計畫嚴格劃分兩種使用模式：
> 1. **模式 A（個人私密提醒 - CLI / Desktop）**：由用戶本人的金鑰簽署，到期時 Relay 直接推播至用戶桌面/手機客戶端。
> 2. **模式 B（Agent 頻道代理提醒 - Agent Workflow）**：用戶在聊天室請求 Agent（如 Fizz）提醒時，Agent 為自己設定 kind:30300 事件（Agent 為 author，Payload 記下用戶與頻道）；到期時 Relay 喚醒 Agent，Agent 於頻道內主動發訊「⏰ @用戶 時間到了，記得喝水！」。

---

## 一、雙模式運作流程設計

### 1.1 模式 A：用戶端個人私密提醒 (CLI / Desktop)

```
用戶於 CLI 輸入: buzz reminders create --content "開會" --at "30m"
    ↓
CLI 以當前登入用戶金鑰簽名、以 NIP-44 自加密 Content (status: pending, note: "開會")
    ↓
附加標籤: ["d", <128-bit-hex>], ["not_before", <ts>], ["alt", "Encrypted reminder"]
    ↓
發送至 Relay (kind:30300)
    ↓
Relay 排程器到期認領 → 透過 WebSocket 僅投遞給該用戶的客戶端 (pk == author)
    ↓
用戶 Desktop / Mobile 客戶端收到事件，解密顯示系統通知
```

### 1.2 模式 B：Agent 頻道代理提醒 (自然語言委託)

```
用戶於頻道發訊: "@Fizz 30分鐘後提醒我喝水"
    ↓
Fizz 解析意圖 → 呼叫 create_reminder 工具 (記錄目標 channel_id、用戶 pubkey、提醒文字)
    ↓
以 Agent 金鑰簽署 kind:30300 (Agent 為 author，包含 target 與 note)
    ↓
Relay 排程器到期認領 → 透過 WebSocket 投遞給 Agent 連線 (pk == agent_author)
    ↓
Agent 接收到期信號 (Due Signal) 喚醒 → 於原頻道發送 kind:1 或 kind:9 訊息:
"⏰ @可愛小寶寶 30 分鐘到了，記得要飲水囉！"
```

---

## 二、事件規格 (嚴格對齊 NIP-ER 標準與既有代碼)

### 2.1 標籤 (Tags) 規範

| 標籤名 | 必填 | 內容規範 | 備註 |
|:---|:---:|:---|:---|
| `d` | ✅ | 128 位元隨機熵十六進位字串（16 raw random bytes, 32 字元） | **嚴禁**使用 `Uuid::new_v4()`（僅 122 bit 熵，違背 NIP-ER line 58） |
| `not_before` | ✅ | Unix timestamp 秒數（純 ASCII 數字，除 "0" 外不可有前導零） | 代表提醒觸發時間 |
| `alt` | ✅ | 固定為 `"Encrypted reminder"` | 符合 NIP-31 降級規範，E2E 測試必備 |
| `expiration` | ❌ | **pending 狀態嚴禁填寫** | 僅在狀態轉為 `done` 或 `cancelled` 時填寫 30-90 天抖動清理時間 |

> [!CAUTION]
> **嚴禁**在 `kind:30300` 加上 `p` tag 試圖轉發他人，Relay 的 `filter_fanout_by_access` 會嚴格阻斷非作者連線，加 `p` tag 無任何路由效果。

### 2.2 內容 (Content) 結構規範

`.content` 必須使用 **NIP-44 v2 加密給作者自己 (Self-Encryption)**。解密後的 Plaintext **必須為嚴格的 JSON 物件**（完全相容 `desktop/src/features/reminders/lib/reminderService.ts`）：

```json
{
  "target": {
    "eventId": "optional_64char_hex",
    "channelId": "optional_uuid_string",
    "preview": "optional_preview_text",
    "authorPubkey": "optional_pubkey_hex"
  },
  "status": "pending",
  "note": "喝水"
}
```

* `status` **必須**為 `"pending"`、`"done"`、`"cancelled"` 其中之一。
* 提醒文字**必須**置於 `note` 欄位（不可為純字串，非 JSON 格式將被桌面客戶端 Fail-Closed 丟棄）。

---

## 三、實作任務清單

### Phase 1：CLI 指令實作 (`buzz-cli`)

#### 1.1 新增 `buzz reminders` 子命令群組
* **檔案**：`crates/buzz-cli/src/commands/reminders.rs` (新建)

```rust
// 指令規格
buzz reminders create
    --content <TEXT>           // 提醒內容 (寫入 JSON note 欄位)
    --at <TIMESTAMP|DURATION>  // 支援 ISO 8601 或相對時間 "30m", "2h", "1d"
    --channel <UUID>           // 選填，關聯頻道 ID
    --dry-run                  // 僅輸出欲發送之 Event JSON，不實際廣播

buzz reminders list
    --limit <N>                // 預設 50，拉取當前用戶所有 kind:30300，經 NIP-44 解密後輸出

buzz reminders cancel <D_TAG>  // 透過發布省略 not_before、status 為 cancelled 之事件替換原提醒
```

#### 1.2 取消與狀態流轉邏輯 (NIP-33 參數化可替換)
* **取消操作**：不可僅發 NIP-09。正確流程為發布同 `d` tag 之新版本：
  - 移除 `not_before` 標籤（Relay 排程器立即停止監聽）。
  - 設定 `expiration = now + 30~90 days`（Jittered cleanup）。
  - Content 內容更新為 `status: "cancelled"`。
* **完成操作**：狀態更新為 `status: "done"`，同樣移除 `not_before`。

#### 1.3 時間解析與驗證工具
* **檔案**：`crates/buzz-cli/src/validate.rs` (擴充)
* 驗證 `not_before` 不得小於當前時間。
* 驗證 `not_before - now` 不得超過環境變數 `SPROUT_MAX_NOT_BEFORE_DELTA`（Relay 最大容許預約跨度，防止被 Relay 拒收）。

---

### Phase 2：Agent 代理提醒與定時回覆機制 (`buzz-agent`)

#### 2.1 修正工具定義與型別 (`ToolDef`)
* **檔案**：`crates/buzz-agent/src/builtin.rs`
* 欄位必須使用 `pub input_schema: Value`（非 `parameters`）：

```rust
pub fn create_reminder_def() -> ToolDef {
    ToolDef {
        name: "create_reminder".into(),
        description: "委託後台排程器建立定時提醒事件。到期時後台會喚醒 Agent 於當前頻道發送提醒通知。".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "提醒具體內容 (例如: 喝水、開會)"
                },
                "at": {
                    "type": "string",
                    "description": "何時提醒：相對時間 (如 '30m', '2h') 或 ISO8601 時間戳"
                },
                "channel_id": {
                    "type": "string",
                    "description": "觸發提醒時欲推播的頻道 UUID"
                }
            },
            "required": ["content", "at", "channel_id"]
        }),
    }
}
```

#### 2.2 Agent 工具執行與到期閉環機制
* Agent 執行 `create_reminder` 時，透過呼叫內建 CLI 或 MCP 服務發送事件。
* Agent 自身維護 WebSocket 監聽自身 `kind:30300` 事件。
* 當 Relay 投遞 Due Signal 時，Agent 捕捉事件並解析 `note` 與 `channel_id`，隨即調用 `messages send` 在該頻道回覆提醒用戶。

---

## 四、驗收標準 (DoD)

### 4.1 核心功能驗收

| 測試案例 | 預期結果 | 驗證方式 |
|:---|:---|:---|
| `buzz reminders create --content "喝水" --at "1m"` | 1 分鐘後 Relay 發出 Due Signal，本地客戶端收到 | E2E 整合測試 |
| 檢查 Content 格式 | 解密後為包含 `status: "pending"` 與 `note` 之 JSON | 單元測試 |
| 檢查 `d` tag 熵值 | 長度為 32 字元 hex，具備 128 bit 熵 | 單元測試 |
| 檢查 `alt` 標籤 | 必定存在 `["alt", "Encrypted reminder"]` | 單元測試 |
| `buzz reminders cancel <d_tag>` | 發布無 `not_before` 且 `status: cancelled` 之事件，不再觸發 | 整合測試 |
| 自然語言委託「@Fizz 30分鐘後提醒我喝水」 | Fizz 呼叫 `create_reminder`，到期後 Fizz 於頻道主動發訊通知老闆 | E2E 完整會話測試 |

### 4.2 程式碼品質與編譯要求
- [ ] `cargo check -p buzz-cli -p buzz-agent` 零編譯錯誤。
- [ ] `cargo clippy --all-targets --all-features` 零警告。
- [ ] 新增單元測試覆蓋率 ≥ 90%。

---

## 五、Mode B 基礎設施補充設計（解決 Agent Daemon 化與簽名發送鏈路）

> **背景**：@Sentry 複審指出 Mode B 存在 4 大關鍵缺口，必須在開工前補充完整設計。以下為各缺口之具體工程方案。

### 5.1 Agent Daemon 化方案（持久 WebSocket 監聽 Due Signal）

#### 5.1.1 架構定位
`buzz-agent` 目前為 **ACP 按需喚醒** 架構（`crates/buzz-agent/src/agent.rs`），無長連線基礎設施。需引入 **Agent Daemon 子系統**，於 Agent 進程內部維護一條持久 WebSocket 連線至 Relay，專門訂閱自身簽署的 `kind:30300` 事件。

#### 5.1.2 技術規格

| 組件 | 檔案位置 | 職責 |
|------|----------|------|
| **Daemon 生命週期管理** | `crates/buzz-agent/src/daemon.rs` (新建) | `start()` / `stop()` / `restart()`，掛載於 `App` 啟動/關閉鉤子 |
| **WebSocket 客戶端** | `crates/buzz-agent/src/daemon_ws.rs` (新建) | 基於 `tokio-tungstenite`，自動重連、心跳、背壓控制 |
| **訂閱管理器** | `crates/buzz-agent/src/daemon_sub.rs` (新建) | 動態增刪 `REQ` 過濾器（`kinds=[30300]`, `authors=[agent_pubkey]`） |
| **事件分發器** | `crates/buzz-agent/src/daemon_dispatch.rs` (新建) | 解析 Due Signal → 提取 `note`/`channel_id`/`target` → 觸發頻道發送 |

#### 5.1.3 關鍵流程

```
Agent Daemon 啟動
    ↓
從配置/環境讀取 Agent Nostr Keys (pubkey/privkey)
    ↓
建立 WebSocket → Relay (NIP-42 AUTH)
    ↓
發送 REQ: {"kinds":[30300], "authors":[agent_pubkey]}
    ↓
持久監聽 EVENT 幀
    ↓
收到 EVENT (kind:30300, pk == agent_pubkey)
    ↓
驗證簽名 → 以 NIP-44 自解密 Content
    ↓
解析 JSON: {note, target.channelId, status}
    ↓
若 status == "pending" 且 channelId 有效
    ↓
調用內建工具 `send_channel_message(channel_id, note)` 發送提醒
```

#### 5.1.4 可靠性保證

| 風險 | 緩解機制 |
|------|----------|
| Agent 宕機/重啟 | Relay `delivered_at` 機制：重啟後自動補發未認領 Due Signal |
| 多 Agent 實例競爭 | **單例鎖**：`tokio::sync::Mutex` + Redis 分佈式鎖（`buzz_agent_daemon:{agent_pubkey}`）確保單 Pod 單實例 |
| 網路抖動斷線 | 指數退避重連（1s, 2s, 4s, 8s... max 60s），斷線期間 Relay 累積 Due Signal，重連後自動同步 |

---

### 5.2 Agent 簽名鍵材獲取（RunCtx 擴充與內建簽名器）

#### 5.2.1 方案選型：內建簽名器（推薦）

> **拒絕方案**：CLI 子進程簽名（需序列化事件、跨進程傳遞、安全性風險）  
> **採納方案**：直接在 `builtin.rs` 內使用 Agent 自身的 Nostr Keys 簽名發送，零外賴、零序列化開銷。

#### 5.2.2 RunCtx 擴充（最小侵入）

**檔案**：`crates/buzz-agent/src/agent.rs`（`RunCtx` 結構體）

```rust
pub struct RunCtx<'a> {
    // ... 既有欄位 ...
    
    /// Agent 自身的 Nostr Keys（含 privkey），僅在 Mode B Daemon 模式下注入
    /// 由 App 啟動時從配置/環境變數載入，透過 Arc 共享
    pub agent_keys: Option<Arc<nostr::Keys>>,
}
```

> **安全性**：`agent_keys` 僅在 Daemon 子系統與 `create_reminder`/`send_channel_message` 內建工具中可見，**不暴露給 LLM 提示詞**，不可被工具參數竊取。

#### 5.2.3 內建簽名工具實作

**檔案**：`crates/buzz-agent/src/builtin.rs`

```rust
pub fn sign_and_send_event_def() -> ToolDef {
    ToolDef {
        name: "sign_and_send_event".into(),
        description: "以 Agent 金鑰簽署並發送 Nostr 事件（Mode B 專用）".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "kind": {"type": "integer", "description": "事件 kind，如 30300"},
                "content": {"type": "string", "description": "事件內容（已為 NIP-44 密文或明文 JSON）"},
                "tags": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}, "description": "標籤陣列，如 [[\"d\", \"...\"], [\"not_before\", \"...\"]]"}
            },
            "required": ["kind", "content", "tags"]
        }),
    }
}

async fn execute_sign_and_send_event(
    args: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref()
        .ok_or_else(|| AgentError::InvalidParams("Agent keys not available in this context".into()))?;
    
    let kind: u16 = args["kind"].as_u64().ok_or(...)?.try_into()?;
    let content = args["content"].as_str().ok_or(...)?;
    let tags: Vec<Vec<String>> = serde_json::from_value(args["tags"].clone())?;
    
    // 建構並簽名
    let mut builder = EventBuilder::new(Kind::Custom(kind), content);
    for tag in tags {
        builder = builder.tag(Tag::parse(tag)?);
    }
    let event = builder.sign_with_keys(keys.as_ref())?;
    
    // 透過 Relay 發送（複用既有 WebSocket 或 HTTP 端點）
    send_event_via_relay(&event, ctx).await
}
```

---

### 5.3 事件發送通道（內建工具替代 MCP Server）

> **決策**：**不建立獨立 MCP Server**。直接在 `buzz-agent` 內建工具 `send_channel_message` 完成「以 Agent 身份在指定頻道發送提醒訊息」。

#### 5.3.1 內建工具規格

```rust
pub fn send_channel_message_def() -> ToolDef {
    ToolDef {
        name: "send_channel_message".into(),
        description: "以 Agent 身份在指定頻道發送提醒通知（Mode B 到期喚醒後調用）".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "channel_id": {"type": "string", "description": "目標頻道 UUID"},
                "text": {"type": "string", "description": "提醒文字（含 @mention）"},
                "reply_to": {"type": "string", "description": "可選：回覆的事件 ID"}
            },
            "required": ["channel_id", "text"]
        }),
    }
}

async fn execute_send_channel_message(
    args: &Value,
    ctx: &RunCtx<'_>,
) -> Result<ToolResult, AgentError> {
    let keys = ctx.agent_keys.as_ref().ok_or(...)?;
    let channel_id = Uuid::parse_str(args["channel_id"].as_str().ok_or(...)?)?;
    let text = args["text"].as_str().ok_or(...)?;
    let reply_to = args["reply_to"].as_str().and_then(|s| EventId::from_hex(s).ok());
    
    // 建構 kind:1 文字訊息
    let mut builder = EventBuilder::new(Kind::TextNote, text)
        .tag(Tag::parse(["h", &channel_id.to_string()])?);
    if let Some(rt) = reply_to {
        builder = builder.tag(Tag::parse(["e", &rt.to_hex(), "", "reply"])?)
                         .tag(Tag::parse(["e", &rt.to_hex(), "", "root"])?)?;
    }
    let event = builder.sign_with_keys(keys.as_ref())?;
    
    send_event_via_relay(&event, ctx).await
}
```

#### 5.3.2 發送路徑選擇

| 方案 | 優缺點 | 採納 |
|------|--------|------|
| **WebSocket 同連線發送** | 復用 Daemon 既有連線、零延遲 | ✅ **首選** |
| HTTP `/events` 端點 | 需額外 NIP-98 Auth、較高延遲 | 降級備援 |

---

### 5.4 失敗模式完整矩陣

| 失敗場景 | 檢測機制 | 恢復動作 | 驗收標準 |
|----------|----------|----------|----------|
| **Agent 進程崩潰重啟** | Daemon `stop()` 時取消訂閱；Relay 保留 `delivered_at = NULL` | 重啟後 Daemon 重新訂閱 → Relay 推送未認領 Due Signal | 整合測試：Kill -9 Agent → 重啟 → 驗證提醒補發 |
| **WebSocket 斷線 > 30s** | 心跳超時（30s 無 PONG） | 自動重連 + 重新發送 REQ | 混沌測試：斷網 60s → 恢復 → 驗證提醒不遺漏 |
| **多 Pod 部署競爭** | Redis 分佈式鎖 `buzz_agent_daemon:{agent_pubkey}` TTL 10s | 僅持鎖者運行 Daemon；其餘進程進入 Standby | 壓測：5 Pod 同時啟動 → 僅 1 個 Daemon 存活 |
| **Relay 重啟/升級** | WebSocket 斷線觸發重連邏輯 | 重連後自動 REQ 同步 | 整合測試：重啟 Relay → 驗證 Agent 自動恢復監聽 |
| **NIP-44 解密失敗** | `nostr::nips::nip44::decrypt` 返回 Err | 記錄錯誤、跳過該事件、發送告警 | 單元測試：餵入壞密文 → 驗證不 Panic、不阻塞後續 |
| **簽名金鑰輪換** | 配置熱重載檢測 `BUZZ_AGENT_KEYS` 變更 | Daemon 优雅重啟、載入新 Keys | 運維演練：修改 env → SIGHUP → 驗證新簽名有效 |

---

### 5.5 實作任務擴充（Phase 2 追加）

| 任務編號 | 工作項目 | 檔案 | 預估工時 |
|----------|----------|------|----------|
| 2.3 | Agent Daemon 核心骨架（生命週期、配置載入） | `crates/buzz-agent/src/daemon.rs` | 4h |
| 2.4 | WebSocket 客戶端與自動重連 | `crates/buzz-agent/src/daemon_ws.rs` | 3h |
| 2.5 | 訂閱管理與 Due Signal 分發 | `crates/buzz-agent/src/daemon_sub.rs` / `daemon_dispatch.rs` | 3h |
| 2.6 | RunCtx 注入 `agent_keys` | `crates/buzz-agent/src/agent.rs` | 1h |
| 2.7 | 內建工具 `sign_and_send_event` / `send_channel_message` | `crates/buzz-agent/src/builtin.rs` | 3h |
| 2.8 | Daemon 單例鎖（Redis）與單元測試 | `crates/buzz-agent/src/daemon_lock.rs` | 2h |
| 2.9 | 整合測試：完整 Mode B 閉環 | `crates/buzz-agent/tests/mode_b_integration.rs` | 3h |
| **小計** | | | **19h** |

---

### 5.6 整體實作時程更新

| 階段 | 原估工時 | 新增 | 修正後 |
|------|----------|------|--------|
| Phase 1 (CLI) | 11h | 0h | 11h |
| Phase 2 (Agent) | 13h | **19h** | **32h** |
| Phase 3 (Workflow 範本) | 1h | 0h | 1h |
| **總計** | **25h** | **19h** | **44h** |

---

### 5.7 開工授權策略（雙軌並行）

| 軌道 | 範圍 | 前置條件 | 授權狀態 |
|------|------|----------|----------|
| **軌道 A（Mode A）** | CLI 指令、個人私密提醒、Desktop 整合 | v2.0 計畫書審核通過 | ⏳ **待老闆授權** |
| **軌道 B（Mode B）** | Agent Daemon、頻道代理提醒、自然語言委託 | 第 5 節補充設計審核通過 | ⏳ **待補充設計二次審核** |

> **建議**：老闆先授權 **軌道 A 立即開工**（可獨立交付價值），軌道 B 待補充設計通過後另開分支並行。

---

## 六、下一步行動（更新版）

1. **@Sentry**：對本第 5 節「Mode B 基礎設施補充設計」進行二次邏輯推演審核
2. **老闆**：確認「Mode A 先行立項」或「雙軌並行立項」
3. **@Pollen**：收到授權後，按軌道 A → 軌道 B 順序嚴格實作

---

*本計畫書 v2.1（含第 5 節）已存檔於 `C:\Users\user\.buzz\PLANS\SCHEDULED_REMINDER_NIP_ER_IMPLEMENTATION_PLAN.md`。*
