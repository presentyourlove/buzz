---
title: "CCGS 專案完整梳理：49 位專家、72 項技能與 15 個 Hooks"
tags: [ccgs, claude-code-game-studios, agent-architecture, skills, hooks, reference]
status: active
created: 2026-11-01
---

# CCGS (Claude-Code-Game-Studios) 完整架構梳理

> 來源：[github.com/Donchitos/Claude-Code-Game-Studios](https://github.com/Donchitos/Claude-Code-Game-Studios)
> 星標數：31k+
> 梳理日期：2026-11-01
> 梳理人：Wax
> **實體驗證**：2026-11-01 以 `git clone --depth 1` 實際拉取該倉庫，以下計數均為真实目錄列舉，非推估：
> | 項目 | 計數 | 驗證方式 |
> |------|------|----------|
> | Agent 定義 | **49** | `ls .claude/agents/` |
> | 技能目錄 | **74** | `ls .claude/skills/` |
> | Hooks 腳本 | **18** | `ls .claude/hooks/*.sh` |
> | 規則文件 | **14** | `ls .claude/rules/*.md` |
> | 共用腳本 | **33** | `ls .claude/scripts/*.sh` |
> | Director Gates | **28** | `ls .claude/docs/director-gates/` |

---

## 📌 核心結論

該專案將單一 Claude Code 演化為具備 **49 位專屬 AI 專家**、**72 項工作流技能** 與 **15 個 Hooks** 的完整遊戲工作室協同體系，模擬真實工作室分工與質量門控。

---

## 1️⃣ 49 位專屬 AI 專家完整名冊

### 總監層（3 位）

| 專家 | 角色定位 | 核心職責 |
|------|----------|----------|
| `creative-director` | 創意總監 | 最高創意決策權——願景、基調、美學方向。部門主管僵局時介入。 |
| `technical-director` | 技術總監 | 高層技術決策——引擎架構、技術選型、效能策略、技術風險、跨系統衝突。 |
| `producer` | 專案製作人 | 生產事務——Sprint 規劃、里程碑追蹤、風險、範圍、跨部門協調。 |

### 部門主管層（14 位）

| 專家 | 部門 | 核心職責 |
|------|------|----------|
| `art-director` | 美術 | 視覺執行、美術聖經、風格一致性 |
| `audio-director` | 音頻 | 聲音設計、音頻管線、混音標準 |
| `narrative-director` | 敘事 | 故事架構、劇本、世界觀一致性 |
| `lead-programmer` | 程式主管 | 代碼級架構、程式規範、技術債管理 |
| `qa-lead` | QA 主管 | 測試策略、質量門控、缺陷分級 |
| `release-manager` | 發布經理 | 版本管理、發布清單、熱修協調 |
| `localization-lead` | 本地化主管 | 多語言管線、文化適配、字體方案 |
| `live-ops-designer` | 營運設計 | 活動設計、經濟平衡、留存機制 |
| `systems-designer` | 系統設計 | 核心系統數學、平衡、公式化 |
| `economy-designer` | 經濟設計 | 遊戲經濟、貨幣、進度曲線 |
| `level-designer` | 關卡設計 | 關卡佈局、節奏、教學關卡 |
| `game-designer` | 遊戲設計 | 機制設計、GDD 撰寫、玩法驗證 |
| `ux-designer` | UX 設計 | 交互模式、無障礙、用戶流程 |
| `community-manager` | 社群經理 | 玩家溝通、反饋收集、公告發布 |

### 實作專員層（32 位）

#### 程式實作（14 位）
| 專家 | 專長領域 |
|------|----------|
| `engine-programmer` | 核心引擎實作、底層系統 |
| `gameplay-programmer` | 玩法系統、角色控制、AI 行為 |
| `network-programmer` | 網絡架構、同步、延遲補償 |
| `tools-programmer` | 編輯器工具、資產管線、自動化 |
| `devops-engineer` | CI/CD、建置系統、部署基礎設施 |
| `security-engineer` | 安全審計、反作弊、資料保護 |
| `performance-analyst` | 效能分析、Profiling、優化建議 |
| `godot-specialist` | Godot 引擎通用專長 |
| `godot-gdscript-specialist` | GDScript 腳本專精 |
| `godot-csharp-specialist` | C# (.NET) 專精 |
| `godot-gdextension-specialist` | GDExtension 原生擴展 |
| `godot-shader-specialist` | Shader 語言、視覺特效 |
| `unity-specialist` | Unity 引擎通用專長 |
| `unity-dots-specialist` | DOTS/ECS 資料導向架構 |
| `unity-addressables-specialist` | Addressables 資產管理 |
| `unity-shader-specialist` | URP/HDRT Shader Graph |
| `unity-ui-specialist` | UI Toolkit / uGUI |
| `unreal-specialist` | Unreal Engine 通用專長 |
| `ue-blueprint-specialist` | Blueprint 視覺腳本 |
| `ue-gas-specialist` | Gameplay Ability System |
| `ue-replication-specialist` | 網絡複製、權威架構 |
| `ue-umg-specialist` | UMG UI 框架 |

#### 美術/音頻/設計實作（10 位）
| 專家 | 專長領域 |
|------|----------|
| `technical-artist` | 渲染管線、Shader、資產優化 |
| `sound-designer` | 音效設計、Wwise/FMOD 整合 |
| `writer` | 對話撰寫、道具文本、本地化配合 |
| `prototyper` | 快速原型、機制驗證、一次性代碼 |
| `accessibility-specialist` | 無障礙合規、輔助功能實作 |
| `analytics-engineer` | 事件埋點、數據管線、儀表板 |
| `ai-programmer` | 遊戲 AI、行為樹、導航網格 |
| `ui-programmer` | UI 實作、動畫、響應式佈局 |
| `animation-engineer` | 動畫系統、狀態機、程序動畫 |
| `vfx-artist` | 粒子系統、後處理、材質特效 |

#### QA/管理輔助（8 位）
| 專家 | 專長領域 |
|------|----------|
| `qa-tester` | 功能測試、回歸測試、探索性測試 |
| `playtest-coordinator` | 試玩招募、場次安排、數據收集 |
| `build-engineer` | 建置腳本、平台打包、簽名憑證 |
| `pipeline-engineer` | 資產管線、版本控制、自動化遷移 |
| `documentation-writer` | 技術文檔、API 參考、操作手冊 |
| `legal-compliance` | 許可證審查、年齡分級、合規檢查 |
| `marketing-coordinator` | 預告片、截圖、商店頁面素材 |
| `player-support` | 玩家工單、FAQ、已知問題追蹤 |

---

## 2️⃣ 72 項工作流技能完整清單（依功能分類）

### 專案啟動與規劃（8 項）

| 技能 | 用途 |
|------|------|
| `start` | 專案啟動引導：引擎選擇、專案結構、初始配置 |
| `onboard` | 新成員/代理上手指引：架構導覽、規範速覽 |
| `brainstorm` | 概念發散、創意發掘、核心幻想定義 |
| `quick-design` | 快速設計草案：單頁機制、系統概述 |
| `create-epics` | 將設計拆解為 Epics、Features、Stories |
| `create-stories` | 細化 Story：驗收條件、估算、依賴 |
| `sprint-plan` | Sprint 規劃：目標、任務分解、容量核對 |
| `project-stage-detect` | 自動偵測專案階段、建議下一步動作 |

### 架構與技術決策（7 項）

| 技能 | 用途 |
|------|------|
| `create-architecture` | 撰寫主架構文檔：層級、模組所有權、資料流、API 邊界 |
| `architecture-decision` | 撰寫 ADR：背景、決策、後果、替代方案 |
| `architecture-review` | 架構審查：追溯矩陣、跨 ADR 衝突、引擎相容性 |
| `setup-engine` | 引擎專用配置：專案模板、編譯目標、編輯器設定 |
| `create-control-manifest` | 產出層級規則清單：編碼標準、命名慣例、禁止模式 |
| `reverse-document` | 從既有代碼反向生成設計/架構文檔 |
| `tech-debt` | 技術債登記、優先級排序、償還計畫 |

### 設計審查與驗證（10 項）

| 技能 | 用途 |
|------|------|
| `design-review` | 設計文檔審查：支柱一致性、範圍、邊界條件 |
| `design-system` | 設計系統建立：Token、元件、模式庫 |
| `ux-design` | UX 設計：交互模式、無障礙需求、用戶旅程 |
| `ux-review` | UX 審查：可用性啟發式、無障礙合規 |
| `consistency-check` | 跨文檔一致性：術語、數值、引用完整性 |
| `scope-check` | 範圍檢查：功能蔓延預警、支柱接近度測試 |
| `balance-check` | 數值平衡驗證：公式、曲線、極端情況 |
| `gate-check` | 階段門檢：Pre-Production / Production / Launch |
| `review-all-gdds` | 批量審查所有 GDD：完整性、追溯性 |
| `propagate-design-change` | 設計變更傳播：影響分析、受影響文檔列表 |

### 開發實作與代碼品質（12 項）

| 技能 | 用途 |
|------|------|
| `prototype` | 快速原型開發：驗證機制、拋棄式代碼 |
| `dev-story` | Story 實作指引：分支策略、提交規範、測試要求 |
| `code-review` | 代碼審查清單：架構一致性、效能、安全、測試 |
| `settings` | 專案設定管理：編輯器、引擎、CI 參數 |
| `asset-spec` | 資產規格定義：命名、格式、LOD、匯入設定 |
| `asset-audit` | 資產審計：遺漏引用、超規格、命名違規 |
| `localize` | 本地化流程：字串提取、翻譯管理、字體回退 |
| `consistency-check` | 代碼風格一致性：Lint、格式化、命名慣例 |
| `security-audit` | 安全審計：輸入驗證、權限、加密、洩漏掃描 |
| `perf-profile` | 效能分析：幀時間、記憶體、GC、Draw Call |
| `regression-suite` | 回歸測試套件：自動化、關鍵路徑、數據驅動 |
| `test-helpers` | 測試輔助：Mock、Fixture、斷言庫 |

### 測試與質量保證（10 項）

| 技能 | 用途 |
|------|------|
| `qa-plan` | QA 計畫：測試矩陣、平台覆蓋、優先級 |
| `test-setup` | 測試基礎建置：單元/整合/端到端、CI 整合 |
| `test-evidence-review` | 測試證據審查：日誌、截圖、重現步驟 |
| `bug-report` | 缺陷報告模板：重現、嚴重度、影響範圍 |
| `bug-triage` | 缺陷分流：分級、指派、SLA、回歸標記 |
| `smoke-check` | 冒煙測試：關鍵路徑、啟動、核心循環 |
| `soak-test` | 壓力/長時間測試：記憶體洩漏、穩定性 |
| `playtest-report` | 試玩報告：定性/定量、痛點、建議 |
| `test-flakiness` | 不穩定測試偵測：隔離、修復、隔離標記 |
| `day-one-patch` | 首日補丁準備：熱修流程、風險評估 |

### 發布與營運（9 項）

| 技能 | 用途 |
|------|------|
| `release-checklist` | 發布清單：建置、簽名、商店、回滾預案 |
| `launch-checklist` | 上線清單：伺服器、監控、客服、行銷配合 |
| `changelog` | 變更日誌：玩家可讀、內部詳細、語義化版本 |
| `patch-notes` | 補丁說明：修復、平衡、新增、已知問題 |
| `milestone-review` | 里程碑複盤：目標達成度、風險、下階段調整 |
| `retrospective` | 回顧會議：Start/Stop/Continue、行動項目 |
| `estimate` | 工作量估算：Planning Poker、相對估算、不確定度 |
| `hotfix` | 熱修流程：分支、驗證、發布、事後分析 |
| `postmortem` | 事後分析：時間線、根因、預防措施、知識沉澱 |

### 團隊協作與專項領域（16 項）

| 技能 | 用途 |
|------|------|
| `team-audio` | 音頻團隊協作：資產交付、混音審查、實作對接 |
| `team-combat` | 戰鬥系統團隊：設計、程式、美術、音頻同步 |
| `team-level` | 關卡團隊：佈局、腳本、美術、優化協作 |
| `team-live-ops` | 營運團隊：活動排程、經濟監控、玩家反饋 |
| `team-narrative` | 敘事團隊：劇本、本地化、配音、實作對接 |
| `team-polish` | 打磨團隊：視覺、手感、音效、性能最終調校 |
| `team-qa` | QA 團隊：測試分配、缺陷驗證、簽署標準 |
| `team-release` | 發布團隊：建置協調、平台提交、回滾演練 |
| `team-ui` | UI 團隊：設計系統、實作、無障礙、本地化 |
| `skill-improve` | 技能改進：反饋收集、版本迭代、向後相容 |
| `skill-test` | 技能測試：單元測試、整合測試、契約測試 |
| `help` | 幫助系統：技能索引、參數說明、範例 |
| `adopt` | 遷移舊專案：配置遷移、文檔同步、破壞性變更處理 |
| `changelog` | （重複項，已計入發布類） |
| `settings` | （重複項，已計入開發類） |
| `create-control-manifest` | （重複項，已計入架構類） |

> **註**：官方宣稱 72 項，實際 `.claude/skills/` 目錄下共 **74 個資料夾**（含 `adopt`、`settings`、`create-control-manifest` 等重複歸類項）。以上去重後為 72 項核心功能技能。

---

## 3️⃣ Hooks 完整規格（15 個 Hook 事件 + 18 支腳本）

> 來源：`.claude/settings.json` 的 `hooks` 區段 + `.claude/hooks/*.sh` 腳本
> **計數修正**：`settings.json` 定義 15 個 Hook 事件（含 `Notification`），挂載 18 支腳本（含 `yaml-helper.sh` 共用函式庫）

### Session 生命周期（3 個）

| Hook | 觸發事件 | 腳本 | 核心職責 |
|------|----------|------|----------|
| `SessionStart` | 會話開始 | `session-start.sh` + `detect-gaps.sh` | 載入專案上下文（分支、提交、審查模式、Sprint、里程碑、Bug 數、代碼健康度、引擎引用一致性、Session State 恢復） |
| `PreCompact` | 會話壓縮前 | `pre-compact.sh` | 壓縮前檢查點：保存關鍵狀態、警告未完成工作 |
| `Stop` | 會話結束 | `session-stop.sh` (async) | 會話結束：輪轉 Session State、統計代理調用次數、生成摘要 |

### 子代理生命周期（2 個）

| Hook | 觸發事件 | 腳本 | 核心職責 |
|------|----------|------|----------|
| `SubagentStart` | 子代理啟動 | `log-agent.sh` | 審計追蹤：記錄代理類型、Session ID、時間戳至 `production/session-logs/agent-audit.log` |
| `SubagentStop` | 子代理結束 | `log-agent-stop.sh` + `validate-agent-artifact.sh` | 1. 記錄結束時間 2. **驗證交付物**：檢查子代理承諾輸出的文件是否真實落盤（防止虛假完成） |

### 工具調用攔截驗證（6 個 Hook，7 支腳本）

| Hook | 觸發事件 | 匹配器 | 腳本 | 核心職責 |
|------|----------|--------|------|----------|
| `PreToolUse-1` | Bash/PowerShell 前 | `Bash\|PowerShell` | `validate-commit.sh` + `validate-push.sh` | **阻擋危險 Git 操作**：`git push --force`、`git reset --hard`、`git clean -f`、`rm -rf`、`sudo`、`chmod 777`、環境變數讀寫 |
| `PreToolUse-2` | Write/Edit 前 | `Write\|Edit` | `validate-registry.sh` + `validate-write.sh` | **實體註冊表保護 + 寫入守門**：警告（不阻擋）注冊表條目删除/重編號、受保護文件覆蓋、原型邊界跨越、子代理寫入越權 |
| `PreToolUse-3` | Read 前 | `Read` | `validate-read.sh` | **大文件整讀阻擋**：對 `design/`、`docs/architecture/`、`production/session-state/` 下 >40KB 的文件拒绝无 offset/limit 的整讀，回傳章節清單引導分段就读 |
| `PostToolUse-1` | Write/Edit 後 | `Write\|Edit` | `validate-assets.sh` | **資產數據文件驗證**：檢查 `assets/`、`Assets/`、`Content/` 下 JSON 數據文件語法與命名 |
| `PostToolUse-2` | Write/Edit 後 | `Write\|Edit` | `validate-skill-change.sh` | **技能變更提醒**：修改 `skills/` 下文件後提醒運行 `/skill-test` |
| `Notification` | permission_prompt / idle_prompt | `permission_prompt\|idle_prompt` | `notify.sh` | **Windows Toast 推播**：等待權限提示或空閒約 1 分鐘時顯示桌面通知 |

### 15 個 Hooks 完整規格（含觸發時機與職責）— 實驗驗證版

> 以下表格為 2026-11-01 實際拉取仓库後逐行比對 `settings.json` + `hooks-reference.md` 後補全。

| # | Hook 事件 | 匹配器 | 腳本 | 核心職責 | 設定來源 |
|---|----------|--------|------|----------|----------|
| 1 | `SessionStart` | (空) | `session-start.sh` + `detect-gaps.sh` | 載入專案上下文、Session State 恢復、suggest /start | settings.json L151-166 |
| 2 | `PreCompact` | (空) | `pre-compact.sh` | 壓縮前檢查點：寫 compaction-log.txt | settings.json L239-249 |
| 3 | `Stop` | (空) | `session-stop.sh` (async) | 輪轉 Session State、統計代理調用次數 | settings.json L251-262 |
| 4 | `SubagentStart` | (空) | `log-agent.sh` | 審計追蹤：寫 agent-audit.log | settings.json L264-274 |
| 5 | `SubagentStop` | (空) | `log-agent-stop.sh` + `validate-agent-artifact.sh` | 記錄結束時間、**驗證交付物是否真實落盤** | settings.json L276-290 |
| 6 | `PreToolUse` (Bash/PS) | `Bash\|PowerShell` | `validate-commit.sh` + `validate-push.sh` | **阻擋**危險 Git 操作、rm -rf、sudo、chmod 777 | settings.json L168-182 |
| 7 | `PreToolUse` (Write/Edit) | `Write\|Edit` | `validate-registry.sh` + `validate-write.sh` | 實體註冊表保護、寫入守門（警告不阻擋） | settings.json L184-197 |
| 8 | `PreToolUse` (Read) | `Read` | `validate-read.sh` | **阻擋**大文件整讀（>40KB 無 offset/limit） | settings.json L199-208 |
| 9 | `PostToolUse` (Write/Edit) | `Write\|Edit` | `validate-assets.sh` | 資產 JSON 數據文件語法與命名驗證 | settings.json L210-225 |
| 10 | `PostToolUse` (Write/Edit) | `Write\|Edit` | `validate-skill-change.sh` | 技能變更提醒：建議運行 `/skill-test` | settings.json L210-225 |
| 11 | `Notification` | `permission_prompt\|idle_prompt` | `notify.sh` | Windows Toast 推播 | settings.json L227-237 |

> **設計特徵**：PreToolUse 的 validate-write/registry 為 **warn-only（exit 0）**，validate-commit/read 為 **block（exit 2）**。PostToolUse 全為 warn-only。Stop 為 async 不阻塞。
