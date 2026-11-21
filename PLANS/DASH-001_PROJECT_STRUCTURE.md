# DASH-001：dashboard/ 專案骨架標準目錄結構

> 供初始化腳本（`pnpm create vite@latest dashboard -- --template vue-ts` 後）直接套用的標準結構。
> 技術棧：Vue 3.5 + TypeScript 5.6 + Vite 6 + Pinia 3 + Vue Router 4 + uPlot 1.6 + D3.js 7.9 + @dnd-kit 6.1 + Vitest 2.1 + MSW 2.6

---

## 完整目錄樹

```
dashboard/
├── .github/
│   └── workflows/
│       └── ci.yml                # CI：lint + typecheck + test + build
├── .husky/
│   ├── pre-commit                # biome check --apply + lint-staged
│   └── pre-push                  # pnpm run typecheck && pnpm run test:unit
├── .vscode/
│   ├── extensions.json           # 推薦擴充：Vue, Biome, Vitest
│   └── settings.json             # 格式化存檔、TS 重啟
├── public/
│   ├── favicon.ico
│   └── manifest.json             # PWA 基礎設定
├── src/
│   ├── assets/
│   │   ├── styles/
│   │   │   ├── _variables.css    # CSS 變數：色板、間距、斷點、z-index
│   │   │   ├── _reset.css        # normalize + 基礎重置
│   │   │   ├── _typography.css   # 字階系統（rem-based，支援縮放）
│   │   │   └── main.css          # 入口，import 以上四檔
│   │   └── fonts/                # 自訂字體（可選）
│   ├── components/
│   │   ├── common/               # 通用基礎元件（無業務邏輯）
│   │   │   ├── BaseButton.vue
│   │   │   ├── BaseInput.vue
│   │   │   ├── BaseSelect.vue
│   │   │   ├── BaseCard.vue
│   │   │   ├── BaseBadge.vue
│   │   │   ├── BaseSpinner.vue
│   │   │   ├── BaseModal.vue
│   │   │   ├── BaseDrawer.vue
│   │   │   ├── BaseTooltip.vue
│   │   │   └── index.ts          # 統一導出
│   │   ├── dashboard/            # 儀表板專用元件
│   │   │   ├── OrgChart.vue
│   │   │   ├── OrgChartNode.vue
│   │   │   ├── AgentDetailDrawer.vue
│   │   │   ├── TaskBoard.vue
│   │   │   ├── TaskColumn.vue
│   │   │   ├── TaskCard.vue
│   │   │   ├── MetricsPanel.vue
│   │   │   ├── TokenUsageChart.vue
│   │   │   ├── SystemMetricsChart.vue
│   │   │   ├── Toolbar.vue
│   │   │   └── index.ts
│   │   └── layout/               # 版面框架元件
│   │       ├── AppHeader.vue
│   │       ├── AppSidebar.vue
│   │       ├── AppFooter.vue
│   │       └── index.ts
│   ├── composables/
│   │   ├── useApi.ts             # 統一 fetch 封裝、Auth、錯誤碼映射
│   │   ├── useWebSocket.ts       # 單例 WS 連線、重連、事件分發
│   │   ├── useDebounce.ts
│   │   ├── useThrottle.ts
│   │   ├── useLocalStorage.ts
│   │   ├── useMediaQuery.ts
│   │   └── index.ts
│   ├── stores/
│   │   ├── agents.ts             # Agent 清單、狀態、指標（Pinia）
│   │   ├── tasks.ts              # 任務看板狀態、篩選、樂觀更新
│   │   ├── metrics.ts            # 系統指標時間序列（uPlot 資料格式）
│   │   ├── ui.ts                 # 全域 UI 狀態：sidebar 折疊、主題、全螢幕
│   │   └── index.ts
│   ├── types/
│   │   ├── api.ts                # openapi-typescript 產生（嚴禁手改）
│   │   ├── agent.ts              # Agent、AgentStatus、AgentMetrics、AgentRole
│   │   ├── task.ts               # Task、TaskStatus、TaskProgress、TaskFilter
│   │   ├── metric.ts             # MetricPoint、MetricSeries、TimeRange
│   │   ├── websocket.ts          # WS 訊息聯集型別：AgentStatusMsg、TaskUpdateMsg…
│   │   └── index.ts
│   ├── utils/
│   │   ├── format.ts             # 日期、數字、位元組、持續時間格式化
│   │   ├── color.ts              # 狀態色、角色色、漸層產生
│   │   ├── d3-helpers.ts         # D3 比例尺、力導向預設配置
│   │   ├── uplot-helpers.ts      # uPlot 共用 options 工廠
│   │   ├── validation.ts         # Zod schemas（表單驗證）
│   │   └── index.ts
│   ├── views/
│   │   ├── DashboardView.vue     # 主儀表板：OrgChart + MetricsPanel
│   │   ├── TasksView.vue         # 任務看板頁
│   │   ├── AgentsView.vue        # 特工清單/詳情頁（未來擴充）
│   │   ├── SettingsView.vue      # 設定頁（未來擴充）
│   │   └── NotFoundView.vue      # 404
│   ├── router/
│   │   ├── index.ts              # createRouter、路由表、導航守衛
│   │   └── routes.ts             # 路由定義陣列（懶加載 import()）
│   ├── plugins/
│   │   ├── pinia.ts              # createPinia + persist 插件
│   │   ├── biome.ts              # 僅開發環境：Biome 診斷面板
│   │   └── index.ts
│   ├── App.vue                   # 根元件：<RouterView> + 全域 Toast/Modal Portal
│   ├── main.ts                   # 入口：createApp、掛載插件、掛載 router、全域 CSS
│   ├── env.d.ts                  # Vite 型別宣告
│   └── shims-vue.d.ts            # *.vue 模組宣告
├── tests/
│   ├── unit/
│   │   ├── composables/
│   │   │   ├── useApi.test.ts
│   │   │   └── useWebSocket.test.ts
│   │   ├── stores/
│   │   │   ├── agents.test.ts
│   │   │   └── tasks.test.ts
│   │   └── utils/
│   │       ├── format.test.ts
│   │       └── color.test.ts
│   ├── integration/
│   │   └── api-contract.test.ts  # MSW 對接 OpenAPI 契約測試
│   ├── e2e/
│   │   └── dashboard.spec.ts     # Playwright 關鍵流程（可選，Phase 2）
│   ├── mocks/
│   │   ├── handlers.ts           # MSW REST handlers
│   │   ├── ws-handlers.ts        # MSW WebSocket handlers
│   │   └── data.ts               # Mock 資料工廠
│   ├── setup.ts                  # Vitest 全域設定（@testing-library/vue、vi.fn…）
│   └── vitest.config.ts          # Vitest 專用配置（環境、覆蓋率、別名）
├── .env.example                  # 環境變數範本
├── .env.local                    # 本地覆蓋（gitignore）
├── .gitignore
├── .npmrc                        # strict-peer-dependencies=false 等
├── biome.json                    # Biome 配置（格式化 + lint）
├── tsconfig.json                 # TypeScript 專案配置
├── tsconfig.node.json            # Vite/Node 專用配置
├── vite.config.ts                # Vite 配置：別名、插件、建構優化
├── vitest.config.ts              # Vitest 根配置（引用 tests/vitest.config.ts）
├── package.json
├── pnpm-lock.yaml
├── README.md                     # 專案說明、啟動、腳本、環境變數
└── openapi.json                  # 後端 OpenAPI 3.1 規格（CI 時自動下載/驗證）
```

---

## 關鍵配置檔案內容要點

### `package.json` 核心腳本
```json
{
  "scripts": {
    "dev": "vite",
    "build": "vue-tsc --noEmit && vite build",
    "preview": "vite preview",
    "typecheck": "vue-tsc --noEmit",
    "lint": "biome check --apply src/",
    "format": "biome format --write src/",
    "test:unit": "vitest run --coverage",
    "test:watch": "vitest",
    "test:e2e": "playwright test",
    "generate:types": "openapi-typescript openapi.json -o src/types/api.ts",
    "prepare": "husky install"
  }
}
```

### `vite.config.ts` 關鍵別名
```ts
export default defineConfig({
  resolve: {
    alias: {
      '@': '/src',
      '@components': '/src/components',
      '@stores': '/src/stores',
      '@types': '/src/types',
      '@utils': '/src/utils',
      '@views': '/src/views',
      '@composables': '/src/composables',
    },
  },
  plugins: [vue(), vueJsx()],
  build: {
    target: 'es2022',
    cssCodeSplit: true,
    rollupOptions: { output: { manualChunks: { vendor: ['vue', 'pinia', 'vue-router', 'd3', 'uplot'] } } },
  },
})
```

### `.env.example`
```bash
VITE_API_BASE=http://localhost:3000/api/v1
VITE_WS_BASE=ws://localhost:3000/ws
VITE_APP_TITLE=Buzz Dashboard
VITE_ENABLE_MOCK=false
```

---

## 初始化指令單列表（可直接複製執行）

```bash
# 1. 建立專案
pnpm create vite@latest dashboard -- --template vue-ts
cd dashboard

# 2. 核心依賴
pnpm add vue@latest pinia@latest vue-router@latest @vueuse/core
pnpm add d3@latest uplot@latest @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
pnpm add zod@latest date-fns@latest clsx@latest tailwind-merge@latest

# 3. 開發依賴
pnpm add -D typescript@latest vue-tsc@latest vite@latest @vitejs/plugin-vue@latest @vitejs/plugin-vue-jsx@latest
pnpm add -D @biomejs/biome@latest @biomejs/biome@latest
pnpm add -D vitest@latest @vitest/coverage-v8@latest @testing-library/vue@latest @testing-library/user-event@latest jsdom@latest msw@latest
pnpm add -D openapi-typescript@latest
pnpm add -D husky@latest lint-staged@latest
pnpm add -D playwright@latest @playwright/test@latest

# 4. TypeScript 專案引用（可選，大型專案建議）
# 5. 複製上述目錄結構檔案（或使用生成器腳本）
# 6. 安裝 husky hooks
pnpm prepare

# 7. 首次型別生成（需後端 openapi.json 就緒）
pnpm generate:types

# 8. 啟動開發
pnpm dev
```

---

## 驗收清單（DASH-001 Definition of Done）

- [ ] `pnpm dev` 啟動無錯誤，瀏覽器 `http://localhost:5173` 顯示空白布局頁（Header + Sidebar + RouterView）
- [ ] `pnpm typecheck` 通過（`vue-tsc --noEmit` zero errors）
- [ ] `pnpm lint` 通過（Biome zero warnings）
- [ ] `pnpm test:unit` 通過（覆蓋率 ≥ 80% on composables/stores/utils）
- [ ] `useApi.get('/health')` 回傳 200，`useWebSocket` 連線成功並收到 `hello` 事件（需 Mock Server 或真實後端）
- [ ] 目錄結構與上表 100% 一致，無多餘/遺漏檔案
- [ ] `openapi-typescript` 產生 `src/types/api.ts` 無報錯，型別可被 `useApi` 正確推導
- [ ] 環境變數 `VITE_API_BASE`、`VITE_WS_BASE` 在 `.env.local` 生效，三環境切換驗證通過

---

> **備註**：此結構為 DASH-001
