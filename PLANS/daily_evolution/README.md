# 每日系統進化系統 (Daily Evolution System)

Buzz 自動化每日系統進化提案生成與健康自癒架構。

## 系統概述

本系統於每日 22:30 (Asia/Taipei 時區) 自動執行，完成以下任務：
1. 抓取目標頻道 (`5ca019f6-1a27-592a-b015-4baf0eb33572`) 當日之所有交流對話
2. 讀取 `WORK_LOGS/` 目錄下的收割日誌與工作紀錄
3. 深入分析對話脈絡，萃取技術主題、待辦事項、阻礙項目與拍板決策
4. 生成 100% 繁體中文標準 Markdown 計畫書，存檔至 `PLANS/DAILY_EVOLUTION_PROPOSAL_YYYYMMDD.md`
5. 自動將**精煉之建議進化項目**（剔除分析雜訊）直接推送至指定頻道供老闆審核
6. 全自動雙向同步所有產出物至使用者全域目錄 `C:/Users/user/.buzz/`

## 模組與交付檔案清單

| 檔案名稱 | 語言/型態 | 職責說明 |
|----------|-----------|----------|
| `daily_evolution.py` | Python 3 | 核心分析與進化提案引擎：整合對話萃取、技能合成辨識、雙軌推播格式 |
| `skill_synthesizer.py` | Python 3 | 經驗提煉與技能合成器：自動從日常對話與 Obsidian 第二大腦中提煉高頻 SOP，草擬標準 SKILL.md 候選 |
| `activate_skill.py` | Python 3 | 技能安全審查與一鍵落庫掛載工具：Sentry 安全總監把關、雙軌原子落庫至專案與使用者全域技能庫 |
| `test_skill_synthesis.py` | Python 3 | 技能合成與落庫單元測試套件：覆蓋率 100%，驗證 Frontmatter 合規性與防注入安全攔截 |
| `health_check.py` | Python 3 | 系統健康自檢與監控腳本：驗證 CLI、Relay、金鑰、技能模組與測試套件 |
| `test_daily_evolution.py` | Python 3 | 單元測試套件：覆蓋率高達 100%，確保分析與過濾邏輯健全 |
| `trigger_windows.mjs` | Node.js (ESM) | Windows 原生觸發器：收割、分析、遞迴同步產出至全域目錄 |
| `check_pending_decisions.mjs` | Node.js (ESM) | 待確認決策與審核掃描腳本 |
| `run_daily_evolution.bat` | Batch | Windows 一鍵啟動批次檔（整合健康自檢與環境變數自動注入） |
| `run_daily_evolution.sh` | Bash | POSIX 環境定時觸發腳本 (Cron) |
| `daily-harvest.ts` | TypeScript | 訊息收割管線：產出 NDJSON 格式之收割日誌 |
| `GUIDES/COMMUNICATION_DISCIPLINE.md` | Markdown | 團隊溝通紀律與結論先行 (Answer-First) 準則 |
| `GUIDES/AGENT_RESPONSIBILITY_MATRIX.md` | Markdown | 團隊成員與 Agent 之 RACI 職責分工矩陣 |
| `RESEARCH/RCA_BUZZ_RECENT_FAILURES.md` | Markdown | 近期關鍵異常（429、503、Preflight）根本原因分析報告 |
| `PLANS/ACTION_ITEMS_KANBAN.md` | Markdown | 系統待辦事項優先級看板 (P0/P1/P2) |
| `PLANS/BLOCKERS_RESOLUTION.md` | Markdown | 系統關鍵阻礙排解標準指南 |
| `OUTBOX/IMPLEMENTATION_LOG_20261028.md` | Markdown | 第一天全量進化項目實作追蹤日誌 (13/13 100% 完成) |
| `OUTBOX/day1_evolution_full.patch` | Patch | 第一天全量進化代碼變更標準 Git 補丁檔 |

## 執行與測試方式

```powershell
# 1. 執行單元回歸測試 (含進化分析與技能合成 13 項測試)
python scripts/daily_evolution/test_daily_evolution.py
python scripts/daily_evolution/test_skill_synthesis.py

# 2. 執行系統健康自我檢查 (含技能系統模組驗證)
python scripts/daily_evolution/health_check.py

# 3. 執行經驗提煉與技能合成 (產出候選至 PLANS/SKILL_CANDIDATES/)
python scripts/daily_evolution/skill_synthesizer.py

# 4. 批准並落庫技能 (經老闆同意後，執行 Sentry 審核並雙軌掛載)
python scripts/daily_evolution/activate_skill.py --skill <技能名稱>

# 5. Windows 原生一鍵全流程執行 (健康自檢 -> 收割 -> 分析與技能合成 -> 提案推播 -> 遞迴同步)
.\scripts\daily_evolution\run_daily_evolution.bat
```
