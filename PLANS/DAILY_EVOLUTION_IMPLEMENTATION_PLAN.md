---
title: "DAILY_EVOLUTION_IMPLEMENTATION_PLAN"
tags: [daily-evolution, implementation-plan, system-health, ci-cd, agent-discipline]
status: active
created: 2026-11-08
---

# 每日系統進化建議實作計畫書

## 來源依據
- 參考訊息：`buzz://message?channel=af946a63-dd43-4050-baea-65140d47fc42&id=48865efa201651618fb6a25d4a09d92f39d68578fd8a6cdc484fe6865bee41f4&thread=dd30e5042cb439cec28e2a62aca9f2b107ba6972e2fcdb3dfb619a4a8346c98d`
- 來源 Agent：Honey2 (每日系統進化建議)
- 核心訴求：落實 7 大面向之系統級改善與自動化

---

## 實作範圍與分期策略 (Phased Delivery)

本計畫依 KISS 與 YAGNI 原則，拆分為 **第一期 MVP 輕量落地版** 與 **第二期 企業高可用版**。預設僅交付第一期，第二期視第一期驗收後由主理人另行授權。

| 階段 | 目標 | 交付物 | 預估工時 |
|------|------|--------|----------|
| **第一期 MVP** | 建立可執行的每日進化流程骨架，包含：開工紀律檢核、共識入庫自動化、瓶頸 RCA 模板、代碼體檢基線、任務同步看板、知識同步腳本、CI/CD 熔斷基礎設施 | 7 支獨立可執行腳本 + 1 個統一進入點 CLI + 文檔化操作手冊 | ~2-3 人日 |
| **第二期 企業版** | 全自動化排程、多專案隔離、歷史趨勢儀表板、智能告警、Agent 行為審計日誌 | 完整 Daemon 服務 + Web 儀表板 + 多租戶支援 | 視第一期驗收後另行規劃 |

---

## 七大面向實作規格 (第一期 MVP)

### 1. 落實開工紀律 — 標準派工流程守門員
**檔案**：`scripts/daily_evolution/01_work_discipline_guard.py`
**職責**：
- 掃描 `C:\Users\user\.buzz\PLANS\` 中所有 `status: active` 計畫書
- 驗證每份計畫書是否具備：`buzz projects create` 專案綁定、`buzz issues create` 工單建立、Sentry 審核通過記錄、主理人明確授權指令
- 偵測「未經授權擅自立項」、「Sentry 審核前私自開工」、「Agent 職責混淆」三大違規模式
- 輸出 JSON 格式違規報告，供統一進入點匯總

**驗收標準**：
- [ ] 能正確解析 100% 現有 PLANS 目錄下的 YAML frontmatter
- [ ] 違規偵測準確率 ≥ 95%（以人工複核 20 筆歷史計畫為基準）
- [ ] 執行時間 < 3 秒

---

### 2. 共識入庫 — AnyModelBot 方案B 深度分析報告轉專案規範
**檔案**：`scripts/daily_evolution/02_consensus_ingestion.py`
**職責**：
- 讀取 `C:\Users\user\.buzz\RESEARCH\ANYMODELBOT_OPTION_B_ANALYSIS.md`（若存在）或指定路徑
- 自動抽取：架構決策、API 契約、資料模型、遷移策略、風險登記
- 依 `daily_evolution_schema.json` 產出標準化規範文件至 `C:\Users\user\.buzz\PLANS\ANYMODELBOT_OPTION_B_SPEC.md`
- 自動建立對應的 `buzz projects create` / `buzz issues create` 指令腳本（乾跑模式，輸出至 `OUTBOX/`）

**驗收標準**：
- [ ] 能處理 Markdown 表格、代碼區塊、Mermaid 圖表等複雜結構
- [ ] 產出規範文件通過 `buzz projects create --dry-run` 語法檢查
- [ ] 關鍵決策點追溯性 100%（每條規範標註源報告行號）

---

### 3. 瓶頸 RCA — 根因分析模板化與自動化
**檔案**：`scripts/daily_evolution/03_bottleneck_rca.py`
**職責**：
- 讀取 `C:\Users\user\.buzz\WORK_LOGS\` 當日所有 `*.md` 日誌
- 以關鍵字（`ERROR`、`FAILED`、`BLOCKED`、`TIMEOUT`、`RETRY`、`CIRCUIT_BREAK`）掃描異常事件
- 套用 **5 Why 模板** 產出結構化 RCA 紀錄：
  ```yaml
  event_id: "YYYYMMDD-HHMMSS-<short-hash>"
  symptom: "..."
  timeline: [...]
  root_cause: "..."
  corrective_action: "..."
  preventive_action: "..."
  tags: [ci, agent, network, dependency, resource]
  ```
- 寫入 `C:\Users\user\.buzz\RESEARCH\RCA_YYYYMMDD.md` 並更新 `RCA_INDEX.json`

**驗收標準**：
- [ ] 能從 50MB 日誌中在 5 秒內完成掃描
- [ ] 模板欄位 100% 非空（強制填寫機制）
- [ ] 歷史 RCA 可由 `RCA_INDEX.json` 快速檢索

---

### 4. 代碼體檢 — 邊界防護與整合驗證基線
**檔案**：`scripts/daily_evolution/04_code_health_check.py`
**職責**：
- 執行 `cargo check --workspace --all-targets` 捕獲型別錯誤
- 執行 `cargo clippy --workspace --all-targets -- -D warnings` 強制零警告
- 執行 `cargo test --workspace --lib --bins -- --test-threads=4` 單元測試全跑
- 檢查 `buzz-sdk`、`buzz-agent`、`buzz-core` 三大核心 crate 的公共 API 破壞性變更（對比 `git diff HEAD~1`）
- 產出健康度評分卡（JSON）：`{compile: pass/fail, clippy: pass/fail, test: pass/fail, api_break: true/false, score: 0-100}`

**驗收標準**：
- [ ] 單次執行 < 120 秒（含完整測試）
- [ ] API 破壞偵測零漏報（對比手動 `cargo public-api` 基準）
- [ ] 評分卡可作為 CI 門檻判斷依據

---

### 5. 任務同步 — 活躍成員進度對齊看板
**檔案**：`scripts/daily_evolution/05_task_sync_board.py`
**職責**：
- 查詢 `buzz issues list --channel <project-channel> --status open` 所有專案工單
- 交叉比對 `buzz agents list` 當前線上 Agent 與指派關係
- 產出三欄看板：`To Do` / `In Progress` / `Done`（按 `updated_at` 排序）
- 標記「超過 48h 無更新」、「指派 Agent 離線」、「依賴阻塞」三種異常狀態
- 輸出 Markdown 看板至 `C:\Users\user\.buzz\OUTBOX\TASK_SYNC_YYYYMMDD.md`

**驗收標準**：
- [ ] 看板資料即時性 ≤ 5 分鐘延遲
- [ ] 異常標記準確率 100%（人工抽樣驗證）
- [ ] 支援多專案頻道並行顯示

---

### 6. 知識同步 — 工作日誌同步至 Obsidian 第二大腦
**檔案**：`scripts/daily_evolution/06_knowledge_sync.py`
**職責**：
- 讀取環境變數 `OBSIDIAN_VAULT_PATH`（預設 `D:/ObsidianVault`）
- 同步來源：`C:\Users\user\.buzz\WORK_LOGS\`、`C:\Users\user\.buzz\PLANS\`、`C:\Users\user\.buzz\RESEARCH\`、`C:\Users\user\.buzz\GUIDES\`
- 策略：**單向增量推送**（本地 → Obsidian），以 `mtime` + `sha256` 判斷變更
- 保留目錄結構，檔名統一轉為 `YYYYMMDD_HHMMSS_<slug>.md`
- 產出同步報告：`{synced: N, skipped: M, conflicts: K, errors: []}`

**驗收標準**：
- [ ] 1000 檔案同步 < 10 秒
- [ ] 零資料遺失（對比雙端檔案數與雜湊）
- [ ] 衝突檔案自動備份至 `.conflicts/` 不覆寫

---

### 7. CI/CD 自癒 — 建置腳本與守護程式熔斷機制優化
**檔案**：`scripts/daily_evolution/07_cicd_self_heal.py`
**職責**：
- 讀取 `.github/workflows/*.yml`、`lefthook.yml`、`renovate.json`
- 註入/更新三層熔斷規則：
  1. **編譯層**：`cargo check` 失敗 → 立即阻斷合併，發送 Nostr 通知 `@主理人`
  2. **測試層**：單元測試連續 3 次失敗（同一 commit SHA）→ 自動標記 `circuit_break: true`，暫停後續部署
  3. **部署層**：`buzz` CLI 部署指令非零退出碼 → 自動回滾至上一個綠色標籤
- 產出修補後的工作流檔案至 `OUTBOX/.github/workflows/`，附帶 `diff` 檔供人工審閱

**驗收標準**：
- [ ] 熔斷邏輯可在本地用 `act` 模擬觸發驗證
- [ ] 通知送達率 100%（Nostr 事件簽名驗證通過）
- [ ] 回滾操作冪等且可在 30 秒內完成

---

## 統一進入點 CLI

**檔案**：`scripts/daily_evolution/run_daily_evolution.py`
**用法**：
```bash
python scripts/daily_evolution/run_daily_evolution.py \
    --date 2026-11-08 \
    --out-dir C:\Users\user\.buzz\OUTBOX\DAILY_EVOLUTION_20261108 \
    --obsidian-vault D:/ObsidianVault \
    --notify-pubkey 2d29f28fd5106795c630120d7e5b062bc4f5caa8057b4a6a374b7468cdc5de8e \
    --dry-run false
```

**行為**：
1. 依序執行 01-07 七支腳本，捕獲各自 stdout/stderr 與退出碼
2. 聚合為單一 `DAILY_EVOLUTION_REPORT_YYYYMMDD.json`
3. 產出人類可讀的 `DAILY_EVOLUTION_SUMMARY_YYYYMMDD.md`
4. 若 `--dry-run false` 且有異常（退出碼 ≠ 0 或熔斷觸發），自動發送 Nostr Kind:1 通知至主理人

---

## 目錄結構落地圖

```
C:\Users\user\.buzz\
├── PLANS\
│   └── DAILY_EVOLUTION_IMPLEMENTATION_PLAN.md     ← 本檔
├── scripts\
│   └── daily_evolution\
│       ├── __init__.py
│       ├── run_daily_evolution.py                 ← 統一進入點
│       ├── 01_work_discipline_guard.py
│       ├── 02_consensus_ingestion.py
│       ├── 03_bottleneck_rca.py
│       ├── 04_code_health_check.py
│       ├── 05_task_sync_board.py
│       ├── 06_knowledge_sync.py
│       └── 07_cicd_self_heal.py
├── OUTBOX\
│   ├── DAILY_EVOLUTION_YYYYMMDD\
│   │   ├── DAILY_EVOLUTION_REPORT_YYYYMMDD.json
│   │   └── DAILY_EVOLUTION_SUMMARY_YYYYMMDD.md
│   └── .github\workflows\                         ← 熔斷修補輸出
└── RESEARCH\
    ├── RCA_YYYYMMDD.md
    └── RCA_INDEX.json
```

---

## 相依性與環境需求

| 相依項目 | 版本/來源 | 用途 |
|----------|-----------|------|
| Python | 3.11+ | 腳本執行環境 |
| `pynostr` | `pip install pynostr` | Nostr 事件簽名與發送 |
| `pyyaml` | `pip install pyyaml` | 解析 frontmatter |
| `buzz` CLI | 已安裝於 PATH | 查詢工單、專案、Agent 狀態 |
| `cargo` | 1.80+ | Rust 專案建置與測試 |
| `git` | 2.45+ | 版本控制操作 |
| `act` | 可選 | 本地 GitHub Actions 模擬 |

**環境變數**：
```bash
export BUZZ_RELAY_URL="wss://presentyourlove.communities.buzz.xyz"
export BUZZ_PRIVATE_KEY="<agent-hex-key>"
export BUZZ_AUTH_TAG="<owner-auth-tag>"
export OBSIDIAN_VAULT_PATH="D:/ObsidianVault"
```

---

## 回滾機制

若任一腳本執行導致系統不穩，執行以下指令還原：

```bash
# 1. 還原 OUTBOX 產出
rm -rf "C:\Users\user\.buzz\OUTBOX\DAILY_EVOLUTION_$(date +%Y%m%d)"

# 2. 還原 GitHub Actions 工作流（若已套用）
git -C "c:/私人/ai agent/buzz" checkout -- .github/workflows/

# 3. 還原 Obsidian 同步（若有衝突備份）
cp -r "D:/ObsidianVault/.conflicts/$(date +%Y%m%d)/*" "D:/ObsidianVault/" 2>/dev/null || true

# 4. 清理暫存 RCA 檔案
rm -f "C:\Users\user\.buzz\RESEARCH\RCA_$(date +%Y%m%d).md"
```

---

## 驗收清單 (Definition of Done)

| # | 項目 | 驗證方式 | 負責角色 |
|---|------|----------|----------|
| 1 | 七支腳本單獨執行零報錯 | `python 0X_*.python --help` + 實測 | Honey2 |
| 2 | 統一進入點端到端跑通 | `run_daily_evolution.py --dry-run true` | Honey2 |
| 3 | 違規偵渂準確率 ≥ 95% | 人工複核 20 筆歷史計畫 | Sentry |
| 4 | 代碼體檢評分卡可作 CI 門檻 | 注入 CI 驗證紅綠燈 | Pollen |
| 5 | Nostr 通知主理人送達 | 主理人確認收到 Kind:1 事件 | Honey2 |
| 6 | 文檔化操作手冊完整 | `OUTBOX/DAILY_EVOLUTION_HANDBOOK.md` 存在 | Honey2 |

---

## 後續行動

> **計畫書已就緒，請 @Sentry 進行架構完備性與品質審核。**

審核通過後，由主理人親口下達「同意/開工/實作」指令，再由 Pollen 進行代碼實作。
