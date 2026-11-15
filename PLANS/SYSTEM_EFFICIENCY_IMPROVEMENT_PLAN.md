---
title: "系統效率提升計畫 - 實作計畫書"
tags: [system-efficiency, performance, agent-optimization, cache, automation]
status: draft
created: 2026-10-31
version: "1.3"
---

# 系統效率提升計畫 - 完整實作計畫書

> **授權狀態**：主理人 (可愛小寶寶) 已回覆「今日嘅進化計劃書冇問題。可以進行規劃同審查」
> **目標頻道**：General 頻道 (5ca019f6-1a27-592a-b015-4baf0eb33572)
> **執行團隊**：Honey2 (規畫) ➔ Sentry (審查) ➔ Pollen (實作)
> **當前階段**：**Mode B (第二階段) 規劃中** — Phase 2 快取層優化
> **審核狀態**：Mode A 已驗收通過 (QA PASSED)，Mode B **條件通過** — 已修正 Sentry 6 項必修項

---

## 🎯 核心目標 (Mode B 聚焦)

根據 Fizz 呈報的進化建議書第二支柱：**快取優化策略** — 針對大型專案進行記憶體快取配置，減少讀取磁碟時間。

**Mode B 具體目標**：
1. **後端記憶體快取層**：引入 `HistoryItemCache`，消除 `truncate_history` / `handoff` 的重複計算
2. **前端持久化快取**：`searchHitEventCache` 遷移至 IndexedDB，`agentWorkingSignal` 新增 localStorage 同步
3. **配置管理**：新增快取相關環境變數與運行時調整能力

---

## 📋 Mode B 現狀盤點與差距分析

### 2.1 後端關鍵熱點 (實測數據參考)

| 熱點函數 | 位置 | 調用頻率 | 現況耗時 | 目標耗時 |
|----------|------|----------|----------|----------|
| `truncate_history` | `handoff.rs:399` | 每輪對話 | ~15ms (1000 項) | **< 5ms** |
| `should_handoff` | `handoff.rs:244-263` | 每輪對話 | ~3ms | **< 1ms** |
| `estimate_history_tokens` | `handoff.rs:480-487` | 大量調用 | 重複計算 | **快取命中** |
| `HistoryItem::context_pressure_bytes` | `types.rs:80-82` | 極高 | 遍歷計算 | **快取命中** |

### 2.2 前端快取現況

| 模組 | 現況 | 問題 | 目標 |
|------|------|------|------|
| `searchHitEventCache.ts` | 記憶體 `Map` | 重啟失效、冷啟動無命中 | **IndexedDB 持久化**，冷啟動 < 2s 恢復 |
| `agentWorkingSignal.ts:69` (`stateCache`) | 記憶體 `Map` (衍生自 `typingByChannel`) | 頁面刷新重置、衍生狀態不可直接持久化 | **持久化 `typingByChannel` (第 62 行) 為源頭資料**，啟動時重建衍生狀態 |

---

## 🛠️ Mode B 詳細實作規格

### Phase 2.1：後端記憶體快取層 (12h)

#### 2.1.1 新增 `HistoryItemCache` 結構體 (`types.rs`)

**插入位置**：`types.rs` 第 121 行後 (在 `HistoryItem` 實作之後，`ToolCall` 結構體之前)

```rust
use std::sync::Arc;
use dashmap::DashMap;
use blake3;

/// HistoryItem 尺寸快取
/// 
/// 鍵：HistoryItem 內容的 blake3 雜湊 (32 bytes)
/// 值：(estimated_bytes, context_pressure_bytes)
/// TTL：配置化，預設 5 分鐘
#[derive(Debug, Clone)]
pub struct HistoryItemCache {
    inner: Arc<DashMap<[u8; 32], (usize, usize, std::time::Instant)>>,
    ttl: std::time::Duration,
    max_entries: usize,
}

impl HistoryItemCache {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            ttl: std::time::Duration::from_secs(ttl_secs),
            max_entries,
        }
    }

    /// 計算內容雜湊 (blake3)
    fn hash_item(item: &HistoryItem) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        match item {
            HistoryItem::User(s) => hasher.update(s.as_bytes()),
            HistoryItem::Assistant { text, tool_calls, reasoning_details } => {
                hasher.update(text.as_bytes());
                for tc in tool_calls {
                    hasher.update(tc.provider_id.as_bytes());
                    hasher.update(tc.name.as_bytes());
                    if let Ok(bytes) = serde_json::to_vec(&tc.arguments) {
                        hasher.update(&bytes);
                    }
                    if let Ok(bytes) = serde_json::to_vec(&tc.provider_extra) {
                        hasher.update(&bytes);
                    }
                }
                if let Some(rd) = reasoning_details {
                    if let Ok(bytes) = serde_json::to_vec(rd) {
                        hasher.update(&bytes);
                    }
                }
            }
            HistoryItem::ToolResult(r) => {
                hasher.update(r.provider_id.as_bytes());
                for c in &r.content {
                    match c {
                        ToolResultContent::Text(t) => hasher.update(t.as_bytes()),
                        ToolResultContent::Image { data, mime_type } => {
                            hasher.update(data.as_bytes());
                            hasher.update(mime_type.as_bytes());
                        }
                    }
                }
                hasher.update(&[r.is_error as u8]);
            }
        }
        *hasher.finalize().as_bytes()
    }

    /// 獲取或計算尺寸
    pub fn get_or_compute(&self, item: &HistoryItem) -> (usize, usize) {
        let key = Self::hash_item(item);
        let now = std::time::Instant::now();

        // 快取命中且未過期
        if let Some(entry) = self.inner.get(&key) {
            let (est, pressure, cached_at) = *entry;
            if now.duration_since(cached_at) < self.ttl {
                return (est, pressure);
            }
        }

        // 計算並存入
        let est = item.estimated_bytes();
        let pressure = item.context_pressure_bytes();
        
        // LRU 淘汰：超過上限時移除最舊 20%
        if self.inner.len() >= self.max_entries {
            let to_remove = (self.max_entries / 5).max(1);
            let mut entries: Vec<_> = self.inner.iter()
                .map(|e| (*e.key(), *e.value()))
                .collect();
            entries.sort_by_key(|(_, (_, _, t))| *t);
            for (k, _) in entries.into_iter().take(to_remove) {
                self.inner.remove(&k);
            }
        }

        self.inner.insert(key, (est, pressure, now));
        (est, pressure)
    }

    /// 清理過期條目 (可定期調用)
    pub fn cleanup_expired(&self) {
        let now = std::time::Instant::now();
        self.inner.retain(|_, (_, _, cached_at)| now.duration_since(*cached_at) < self.ttl);
    }
}
```

**新增依賴** (`crates/buzz-agent/Cargo.toml`)：
```toml
dashmap = "6"
blake3 = "1"
```

#### 2.1.2 整合至 `handoff.rs`

**修改 `estimate_history_tokens` (第 480-487 行)**：

```rust
// 引入快取
use crate::types::HistoryItemCache;

// 在 RunCtx 或 Handoff 相關結構體中持有快取實例
// 參考 lib.rs:824 RunCtx 建構時注入

fn estimate_history_tokens(history: &[HistoryItem], cache: Option<&HistoryItemCache>) -> u64 {
    let total_bytes: usize = if let Some(cache) = cache {
        history.iter().map(|item| {
            let (_, pressure) = cache.get_or_compute(item);
            pressure
        }).sum()
    } else {
        history.iter().map(HistoryItem::context_pressure_bytes).sum()
    };
    estimate_tokens_from_bytes(total_bytes)
}
```

**同步修改呼叫點 (2 處)**：
1. `handoff.rs:234` — `handoff()` 內部統計 token 時呼叫
2. `handoff.rs:266` — `projected_handoff_input_tokens()` 內部呼叫

**修改 `should_handoff` (第 244-263 行)**：
```rust
fn should_handoff(&self) -> bool {
    // ... 既有邏輯 ...
    // 使用快取版本的 projected_handoff_input_tokens
}
```

#### 2.1.3 `config.rs` 新增快取配置與驗證

**在 `Config` 結構體中新增 (第 500 行左右)**：
```rust
/// HistoryItem 尺寸快取 TTL (秒)，預設 300 (5 分鐘)
pub history_cache_ttl_secs: u64,
/// HistoryItem 快取最大條目數，預設 10000
pub history_cache_max_entries: usize,
```

**在 `Config::from_env()` 中載入 (第 531-599 行區塊)**：
```rust
history_cache_ttl_secs: std::env::var("BUZZ_AGENT_HISTORY_CACHE_TTL")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(300),
history_cache_max_entries: std::env::var("BUZZ_AGENT_HISTORY_CACHE_MAX")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(10_000),
```

**在 `Config::validate()` 新增驗證**：
```rust
// Config::validate() 內新增
if self.history_cache_ttl_secs < 60 {
    return Err(AgentError::InvalidParams(
        "BUZZ_AGENT_HISTORY_CACHE_TTL must be >= 60 seconds".into()
    ));
}
if self.history_cache_max_entries < 100 {
    return Err(AgentError::InvalidParams(
        "BUZZ_AGENT_HISTORY_CACHE_MAX must be >= 100 entries".into()
    ));
}
```

#### 2.1.4 `lib.rs` 注入快取實例

**在 `run_prompt` (`lib.rs:824` 附近) 建構 `RunCtx` 時**：

```rust
// lib.rs:763 spawn_prompt -> run_prompt -> RunCtx 建構
let history_cache = Arc::new(HistoryItemCache::new(
    app.cfg.history_cache_ttl_secs,
    app.cfg.history_cache_max_entries,
));
// 擴充 RunCtx 結構體新增 history_cache: Arc<HistoryItemCache> 欄位
// 在 RunCtx 構造處注入 history_cache
```

---

### Phase 2.2：前端持久化快取遷移 (8h)

#### 2.2.1 `searchHitEventCache.ts` 遷移至 IndexedDB

**新增依賴** (`desktop/package.json`)：
```json
"idb": "^8.0.0"
```

**重構 `desktop/src/app/navigation/searchHitEventCache.ts`**：

```typescript
import { openDB, DBSchema, IDBPDatabase } from 'idb';

interface SearchHitCacheDB extends DBSchema {
  searchHits: {
    key: string;           // `${channelId}:${messageId}`
    value: {
      event: RelayEvent;
      channelId: string;
      messageId: string;
      cachedAt: number;    // Date.now()
      ttl: number;         // TTL in ms
    };
    indexes: { 'by-channel': string; 'by-expires': number };
  };
}

const DB_NAME = 'buzz-search-hit-cache';
const DB_VERSION = 1;
const DEFAULT_TTL_MS = 24 * 60 * 60 * 1000; // 24h

let dbPromise: Promise<IDBPDatabase<SearchHitCacheDB>> | null = null;

function getDB() {
  if (!dbPromise) {
    dbPromise = openDB<SearchHitCacheDB>(DB_NAME, DB_VERSION, {
      upgrade(db) {
        const store = db.createObjectStore('searchHits', { keyPath: 'key' });
        store.createIndex('by-channel', 'channelId');
        store.createIndex('by-expires', 'cachedAt');
      },
    });
  }
  return dbPromise;
}

export async function cacheSearchHitEvent(hit: SearchHit): Promise<void> {
  const db = await getDB();
  const key = `${hit.channelId}:${hit.event.id}`;
  await db.put('searchHits', {
    key,
    event: hit.event,
    channelId: hit.channelId,
    messageId: hit.event.id,
    cachedAt: Date.now(),
    ttl: DEFAULT_TTL_MS,
  });
}

export async function getCachedSearchHitEvent(
  targetMessageId: string,
  channelId?: string
): Promise<RelayEvent | null> {
  const db = await getDB();
  
  if (channelId) {
    // 精確查詢
    const key = `${channelId}:${targetMessageId}`;
    const entry = await db.get('searchHits', key);
    if (entry && !isExpired(entry)) return entry.event;
    return null;
  }
  
  // 跨頻道查詢：使用索引
  const entries = await db.getAllFromIndex('searchHits', 'by-channel', channelId ?? '');
  for (const entry of entries) {
    if (entry.messageId === targetMessageId && !isExpired(entry)) {
      return entry.event;
    }
  }
  return null;
}

function isExpired(entry: SearchHitCacheDB['searchHits']['value']): boolean {
  return Date.now() - entry.cachedAt > entry.ttl;
}

// 定期清理過期條目 (每小時)
setInterval(async () => {
  const db = await getDB();
  const now = Date.now();
  const tx = db.transaction('searchHits', 'readwrite');
  const index = tx.store.index('by-expires');
  let cursor = await index.openCursor();
  while (cursor) {
    if (now - cursor.value.cachedAt > cursor.value.ttl) {
      await cursor.delete();
    }
    cursor = await cursor.continue();
  }
  await tx.done;
}, 60 * 60 * 1000);
```

#### 2.2.2 `agentWorkingSignal.ts` 新增 localStorage 同步 (重設計)

**現況分析 (參考 Sentry 審核)**：
- 第 62 行：`typingByChannel` — 源頭資料 `Map<ChannelId, Set<Pubkey>>` (直接寫入)
- 第 69 行：`stateCache` — 衍生狀態 `Map<Key, AgentWorkingState>` (唯讀，不可直接持久化)
- **無 `setAgentWorkingState` / `clearAgentWorkingState` 函式**

**實作策略**：持久化 `typingByChannel` 為源頭，啟動時重建衍生狀態。

```typescript
// 第 62 行附近新增常數
const STORAGE_KEY = 'buzz:agent-working-signal';
const STORAGE_VERSION = 1;

interface PersistedTypingState {
  version: number;
  updatedAt: number;
  // key: `${pubkey}|${channelId}` 格式與 typingByChannel 一致
  typingByChannel: Record<string, string[]>; // channelId -> pubkey[]
}

// 啟動時恢復 typingByChannel
function loadPersistedTyping(): Map<string, Set<string>> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Map();
    const parsed: PersistedTypingState = JSON.parse(raw);
    if (parsed.version !== STORAGE_VERSION) return new Map();
    const map = new Map<string, Set<string>>();
    for (const [channelId, pubkeys] of Object.entries(parsed.typingByChannel)) {
      // 過期清理：超過 10 分鐘視為失效
      if (Date.now() - parsed.updatedAt < 10 * 60 * 1000) {
        map.set(channelId, new Set(pubkeys));
      }
    }
    return map;
  } catch {
    return new Map();
  }
}

// 寫入 typingByChannel 時持久化 (去抖動 500ms)
let persistDebounce: ReturnType<typeof setTimeout> | null = null;
function schedulePersist(typingMap: Map<string, Set<string>>) {
  if (persistDebounce) clearTimeout(persistDebounce);
  persistDebounce = setTimeout(() => {
    const typingByChannel: Record<string, string[]> = {};
    for (const [channelId, pubkeys] of typingMap.entries()) {
      typingByChannel[channelId] = Array.from(pubkeys);
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      version: STORAGE_VERSION,
      updatedAt: Date.now(),
      typingByChannel,
    }));
  }, 500);
}

// 在 reportChannelBotTyping (或對應寫入 typingByChannel 的函式) 後調用 schedulePersist(typingByChannel)
// 啟動模組時：typingByChannel = loadPersistedTyping()
```

---

## 🔒 Mode B 風險評估與失敗模式矩陣 (擴充)

| 編號 | 風險場景 | 觸發條件 | 檢測機制 | 即時降級行為 | 恢復策略 | 風險等級 |
|------|----------|----------|----------|--------------|----------|----------|
| **FM-01** | **快取一致性失效** | 記憶體快取與實際 history 內容不一致 | 驗證腳本比對 `estimated_bytes` 與實際序列化大小 | 自動失效快取、回退至即時計算模式 | 下次驗證週期自動重建快取 | 🟠 Medium |
| **FM-02** | **IndexedDB 寫入失敗** | 儲存空間不足、隱私模式禁用、Safari ITP | `IDBDatabase.onerror`、QuotaExceededError | 降級為純記憶體快取，控制台警告 | 用戶授權存儲後自動遷移 | 🟡 High |
| **FM-03** | **blake3 雜湊碰撞** | 極大量 history 項目 (理論上極低) | 單元測試驗證雜湊分佈 | 雙重檢查：雜湊命中時二次驗證內容 | 升級為 SHA-256 (性能權衡) | 🟢 Low |
| **FM-04** | **DashMap 記憶體洩漏** | 長期運行、清理任務失敗 | RSS 監控 > 500MB、快取條目數監控 | 強制 `cleanup_expired()`、LRU 淘汰 20% | 重啟 Agent 進程 | 🟠 Medium |
| **FM-05** | **localStorage 配額超限** | 5MB 限制、大量 Agent/Channel | `QuotaExceededError` | 僅保留最近 50 條、壓縮序列化 | 用戶清理瀏覽器數據 | 🟡 High |
| **FM-06** | **環境變數未配置** | 部署遺漏 `BUZZ_AGENT_HISTORY_CACHE_TTL` | 啟動時配置驗證 | 使用內建預設值 (300s/10000)、記錄警告 | 部署清單檢查 | 🟢 Low |

---

## 🛡️ Mode B 回滾機制 (擴充)

### 後端回滾
```bash
# 1. 還原代碼
git -C "c:/私人/ai agent/buzz" stash
# 2. 移除新增依賴
# 編輯 crates/buzz-agent/Cargo.toml 移除 dashmap, blake3
# 3. 重建
cargo build --release -p buzz-agent
# 4. 重啟服務
```

### 前端回滾
```bash
# 1. 還原代碼
git -C "c:/私人/ai agent/buzz/desktop" stash
# 2. 移除 idb 依賴
pnpm remove idb
# 3. 重建
pnpm build
# 4. 重啟 Tauri
```

### 數據清理 (如需)
```bash
# 清理 IndexedDB (瀏覽器開發者工具 Console)
# indexedDB.deleteDatabase('buzz-search-hit-cache')

# 清理 localStorage
# localStorage.removeItem('buzz:agent-working-signal')
```

---

## ✅ Mode B 驗收標準 (Definition of Done)

| 指標 | 目標值 | 測試方式 |
|------|--------|----------|
| **truncate_history 執行時間** | **< 5ms** (1000 項 history) | `cargo test -p buzz-agent truncate_history` + `cargo bench` |
| **should_handoff 判斷延遲** | **< 1ms** | 單元測試 `handoff.rs` + 基準測試 |
| **快取命中率** | **> 85%** (穩態運行) | 添加 metrics: `history_cache_hits_total` / `history_cache_misses_total` |
| **前端冷啟動搜尋恢復** | **< 2s** 顯示歷史命中 | 手動測試：關閉/開啟 Desktop，搜尋既有訊息 |
| **Agent 狀態跨 Session 保持** | 重啟後 10 分鐘內狀態保持 | 手動測試：設定工作狀態 → 關閉/開啟 Desktop → 驗證狀態 |
| **IndexedDB 寫入成功率** | **> 99.9%** | 壓力測試：1000 次寫入、隱私模式、配額邊界 |
| **配置熱加載** | 修改環境變數無需重啟 | `SIGHUP` 觸發配置重讀 (若支援) 或重啟驗證 |

---

## 📁 Mode B 檔案變更路徑一覽 (修正版)

| 檔案路徑 | 變更類型 | 說明 |
|----------|----------|------|
| `c:\私人\ai agent\buzz\crates\buzz-agent\src\types.rs` | **修改** | 新增 `HistoryItemCache` 結構體、blake3 雜湊、DashMap 快取 |
| `c:\私人\ai agent\buzz\crates\buzz-agent\src\handoff.rs` | **修改** | `estimate_history_tokens` 整合快取 (簽名變更)、同步修改 2 處呼叫點 (L234, L266)、`should_handoff` 使用快取 |
| `c:\私人\ai agent\buzz\crates\buzz-agent\src\lib.rs` | **修改** | `run_prompt` 注入 `history_cache` 至 `RunCtx` (L824) |
| `c:\私人\ai agent\buzz\crates\buzz-agent\src\config.rs` | **修改** | 新增 `history_cache_ttl_secs`、`history_cache_max_entries` 環境變數 + `validate()` 驗證 |
| `c:\私人\ai agent\buzz\crates\buzz-agent\Cargo.toml` | **修改** | 新增 `dashmap`、`blake3` 依賴 |
| `c:\私人\ai agent\buzz\desktop\src\app\navigation\searchHitEventCache.ts` | **重構** | 遷移至 IndexedDB (`idb`)，新增 TTL、索引、定期清理 |
| `c:\私人\ai agent\buzz\desktop\src\features\agents\agentWorkingSignal.ts` | **重構** | 持久化 `typingByChannel` (L62) 而非衍生 `stateCache`，localStorage 同步、去抖動、版本化 |
| `c:\私人\ai agent\buzz\desktop\package.json` | **修改** | 新增 `idb` 依賴 |

---

## 🧪 單元測試與整合測試計畫 (新增)

### 後端測試 (`cargo test -p buzz-agent`)

| 測試項目 | 目標 | 覆蓋 |
|----------|------|------|
| `HistoryItemCache::get_or_compute` | 快取命中/未命中/過期/淘汰 | `types.rs` |
| `HistoryItemCache::hash_item` | 各 `HistoryItem` 變體雜湊一致性 | `types.rs` |
| `estimate_history_tokens` (有/無快取) | 輸出一致性、快取命中率統計 | `handoff.rs` |
| `should_handoff` | 快取路徑判斷正確性 | `handoff.rs` |
| `Config::validate` | TTL/MaxEntries 邊界值拒絕 | `config.rs` |

### 前端測試 (Vitest + Playwright)

| 測試項目 | 目標 | 覆蓋 |
|----------|------|------|
| `searchHitEventCache.ts` IndexedDB CRUD | 寫入/讀取/TTL 過期/索引查詢 | `searchHitEventCache.ts` |
| `agentWorkingSignal.ts` localStorage 同步 | `typingByChannel` 持久化/恢復/去抖動/版本遷移 | `agentWorkingSignal.ts` |
| E2E 冷啟動 | Desktop 重啟後 < 2s 恢復搜尋命中 | 整合 |

### 基準測試 (`cargo bench`)

| 基準 | 目標值 |
|------|--------|
| `truncate_history` (1000 項) | **< 5ms** |
| `should_handoff` | **< 1ms** |
| `HistoryItemCache::get_or_compute` (命中) | **< 0.1ms** |
| blake3 雜湊計算/項 | **< 0.05ms** |

---

## 📅 Mode B 時程規劃 (24h 含 4h 緩衝)

| 週期 | 工作項目 | 預估工時 | 交付物 | 驗收點 |
|------|----------|----------|--------|--------|
| **Week 1** | 後端快取核心 (`HistoryItemCache` + `handoff.rs` 整合) | 8h | 單元測試通過、基準測試 < 5ms | `cargo test` + `cargo bench` |
| **Week 1** | 配置與驗證 + `lib.rs` 注入 | 4h | `config.rs` 驗證、`RunCtx` 注入、環境變數生效 | 啟動驗證配置 + 快取實例注入 |
| **Week 2** | 前端 IndexedDB 遷移 | 5h | `searchHitEventCache.ts` 重構 + 測試 | E2E 測試：冷啟動 < 2s |
| **Week 2** | 前端 localStorage 同步 (重設計) | 3h | `agentWorkingSignal.ts` 持久化 `typingByChannel` | 跨 Session 狀態保持 10min |
| **Week 2** | 整合測試、基準測試、文檔更新 | 4h | 壓力測試、metrics、更新計畫書 v1.3 | 全指標達標、Sentry 複審通過 |
| **總計** | | **24h** (含 4h 緩衝) | **完整可交付 Mode B** | |

> **註**：原估 20h，含 4h 緩衝 (依賴編譯、跨平台驗證、Sentry 回饋修正、測試補強)

---

## 🚀 下一步行動

**已完成修正 (Sentry 6 項必修項)**：
1. ✅ `agentWorkingSignal.ts` 引用修正：第 69 行 `stateCache` (衍生) → 第 62 行 `typingByChannel` (源頭)
2. ✅ 移除假設的 `setAgentWorkingState`/`clearAgentWorkingState`，改為持久化 `typingByChannel` + 啟動重建
3. ✅ `RunCtx` 構造點修正：`agent.rs:763` → `lib.rs:824`
4. ✅ `estimate_history_tokens` 簽名變更同步 2 處呼叫點：`handoff.rs:234`、`handoff.rs:266`
5. ✅ `Config::validate()` 新增 `history_cache_ttl_secs >= 60`、`history_cache_max_entries >= 100` 驗證
6. ✅ 前端 localStorage 同步重設計：基於 `typingByChannel` 而非衍生 `stateCache`

**新增內容**：
- ✅ 單元測試/整合測試/基準測試計畫
- ✅ 檔案變更路徑修正版
- ✅ 24h 時程 (含 4h 緩衝)

**當前狀態**：v1.3 已就緒，請 @Sentry 進行二次審核。

審核通過後 → 主理人 (可愛小寶寶) 授權 → @Pollen 實作

> 嚴禁 Pollen 在 Sentry 審核通過 + 主理人授權前動工

---
*文件版本：1.3 | 建立者：Honey2 | 審核狀態：條件通過 — 已修正 Sentry 6 項必修項，待二次審核*
*基於：SYSTEM_EFFICIENCY_IMPROVEMENT_PLAN.md v1.2 (Mode A 已驗收通過)*

