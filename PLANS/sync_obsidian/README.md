# 🧠 Buzz Obsidian Memory Sync (M1, M2, M3 全域記憶同步系統)

本系統為 **Buzz Agent 團隊**（秘書 [[Fizz]]、雜工 [[Honey]]、工程師 [[Pollen]]、清潔工 [[Wax]]）與 **Obsidian 第二大腦** 之間的自動化雙向同步橋樑。系統負責將熱記憶 (Core Memory)、冷記憶 (Cold Memory) 與歷史對話摘要 (Conversation Digest) 結構化沉澱為高度互聯的 Markdown 知識庫，並提供本機 Git 自動版本追蹤。

---

## 📁 模組架構 (`sync_obsidian/`)

```text
sync_obsidian/
├── config.yaml              # 同步設定檔（Vault 目標路徑、各項輪詢頻率、過濾標籤與遮罩規則）
├── config-loader.mjs        # 設定載入器（零依賴安全 YAML 解析與容錯回退機制）
├── sanitizer.mjs            # 敏感資料安全過濾器（[PROTECTED_SECRET] 遮罩與黑名單標籤攔截）
├── buzz-client.mjs          # 記憶提取用戶端（CLI 與本地代理人資料庫雙軌容錯）
├── obsidian-writer.mjs      # Frontmatter 生成與 UTF-8 無 BOM 寫檔器
├── cold-memory-sync.mjs     # M2: 冷記憶增量同步器（掃描 PLANS 目錄規格書並增量歸檔）
├── cold-index-builder.mjs   # M2: 冷記憶總索引建置器（自動生成 01-Cold-Memory/INDEX.md）
├── conversation-parser.mjs  # M3: 歷史對話討論串解析器（聚類各時期重點 Thread）
├── semantic-digester.mjs    # M3: 語意去蕪存菁提煉器（Wax 核心：提煉決策、共識與雙向連結）
├── digest-index-builder.mjs # M3: 對話脈絡總索引建置器（自動生成 02-Conversation-Digest/INDEX.md）
├── git-sync.mjs             # 本地 Git 版本控制自動化模組（自動 init、diff 檢查與 Conventional Commit）
├── state-store.mjs          # 同步游標狀態與心跳記錄器 (last-sync.json, heartbeat.json)
├── main.mjs                 # 主排程器與 CLI 入口（整合 M1 + M2 + M3 完整循環）
└── README.md                # 模組使用與技術規格說明書
```

---

## 🏛️ Obsidian Vault 對應目錄結構 (`buzz-obsidian/`)

```text
buzz-obsidian/
├── 歡迎.md                                   # 保管庫最高規範與核心原則總覽
├── 00-Core-Memory/                           # M1: 核心長期記憶（單檔覆寫、權威原則）
│   └── core-memory.md                        # 系統核心原則、身份定位與第二大腦導航雷達
├── 01-Cold-Memory/                           # M2: 冷記憶（技術規格、架構設計、可行性分析）
│   ├── INDEX.md                              # 冷記憶知識庫總索引地圖
│   ├── OBSIDIAN_MEMORY_SYNC.md               # 雙向同步實施計畫
│   ├── UI_TRADITIONAL_CHINESE_LOCALIZATION_PLAN.md # 繁體中文化計畫
│   ├── CONTEXT_DIGESTION_GATEWAY_FEASIBILITY.md    # 消化閘道可行性分析
│   ├── context_digest_phase2_design.md       # Phase 2 原生 ActionSink 設計
│   ├── context_digest_phase3_design.md       # Phase 3 RAG 檢索增強設計
│   └── context_digest_workflow.md            # 上下文消化工作流
└── 02-Conversation-Digest/                   # M3: 對話脈絡摘要（去蕪存菁、決策提煉）
    ├── INDEX.md                              # 歷史對話討論串日期總索引
    ├── 2026-10-24_ui-localization.md         # UI 在地化開工討論
    ├── 2026-10-24_context-digestion-gateway.md # 消化閘道與 Phase 2/3 研討 (41 則討論串)
    ├── 2026-10-25_mobile-control-feasibility.md# 行動端控制探討
    ├── 2026-10-25_obsidian-second-brain-integration.md # 第二大腦整合與 Wax 誕生 (22 則討論串)
    └── 2026-10-25_m1-m2-m3-execution-and-verification.md # 429 限流調查與交付推進
```

---

## 🚀 執行指南

### 1. 單次執行（立即全域同步驗收）
```powershell
node "C:\Users\user\.buzz\PLANS\sync_obsidian\main.mjs"
# 或
node "C:\Users\user\.buzz\PLANS\sync_obsidian\main.mjs" --once
```

### 2. 背景常駐服務（定時輪詢）
```powershell
node "C:\Users\user\.buzz\PLANS\sync_obsidian\main.mjs" --daemon
```
* 自動按設定檔輪詢：Core Memory 每 300 秒、Cold Memory 每 600 秒、Digest 每 1800 秒。
* 具備嚴格的實質內容差異檢驗（Idempotency），無實質變更時跳過 Git Commit，防止空提交干擾版控。

---

## 🧭 Agent 如何讀取整理後的記憶（防失憶機制）

1. **核心記憶導航雷達**：
   在 `00-Core-Memory/core-memory.md` 中注入了直通冷記憶與對話摘要的導航指引。
2. **總索引尋標**：
   Agent 調用 `read_file` 讀取 `01-Cold-Memory/INDEX.md` 或 `02-Conversation-Digest/INDEX.md`，即可在 1 個請求內掌握所有沉澱的技術規劃與歷史決策結論。
3. **雙向圖譜網絡**：
   所有 Markdown 筆記均標註 `[[WikiLink]]` 雙向連結，Obsidian Graph View 自動串接整體知識圖譜。
