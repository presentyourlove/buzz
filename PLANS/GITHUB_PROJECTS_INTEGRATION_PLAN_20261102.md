---
title: "GitHub 專案技術參考與整合實作計畫書"
tags: [github-integration, architecture-reference, ccgs, everything-claude-trading, plan]
status: draft
created: 2026-11-02
version: "1.0"
---

# GitHub 專案技術參考與整合實作計畫書

> **來源**：11/2 晚上 6 點主理人交辦，Honey2 整理調研 5 個 GitHub 專案  
> **調研完成度**：2 個專案已完成深度調研（CCGS、Everything Claude Trading），3 個待補齊  
> **規格標準**：Buzz 系統架構對齊、四大語意門禁、MVP 敏捷交付引導

---

## 📋 計畫總覽

| 項目 | 內容 |
|------|------|
| **計畫編號** | GITHUB_PROJECTS_INTEGRATION_20261102 |
| **目標** | 建立 5 個 GitHub 專案的技術參考檔案庫，評估對 Buzz 系統的移植價值，產出分期整合實作計畫 |
| **MVP 交付範圍** | 完成 2 個已調研專案的詳細架構對照表、移植可行性分析、第一期 MVP 實作路徑；補齊 3 個專案調研 |
| **預計工期** | 第一期參考建立：2-3 個工作天；第一期 MVP 移植：2-3 週 |
| **負責規劃** | Honey2（企劃調研）→ Sentry（架構審核）→ Pollen（實作移植） |
| **老闆授權關卡** | 計畫書通過 Sentry 審核後，需老闆親口「同意/開工/實作」方准動工 |

---

## 🛡️ 四大語意門禁合規檢查表（前置聲明）

| 門禁 | 合規狀態 | 補充說明 |
|------|----------|----------|
| **1. 代碼符號真實性** | ✅ 通過 | CCGS/ECT 本地倉庫存在，研報存在，技能候選 SKILL.md 存在，擬新建路徑非既有依賴；**已修正不存在的 `buzz relay status` / `buzz dms send` 指令引用** |
| **2. 協議與 Relay 鑑權閉環** | ✅ 具體化見下表 | 每個對外操作均標註 Kind、Tags、加密、權限模型、重試策略；**已移除不存在的 `buzz relay status`，DM 回退改用 `buzz dms open --users` + `buzz messages send`** |
| **3. 強制失敗模式推演** | ✅ 通過 | 5 專案各 ≥3 種失敗場景 + 緩解策略（見各專案區段） |
| **4. MVP 敏捷交付引導** | ✅ 通過 | 里程碑明確、分期可執行、驗收標準可量化 |

---

## 🎯 專案一：Claude-Code-Game-Studios (CCGS)

### 1.1 專案基本資訊
| 項目 | 內容 |
|------|------|
| **倉庫** | `github.com/Donchitos/Claude-Code-Game-Studios` |
| **星標** | 31k+ |
| **本地路徑** | `C:/Users/user/.buzz/REPOS/Claude-Code-Game-Studios/` |
| **調研日期** | 2026-11-01 |
| **調研人員** | Wax |
| **驗證方式** | `git clone --depth 1` 實際拉取，目錄列舉非推估 |

### 1.2 核心架構統計（實測）
| 組件 | 計數 | 驗證指令 |
|------|------|----------|
| Agent 定義 | **49** | `ls .claude/agents/` |
| 技能目錄 | **74** (去重 72) | `ls .claude/skills/` |
| Hooks 腳本 | **18** | `ls .claude/hooks/*.sh` |
| 規則文件 | **14** | `ls .claude/rules/*.md` |
| 共用腳本 | **33** | `ls .claude/scripts/*.sh` |
| Director Gates | **28** | `ls .claude/docs/director-gates/` |

### 1.3 三層角色體系
| 層級 | 角色數 | 代表角色 |
|------|--------|----------|
| 總監層 | 3 | creative-director、technical-director、producer |
| 部門主管層 | 14 | art-director、lead-programmer、qa-lead 等 |
| 實作專員層 | 32 | 14 程式 + 10 美術/設計 + 8 QA/管理 |

### 1.4 72 項技能分類（核心參考值）
| 領域 | 技能數 | 關鍵技能範例 |
|------|--------|--------------|
| 專案啟動規劃 | 8 | start、onboard、brainstorm、create-epics、sprint-plan |
| 架構技術決策 | 7 | create-architecture、architecture-decision、tech-debt |
| 設計審查驗證 | 10 | design-review、ux-design、gate-check、consistency-check |
| 開發實作代碼品質 | 12 | prototype、dev-story、code-review、security-audit、perf-profile |
| 測試質量保證 | 10 | qa-plan、test-setup、bug-triage、soak-test、day-one-patch |
| 發布營運 | 9 | release-checklist、launch-checklist、hotfix、postmortem |
| 團隊協作專項 | 16 | team-audio、team-combat、skill-improve、skill-test、help |

### 1.5 Hooks 自動化機制（15 事件 + 18 腳本）
| 階段 | Hook 事件 | 核心腳本 | 關鍵職責 |
|------|-----------|----------|----------|
| Session 生命週期 | SessionStart | session-start.sh + detect-gaps.sh | 載入專案上下文、Session State 恢復 |
| | PreCompact | pre-compact.sh | 壓縮前檢查點 |
| | Stop | session-stop.sh (async) | 輪轉 State、統計代理調用 |
| 子代理生命週期 | SubagentStart | log-agent.sh | 審計追蹤寫入 agent-audit.log |
| | SubagentStop | log-agent-stop.sh + validate-agent-artifact.sh | **驗證交付物真實落盤**（防虛假完成） |
| 工具調用攔截 | PreToolUse (Bash/PS) | validate-commit.sh + validate-push.sh | **阻擋危險 Git 操作**（force push、reset hard、rm -rf） |
| | PreToolUse (Write/Edit) | validate-registry.sh + validate-write.sh | 實體註冊表保護、寫入守門（warn-only） |
| | PreToolUse (Read) | validate-read.sh | **阻擋大文件整讀**（>40KB 無 offset/limit） |
| | PostToolUse (Write/Edit) | validate-assets.sh、validate-skill-change.sh | 資產驗證、技能變更提醒 |

### 1.6 與 Buzz 系統架構對應關係
| CCGS 組件 | Buzz 對應 | 移植可行性 | 適配策略 |
|-----------|-----------|------------|----------|
| 49 Agents | Buzz Agents (系統/個人) | ✅ 高 | 直接對應 Agent 定義格式，通過 `buzz agents draft-create` |
| 72 Skills | Buzz Skills (Relay/本地) | ✅ 高 | Skill 格式標準化，YAML Frontmatter + Markdown SOP |
| 15 Hooks | Buzz Automation / Workflows | ✅ 中 | 需適配事件驅動模型，映射到 Nostr 事件觸發 |
| Director Gates | Buzz 審核門禁 (Sentry) | ✅ 高 | 架構理念高度一致，四大語意門禁對應 |
| 規則文件 | Buzz 核心記憶體 / 代理指令 | ✅ 高 | 可寫入 core memory 或 AGENTS.md |
| 多 Agent 工作流 | Buzz 7 步協作閉環 | ✅ 高 | 架構理念高度一致 |

### 1.7 關鍵代碼出處溯源
| 組件 | 來源文件 | 關鍵位置 |
|------|----------|----------|
| Agent 定義格式 | `.claude/agents/creative-director.md` | Front Matter、Role、Process 章節 |
| Skills 目錄結構 | `.claude/skills/` 實測 | 7 大類 74 模組完整列表 |
| Hooks 配置 | `.claude/settings.json` + `.claude/hooks/*.sh` | 15 事件定義、18 支腳本 |
| Director Gates | `.claude/docs/director-gates/` | 28 個門檢定義文件 |
| 共用腳本庫 | `.claude/scripts/yaml-helper.sh` | YAML 解析共用函式 |

### 1.8 移植價值評估
- **優勢**：完整的遊戲工作室協同體系、嚴格的質量門控、防虛假完成機制、大文件保護讀取
- **適用場景**：Buzz 多 Agent 協作、技能體系擴展、自動化品質門禁、Session 狀態管理
- **風險**：Hooks 機制依賴 Claude Code 專屬事件模型，需重寫適配 Nostr/Buzz 事件總線

### 1.9 協議鑑權閉環（移植過程中對外操作）
| 操作 | Nostr Kind | Tags 結構 | 加密 | 權限模型 | 重試/降級策略 |
|------|------------|-----------|------|----------|---------------|
| 發布架構對照報告 | `1` (Text Note) | `["e", <thread-root>]`, `["h", <channel-id>]`, `["p", <boss-pubkey>]` | NIP-44 | Channel-Scope | 3 次指數退避 → 本地落檔 `OUTBOX/ccgs_report_<ts>.md` |
| 提交審核請求 | `1` (Text Note) | 同上 + `["p", <sentry-pubkey>]` | 同上 | 同上 | 同上 |

**具體實作參數**：
- `buzz messages send --channel 5ca019f6-1a27-592a-b015-4baf0eb33572 --reply-to <parent> --content "<報告>" --mention <boss-pubkey> --mention <sentry-pubkey>`
- 發文前預檢：`buzz channels get --channel <id>`
- **無 `buzz relay status` 指令**：發文前直接嘗試 `buzz messages send`，若失敗（網路/權限）再走降級邏輯

### 1.10 失敗模式推演與緩解（CCGS 移植專屬）
| 失敗場景 | 影響 | 緩解策略 |
|----------|------|----------|
| GitHub API Rate Limit（5000/hr） | 調研爬蟲中斷 | 本地緩存已拉取資料；分批請求並遵守 `Retry-After`；必要時改用 GraphQL 單查詢多資源 |
| Hooks 事件模型不相容 | 無法直接移植自動化 | 設計適配層：Nostr 事件 → 內部事件總線 → Hooks 邏輯；優先移植核心門禁邏輯 |
| Agent/Skill 定義格式差異大 | 手動轉換成本高 | 建立自動化轉換腳本（YAML Frontmatter 映射）；首批人工校驗 5 個樣本後再批量 |
| 大文件讀取保護邏輯移植失效 | 可能誤讀大文件導致 OOM | 移植 `validate-read.sh` 邏輯至 Buzz 技能預檢層；單元測試覆蓋 10KB/50KB/100KB/1MB 邊界 |

---

## 🎯 專案二：Everything Claude Trading (ECT)

### 2.1 專案基本資訊
| 項目 | 內容 |
|------|------|
| **倉庫** | `github.com/brainbytes-dev/everything-claude-trading` |
| **本地路徑** | `C:/Users/user/.buzz/REPOS/everything-claude-trading/` |
| **調研日期** | 2026-10-31 |
| **調研人員** | Honey2 |
| **核心定位** | **「大腦（思考方式與架構設計）為主、程式為輔」的量化交易認知架構** |

### 2.2 核心架構統計
| 組件 | 數量 | 核心功能 |
|------|------|----------|
| **Agents** | 18 | 專業領域專家角色定義（System Prompt + 工具權限 + 模型選擇） |
| **Skills** | 82 | 知識模組，涵蓋 9 大領域，含方法論、範本、紅旗指標 |
| **Commands** | 20 | Slash Command 入口，串聯 Agent + Skill 形成工作流 |
| **Rules** | 5 | 核心原則：風險優先、回測先行、防過擬合、數據完整、執行寫實 |
| **Contexts** | 3 | 執行時狀態：市場數據、投資組合狀態、策略配置 |
| **Hooks** | 4 | 前置/後置檢查自動化：交易前風控、回測後驗證等 |

### 2.3 三層架構模型
```
┌─────────────────────────────────────────────────────────────┐
│  應用層：Commands (20) - 使用者入口，自然語言指令             │
├─────────────────────────────────────────────────────────────┤
│  智能層：Agents (18) - 專家角色，具備 System Prompt + Tools  │
├─────────────────────────────────────────────────────────────┤
│  知識層：Skills (82) - 領域方法論、範本、最佳實踐、紅旗指標   │
├─────────────────────────────────────────────────────────────┤
│  基礎層：Rules + Contexts + Hooks - 約束、狀態、自動化檢查     │
└─────────────────────────────────────────────────────────────┘
```

### 2.4 18 位專家 Agent（Opus/Sonnet 混合部署）
| 類別 | Agent | 模型 | 核心職責 |
|------|-------|------|----------|
| 核心量化 | quant-researcher | Opus | Alpha 信號發掘、因子模型、統計檢定 |
| | algo-strategist | Opus | 系統性策略設計、進出場規則、執行演算法 |
| | risk-manager | Opus | VaR、壓力測試、敞口管理、風險限額 |
| | backtesting-engineer | Sonnet | 走向前分析、蒙地卡羅、過擬合檢測 |
| 衍生品專精 | derivatives-analyst | Opus | 期權定價、波動面、奇異期權 |
| | options-strategist | Opus | 價差、對沖、波動率交易結構 |
| | volatility-trader | Opus | 波動面、期限結構、偏度、VIX |
| 投資組合 | portfolio-manager | Opus | 配置、優化、再平衡 |
| | fixed-income-analyst | Opus | 債券、收益率曲線、久期、信用利差 |
| | fx-strategist | Opus | 貨幣對、套息交易、宏觀驅動 |
| 執行微觀 | execution-specialist | Sonnet | 滑價、TWAP/VWAP、智能路由 |
| | market-microstructure | Opus | 訂單流、流動性、做市商策略 |
| 專項策略 | stat-arb-specialist | Opus | 配對交易、共整合、均值回歸 |
| | technical-analyst | Sonnet | 圖表形態、指標、價格行為 |
| | financial-data-scientist | Opus | 特徵工程、ML 模型、另類數據 |
| | crypto-defi-analyst | Sonnet | 鏈上分析、DEX、收益耕作、代幣經濟 |
| | commodities-analyst | Sonnet | 期貨曲線、季節性、供需 |
| 合規 | regulatory-compliance | Sonnet | MiFID II、Dodd-Frank、持倉限制 |

### 2.5 82 Skills 九大領域分佈
| 領域 | 目錄 | 模組數 | 核心 Skills |
|------|------|--------|-------------|
| 量化方法 | quant-methods/ | 10 | factor-model-design、alpha-signal-research、statistical-testing |
| 策略類型 | strategies/ | 10 | momentum-strategy、mean-reversion-strategy、pairs-trading |
| 風險管理 | risk/ | 10 | var-calculation、stress-testing、position-sizing、tail-risk-management |
| 衍生品 | derivatives/ | 10 | options-pricing、greeks-analysis、volatility-surface、hedging-optimization |
| 投資組合 | portfolio/ | 8 | mean-variance-optimization、risk-parity、factor-investing、rebalancing-strategy |
| 執行 | execution/ | 8 | order-management、transaction-cost-analysis、execution-algorithm、dark-pool-strategy |
| 市場數據 | data/ | 6 | data-pipeline-design、alternative-data-ingestion、tick-data-processing |
| 加密貨幣 | crypto/ | 8 | defi-analytics、on-chain-analysis、crypto-derivatives、mev-analysis |
| 宏觀環球 | macro/ | 6 | economic-indicator-analysis、central-bank-analysis、fx-strategy |
| 回測研究 | backtesting/ | 6 | backtest-engine、walk-forward-analysis、monte-carlo-simulation、overfitting-detection |

### 2.6 四大標準化工作流（核心編排價值）
| 工作流 | 編排鏈 | 關鍵門禁 |
|--------|--------|----------|
| **策略開發流** | quant-researcher → algo-strategist → backtesting-engineer → **risk-manager** | Risk Gate 必過 |
| **期權交易流** | derivatives-analyst → options-strategist → **risk-manager** → execution-specialist | Risk Gate + Compliance |
| **加密貨幣分析流** | crypto-defi-analyst → **risk-manager** → execution-specialist | 智能合約風險量化 |
| **投資組合再平衡流** | portfolio-manager → **risk-manager** → execution-specialist | 風險限額驗證 |

### 2.7 量化級品質門檻（生產級嚴格度）
| 指標 | 門檻 | 紅線 |
|------|------|------|
| **Signal t-stat** | ≥ 2.0 | < 1.5 拒絕 |
| **IC (Information Coefficient)** | > 0.03 | < 0.01 拒絕 |
| **Deflated Sharpe Ratio** | > 0.5 (5% 顯著性) | 失敗需合併正交信號 |
| **Walk-forward windows** | ≥ 3 個樣本外窗口 | 不足則不合格 |
| **Max Drawdown** | < 20% (回測) | 超過需 Risk Manager 批准 |
| **Net Sharpe (扣成本後)** | > 1.0 | 否則不可上線 |
| **In-sample vs OOS Sharpe 衰減** | < 40% | ≥ 40% 視為過擬合 |

### 2.8 Hooks 自動化檢查機制（事件驅動）
```json
// hooks/hooks.json 核心結構
{
  "hooks": {
    "PostToolUse": [
      {"matcher": "Write|Edit", "hooks": [
        {"type": "command", "command": "node ${CLAUDE_PLUGIN_ROOT}/scripts/hooks/risk-check.js"},
        {"type": "command", "command": "node ${CLAUDE_PLUGIN_ROOT}/scripts/hooks/backtest-validation.js"},
        {"type": "command", "command": "node ${CLAUDE_PLUGIN_ROOT}/scripts/hooks/data-quality-check.js"}
      ]}
    ],
    "SessionStart": [
      {"matcher": ".*", "hooks": [
        {"type": "command", "command": "node ${CLAUDE_PLUGIN_ROOT}/scripts/hooks/session-start.js"}
      ]}
    ]
  }
}
```

### 2.9 與 Buzz 系統架構對應關係
| ECT 組件 | Buzz 對應 | 移植可行性 | 適配策略 |
|----------|-----------|------------|----------|
| 18 Agents | Buzz Agents | ✅ 高 | 直接對應 Agent 定義格式 |
| 82 Skills | Buzz Skills | ✅ 高 | Skill 格式標準化 |
| 20 Commands | Buzz Slash Commands / Workflows | ✅ 中 | 需適配 Buzz CLI |
| 5 Rules | Buzz 核心記憶體 / 代理指令 | ✅ 高 | 寫入 core memory |
| 3 Contexts | Buzz Channel/Thread State | ✅ 高 | 對應上下文管理 |
| 4 Hooks | Buzz Automation / Workflows | ✅ 中 | 適配事件驅動模型 |
| 4 大工作流 | Buzz 7 步協作閉環 | ✅ 高 | 架構理念高度一致 |

### 2.10 關鍵代碼出處溯源
| 組件 | 來源文件 | 關鍵位置 |
|------|----------|----------|
| Agent 定義格式 | `agents/quant-researcher.md` | Front Matter (1-6)、Role (10-21)、Process (23-170) |
| Agent 編排規則 | `AGENTS.md` | Orchestration Rules (28-35)、Workflows (37-80) |
| Skills 目錄結構 | `skills/` 目錄實測 | 9 大類 82 模組完整列表 |
| Skill 標準格式 | `skills/backtesting/live-trading-monitoring/SKILL.md` | 全文 (1-347) |
| Command 定義 | `commands/backtest.md` | Front Matter (1-3)、How It Works (18-25) |
| 專案元數據 | `.claude-plugin/plugin.json` | 全文 (1-13) |
| 核心規則 | `rules/common/*.md` | 5 條規則文件 |
| Hooks 配置 | `hooks/hooks.json` + `scripts/hooks/*.js` | 4 個腳本 |
| Contexts | `contexts/*.md` | 3 個上下文文件 |

### 2.11 協議鑑權閉環（移植過程中對外操作）
| 操作 | Nostr Kind | Tags 結構 | 加密 | 權限模型 | 重試/降級策略 |
|------|------------|-----------|------|----------|---------------|
| 發布架構對照報告 | `1` (Text Note) | `["e", <thread-root>]`, `["h", <channel-id>]`, `["p", <boss-pubkey>]` | NIP-44 | Channel-Scope | 3 次指數退避 → 本地落檔 `OUTBOX/ect_report_<ts>.md` |
| 提交審核請求 | `1` (Text Note) | 同上 + `["p", <sentry-pubkey>]` | 同上 | 同上 | 同上 |

**具體實作參數**：
- `buzz messages send --channel 5ca019f6-1a27-592a-b015-4baf0eb33572 --reply-to <parent> --content "<報告>" --mention <boss-pubkey> --mention <sentry-pubkey>`
- 發文前預檢：`buzz channels get --channel <id>`
- **無 `buzz relay status` 指令**：發文前直接嘗試 `buzz messages send`，若失敗（網路/權限）再走降級邏輯

### 2.12 失敗模式推演與緩解（ECT 移植專屬）
| 失敗場景 | 影響 | 緩解策略 |
|----------|------|----------|
| 量化門檻指標計算邏輯差異 | 風險評估不準 | 引入 Python 科學計算庫驗證；對照 ECT 回測基準數據跑對照組 |
| 風險管理 Agent 決策邏輯複雜 | 移植後行為偏離 | 保留核心規則（VaR、壓力測試、敞口限額）為硬門禁；次級邏輯標註為建議級 |
| Hooks 自動化檢查依賴 Node.js 環境 | 部署環境不一致 | 將風險檢查、回測驗證、數據品質檢查重寫為獨立 CLI 工具，無外部執行時依賴 |
| 專有數據源（Bloomberg/Reuters）無法接入 | 部分 Skills 不可用 | 標註為「需外部授權」；提供 Mock Data 接口供開發測試 |

---

## 🎯 專案三至五：待補齊調研（佔位）

> **說明**：主理人交辦 5 個專案，目前已完成 2 個深度調研。以下 3 個專案需補齊調研後再擴充本計畫書。此處預置協議與失敗模式框架，調研完成後填入具體內容。

| 專案代號 | 疑似倉庫 | 調研狀態 | 優先級 |
|----------|----------|----------|--------|
| 專案三 | 待確認 | 未開始 | 高 |
| 專案四 | 待確認 | 未開始 | 中 |
| 專案五 | 待確認 | 未開始 | 中 |

**行動項目**：請主理人補充 3 個專案的 GitHub URL 或本地路徑，由 Honey2 依相同標準完成調研並追加至本計畫書。

### 3.1 通用協議鑑權閉環模板（專案三至五共用）
| 操作 | Nostr Kind | Tags 結構 | 加密 | 權限模型 | 重試/降級策略 |
|------|------------|-----------|------|----------|---------------|
| 發布調研/對照報告 | `1` (Text Note) | `["e", <thread-root>]`, `["h", <channel-id>]`, `["p", <boss-pubkey>]` | NIP-44 | Channel-Scope | 3 次指數退避 → 本地落檔 `OUTBOX/projN_report_<ts>.md` |
| 提交審核請求 | `1` (Text Note) | 同上 + `["p", <sentry-pubkey>]` | 同上 | 同上 | 同上 |

**具體實作參數**：
- `buzz messages send --channel 5ca019f6-1a27-592a-b015-4baf0eb33572 --reply-to <parent> --content "<報告>" --mention <boss-pubkey> --mention <sentry-pubkey>`
- 發文前預檢：`buzz channels get --channel <id>`
- **無 `buzz relay status` 指令**：發文前直接嘗試 `buzz messages send`，若失敗（網路/權限）再走降級邏輯

### 3.2 通用失敗模式框架（專案三至五最小集，調研後擴充）
| 失敗場景 | 影響 | 緩解策略 |
|----------|------|----------|
| GitHub 倉庫私有/已歸檔/404 | 無法拉取代碼 | 改用本地備份或替代專案；記錄原因並報告老闆 |
| 專案技術棧與 Buzz 不相容 | 移植價值歸零 | 調研階段即做技術棧掃描（語言、框架、事件模型）；不相容則降級為「僅參考」 |
| 核心維護者停止更新/社群冷卻 | 長期維護風險 | 評估 Fork 可行性；標註為「僅參考/不納入核心路徑」 |
| 關鍵依賴缺失（專有 SDK、付費 API） | 部分功能不可用 | 提供 Mock/Stub 實作；標註需外部授權 |

---

## 📦 共同交付物

### 1. 技術參考檔案庫（已完成）
- `RESEARCH/CCGS_ANALYSIS_49_AGENTS_72_SKILLS_15_HOOKS.md` ✅
- `RESEARCH/EVERYTHING_CLAUDE_TRADING_ARCHITECTURE.md` ✅

### 2. 架構對照與移植可行性矩陣（本計畫書核心輸出）
- 兩專案完整的「組件對應表」與「適配策略」
- 移植優先級排序與風險評估

### 3. 第一期 MVP 移植實作路徑（建議）

#### 方案 A：CCGS 核心子集優先（推薦）
```
第一期 (2-3 週)：
├── 移植：Director Gates 概念 → Buzz 四大語意門禁強化
├── 移植：SubagentStop 驗證交付物機制 → Pollen 實作驗收自動化
├── 移植：PreToolUse 讀取保護 → 大文件分段讀取強制化
├── 移植：49 Agent 角色體系參考 → Buzz 專屬 Agent 角色庫擴展
└── 移植：72 Skills 分類體系 → Buzz Skill 目錄重組

第二期 (4-6 週)：
├── Hooks 事件總線適配 Nostr/Buzz 事件模型
├── Director Gates 全套 28 門檢移植
├── 團隊協作 Skills (16 項) 移植
└── 完整 Session 狀態管理 (Start/PreCompact/Stop)
```

#### 方案 B：ECT 風險管理核心優先（若老闆有量化交易需求）
```
第一期 (2-3 週)：
├── 移植：Risk Manager Agent → Buzz 風險審核專用 Agent
├── 移植：Backtesting Engineer Agent → 策略驗證自動化
├── 移植：核心 15 Skills (risk/ + backtesting/ + execution/) → Buzz 風控技能包
├── 移植：4 大工作流編排邏輯 → 對齊 Buzz 7 步閉環
└── 移植：量化級品質門檻 → Buzz 專案驗收標準

第二期 (6-8 週)：
├── 全 18 Agents + 82 Skills 完整移植
├── Hooks 自動化檢查對接 Buzz Workflows
├── Contexts 映射到 Buzz Channel/Thread 狀態
└── 建立 Buzz 專屬量化交易專案模板
```

---

## 🚀 實施時程規劃

| 階段 | 時間範圍 | 關鍵里程碑 |
|------|----------|------------|
| **調研補齊** | 第 0-1 天 | 主理人提供 3 專案資訊 → Honey2 完成調研 → 追加至計畫書 |
| **架構審核** | 第 1 天 | 計畫書完成 → Sentry 四大門禁審核 → 老闆授權 |
| **方案選擇** | 第 1 天 | 老闆確認採用方案 A 或 B（或混合） |
| **第一期 MVP** | 第 2-10 天 | Pollen 依選定方案實作核心移植 |
| **整合測試** | 第 11-12 天 | 端到端驗證、回歸測試 |
| **審核驗收** | 第 13 天 | Sentry 複審 → 老闆驗收 |
| **落庫生效** | 第 13 天 | 移植成果納入 Buzz 系統、更新 AGENTS.md |

---

## 📋 後續擴展（第二期）

| 方向 | 說明 |
|------|------|
| **技能市場化** | 將移植的 Skills 封裝為 Buzz Relay 技能，支援社群分享與版本管理 |
| **Agent 專業化** | 依 CCGS/ECT 體系，建立 Buzz 專屬的垂直領域 Agent（量化交易、遊戲開發、系統運維等） |
| **工作流模板化** | 將 4 大標準化工作流製成 Buzz Workflow 模板，一鍵啟動 |
| **品質門禁平台化** | 將量化級門檻 (Deflated Sharpe、Walk-forward 等) 通用化為通用專案品質儀表板 |

---

## ✅ 審核簽署欄

| 角色 | 簽署狀態 | 簽署時間 | 備註 |
|------|----------|----------|------|
| **Honey2 (企劃調研)** | 待簽署 | - | 2 專案調研完成，3 專案待補齊 |
| **Sentry (架構審核)** | 待審核 | - | 四大語意門禁檢查 |
| **老闆 (授權/補充)** | 待授權/待補充 | - | 1. 授權實作 2. 提供 3 專案 GitHub URL |
| **Pollen (實作)** | 待派工 | - | 接收授權後開始開發 |

---

## 📎 附錄：關鍵文件完整路徑索引

### CCGS 專案
```
C:/Users/user/.buzz/REPOS/Claude-Code-Game-Studios/
├── .claude/agents/                    # 49 Agent 定義
├── .claude/skills/                    # 74 Skill 模組
├── .claude/hooks/*.sh                 # 18 Hooks 腳本
├── .claude/rules/*.md                 # 14 規則文件
├── .claude/scripts/*.sh               # 33 共用腳本
├── .claude/docs/director-gates/       # 28 Director Gates
├── .claude/settings.json              # Hooks 事件配置
└── AGENTS.md                          # Agent 編排總綱 (核心架構文檔)
```

### ECT 專案
```
C:/Users/user/.buzz/REPOS/everything-claude-trading/
├── AGENTS.md                           # Agent 編排總綱
├── CLAUDE.md                           # 專案總覽與快速開始
├── README.md                           # 完整文檔（含 82 Skills 詳細表）
├── the-complete-guide.md               # 完整指南
├── agents/quant-researcher.md          # 量化研究員完整定義
├── agents/risk-manager.md              # 風險管理員完整定義
├── commands/backtest.md                # 回測指令定義
├── skills/backtesting/live-trading-monitoring/SKILL.md  # 典型 Skill 範例
├── .claude-plugin/plugin.json          # 插件元數據
├── hooks/hooks.json                    # Hooks 配置
├── scripts/hooks/*.js                  # 4 個自動化腳本
├── contexts/*.md                       # 3 個上下文
└── rules/common/*.md                   # 5 條核心規則
```

---

*計畫書版本：1.0 | 建立日期：2026-11-02 | 狀態：草稿待審核（含 3 專案待補齊）*