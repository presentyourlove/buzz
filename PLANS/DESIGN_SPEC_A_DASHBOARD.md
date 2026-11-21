# DESIGN_SPEC_A_DASHBOARD.md

## 獨立本地 Web 儀表板 — 設計規格書 v1.0

> **計畫代號**：A
> **所屬計畫**：多代理增強計畫（Queued 待命佇列）
> **版本**：v1.0
> **作者**：Honey2
> **建立日期**：2026-11-19
> **審核狀態**：待 Sentry 審核

---

## 1. 設計目標與範圍

### 1.1 核心目標
提供一個**零侵入、本地部署、輕量級**的 Web 儀表板，於 `127.0.0.1:8765` 運行，專供主理人監控：
- 特工組織圖與狀態即時視覺化
- 任務執行進度與日誌流
- 系統資源佔用（CPU、記憶體、Token 用量）

### 1.2 非目標（Out of Scope）
- ❌ 任何對 Buzz 桌面版（block/buzz-app）原始碼的修改
- ❌ 對 Relay 核心代碼的侵入式變更
- ❌ 跨網路訪問（僅限本地回環介面）
- ❌ 多用戶權限體系（單用戶本地工具）

### 1.3 設計原則
| 原則 | 說明 |
|------|------|
| **Zero-Intrusion** | 純讀取 API + WebSocket 訂閱，不寫入 Buzz 事件總線 |
| **Local-First** | 僅綁定 `127.0.0.1`，無 TLS，無公網暴露 |
| **Lightweight** | 靜態前端 + 輕量後端（< 50MB 記憶體，< 2% CPU 閒置） |
| **Real-time** | WebSocket 推送狀態變更，無輪詢 |
| **Schema-Driven** | API 契約以 OpenAPI 3.1 定義，前後端型別共享 |

---

## 2. 架構總覽

```
┌─────────────────────────────────────────────────────────────────┐
│                      127.0.0.1:8765                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │  靜態前端     │    │  輕量後端     │    │  Buzz 事件總線    │  │
│  │  (Vite+Vue3)  │◄───│  (Axum/Rust)  │◄───│  (Nostr WebSocket)│  │
│  │  Port 8765   │    │  Port 8766    │    │  ws://relay:3000  │  │
│  └──────────────┘    └──────────────┘    └──────────────────┘  │
│         │                   │                    │              │
│         │        ┌──────────┴──────────┐        │              │
│         │        │   狀態聚合引擎       │        │              │
│         │        │  - Agent Registry  │        │              │
│         │        │  - Task Tracker    │        │              │
│         │        │  - Metrics Collector│       │              │
│         │        └─────────────────────┘        │              │
└─────────│──────────────────────────────────────│──────────────┘
          │                                      │
          ▼                                      ▼
   瀏覽器渲染                              訂閱幾類事件：
   - 組織圖 SVG/Canvas                      - Kind:39002 (Channel membership)
   - 任務看板                               - Kind:30300 (Agent status)
   - 即時日誌流                             - Kind:40002 (Task updates)
   - 資源圖表                               - Kind:30301 (Metrics heartbeat)
```

### 2.1 技術棧選型

| 層級 | 技術 | 版本 | 理由 |
|------|------|------|------|
| 後端框架 | Axum | 0.7+ | Rust 原生、效能佳、型別安全 |
| 非同步運行時 | Tokio | 1.38+ | 標準生態 |
| WebSocket | tokio-tungstenite | 0.22+ | 成熟 WebSocket 客戶端 |
| 序列化 | serde + serde_json | 1.0+ | 標準 |
| 配置管理 | figment | 0.10+ | 多源配置合併 |
| 前端建構 | Vite | 5.x | 快速 HMR |
| 前端框架 | Vue 3 | 3.4+ | Composition API、體積小 |
| 狀態管理 | Pinia | 2.1+ | 輕量、TypeScript 友好 |
| 圖表渲染 | D3.js + Vue | 7.x | 組織圖靈活佈局 |
| 即時圖表 | uPlot | 1.6+ | 高效能時間序列 |
| 型別共享 | TypeScript + openapi-typescript | 5.x / 6.x | 契約優先開發 |

---

## 3. API 契約（OpenAPI 3.1 摘要）

### 3.1 基礎資訊
```
Base URL: http://127.0.0.1:8766/api/v1
WebSocket: ws://127.0.0.1:8766/ws
Content-Type: application/json
```

### 3.2 核心端點

#### GET /agents
**回應**：`AgentListResponse`
```json
{
  "agents": [
    {
      "pubkey": "hex",
      "display_name": "string",
      "role": "string",
      "status": "online|offline|busy|error",
      "current_task": "string|null",
      "capabilities": ["string"],
      "last_heartbeat": "ISO8601",
      "metadata": {}
    }
  ],
  "total": 5
}
```

#### GET /agents/{pubkey}
**回應**：`AgentDetailResponse`（含完整任務歷史、Token 用量統計）

#### GET /tasks
**查詢參數**：`status?=pending|running|completed|failed` `agent_pubkey?` `limit?=50`
**回應**：`TaskListResponse`

#### GET /tasks/{task_id}
**回應**：`TaskDetailResponse`（含步驟執行日誌、輸出產物連結）

#### GET /metrics/system
**回應**：`SystemMetricsResponse`
```json
{
  "cpu_percent": 12.3,
  "memory_mb": 245,
  "disk_mb": 1024,
  "network_io": {"rx_bytes": 1024, "tx_bytes": 512},
  "timestamp": "ISO8601"
}
```

#### GET /metrics/tokens
**回應**：`TokenMetricsResponse`
```json
{
  "by_agent": {
    "pubkey": {"input": 1000, "output": 500, "cost_usd": 0.0012}
  },
  "total": {"input": 5000, "output": 2500, "cost_usd": 0.006},
  "window": "24h"
}
```

#### GET /org-chart
**回應**：`OrgChartResponse`
```json
{
  "nodes": [
    {"id": "pubkey", "label": "Fizz", "type": "coordinator", "children": ["pubkey2", "pubkey3"]}
  ],
  "edges": [{"from": "pubkey", "to": "pubkey2", "relation": "delegates_to"}]
}
```

### 3.3 WebSocket 事件流

**連線**：`ws://127.0.0.1:8766/ws`
**訂閱訊息**：
```json
{"type": "subscribe", "topics": ["agents", "tasks", "metrics", "logs"]}
```

**推送事件類型**：
| 事件 | Payload | 觸發時機 |
|------|---------|----------|
| `agent.status` | `AgentStatusUpdate` | 特工上線/離線/狀態變更 |
| `task.update` | `TaskUpdate` | 任務建立/開始/完成/失敗/進度 |
| `metric.tick` | `SystemMetrics` | 每 5 秒心跳 |
| `log.entry` | `LogEntry` | 特工輸出日誌行 |

---

## 4. 後端實作規格

### 4.1 專案結構
```
dashboard-backend/
├── Cargo.toml
├── src/
│   ├── main.rs              # 入口、配置載入、伺服器啟動
│   ├── config.rs            # 配置結構（Figment）
│   ├── state.rs             # 共享狀態
│   ├── nostr/               # Nostr 客戶端封裝
│   │   ├── client.rs        # WebSocket 連線與重連邏輯
│   │   ├── subscriptions.rs # 訂閱管理
│   │   └── event_parser.rs  # 事件解析為領域模型
│   ├── domain/              # 領域模型
│   │   ├── agent.rs
│   │   ├── task.rs
│   │   ├── metrics.rs
│   │   └── org_chart.rs
│   ├── api/                 # REST API 路由
│   │   ├── routes.rs
│   │   ├── handlers/
│   │   └── dto.rs           # 請求/回應 DTO
│   ├── ws/                  # WebSocket 伺服器
│   │   ├── server.rs
│   │   ├── broadcaster.rs   # 訊息廣播
│   │   └── session.rs
│   └── aggregation/         # 狀態聚合引擎
│       ├── agent_registry.rs
│       ├── task_tracker.rs
│       └── metrics_collector.rs
└── tests/
    └── integration_tests.rs
```

### 4.2 關鍵元件設計

#### 4.2.1 Nostr 客戶端（`nostr/client.rs`）
- **連線目標**：`BUZZ_RELAY_URL`（環境變數，預設 `ws://localhost:3000`）
- **認證**：使用 `BUZZ_PRIVATE_KEY` 簽署 NIP-42 AUTH
- **重連策略**：指數退避（1s, 2s, 4s, 8s, max 30s）+ 抖動 ±20%
- **訂閱管理**：
  - 單一長連線，多重過濾器
  - 過濾器幂：`kinds: [30300, 30301, 39002, 40002]` + `#h` 頻道標籤
  - 自動處理 `EOSE` 與斷線後追趕

#### 4.2.2 狀態聚合引擎（`aggregation/`）
- **AgentRegistry**：以 `pubkey` 為鍵的 `DashMap`，維護線程安全的特工狀態
- **TaskTracker**：任務 ID 為鍵，記錄完整生命週期；支援依特工、狀態、時間範圍查詢
- **MetricsCollector**：每 5 秒採樣一次系統指標；Token 用量從 Kind:30301 事件累加

#### 4.2.3 組織圖構建（`domain/org_chart.rs`）
- **資料來源**：
  - Kind:39002 (membership) → 特工所屬頻道
  - Kind:30300 (agent status) → 特工角色、能力
  - 靜態配置檔 `org-chart.toml` → 固定層級關係（協調者→執行者）
- **演算法**：拓樸排序構建有向無環圖；前端以 D3 force-directed 或層級佈局渲染

### 4.3 配置檔（`config.toml`）
```toml
[server]
host = "127.0.0.1"
http_port = 8766
ws_port = 8766  # 共用 port，升級協定

[nostr]
relay_url = "ws://localhost:3000"
private_key_env = "BUZZ_PRIVATE_KEY"
reconnect_base_ms = 1000
reconnect_max_ms = 30000

[subscriptions]
channel_filter = "5ca019f6-1a27-592a-b015-4baf0eb33572"  # general 頻道
kinds = [30300, 30301, 39002, 40002]

[aggregation]
metrics_interval_sec = 5
task_retention_days = 30
max_log_entries_per_task = 10000

[org_chart]
static_config = "org-chart.toml"
```

---

## 5. 前端實作規格

### 5.1 專案結構
```
dashboard-frontend/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src/
│   ├── main.ts
│   ├── App.vue
│   ├── styles/
│   │   └── main.css
│   ├── components/
│   │   ├── OrgChart.vue
│   │   ├── TaskBoard.vue
│   │   ├── AgentCard.vue
│   │   ├── MetricsPanel.vue
│   │   ├── TokenUsageChart.vue
│   │   └── LogStream.vue
│   ├── composables/
│   │   ├── useApi.ts
│   │   ├── useWebSocket.ts
│   │   └── useOrgChart.ts
│   ├── stores/
│   │   ├── agents.ts
│   │   ├── tasks.ts
│   │   └── metrics.ts
│   ├── types/
│   │   └── api.ts        # 由 openapi-typescript 生成
│   └── utils/
│       └── format.ts
└── public/
    └── favicon.ico
```

### 5.2 關鍵介面設計

#### 5.2.1 組織圖（`OrgChart.vue`）
- **渲染引擎**：D3.js v7 force-directed 或層級佈局
- **節點樣式**：
  - 協調者：六邊形、主色
  - 執行者：圓形、次色
  - 狀態指示：綠點=在線、黃點=忙碌、紅點=錯誤、灰點=離線
- **交互**：點擊節點 → 側邊欄顯示特工詳情；拖曳平移、滾輪縮放

#### 5.2.2 任務看板（`TaskBoard.vue`）
- **欄位**：待處理 → 執行中 → 已完成 → 失敗
- **卡片資訊**：任務 ID、標題、指派特工、開始時間、預估完成、進度條
- **即時更新**：WebSocket `task.update` 推送觸發樂觀更新

#### 5.2.3 指標面板（`MetricsPanel.vue` + `TokenUsageChart.vue`）
- **系統指標**：uPlot 繪製 CPU/記憶體/網路 時間序列（保留 1 小時、解析度 5 秒）
