---
title: "Obsidian Memory Sync Implementation Plan"
tags: [obsidian, memory, sync, nostr, architecture]
status: active
created: 2026-10-25
updated: 2026-10-25
---

# Obsidian Memory Sync 實作計畫書

## 1. 專案概覽

**目標**：建立 Buzz Agent 記憶系統與 Obsidian Vault 的雙向同步機制，讓日常對話、核心記憶、冷記憶自動沉澱為可查詢、可連結的 Markdown 知識庫。

**範圍**：
- Core Memory（熱記憶）同步
- Cold Memory（冷記憶）同步
- 對話脈絡定期匯總歸檔
- Obsidian 端雙向編輯支援（階段二）

**非範圍**：
- 即時流式同步（Webhook 架構留待階段二）
- 多 Vault 多使用者權限控管
- 圖譜視覺化（交由 Obsidian 原生 Graph View）

---

## 2. 架構設計

### 2.1 高層資料流

```
┌──────────────┐     定期拉取      ┌──────────────────┐     渲染 MD      ┌─────────────────┐
│  Buzz Relay  │ ───────────────▶ │  同步腳本         │ ──────────────▶ │  Obsidian Vault │
│  (Nostr)     │   buzz mem get   │  (Python/Node)   │   YAML frontmatter│  (.md files)    │
└──────────────┘                  └──────────────────┘                  └─────────────────┘
        ▲                                 │                                      │
        │                                 ▼                                      ▼
        │                        ┌──────────────────┐                  ┌─────────────────┐
        └─────────────────────── │ Git / rsync      │ ◀────────────── │ 人工編輯/連結    │
                                 │ 雙向同步機制      │    Git commit   │ [[WikiLink]]    │
                                 └──────────────────┘                  └─────────────────┘
```

### 2.2 同步策略

| 記憶類型 | 同步頻率 | 衝突策略 | 檔案命名 |
|----------|----------|----------|----------|
| Core Memory | 每 5 分鐘 | 覆寫（單一權威源） | `core-memory.md` |
| Cold Memory | 每 10 分鐘 | 合併（以 event_id 去重） | `cold-memory/{slug}.md` |
| 對話摘要 | 每 30 分鐘 | 追加（每 thread 一檔） | `conversation-digest/YYYY-MM-DD_{thread_id_short}.md` |

---

## 3. 檔案結構規劃

### 3.1 Obsidian Vault 目標結構

```
Obsidian Vault/
├── 00-Buzz-Memory/
│   ├── core-memory.md                          # 核心記憶（單檔覆寫）
│   ├── cold-memory/
│   │   ├── INDEX.md                            # 索引頁（自動生成）
│   │   ├── PROJECT_ALPHA_PLAN.md
│   │   ├── OAUTH_FLOW_NOTES.md
│   │   └── ...
│   ├── conversation-digest/
│   │   ├── INDEX.md                            # 日期索引
│   │   ├── 2026-10-25_d11cabd0.md
│   │   └── ...
│   └── sync-meta/
│       ├── last-sync.json                      # 同步游標
│       └── conflict-log.json                   # 衝突記錄
```

### 3.2 Markdown Frontmatter 規格

**Core Memory (`core-memory.md`)**：
```yaml
---
title: "Buzz Agent Core Memory"
source: "buzz://memory/core"
synced_at: "2026-10-25T05:30:00Z"
version: 42
tags: [core, identity, rules]
---
```

**Cold Memory (`cold-memory/{slug}.md`)**：
```yaml
---
title: "OAuth Flow Notes"
slug: "OAUTH_FLOW_NOTES"
source: "buzz://memory/cold/OAUTH_FLOW_NOTES"
event_id: "abc123..."           # 來源 Nostr 事件 ID
created_at: "2026-10-20T10:00:00Z"
updated_at: "2026-10-25T05:30:00Z"
tags: [oauth, auth, research]
status: active
---
```

**對話摘要 (`conversation-digest/...md`)**：
```yaml
---
title: "Thread: 記憶與 Obsidian 整合討論"
thread_root: "d11cabd00d3c7a6bf62df82cdb3b424b91be3cf5f7848917ac1536ee1ede875a"
channel: "general"
channel_id: "5ca019f6-1a27-592a-b015-4baf0eb33572"
participants: ["可愛小寶寶", "Fizz", "Honey"]
message_count: 10
date_range: "2026-10-25T03:24:10Z ~ 2026-10-25T05:07:56Z"
synced_at: "2026-10-25T05:35:00Z"
tags: [memory, obsidian, architecture]
---
```

---

## 4. 技術實作細節

### 4.1 同步腳本模組劃分

```
sync_obsidian/
├── main.py                     # 入口點、排程器
├── buzz_client.py              # Buzz CLI 封裝（get/set mem, messages get）
├── obsidian_writer.py          # Markdown + Frontmatter 生成、寫入
├── conversation_digester.py    # Thread 摘要生成（可接 LLM 或啟發式）
├── git_sync.py                 # Git commit / push / pull
├── state_store.py              # 同步游標、去重集合持久化
└── config.yaml                 # 設定檔（Vault 路徑、頻率、過濾規則）
```

### 4.2 關鍵演算法

**增量同步（Cold Memory）**：
```python
def sync_cold_memory():
    # 1. 讀取本地狀態：已同步 event_id 集合
    synced_ids = state_store.get_synced_event_ids("cold")
    
    # 2. 從 Buzz 列出所有 cold memory slugs
    remote_slugs = buzz_client.list_cold_memory_slugs()
    
    for slug in remote_slugs:
        mem = buzz_client.get_cold_memory(slug)
        if mem.event_id in synced_ids and mem.updated_at <= state_store.get_last_sync(slug):
            continue  # 無變更
        
        # 3. 寫入/更新檔案
        obsidian_writer.write_cold_memory(slug, mem)
        state_store.mark_synced("cold", mem.event_id, mem.updated_at)
```

**對話摘要生成**：
- 階段一：啟發式（擷取首/尾 3 則、@mention 統計、關鍵字 TF-IDF）
- 階段二：接入本地 LLM（Ollama / llama.cpp）生成 200 字摘要

### 4.3 Git 雙向同步策略

```bash
# 拉取遠端（Obsidian 可能在別台機器編輯）
git -C "$VAULT" pull --rebase origin main

# 推送本地同步腳本產生的變更
git -C "$VAULT" add -A
git -C "$VAULT" commit -m "chore(sync): $(date -u +%Y-%m-%dT%H:%M:%SZ) auto-sync from Buzz"
git -C "$VAULT" push origin main
```

**衝突檢測**：
- `git merge` 失敗 → 記錄 `conflict-log.json`、發送 Buzz 通知給擁有者
- 人工解決後再次 `git add` + `git commit`

---

## 5. 設定檔範例 (`config.yaml`)

```yaml
vault_path: "~/Obsidian/Buzz-Knowledge"
buzz:
  relay_url: "wss://presentyourlove.communities.buzz.xyz"
  # 環境變數 BUZZ_PRIVATE_KEY, BUZZ_AUTH_TAG 需預先設定

sync_intervals:
  core_memory: 300        # 秒
  cold_memory: 600
  conversation_digest: 1800

conversation_digest:
  enabled: true
  min_messages: 3         # 少於此數不生成摘要
  summarizer: "heuristic" # 或 "ollama:llama3.1:8b"

git:
  auto_commit: true
  auto_push: true
  remote: "origin"
  branch: "main"

filters:
  exclude_channels: []    # 不同步的頻道 ID 列表
  exclude_tags: ["private", "tmp"]
```

---

## 6. 風險評估與緩解

| 風險 | 等級 | 緩解方案 |
|------|------|----------|
| Relay 事件遺漏/重序 | 中 | 以 `created_at` + `event_id` 雙鍵去重；保留 7 天完整日誌供回溯 |
| Obsidian 手改與腳本覆寫衝突 | 高 | Git 紀錄每次變更；衝突時暫停自動推送、發通知人工介入 |
| 機密資料外洩（同步到本地磁碟） | 高 | Vault 應加密（VeraCrypt/系統級加密）；`config.yaml` 排除敏感 tag |
| 同步腳本當機導致資料滯後 | 中 | systemd timer / cron 監控；心跳檢查寫入 `last-sync.json` |
| 大量 Cold Memory 導致 Vault 膨脹 | 低 | 定期歸檔舊 slug；支援 `status: archived` 軟刪除 |

---

## 7. 實作里程碑

| 階段 | 交付物 | 預估工時 | 驗收標準 |
|------|--------|----------|----------|
| **M1: 骨架與 Core Sync** | `sync_obsidian/` 專案、Core Memory 同步、Git 自動提交 | 4h | `core-memory.md` 每 5 分鐘自動更新、Git 歷史可追溯 |
| **M2: Cold Memory + 索引** | Cold Memory 同步、自動生成 `INDEX.md`、去重邏輯 | 3h | 新增/修改 cold memory 10 分鐘內出現在 Vault、索引頁連結正確 |
| **M3: 對話摘要** | Thread 摘要生成、日期索引、參與者統計 | 3h | 指定 thread 生成摘要、Markdown 可閱讀、WikiLink 可點擊 |
| **M4: 雙向編輯支援** | Git pull/rebase、衝突通知、手動解決流程文件 | 2h | Obsidian 端修改 cold memory → 經 Git → Buzz `mem set` 成功 |
| **M5: 穩健化** | systemd service、健康檢查、錯誤重試、文件化 | 2h | 連續運行 7 天無人工干預、日誌完整 |

**總計約 14 小時**（可平行化 M2/M3）

---

## 8. 後續擴充方向

1. **Webhook 即時同步**：Buzz Relay → HTTP endpoint → 觸發增量同步（秒級延遲）
2. **語意搜尋整合**：向量化 Vault 內容 → 本地 RAG 查詢
3. **多 Agent 記憶隔離**：以 `agent_pubkey` 命名空間區分 Honey/Fizz/Pollen 記憶
4. **行動端同步**：Termux / Working Copy (iOS) 同步腳本輕量化版本

---

## 9. 給 @Pollen 的實作備註

- **語言建議**：Python 3.11+（`pyyaml`, `gitpython`, `httpx`）或 Node.js（`gray-matter`, `simple-git`）
- **Buzz CLI 介面**：只依賴 `buzz mem get/set/list`、`buzz messages get`，無需直連 Relay
- **測試策略**：
  - Unit test：frontmatter 生成、去重邏輯、Git 操作 mock
  - Integration test：對真實 Vault 跑一輪完整同步、斷網重聯
- **部署方式**：systemd user service + `Restart=on-failure`，或 Docker Compose sidecar

---

## 10. 已確認事項（老闆 2026-10-25 正式確認）

1. **Obsidian Vault 本機路徑**：
   - `c:\私人\ai agent\buzz-obsidian`

2. **Git 版本控制儲存庫**：
   - 純本機 Git 版本追蹤，暫不接遠端

3. **同步頻率設定**：
   - Core Memory：每 5 分鐘 (300 秒)
   - Cold Memory：每 10 分鐘 (600 秒)
   - 對話脈絡摘要：每 30 分鐘 (1800 秒)

4. **對話摘要深度**：
   - 交由專屬 Agent `Wax（記憶清潔工）` 負責語意深度提煉並生成 `[[雙向連結]]`
   - 同步模組負責排程與落檔

5. **敏感資料過濾**：
   - 標籤黑名單：`#private` `#secret` `#tmp` `#nosync` `#個人` 完全不予寫入
   - 機敏正則遮罩：所有 API Key、Token、密碼自動遮罩為 `[PROTECTED_SECRET]`
   - 完全遵循 `c:\私人\ai agent\buzz-obsidian\歡迎.md` 的規格標準

---

*文件狀態：草稿，待老闆確認後轉為 `active` 並交付執行*
