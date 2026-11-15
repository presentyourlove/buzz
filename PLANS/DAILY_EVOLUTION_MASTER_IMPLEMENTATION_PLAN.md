# 🚀 每日系統進化機制總體實作計畫書 (Master Implementation Plan)

> **版本**：v2.0 (全方位整合版)  
> **發布日期**：2026-11-10  
> **整合來源**：
> 1. `DAILY_EVOLUTION_IMPLEMENTATION_PLAN.md` (7 大面向功能規格)
> 2. `DAILY_EVOLUTION_PROPOSAL_20261109.md` (演進引擎閉環架構與排程)  
> **目標執行特工**：Pollen (代碼實作)  
> **審核與驗收特工**：Sentry (安全與品質門禁)  
> **統籌企劃**：Honey2 / Antigravity

---

## 🧭 一、 總體架構與整合核心原則

本計畫將 **「7 大面向專項模組」** 作為功能基石，無縫嵌入 **「每日演進引擎 (Evolution Engine)」** 的閉環生命週期中，形成「採集 ➔ 分析 ➔ 體檢 ➔ 看板 ➔ 同步 ➔ 熔斷 ➔ 調度」的標準管線。

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       DAILY EVOLUTION ENGINE (統一排程引擎)                  │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. 稽核採集 (Audit & Collect)   ──▶ 01_work_discipline_guard.py (開工紀律) │
│                                  ──▶ 03_bottleneck_rca.py (日誌瓶頸RCA)    │
│                                                                             │
│  2. 共識沉澱 (Consensus Ingest)  ──▶ 02_consensus_ingestion.py (規範入庫)   │
│                                                                             │
│  3. 代碼體檢 (Health Check)      ──▶ 04_code_health_check.py (✅ 已完成)     │
│                                                                             │
│  4. 協同看板 (Coordination)      ──▶ 05_task_sync_board.py (任務對齊看板)   │
│                                                                             │
│  5. 記憶同步 (Knowledge Sync)    ──▶ 06_knowledge_sync.py (Obsidian雙腦同步)│
│                                                                             │
│  6. 門禁自癒 (Safety & CI/CD)    ──▶ 07_cicd_self_heal.py (工作流熔斷保護) │
│                                                                             │
│  7. 統一進入點 (Orchestrator)    ──▶ run_daily_evolution.py (聚合CLI)       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 💡 關鍵優化亮點 (避免踩坑)
1. **防止重複造輪子**：`04_code_health_check.py` 已經在 Phase 4/5 完成修復與驗收（含大輸出截斷、`cargo check` 綠燈），**Pollen 嚴禁改動或重寫 04 腳本**，直接納入管線調用。
2. **目錄收斂一體化**：統一實作於 `c:\私人\ai agent\buzz\scripts\daily_evolution\`，輸出暫存集中於 `C:\Users\user\.buzz\OUTBOX\`。
3. **分批實裝防雪球 (Batching)**：為防止 Pollen 處理過多檔案導致上下文超過 50,000 tokens 觸發卡頓，任務切分為 **3 個獨立工單批次**。
4. **Windows 編碼與路徑安全**：所有 Python 檔案操作強制指定 `encoding="utf-8"`，路徑使用 `pathlib.Path`，避免硬編碼反斜線。

---

## 📦 二、 模組詳細實作規格清單

### 【Batch 1：紀律稽核與共識 RCA (優先推進)】

#### 1. `01_work_discipline_guard.py` — 標準開工紀律守門員
- **路徑**：`scripts/daily_evolution/01_work_discipline_guard.py`
- **職責**：
  - 掃描 `C:\Users\user\.buzz\PLANS\` 中狀態為 `status: active` 的計畫書。
  - 檢核四要素：`buzz projects create` 綁定、`buzz issues create` 工單、Sentry 審核標記、主理人明確授權。
  - 偵測違規：未授權私自立項、未經 Sentry 審核私自開工、特工職責越界。
  - 輸出標準 JSON 格式違規報告。
- **驗收標準**：`python 01_work_discipline_guard.py --json-only` 輸出合法 JSON，掃描耗時 < 3 秒。

#### 2. `02_consensus_ingestion.py` — 深度報告轉專案規範 (補充空檔案)
- **路徑**：`scripts/daily_evolution/02_consensus_ingestion.py`
- **現狀**：目前實體檔案為 0 bytes，需補齊完整代碼。
- **職責**：
  - 讀取深度研究報告（如 `ANYMODELBOT_OPTION_B_ANALYSIS.md` 或任意 Markdown 提案）。
  - 自動抽取：架構決策 (ADR)、API 契約、資料模型、遷移風險。
  - 依 `daily_evolution_schema.json` 產出標準化規格書至 `C:\Users\user\.buzz\PLANS\`。
- **驗收標準**：`python 02_consensus_ingestion.py --input <path> --dry-run` 解析 Markdown 表格與代碼區塊成功。

#### 3. `03_bottleneck_rca.py` — 異常事件與 5-Why 根因分析
- **路徑**：`scripts/daily_evolution/03_bottleneck_rca.py`
- **職責**：
  - 掃描 `C:\Users\user\.buzz\WORK_LOGS\` 與特工日誌。
  - 擷取 `ERROR`、`FAILED`、`TIMEOUT`、`CIRCUIT_BREAK` 異常。
  - 自動套用 5-Why 結構化模板（包含事件 ID、症狀、時間軸、根因、糾正與預防措施）。
  - 產出 `C:\Users\user\.buzz\RESEARCH\RCA_YYYYMMDD.md` 並維護索引 `RCA_INDEX.json`。
- **驗收標準**：50MB 日誌 5 秒內完成檢索，無日誌時優雅退出。

---

### 【Batch 2：協同看板與知識同步】

#### 4. `04_code_health_check.py` — 代碼體檢與編譯防護
- **狀態**：**✅ 已完成 (COMPLETED)**
- **說明**：包含 `cargo check`、輸出截斷控制 (`--max-output-size`)、失敗繼續 (`--continue-on-fail`) 與 JSON 評分卡。**直接保留，無需修改！**

#### 5. `05_task_sync_board.py` — 活躍成員進度對齊看板
- **路徑**：`scripts/daily_evolution/05_task_sync_board.py`
- **職責**：
  - 調用 `buzz issues list --channel <project-channel>` 查詢當前工單。
  - 查詢特工在線狀態（Fizz, Honey, Honey2, Pollen, Sentry, Wax）。
  - 自動渲染 Markdown 狀態看板（分為 `To Do` / `In Progress` / `Done`）。
  - 自動警示：超過 48 小時無更新任務、指派特工離線、依賴阻塞標記。
  - 輸出至 `C:\Users\user\.buzz\OUTBOX\TASK_SYNC_YYYYMMDD.md`。
- **驗收標準**：支援 `--channel` 參數，若無 CLI 環境支援 Mock/離線模式不當機。

#### 6. `06_knowledge_sync.py` — 工作日誌同步至 Obsidian 第二大腦
- **路徑**：`scripts/daily_evolution/06_knowledge_sync.py`
- **職責**：
  - 讀取環境變數 `OBSIDIAN_VAULT_PATH`（預設 `D:/ObsidianVault` 或 `c:/私人/wiki`）。
  - 同步來源：`.buzz/WORK_LOGS/`、`.buzz/PLANS/`、`.buzz/RESEARCH/`。
  - 策略：以 SHA256 雜湊與修改時間判定增量單向推送。
  - 衝突保護：遇到衝突檔案自動備份至 `.conflicts/`，絕不覆寫損壞筆記。
- **驗收標準**：千檔增量掃描 < 10 秒，產出結構化同步報告 JSON。

---

### 【Batch 3：自癒熔斷與統一調度引擎】

#### 7. `07_cicd_self_heal.py` — 工作流自癒與三層熔斷
- **路徑**：`scripts/daily_evolution/07_cicd_self_heal.py`
- **職責**：
  - 檢查 `.github/workflows/`、`Cargo.toml` 與環境完整性。
  - 實作三層防護：
    1. **編譯層**：`cargo check` 失敗時即時阻斷。
    2. **測試層**：單元測試異常連續發生時標記熔斷狀態。
    3. **部署層**：CLI 非零退出碼時發出阻斷告警。
  - 產出修復建議與 diff 報告至 `OUTBOX/`。
- **驗收標準**：`python 07_cicd_self_heal.py --check` 能準確辨識工作流規則合法性。

#### 8. `run_daily_evolution.py` — 統一進入點 CLI 與演進排程
- **路徑**：`scripts/daily_evolution/run_daily_evolution.py`
- **職責**：
  - 循序排程調用 01 ~ 07（跳過或正常執行 04）。
  - 聚合所有子腳本的 JSON 報告，輸出全日匯總：
    - `OUTBOX/DAILY_EVOLUTION_REPORT_YYYYMMDD.json`
    - `OUTBOX/DAILY_EVOLUTION_SUMMARY_YYYYMMDD.md`
  - 支援命令列參數：
    `--date`、`--dry-run`、`--skip [01,02...]`、`--only [01,04...]`、`--json-only`。
- **驗收標準**：`python run_daily_evolution.py --dry-run` 可在 30 秒內全線走通並產出摘要報告。

---

## 🛠️ 三、 Pollen 派工執行指南 (主理人一鍵複製指令)

當主理人向特工蜂群下達任務時，建議分批推進以達到最佳執行效率與穩定度：

### 推薦發布話術：
```text
@Pollen 請依照最新整合實作計畫書實作每日系統進化機制：
📄 計畫書路徑：C:\Users\user\.buzz\PLANS\DAILY_EVOLUTION_MASTER_IMPLEMENTATION_PLAN.md

【執行要點】：
1. 04_code_health_check.py 已經完工驗收，嚴禁重寫或覆蓋！
2. 請優先實作 Batch 1 (01_work_discipline_guard.py, 02_consensus_ingestion.py, 03_bottleneck_rca.py)。
3. 完成後請通知 @Sentry 驗收，再依序推進 Batch 2 與 Batch 3。
```

