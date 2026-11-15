---
title: "Obsidian Digest Sync & System Health Guardian 技能實作計畫書"
tags: [skill-implementation, obsidian-digest-sync, system-health-guardian, sentry-review]
status: pending-sentry-review
created: 2026-11-02
author: "Honey2 (專案架構企劃官)"
target_channel: "5ca019f6-1a27-592a-b015-4baf0eb33572"
reviewer: "Sentry (安全與架構總監)"
implementer: "Pollen (執行工程師)"
approver: "可愛小寶寶 (老闆)"
source_proposal: "DAILY_EVOLUTION_PROPOSAL_20261102.md"
skill_candidates:
  - obsidian-digest-sync
  - system-health-guardian
---

# Obsidian Digest Sync & System Health Guardian 技能實作計畫書

> **來源**：每日系統進化提案 - 2026-11-02 (DAILY_EVOLUTION_PROPOSAL_20261102.md)
> **技能候選檔案**：
> - `C:\Users\user\.buzz\PLANS\SKILL_CANDIDATES\obsidian-digest-sync\SKILL.md`
> - `C:\Users\user\.buzz\PLANS\SKILL_CANDIDATES\system-health-guardian\SKILL.md`

---

## 一、專案背景與目標

### 1.1 老闆指示
主理人於 2026-11-02 晚間下達指示：
> 「11/2晚上10點半出咗個進化建議書。有2個skill建議。我想要計劃書。」

### 1.2 兩項技能候選概覽

| 技能代號 | 名稱 | 觸發詞頻 | 核心價值 |
|---------|------|---------|----------|
| `obsidian-digest-sync` | Obsidian 對話摘要與第二大腦同步技能 | 1168 | 自動化對話摘要提取、標籤分類與 Obsidian 雙向知識庫同步 |
| `system-health-guardian` | Buzz 系統健康監控與熔斷自癒技能 | 5 | 自動化 CLI 狀態診斷、環境變數檢驗、熔斷日誌排查與常駐守護重啟 |

### 1.3 設計鐵律 (遵循既有技能實作標準)
1. **Boss Approval Gatekeeper**：未獲老闆明確回覆「同意」前，嚴禁自動執行任何代碼實體變更
2. **本地優先**：所有中間狀態與日誌儲存於本地 `WORK_LOGS/` 與 `PLANS/`
3. **通訊紀律**：所有進度回報必須透過 Nostr 協議專屬頻道 `5ca019f6-1a27-592a-b015-4baf0eb33572` 呈報
4. **增量修改**：修改既有程式碼必須先讀取原檔，嚴禁整檔覆寫
5. **防崩潰降級**：缺少 API Key 或環境變數時，嚴禁 `process.exit(1)`，必須進入未配置待機模式並優雅報錯

---

## 二、技能一：obsidian-digest-sync 詳細實作規劃

### 2.1 技能規格回顧 (來源：SKILL_CANDIDATES/obsidian-digest-sync/SKILL.md)

**觸發場景**：
- 日常對話結束或定時收割時需要沉澱知識
- 使用者指示將重大決策或技術架構整理至 Obsidian
- 需要維護 `02-Conversation-Digest` 目錄之 Markdown 結構時

**前置需求**：
- **知識庫目錄 `C:/私人/ai agent/buzz-obsidian` 具備讀寫權限**（外部 Vault，需使用者預先配置）
- 內容必須使用 UTF-8 (no BOM) 編碼與繁體中文
- **本計畫包含同步守護程式的實作任務（ODS-9），不依賴既有不存在的 `sync_obsidian/main.mjs`**

**標準作業程序 (SOP)**：
1. 檢查今日對話紀錄並過濾純機器狀態報告雜訊
2. 依據時間戳與主題進行歸納，產生符合 `YYYY-MM-DD_主題.md` 之摘要檔
3. 自動添加 Frontmatter 標籤 (`tags: [digest, buzz-memory]`)
4. **直接寫入 Obsidian Vault 檔案系統**（`02-Conversation-Digest/` 目錄），並原子更新 `INDEX.md`
5. 向頻道回報沉澱筆記之相對路徑與核心提要
6. **由獨立守護程式（ODS-9 實作）負責雙向同步與衝突解決**

### 2.2 架構設計

```
┌─────────────────────────────────────────────────────────────────┐
│                    obsidian-digest-sync 技能架構                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │  對話收割器   │───▶│  摘要生成器   │───▶│  Obsidian    │      │
│  │ Conversation │    │   Digest     │    │   Writer     │      │
│  │  Harvester   │    │  Generator   │    │  (直寫檔案)   │      │
│  └──────────────┘    └──────────────┘    └──────┬───────┘      │
│        │                    │                    │              │
│        ▼                    ▼                    ▼              │
│  ┌──────────────────────────────────────────────────────┐       │
│  │              本地檔案系統寫入                          │       │
│  │  02-Conversation-Digest/YYYY-MM-DD_主題.md            │       │
│  │  INDEX.md (原子讀取→合併→寫入)                        │       │
│  └──────────────────────────────────────────────────────┘       │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────┐       │
│  │           C:/私人/ai agent/buzz-obsidian              │       │
│  │  (外部 Vault，技能僅負責寫入；同步由獨立守護程式處理)  │       │
│  └──────────────────────────────────────────────────────┘       │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────┐       │
│  │         sync_guardian.py (ODS-9，本計畫新增實作)        │       │
│  │  - 檔案監控 (notify/fsnotify)                          │       │
│  │  - 雙向同步邏輯 (Git/rsync/Rclone 可配置)               │       │
│  │  - 衝突解決策略 (last-write-wins / manual / merge)     │       │
│  │  - 以獨立行程/服務運行，不阻塞技能主流程                │       │
│  └──────────────────────────────────────────────────────┘       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 模組化實作清單

| 模組代號 | 檔案路徑 | 職責 | 優先級 |
|---------|---------|------|--------|
| **ODS-1** | `skills/obsidian-digest-sync/harvester.py` | 對話收割：從頻道讀取指定日期範圍訊息，過濾機器狀態報告 | P0 |
| **ODS-2** | `skills/obsidian-digest-sync/digest_generator.py` | 摘要生成：依時間戳/主題歸納，產生 Markdown + Frontmatter | P0 |
| **ODS-3** | `skills/obsidian-digest-sync/obsidian_writer.py` | **Obsidian 直寫**：直接寫入 Vault 檔案系統（`02-Conversation-Digest/`），原子更新 `INDEX.md`，**不依賴外部守護程式 API** | P0 |
| **ODS-4** | `skills/obsidian-digest-sync/__init__.py` | 技能入口：實作 Buzz Native Skill v1.0 標準介面 (`run()`, `validate()`) | P0 |
| **ODS-5** | `skills/obsidian-digest-sync/config.yaml` | 配置檔：知識庫路徑、標籤規則、過濾關鍵字、同步守護程式配置區塊 | P1 |
| **ODS-6** | `skills/obsidian-digest-sync/tests/test_harvester.py` | 單元測試：收割邏輯、過濾邏輯、日期解析 | P1 |
| **ODS-7** | `skills/obsidian-digest-sync/tests/test_digest_generator.py` | 單元測試：摘要格式、Frontmatter 生成、檔名規則 | P1 |
| **ODS-8** | `skills/obsidian-digest-sync/tests/test_obsidian_writer.py` | 單元測試：寫入路徑構建、INDEX 更新、衝突處理 | P1 |
| **ODS-9** | `skills/obsidian-digest-sync/sync_guardian.py` | **同步守護程式（新增實作）**：檔案監控、雙向同步 (Git/rsync/Rclone 可配置)、衝突解決、獨立行程運行 | P1 |
| **ODS-10** | `skills/obsidian-digest-sync/tests/test_sync_guardian.py` | 單元測試：檔案監控、同步邏輯、衝突解決 | P1 |

### 2.4 關鍵技術細節

#### 2.4.1 對話收割與雜訊過濾
```python
# skills/obsidian-digest-sync/harvester.py 核心邏輯
FILTER_KEYWORDS = [
    "系統狀態報告", "心跳檢測", "健康檢查", "進度通報",
    "BUZZ_HEALTH", "DAILY_EVOLUTION", "WORKFLOW_STATUS"
]

def harvest_conversations(channel_id: str, date: str) -> List[Message]:
    """從頻道收割指定日期的對話，自動過濾機器狀態報告雜訊"""
    messages = buzz_messages_get(channel_id, date)
    return [m for m in messages if not is_bot_status_report(m)]
```

#### 2.4.2 Obsidian 直寫與原子 INDEX 更新
- **直寫策略**：技能直接向 `C:/私人/ai agent/buzz-obsidian/02-Conversation-Digest/` 寫入檔案，**不依賴任何外部守護程式 API**
- **INDEX.md 原子更新**：讀取現有 INDEX → 合併新條目 → 寫入暫存檔 → `os.replace()` 原子覆寫，避免並發衝突
- **檔名衝突處理**：同一天同主題 → 自動附加序號 `YYYY-MM-DD_主題_2.md`
- **權限失敗降級**：Vault 目錄不存在或無權限 → 記錄錯誤、標記技能狀態 `BLOCKED_PERMISSION`、向頻道報警，**不崩潰**

#### 2.4.3 同步守護程式 (ODS-9，本計畫新增實作)
```python
# skills/obsidian-digest-sync/sync_guardian.py 核心職責
class SyncGuardian:
    def __init__(self, config: SyncConfig):
        self.vault_path = config.vault_path
        self.sync_method = config.sync_method  # "git" | "rsync" | "rclone"
        self.conflict_strategy = config.conflict_strategy  # "last-write-wins" | "manual" | "merge"
    
    def watch_and_sync(self):
        """使用 notify/fsnotify 監控 Vault 目錄，檢測變更觸發同步"""
        # 1. 檔案系統事件監聽
        # 2. 變更去抖動 (debounce 2s)
        # 3. 執行同步 (Git commit+push / rsync / rclone)
        # 4. 衝突檢測與解決
        # 5. 同步結果回報頻道 (Kind 9, tags: ["sync", "obsidian"])
    
    def resolve_conflict(self, local: Path, remote: Path) -> Path:
        """依策略解決衝突：last-write-wins / manual (建立 .conflict 檔) / merge (三方合併)"""
        ...
```
- **獨立行程**：以獨立 Python 行程/Windows 服務運行，**不阻塞技能主流程**
- **配置化同步後端**：支援 Git (預設)、rsync、Rclone，由 `config.yaml` 指定
- **Nostr 回報**：同步完成/失敗/衝突均發送 Kind 9 事件至頻道，含 `tags: ["sync", "obsidian", "result"]`

#### 2.4.4 容錯與降級
- 知識庫目錄不存在或無權限 → 記錄錯誤、標記技能狀態 `BLOCKED_PERMISSION`、向頻道報警，**不崩潰**
- 同步守護程式未運行 → 技能仍完成本地寫入，標記 `PENDING_SYNC`，守護程式啟動時自動補同步
- 檔案名衝突 (同一天同主題) → 自動附加序號 `YYYY-MM-DD_主題_2.md`
- 同步衝突無法自動解決 → 標記 `MANUAL_RESOLUTION_REQUIRED`、建立 `.conflict` 檔、@mention 老闆

### 2.5 里程碑與工期

| 里程碑 | 任務 | 驗收標準 | 預估工期 |
|--------|------|----------|----------|
| **M1** | ODS-1, ODS-2, ODS-5 | 能從頻道收割對話、生成符合規範的 Markdown 摘要檔 | 0.5 天 |
| **M2** | ODS-3, ODS-4 | 技能能直寫 Vault 檔案系統、原子更新 INDEX、端到端流程通過 | 0.5 天 |
| **M3** | ODS-6, ODS-7, ODS-8 | 單元測試覆蓋率 ≥ 90%，核心流程端到端通過 | 0.5 天 |
| **M4** | ODS-9, ODS-10 | 同步守護程式實作完成、檔案監控/同步/衝突解決測試通過 | 1 天 |
| **M5** | 整合測試、文檔 | 真實頻道端到端演練通過 (寫入→同步→回報)，產出 `SKILL.md` 更新版 | 0.5 天 |

---

## 三、技能二：system-health-guardian 詳細實作規劃

### 3.1 技能規格回顧 (來源：SKILL_CANDIDATES/system-health-guardian/SKILL.md)

**觸發場景**：
- 日常進化流程啟動前之例行前置體檢
- 背景守護行程異常退出或通訊失敗時
- 熔斷器跳脫時之故障排查

**前置需求**：
- 本機具備 Python 3 與 Node.js 執行環境
- 具備讀取 `WORK_LOGS/daily_evolution_failure.log` 權限
- **本計畫包含健康檢查腳本與守護管理腳本的實作任務（SHG-8, SHG-9），不依賴既有不存在的 `health_check.py` / `buzz-background-services.vbs`**

**標準作業程序 (SOP)**：
1. 執行 **內建健康檢查模組 (SHG-1 實作)** 檢查 Buzz CLI 與核心變數
2. 檢查 `WORK_LOGS/` 下之失敗日誌與錯誤堆疊
3. 檢視 3 大背景常駐行程運作狀態 (PID、端口、HTTP 健康端點、心跳)
4. 若發現熔斷日誌，排查根本原因並於確認修復後進行重置
5. 彙整健全性報告呈報至專屬頻道
6. **由獨立守護管理模組 (SHG-9 實作) 負責背景服務的啟停與重啟**

### 3.2 架構設計

```
┌─────────────────────────────────────────────────────────────────┐
│                  system-health-guardian 技能架構                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  環境檢查器   │  │  日誌分析器   │  │  進程監控器   │          │
│  │ EnvChecker   │  │ LogAnalyzer  │  │ ProcessMon   │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         └─────────────────┼─────────────────┘                   │
│                           ▼                                     │
│              ┌────────────────────────┐                          │
│              │     熔斷狀態評估引擎     │                          │
│              │   CircuitBreakerEval   │                          │
│              └───────────┬────────────┘                          │
│                          │                                        │
│           ┌──────────────┼──────────────┐                        │
│           ▼              ▼              ▼                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐             │
│  │  自癒執行器   │  │  守護管理器   │  │  報告生成器   │             │
│  │ SelfHealer   │  │ GuardianMgr  │  │ Reporter     │             │
│  └──────────────┘ └──────────────┘ └──────────────┘             │
│        │              │              │                           │
│        ▼              ▼              ▼                           │
│  buzz-background-  工作流狀態標記    頻道回報                     │
│  services.vbs     (BLOCKED_*)       (健全性報告)
