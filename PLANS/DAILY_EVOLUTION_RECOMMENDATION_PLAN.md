---
title: "每日進化建議書 實作計畫書"
tags: [daily-evolution, agent, pulse, notes, mvp]
status: draft
created: 2026-10-30
---

# 每日進化建議書 實作計畫書 (MVP 輕量版)

## 1. 背景與目標

**需求來源**：老闆要求「每日進化建議書」——由 Agent 每天自動產出一份針對當日專案進展、技術債、風險點、優化建議的結構化報告，發佈到 Buzz 頻道供團隊參考。

**核心目標 (MVP)**：
- 以現有 Buzz 基礎設施 (Pulse + Notes + CLI) 為骨幹，**零新增外部依賴**，在 1 週內交付可用版本
- 由指定 Agent (如 `evolution-advisor`) 每日定時執行，產出 Markdown 格式建議書
- 發佈為 NIP-23 `kind:30023` 長文筆記，並同步在 Pulse「Agents」分頁顯示
- 透過 `buzz notes set` 實現冪等更新 (同一 slug 每日覆寫)，保留歷史版本可追溯

**非目標 (留待第二期)**：
- 複雜的多 Agent 協作編寫、RAG 檢索代碼庫、LLM 自動分析 Git diff
- Webhook/排程器 Daemon、分布式鎖、Redis 等重型基建

---

## 2. 架構設計 (KISS 原則)

### 2.1 整體流程圖

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────────┐
│  系統排程       │────▶│  Evolution Agent │────▶│  Buzz Relay         │
│  (cron /        │     │  (buzz-agent     │     │  - kind:30023 note  │
│   Task Scheduler)     │   內建 ACP)      │     │  - kind:9 pulse msg │
└─────────────────┘     └──────────────────┘     └─────────────────────┘
                                │                        │
                                ▼                        ▼
                        ┌──────────────────┐     ┌─────────────────────┐
                        │  本地上下文蒐集   │     │  桌面端呈現         │
                        │  - git log --since│     │  - Pulse Agents Tab │
                        │  - cargo check    │     │  - Notes 列表/詳情  │
                        │  - 既有 skills    │     │  - AgentActivityCard│
                        └──────────────────┘     └─────────────────────┘
```

### 2.2 關鍵設計決策

| 決策點 | 選擇 | 理由 |
|--------|------|------|
| **排程器** | 系統原生 `cron` / Windows Task Scheduler | 零依賴、穩定、可被 `systemd`/`launchd` 管理 |
| **Agent 執行方式** | `buzz-agent` ACP 進程 + `--prompt` 非互動模式 | 復用既有 Agent 基礎設施，無需新增 Daemon |
| **資料輸出格式** | NIP-23 `kind:30023` (Notes) + `kind:9` (Pulse 通知) | 復用現有 `buzz notes set` / `buzz messages send` CLI |
| **Slug 命名** | `daily-evolution/YYYY-MM-DD` | 符合 `mem/` slug 文法，按日期分組，易於查詢 |
| **上下文蒐集** | 僅調用現有 CLI/工具 (`git`、`cargo`、`buzz mem`) | 避免引入 RAG/向量資料庫，符合 YAGNI |

---

## 3. 實作細項

### 3.1 Agent 定義 (`evolution-advisor`)

**建立方式**：`buzz agents draft-create` (Owner-reviewed)

```json
{
  "display_name": "Evolution Advisor",
  "system_prompt": "你是 Buzz 專案的每日進化顧問。每天執行一次，蒐集：\n1. 最近 24h git 提交摘要 (git log --since=\"24 hours ago\" --oneline)\n2. 專案建置狀態 (cargo check 2>&1 | tail -20)\n3. 現有技能庫摘要 (buzz mem ls)\n4. 開放 Issue/PR 數量 (buzz issues list --state open)\n\n產出結構化 Markdown 報告，包含：\n- ## 執行摘要 (3-5 行)\n- ## 代碼變更概覽\n- ## 建置/測試狀態\n- ## 風險與技術債\n- ## 具體改進建議 (優先級 P0/P1/P2)\n- ## 明日關注點\n\n語言：繁體中文。輸出純 Markdown，無額外解釋。",
  "model": "claude-opus-4-1",
  "tools": ["shell", "read_file", "str_replace", "buzz-cli"]
}
```

**代碼出處參考**：
- `c:\私人\ai agent\buzz\desktop\src\features\agents\ui\` - Agent 管理 UI
- `c:\私人\ai agent\buzz\crates\buzz-cli\src\commands\agents.rs` - CLI 建立 Agent 指令
- `c:\私人\ai agent\buzz\crates\buzz-agent\src\config.rs` - Agent 設定結構

### 3.2 每日執行腳本 (`scripts/daily_evolution.sh`)

```bash
#!/usr/bin/env bash
set -euo pipefail

# 環境變數 (由排程器注入)
# BUZZ_RELAY_URL, BUZZ_PRIVATE_KEY, BUZZ_AUTH_TAG
# EVOLUTION_CHANNEL_ID (目標頻道 UUID)
# EVOLUTION_AGENT_PUBKEY (evolution-advisor 的 pubkey)

DATE=$(date -u +"%Y-%m-%d")
SLUG="daily-evolution/${DATE}"
WORKDIR="c:/私人/ai agent/buzz"
OUTBOX="${HOME}/.buzz/OUTBOX"

cd "${WORKDIR}"

# 1. 蒐集上下文 (輸出到暫存檔)
CONTEXT_FILE=$(mktemp)
{
  echo "=== Git 最近 24h 提交 ==="
  git log --since="24 hours ago" --oneline -20 2>/dev/null || echo "無法讀取 git log"

  echo -e "\n=== Cargo Check 狀態 ==="
  cargo check 2>&1 | tail -30 || echo "建置檢查失敗"

  echo -e "\n=== 開放 Issues ==="
  buzz issues list --state open --limit 10 2>/dev/null || echo "無法查詢 issues"

  echo -e "\n=== 記憶庫清單 ==="
  buzz mem ls 2>/dev/null || echo "無記憶庫"
} > "${CONTEXT_FILE}"

# 2. 呼叫 Agent 產出報告 (ACP 非互動模式)
# 注意：buzz-agent 目前透過 ACP 協議，這裡用 CLI 包裝
REPORT_FILE=$(mktemp)
cat "${CONTEXT_FILE}" | buzz agent run \
  --agent "${EVOLUTION_AGENT_PUBKEY}" \
  --prompt "根據以下專案上下文，產出每日進化建議書：" \
  --output-format markdown \
  > "${REPORT_FILE}" 2>&1 || {
    echo "Agent 執行失敗" >&2
    exit 1
  }

# 3. 發佈為 Note (冪等 upsert)
buzz notes set \
  --name "${SLUG}" \
  --title "每日進化建議書 - ${DATE}" \
  --tag "daily-evolution" \
  --tag "auto-generated" \
  --content - < "${REPORT_FILE}"

# 4. 發佈 Pulse 通知訊息 (引用 Note)
NOTE_NADDR=$(buzz notes get --name "${SLUG}" --format compact | grep "^naddr" | cut -d' ' -f2)
buzz messages send \
  --channel "${EVOLUTION_CHANNEL_ID}" \
  --content "📋 **每日進化建議書 ${DATE}** 已發布\n\n🔗 詳細內容：${NOTE_NADDR}\n\n#daily-evolution #auto-generated" \
  --kind 9

# 清理
rm -f "${CONTEXT_FILE}" "${REPORT_FILE}"
```

**關鍵 CLI 指令參考**：
- `c:\私人\ai agent\buzz\crates\buzz-cli\src\commands\notes.rs:487` - `cmd_set` 冪等 upsert 邏輯
- `c:\私人\ai agent\buzz\crates\buzz-cli\src\commands\messages.rs:578` - `cmd_send_message` 發佈 Pulse 訊息
- `c:\私人/ai agent/buzz/crates/buzz-agent/src/agent.rs` - Agent 執行入口

### 3.3 前端呈現 (零代碼變更)

現有 Pulse 「Agents」分頁已自動顯示 Agent 發佈的 `kind:9` 訊息：
- `c:\私人\ai agent\buzz\desktop\src\features\pulse\ui\PulseView.tsx:153-156` - `agentTimelineQuery` 查詢 Agent 訊息
- `c:\私人\ai agent\buzz\desktop\src\features\pulse\ui\AgentActivityCard.tsx` - Agent 訊息卡片渲染
- `c:\私人\ai agent\buzz\desktop\src\features\pulse\lib\groupAgentNotes.ts` - 同一 Agent 訊息分組

Notes 列表頁可透過 `buzz notes ls --tag daily-evolution` 篩選歷史建議書。

---

## 4. 實作任務拆解 (MVP - 5 工作天)

| 任務編號 | 任務名稱 | 負責角色 | 預估工時 | 驗收標準 |
|----------|----------|----------|----------|----------|
| **T01** | 建立 `evolution-advisor` Agent Draft | @Honey2 | 0.5h | `buzz agents draft-create` 成功，Owner 可在 Desktop 審核 |
| **T02** | 撰寫每日執行 Shell 腳本 | @Pollen | 2h | 腳本可手動執行產出報告、發佈 Note、發送 Pulse 訊息 |
| **T03** | 設定系統排程 | @Pollen | 0.5h | cron/Task Scheduler 每日 02:00 UTC 自動執行 |
| **T04** | 驗證 Pulse/Notes 呈現 | @Honey | 0.5h | 桌面端 Agents 分頁顯示訊息、Notes 列表可篩選 |
| **T05** | 文件化操作手冊 | @Honey2 | 1h | `GUIDES/DAILY_EVOLUTION_OPS.md` 完成 |

**總計**：~4.5h 實際工時，1 週內可完成 (含審核緩衝)

---

## 5. 回滾機制

若 MVP 上線後出現嚴重問題，執行以下指令即可完全回滾：

```bash
# 1. 刪除 Agent Draft (若未核准) 或停用 Agent
buzz agents draft-update --channel <channel-uuid> --display-name "Evolution Advisor" --system-prompt "DISABLED"  # 或由 Owner 在 Desktop 刪除

# 2. 移除系統排程
# Linux/macOS:
crontab -l | grep -v "daily_evolution.sh" | crontab -
# Windows:
schtasks /Delete /TN "BuzzDailyEvolution" /F

# 3. 刪除已發佈的 Notes (可選，保留歷史建議不刪)
# buzz notes rm --name "daily-evolution/2026-10-30"  # 單日刪除

# 4. 確認無殘留進程
pkill -f "daily_evolution.sh" 2>/dev/null || true
```

---

## 6. 驗收測試案例

| 測試案例 | 步驟 | 預期結果 |
|----------|------|----------|
| **TC01** 手動執行腳本 | `./scripts/daily_evolution.sh` | Note 建立成功，Pulse 訊息發送成功，回傳 exit code 0 |
| **TC02** 重複執行同一天 | 同一天再跑一次腳本 | Note 內容更新 (保留相同 slug)，`published_at` 不變，`updated_at` 更新 |
| **TC03** Pulse Agents 分頁 | 開啟桌面端 → Pulse → Agents 分頁 | 看到 `evolution-advisor` 卡片，點開可見建議書摘要 |
| **TC04** Notes 歷史查詢 | `buzz notes ls --tag daily-evolution --limit 7` | 列出最近 7 天建議書，按 `updated_at` 遞減排序 |
| **TC05** 排程自動觸發 | 等待排程時間或手動觸發排程 | 無人值守完成 TC01 所有動作，日誌可追蹤 |

---

## 7. 風險與對策

| 風險 | 機率 | 影響 | 對策 |
|------|------|------|------|
| Agent 執行超時/失敗 | 中 | 當天無建議書 | 腳本設定 `timeout 300`，失敗時發送 Pulse 錯誤通知 |
| Relay 暫時不可用 | 低 | 發佈失敗 | CLI 內建重試；腳本捕獲錯誤寫入本地日誌，下次重試 |
| Slug 衝突 (同日多次) | 低 | 資料覆蓋 | `buzz notes set` 冪等語義保證同 slug 只保留最新版 |
| 內容品質不佳 | 中 | 團隊不信任 | MVP 階段人工審核前 3 份；後續引入評分機制 (第二期) |

---

## 8. 第二期擴展規劃 (參考，不在 MVP 範圍)

- **RAG 增強**：接入 `buzz-search` 語義搜尋，自動檢索相關代碼變更上下文
- **多 Agent 協作**：`code-analyst` + `architect` + `evolution-advisor` 串聯
- **排程器服務化**：內建 `buzz-workflow` 定時觸發，支援多時區、失敗重試、執行歷史
- **儀表板視覺化**：Pulse 新增「每日進化」專屬 Tab，圖表化趨勢指標

---

## 9. 結論

本計畫採用 **極度精簡的 MVP 策略**：
- ✅ **零新依賴** - 全部復用現有 Buzz 基礎設施
- ✅ **零前端改動** - 現有 Pulse/Notes UI 即可呈現
- ✅ **標準化輸出** - NIP-23 Notes + kind:9 Pulse，符合協議規範
- ✅ **可審計可回滾** - 完整指令級回滾機制
- ✅ **1 週交付** - 4.5h 實際工時，風險可控

**請 @Sentry 進行架構完備性與品質審核。**

---

*計畫書版本：v1.0 (MVP)*  
*作者：Honey2 (企劃官)*  
*審核狀態：待 Sentry 審核*
