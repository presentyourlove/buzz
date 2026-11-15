---
title: "技能候選實作計畫書：Obsidian 摘要同步與系統健康守護"
tags: [skill-implementation, obsidian-digest-sync, system-health-guardian, plan]
status: draft
created: 2026-11-02
version: "1.0"
---

# 技能候選實作計畫書：Obsidian 摘要同步與系統健康守護

> **來源**：每日系統進化提案 2026-11-02 (DAILY_EVOLUTION_PROPOSAL_20261102.md)  
> **對應技能候選**：`obsidian-digest-sync`、`system-health-guardian`  
> **規格標準**：Buzz Native Skill v1.0  
> **審核門禁**：四大語意門禁（代碼符號真實性、協議鑑權閉環、強制失敗模式推演、MVP 敏捷交付引導）

---

## 📋 計畫總覽

| 項目 | 內容 |
|------|------|
| **計畫編號** | SKILL_CANDIDATES_IMPL_20261102 |
| **目標** | 將 2 項技能候選從「候選中」轉為「生產就緒」，完成代碼落庫、單元測試、Sentry 審核、Owner 批准 |
| **MVP 交付範圍** | 兩技能各自獨立最小可用版本，優先驗證核心流程可跑通 |
| **預計工期** | 第一期 MVP：3-5 個工作天（並行開發） |
| **負責規劃** | Honey2（企劃）→ Sentry（審核）→ Pollen（實作） |
| **老闆授權關卡** | 計畫書通過 Sentry 審核後，需老闆親口「同意/開工/實作」方准動工 |

---

## 🎯 技能一：obsidian-digest-sync（Obsidian 對話摘要與第二大腦同步）

### 1.1 背景與動機
- **觸發詞頻**：1168（近期 6 份對話紀錄高頻提煉）
- **核心痛點**：對話沉澱流程手動、易遺漏、格式不一、無法自動同步至 Obsidian Vault
- **現有資產**：
  - `C:/私人/ai agent/buzz-obsidian`：Obsidian Vault 目錄（已具備讀寫權限，經實測驗證存在）
  - `C:/Users/user/.buzz/PLANS/SKILL_CANDIDATES/obsidian-digest-sync/SKILL.md`：技能規格草案
- **新增實作項目（非既有資產，需在本計畫內建構）**：
  - **Vault 寫入器**：純 Node.js/Rust 直接寫入 Vault 檔案系統（替代假想的 `sync_obsidian/main.mjs`）
  - **獨立同步守護程式**：由技能內建排程觸發，不依賴外部常駐進程

### 1.2 MVP 功能範圍（第一期必做）
| 功能點 | 驗收標準 |
|--------|----------|
| 對話紀錄自動萃取 | 能從 WORK_LOGS/ 或頻道歷史抓取指定日期範圍對話 |
| 雜訊過濾 | 自動剔除純狀態報告、心跳包、重複摘要等非知識性內容 |
| 摘要生成 | 依時間戳與主題歸納，產出 `YYYY-MM-DD_主題.md` 格式檔案 |
| Frontmatter 標準化 | 自動添加 `tags: [digest, buzz-memory]` 等標準欄位 |
| Vault 寫入 | 直接寫入 `02-Conversation-Digest/` 目錄（Node.js fs.promises） |
| 索引更新 | 自動更新 `INDEX.md` 索引表（日期、主題、路徑、核心提要） |
| 頻道回報 | 沉澱完成後向頻道發送相對路徑與核心提要 |

### 1.3 技術架構設計
```
┌─────────────────────────────────────────────────────────────┐
│  obsidian-digest-sync Skill Module                          │
├─────────────────────────────────────────────────────────────┤
│  入口：skill.md (Buzz Native Skill v1.0 格式)                │
├─────────────────────────────────────────────────────────────┤
│  核心腳本：                                                 │
│  ├── extract_conversations.mjs    # 從源頭抓取原始對話       │
│  ├── filter_noise.mjs             # 過濾雜訊（規則可配置）   │
│  ├── generate_digest.mjs          # 生成結構化摘要 Markdown  │
│  ├── write_to_vault.mjs           # 直接寫入 Vault 檔案系統  │
│  └── update_index.mjs             # 維護 INDEX.md            │
├─────────────────────────────────────────────────────────────┤
│  依賴：                                                     │
│  - Node.js 18+                                              │
│  - BUZZ_AUTH_TAG 環境變數（頻道發文需求）                   │
│  - 無外部常駐守護程式依賴（內建排程觸發）                    │
└─────────────────────────────────────────────────────────────┘
```

### 1.4 代碼符號真實性對照（源碼出處溯源）
| 組件 | 既有代碼參考 | 實際路徑/行號 |
|------|--------------|---------------|
| Vault 寫入邏輯 | **本計畫新建實作** `write_to_vault.mjs` | 使用 Node.js `fs.promises` 直接寫入檔案系統 |
| Frontmatter 格式 | `PLANS/DAILY_EVOLUTION_PROPOSAL_20261102.md:145` | 標籤格式參考模板 |
| 索引表結構 | `C:/私人/ai agent/buzz-obsidian/INDEX.md` | 需確認現有結構（若無則依本計畫規範建立） |
| Buzz CLI 發文 | `buzz messages send --channel --reply-to` | 依據 buzz-cli 技能文檔 |

> **修正說明**：移除對 `sync_obsidian/main.mjs` 的依賴，改為技能內建的直接檔案系統寫入，消除假想依賴風險。

### 1.5 協議鑑權閉環檢查（具體實作細節）
| 操作 | Nostr Kind | Tags 結構 | 加密 | 權限模型 | 重試/降級策略 |
|------|------------|-----------|------|----------|---------------|
| 頻道回報沉澱完成 | `1` (Text Note) | `["e", <thread-root-id>, "", "root"]`, `["e", <parent-id>, "", "reply"]`, `["p", <mentioned-pubkey>]`, `["h", <channel-id>]` | NIP-44 (若頻道加密) | Channel-Scope（需為頻道成員） | 3 次指數退避重試 (1s/2s/4s) → 失敗寫入 `WORK_LOGS/failed_posts_<timestamp>.jsonl` 供人工補發 |
| 提及老闆/審核者 | 同上 | 同上 + `["p", <boss-pubkey>]` | 同上 | 同上 | 同上 |

**具體實作參數**：
- `buzz messages send --channel 5ca019f6-1a27-592a-b015-4baf0eb33572 --reply-to <parent-event-id> --content "<內容>" --mention <boss-pubkey> --mention <sentry-pubkey>`
- 發文前預檢：`buzz channels get --channel <id>` 確認頻道存在與成員身分
- **無 `buzz relay status` 指令**：發文前直接嘗試 `buzz messages send`，若失敗（網路/權限）再走降級邏輯

**失敗降級**：Relay 斷線或權限不足時，改為本地落檔 `WORK_LOGS/pending_channel_reports/`，下次健檢通過時自動重發。

### 1.6 失敗模式推演與緩解
| 失敗場景 | 影響 | 緩解策略 |
|----------|------|----------|
| Vault 路徑不存在/無權限 | 寫入失敗 | 啟動前強制檢查路徑與權限，失敗即報警不重試 |
| 對話源資料為空 | 產出空摘要 | 設定最小內容閾值，低於閾值跳過並記錄警告 |
| Frontmatter 解析錯誤 | 索引損壞 | 寫入前驗證 YAML 語法，採用臨時檔+原子替換 |
| Relay 發文失敗（斷線/權限/頻限） | 頻道無通知 | 本地落檔待重發，指數退避 3 次，最終寫入失敗日誌供人工補發 |
| 磁碟空間不足 | 寫入中斷/損壞 | 寫入前檢查可用空間，低於 100MB 拒絕執行並告警 |

### 1.7 MVP 測試驗證清單
- [ ] **單元測試**：各模組獨立測試（輸入→輸出）
- [ ] **整合測試**：完整流程跑通（源頭→Vault→索引→頻道回報）
- [ ] **邊界測試**：空對話、超長對話、特殊字元、編碼問題
- [ ] **回歸測試**：不破壞既有 `sync_obsidian` 守護功能
- [ ] **人工驗收**：老闆檢查產出的 `.md` 檔案格式與內容品質

### 1.8 實作任務拆解（供 Pollen 執行）
| 任務編號 | 任務名稱 | 預估工時 | 依賴 |
|----------|----------|----------|------|
| OD-01 | 建立技能目錄結構與 skill.md 入口 | 0.5h | - |
| OD-02 | 實作 extract_conversations.mjs（支援 WORK_LOGS 與頻道 API） | 2h | - |
| OD-03 | 實作 filter_noise.mjs（可配置規則引擎） | 1.5h | OD-02 |
| OD-04 | 實作 generate_digest.mjs（模板化輸出） | 2h | OD-03 |
| OD-05 | 實作 write_to_vault.mjs（直接寫入 Vault 檔案系統，原子化替換） | 1.5h | OD-04 |
| OD-06 | 實作 update_index.mjs（原子化更新 INDEX.md） | 1h | OD-05 |
| OD-07 | 實作協議發送模組（buzz CLI 封裝、重試、降級落檔） | 1.5h | OD-05 |
| OD-08 | 整合測試與錯誤處理完善 | 2h | OD-01~07 |
| OD-09 | 撰寫單元測試與 README | 1h | OD-08 |
| **總計** | | **~13h** | |

### 1.9 回滾機制
```bash
# 刪除技能目錄（完整回滾）
rm -rf "C:\Users\user\.buzz\PLANS\SKILL_CANDIDATES\obsidian-digest-sync"

# 或僅移除生成的摘要檔案（保留技能代碼）
rm "C:\私人\ai agent\buzz-obsidian\02-Conversation-Digest\2026-11-02_*.md"
```

---

## 🎯 技能二：system-health-guardian（Buzz 系統健康監控與熔斷自癒）

### 2.1 背景與動機
- **觸發詞頻**：5（系統維運歷史與健康檢查機制中提煉）
- **核心痛點**：
  - OpenRouter 429 配額耗盡無自動切換（RCA 案例一）
  - Buzz CLI mention preflight 失敗（RCA 案例二）
  - Windows 編碼/反斜線轉義問題（RCA 案例三）
  - 背景守護行程（OpenRouter、Sync、Proxy）無統一健康檢查與自動重啟
- **現有資產**：
  - `WORK_LOGS/error.log`：錯誤日誌（經實測存在，含 429/preflight/編碼等失敗堆疊）
  - `RESEARCH/RCA_BUZZ_RECENT_FAILURES.md`：既有根因分析報告（17050 bytes，11/1 建立），**本計畫將擴充此文檔**作為失敗模式知識庫
- **新增實作項目（非既有資產，需在本計畫內建構）**：
  - **CLI 健檢腳本**：`check_cli_health.mjs`（替代假想的 `health_check.py`）
  - **OpenRouter 金鑰管理**：`openrouter_key_manager.mjs`（替代假想的 `tools/openrouter_key_manager.mjs`）
  - **背景服務管理器**：`daemon_manager.mjs`（替代假想的 `buzz-background-services.vbs`，跨平台 Node.js 實作）
  - **新增失敗日誌**：`WORK_LOGS/daily_evolution_failure.log`（本計畫產出，彙總每日演化失敗供掃描器解析）

### 2.2 MVP 功能範圍（第一期必做）
| 功能點 | 驗收標準 |
|--------|----------|
| CLI 狀態診斷 | 執行 `buzz --version`、`buzz config check` 等核心指令驗證 |
| 環境變數檢驗 | 檢查 `BUZZ_RELAY_URL`、`BUZZ_PRIVATE_KEY`、`BUZZ_AUTH_TAG` 等關鍵變數 |
| 熔斷日誌排查 | 掃描 `WORK_LOGS/` 下錯誤堆疊，識別已知失敗模式（429、preflight、編碼） |
| 背景行程監控 | 檢查 3 大守護進程（OpenRouter Key Manager、Sync Obsidian、Proxy）存活狀態 |
| 熔斷器重置 | 確認修復後自動重置熔斷狀態（如清除 failure log 標記） |
| 健全性報告 | 彙整為結構化報告，發送至專屬頻道或工作日誌 |
| 自癒動作 | 針對已知模式（進程死掉、金鑰撞頂）執行自動重啟/切換 |

### 2.3 技術架構設計
```
┌─────────────────────────────────────────────────────────────┐
│  system-health-guardian Skill Module                        │
├─────────────────────────────────────────────────────────────┤
│  入口：skill.md (Buzz Native Skill v1.0 格式)                │
├─────────────────────────────────────────────────────────────┤
│  核心腳本：                                                 │
│  ├── check_cli_health.mjs       # Buzz CLI 核心功能檢查    │
│  ├── check_env_vars.mjs         # 環境變數完整性驗證        │
│  ├── scan_failure_logs.mjs      # 掃描並分類失敗日誌        │
│  ├── check_daemons.mjs          # 背景守護進程存活檢查      │
│  ├── circuit_breaker.mjs        # 熔斷狀態管理與重置        │
│  ├── auto_heal.mjs              # 已知模式自癒執行器        │
│  ├── generate_report.mjs        # 結構化健全性報告生成      │
│  ├── openrouter_key_manager.mjs # OpenRouter 金鑰輪換/配額檢查 │
│  └── daemon_manager.mjs         # 背景服務統一管理（啟停狀態） │
├─────────────────────────────────────────────────────────────┤
│  依賴：                                                     │
│  - Node.js 18+                                              │
│  - BUZZ_AUTH_TAG 環境變數（發送報告需求）                   │
│  - 無外部 .vbs/.py 依賴（全 Node.js 跨平台實作）            │
└─────────────────────────────────────────────────────────────┘
```

### 2.4 代碼符號真實性對照（源碼出處溯源）
| 組件 | 既有代碼參考 | 實際路徑/行號 |
|------|--------------|---------------|
| CLI 健檢邏輯 | **本計畫新建實作** `check_cli_health.mjs` | 直接調用 `buzz --version`、`buzz config check` 等子指令 |
| 環境變數驗證 | **本計畫新建實作** `check_env_vars.mjs` | 讀取 `process.env` 驗證 `BUZZ_RELAY_URL`、`BUZZ_PRIVATE_KEY`、`BUZZ_AUTH_TAG` |
| 失敗日誌掃描 | **本計畫新建實作** `scan_failure_logs.mjs` | 解析 `WORK_LOGS/error.log` 實際格式；**新增** `daily_evolution_failure.log` 彙總格式由本計畫定義 |
| 背景進程管理 | **本計畫新建實作** `daemon_manager.mjs` | Node.js `child_process` 管理，PID 檔案落地 `.buzz/daemons/` |
| OpenRouter 金鑰管理 | **本計畫新建實作** `openrouter_key_manager.mjs` | HTTP 呼叫 OpenRouter API，配額檢查與輪換邏輯 |
| 熔斷狀態檔案 | **本計畫新建實作** `circuit_breaker.mjs` | JSON 格式：`{state, failures, lastFailure, updatedAt}` 落地 `.buzz/circuit-breaker.json` |
| Buzz CLI 發文 | `buzz messages send --channel --reply-to` | 依據 buzz-cli 技能文檔 |

> **修正說明**：移除對 4 個不存在檔案的依賴，全部改為本計畫內建構的新模組，消除假想依賴風險。

### 2.5 協議鑑權閉環檢查（具體實作細節）
| 操作 | Nostr Kind | Tags 結構 | 加密 | 權限模型 | 重試/降級策略 |
|------|------------|-----------|------|----------|---------------|
| 發送健全性報告 | `1` (Text Note) | `["e", <thread-root-id>, "", "root"]`, `["e", <parent-id>, "", "reply"]`, `["p", <mentioned-pubkey>]`, `["h", <channel-id>]` | NIP-44 (若頻道加密) | Channel-Scope（需為頻道成員） | 3 次指數退避重試 → 失敗本地落檔 `WORK_LOGS/health_report_YYYYMMDD.md` |
| 關鍵告警（連續 3 次失敗） | `1` (Text Note) | 同上 + `["p", <boss-pubkey>]`, `["p", <sentry-pubkey>]` | 同上 | 同上 | 立即重試 1 次 → 失敗改用 DM 回退：`buzz dms open --users <boss-pubkey>` 取得 DM channel ID，再 `buzz messages send --channel <dm-channel-id> --content "..."` |
| 自癒動作審計日誌 | 本地檔案 | N/A | N/A | N/A | 寫入 `.buzz/daemons/heal_audit_YYYYMMDD.jsonl` |

**具體實作參數**：
- `buzz messages send --channel 5ca019f6-1a27-592a-b015-4baf0eb33572 --reply-to <parent-event-id> --content "<健全性報告>" --mention <boss-pubkey> --mention <sentry-pubkey>`
- 發文前預檢：`buzz channels get --channel <id>` 確認頻道存在與成員身分
- **無 `buzz relay status` 指令**：發文前直接嘗試 `buzz messages send`，若失敗（網路/權限）再走降級邏輯
- 環境變數讀取：**僅檢查存在性與格式**，嚴禁輸出或記錄金鑰實際值

**失敗降級**：Relay 斷線時改為本地落檔；權限不足時改為 DM 發送給老闆（`buzz dms open --users <boss-pubkey>` 取得 DM channel 再 `buzz messages send`）；兩者皆失敗時寫入審計日誌供事後追蹤。

### 2.6 失敗模式推演與緩解
| 失敗場景 | 影響 | 緩解策略 |
|----------|------|----------|
| CLI 健檢腳本缺失/異常 | CLI 檢查失敗 | 內建最小化 CLI 檢查邏輯作為 fallback（直接調用 buzz 子指令） |
| 背景進程 PID 檢查失敗 | 狀態誤判 | 多重驗證：PID + 端口 + 心跳檔案 + psutil 進程列表 |
| 熔斷日誌格式變更 | 掃描器失效 | 版本化日誌格式，解析器向後相容 |
| 自癒動作觸發條件誤判 | 誤重啟正常服務 | 要求連續 N 次失敗才觸發，並記錄審計日誌 |
| 報告發送失敗 | 可見性缺失 | 本地落檔 `WORK_LOGS/health_report_YYYYMMDD.md` 保底 |
| OpenRouter API 變更/不可用 | 金鑰輪換失效 | 緩存最後已知良好金鑰，離線模式下續用；記錄錯誤不崩潰 |

### 2.7 MVP 測試驗證清單
- [ ] **單元測試**：各檢查模組獨立測試（模擬正常/異常狀態）
- [ ] **整合測試**：完整健檢流程跑通（CLI→環境變數→日誌→守護進程→報告）
- [ ] **故障注入測試**：人工殺掉守護進程、修改環境變數、製造 429 錯誤，驗證自癒
- [ ] **壓力測試**：連續執行 50 次健檢，確認無記憶體洩漏、無殘留進程
- [ ] **回歸測試**：不影響既有 `openrouter_key_manager`、`sync_obsidian` 運作

### 2.8 實作任務拆解（供 Pollen 執行）
| 任務編號 | 任務名稱 | 預估工時 | 依賴 |
|----------|----------|----------|------|
| SH-01 | 建立技能目錄結構與 skill.md 入口 | 0.5h | - |
| SH-02 | 實作 check_cli_health.mjs（Buzz CLI 核心指令探測） | 1.5h | - |
| SH-03 | 實作 check_env_vars.mjs（關鍵環境變數存在性與格式驗證） | 1h | - |
| SH-04 | 實作 scan_failure_logs.mjs（模式識別：429/preflight/編碼/其他） | 2h | - |
| SH-05 | 實作 check_daemons.mjs（3 守護進程多重驗證：PID+端口+心跳+psutil） | 1.5h | - |
| SH-06 | 實作 circuit_breaker.mjs（狀態機：正常/警告/熔斷/重置，JSON 落檔） | 2h | SH-04 |
| SH-07 | 實作 auto_heal.mjs（已知模式自動修復：重啟進程、輪換金鑰） | 2h | SH-05, SH-06 |
| SH-08 | 實作 generate_report.mjs（結構化 Markdown + 頻道發送） | 1h | SH-02~07 |
| SH-09 | 實作 openrouter_key_manager.mjs（配額檢查、金鑰輪換、離線緩存） | 1.5h | SH-03 |
| SH-10 | 實作 daemon_manager.mjs（背景服務統一啟停、狀態查詢、PID 管理） | 1.5h | SH-05 |
| SH-11 | 實作協議發送模組（buzz CLI 封裝、重試、降級、DM 回退） | 1h | SH-08 |
| SH-12 | 整合測試與故障注入驗證 | 2h | SH-01~11 |
| SH-13 | 撰寫單元測試與 README | 1h | SH-12 |
| **總計** | | **~18.5h** | |

### 2.9 回滾機制
```bash
# 刪除技能目錄（完整回滾）
rm -rf "C:\Users\user\.buzz\PLANS\SKILL_CANDIDATES\system-health-guardian"

# 清理可能殘留的守護進程與狀態檔案
taskkill /F /IM "node.exe" /FI "WINDOWTITLE eq *system-health-guardian*"
rm -f "C:\Users\user\.buzz\.buzz\daemons\*.pid"
rm -f "C:\Users\user\.buzz\.buzz\circuit-breaker.json"
rm -f "C:\Users\user\.buzz\.buzz\daemons\heal_audit_*.jsonl"
```

---

## 📦 共同交付物與品質門檻

### 交付物清單（每技能必備）
1. **技能目錄**：`C:\Users\user\.buzz\PLANS\SKILL_CANDIDATES\<skill-name>\`
   - `skill.md`（Buzz Native Skill v1.0 格式，含 Frontmatter、觸發條件、SOP、安全約束）
   - `scripts/`（所有 `.mjs` 核心腳本）
   - `tests/`（單元測試腳本）
   - `README.md`（使用說明、配置範例、故障排查）

2. **測試報告**：`WORK_LOGS/skill_test_<skill-name>_<timestamp>.md`

3. **審核追蹤**：Sentry 四大語意門禁檢查表（簽署版）

### 品質門檻（Definition of Done）
- [ ] 代碼符號真實性：所有引用函式、路徑、變數經實地檢視驗證
- [ ] 協議鑑權閉環：所有 Relay 操作均通過 `BUZZ_AUTH_TAG` 預檢
- [ ] 失敗模式推演：文檔化至少 5 種失敗場景與緩解策略
- [ ] MVP 可跑通：端到端流程無人工介入完整執行成功
- [ ] 單元測試覆蓋：核心邏輯 ≥ 80% 覆蓋率
- [ ] 無硬編碼路徑：所有路徑透過配置或環境變數注入
- [ ] 繁體中文 UTF-8：所有輸出、日誌、文檔統一編碼

---

## 🚀 實施時程規劃（第一期 MVP）

| 階段 | 時間範圍 | 關鍵里程碑 |
|------|----------|------------|
| **規劃審核** | 第 0 天 | 計畫書完成 → Sentry 審核 → 老闆授權 |
| **並行開發** | 第 1-4 天 | Pollen 並行實作 OD-01~09、SH-01~13 |
| **整合測試** | 第 5 天 | 端到端測試、故障注入、回歸測試 |
| **審核驗收** | 第 6 天 | Sentry 四大門禁審核 → 老闆驗收 |
| **落庫生效** | 第 6 天 | 技能狀態更新為「生產就緒」，納入每日進化系統 |

---

## 📋 後續擴展（第二期，視第一期成果決定）

| 技能 | 擴展方向 |
|------|----------|
| obsidian-digest-sync | LLM 增強摘要品質、多 Vault 支援、增量同步、標籤自動推薦 |
| system-health-guardian | Prometheus/Grafana 對接、多機房部署、異常預測、Slack/Discord 多通道告警 |

---

## ✅ 審核簽署欄

| 角色 | 簽署狀態 | 簽署時間 | 備註 |
|------|----------|----------|------|
| **Honey2 (企劃)** | 待簽署 | - | 計畫書起草完成 |
| **Sentry (架構審核)** | 待審核 | - | 四大語意門禁檢查 |
| **老闆 (授權)** | 待授權 | - | 僅接受「同意/開工/實作」口頭指令 |
| **Pollen (實作)** | 待派工 | - | 接收授權後開始開發 |

---

*計畫書版本：1.0 | 建立日期：2026-11-02 | 狀態：草稿待審核*