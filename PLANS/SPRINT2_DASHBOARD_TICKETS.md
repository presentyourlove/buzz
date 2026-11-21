# Sprint 2 儀表板前端實作工單（前 3 張）

> 依據 `DESIGN_SPEC_A_DASHBOARD.md` 規劃，聚焦 MVP 可用價值，技術棧：Vue 3 + TypeScript + Vite + Pinia + uPlot + D3.js。

---

## 工單 1：專案骨架與 API 介面層
**代號**：`DASH-001`  
**優先級**：P0（阻塞所有後續工單）

### 交付物
1. `dashboard/` 專案初始化（Vite + Vue 3 + TS + Pinia + Vue Router）
2. `openapi-typescript` 產生 `src/types/api.ts`，對接後端 OpenAPI 規格
3. `src/composables/useApi.ts`：統一 `fetch` 封裝、自動附加 `Authorization: Bearer <token>`、錯誤統一攔截
4. `src/composables/useWebSocket.ts`：單一連線管理、自動重連（指數退避、上限 30s）、訊息分發到 Pinia store
5. 環境變數 `VITE_API_BASE`、`VITE_WS_BASE`，支援本地/測試/正式三環境

### 驗收標準
- `pnpm dev` 啟動無錯誤，瀏覽器顯示空白布局頁
- `useApi.get('/health')` 回傳 200，`useWebSocket` 連線成功並收到 `hello` 事件
- 單元測試覆蓋 `useApi` 錯誤映射、`useWebSocket` 重連邏輯（Vitest + MSW）

---

## 工單 2：組織圖與特工狀態面板
**代號**：`DASH-002`  
**優先級**：P0（核心可視化）

### 交付物
1. `src/components/OrgChart.vue`：D3.js v7 力導向圖
   - 節點類型：協調者（六邊形）、執行者（圓形）
   - 狀態環：綠/黃/紅/灰對應 `AgentStatus` enum
   - 點擊節點 → 右側 `AgentDetailDrawer.vue` 顯示完整資訊
2. `src/stores/agents.ts`：Pinia store，訂閱 WS `agent.status`、`agent.metrics` 即時更新
3. `src/views/DashboardView.vue`：整合 `OrgChart` + `AgentDetailDrawer` + 頂部工具列（篩選、全螢幕）

### API 需求（後端同步實作）
- `GET /api/v1/agents`：全量特工清單（含角色、能力標籤）
- `GET /api/v1/agents/{id}/metrics`：即時 CPU/記憶體/Token 用量
- WS `agent.status`：`{agent_id, status, timestamp}`
- WS `agent.metrics`：`{agent_id, cpu%, mem_mb, tokens_in, tokens_out}`

### 驗收標準
- 100 節點渲染 < 300ms，拖曳/縮放 60fps
- 狀態變更 < 500ms 反映到 UI，無閃爍
- 無障礙：鍵盤 Tab 導航節點，Enter 開啟詳情，ARIA 標籤完整

---

## 工單 3：任務看板與即時進度
**代號**：`DASH-003`  
**優先級**：P1（核心業務流）

### 交付物
1. `src/components/TaskBoard.vue`：四欄看板（待處理/執行中/已完成/失敗）
   - 卡片：任務 ID、標題、指派特工、開始時間、預估完成、進度條（0-100%）
   - 拖曳排序（`@dnd-kit/core`），拖動觸發 `PATCH /api/v1/tasks/{id}/status`
2. `src/stores/tasks.ts`：Pinia store，訂閱 WS `task.created`、`task.updated`、`task.completed` 樂觀更新
3. `src/views/TasksView.vue`：看板 + 頂部篩選（特工、狀態、時間區間）

### API 需求（後端同步實作）
- `GET /api/v1/tasks`：支援 `?status=&agent_id=&since=&until=&limit=`
- `PATCH /api/v1/tasks/{id}/status`：`{status, progress?, error?}`
- WS `task.created`、`task.updated`、`task.completed`：完整任務物件

### 驗收標準
- 500 任務渲染 < 200ms，虛擬滾動支援
- 狀態流轉動畫 < 200ms，樂觀更新失敗自動回滾並提示
- 篩選組合 < 100ms 回應，URL 同步查詢參數（可分享連結）

---

## 依賴關係與排程
```
DASH-001 (2d) ──► DASH-002 (3d) ──► DASH-003 (3d)
      │              │              │
      └──────────────┴──────────────┘
         共用：useApi、useWebSocket、Pinia、類型
```
**Sprint 2 總工期**：8 個工作天（含緩衝 1 天）  
**交付里程碑**：Day 8 具備「特工拓撲 + 任務看板」雙核心視圖，可接入 E2E 驗收。

---

## 風險與對策
| 風險 | 機率 | 影響 | 對策 |
|------|------|------|------|
| 後端 API 延遲 | 中 | 高 | 工單並行開發：前端先接 Mock Server（MSW），契約以 OpenAPI 為準 |
| D3.js 大量節點效能 | 低 | 中 | 預留 Canvas 渲染降級方案（`d3-force` + `canvas`），Day 3 壓測決策 |
| WS 重連風暴 | 低 | 高 | 客戶端抖動 + 指數退避，伺服器端連線數限流（Nginx `limit_conn`） |
