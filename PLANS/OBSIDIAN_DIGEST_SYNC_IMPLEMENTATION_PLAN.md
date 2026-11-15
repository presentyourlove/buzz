---
title: "Obsidian Digest Sync 技能實作計畫書"
tags: [obsidian-digest-sync, skill, implementation, obsidian, sync]
status: draft
created: 2026-11-02
version: "1.0"
---

# Obsidian Digest Sync 技能實作計畫書

> **技能名稱**：`obsidian-digest-sync`
> **來源提案**：每日系統進化提案 - 2026-11-02 (技能合成候選 #1)
> **狀態**：待老闆批准立項
> **編寫人**：Wax (記憶清潔工與歸檔官)
> **審核人**：@Sentry
> **批准人**：可愛小寶寶（主理人）

---

## 1. 專案背景與核心需求

### 1.1 來自技能候選規範 (SKILL.md)
- **描述**：自動化對話摘要提取、標籤分類與 Obsidian 第二大腦雙向知識庫同步流程
- **觸發詞頻**：1168 (高頻需求)
- **觸發場景**：
  - 日常對話結束或定時收割時需要沉澱知識
  - 使用者指示將重大決策或技術架構整理至 Obsidian
  - 需要維護 `02-Conversation-Digest` 目錄之 Markdown 結構時

### 1.2 核心目標
- **自動化摘要**：從 Buzz 頻道對話中提取核心決策、架構共識、待辦清單
- **結構化歸檔**：產出符合 `YYYY-MM-DD_主題.md` 命名規範的 Markdown 檔案
- **雙向同步**：透過既有 `PLANS/sync_obsidian/main.mjs` 守護程式同步至 `/c/私人/ai agent/buzz-obsidian`
- **索引維護**：自動更新 `02-Conversation-Digest/INDEX.md` 索引表
- **完成通知**：同步完成後透過 Nostr Kind:1 事件發布至頻道，含 `e` 標籤引用根事件、`p` 標籤 @mention 相關代理人

---

## 2. 架構設計

### 2.1 整體架構圖

```
+-----------------------------------------------------------------+
|                   Obsidian Digest Sync Skill                    |
+-----------------------------------------------------------------+
|  +--------------+    +--------------+    +------------------+  |
|  |  Collector   |--->|  Extractor   |--->|  Formatter       |  |
|  |  (buzz msg)  |    |  (LLM摘要)   |    |  (Frontmatter)   |  |
|  +--------------+    +--------------+    +--------+---------+  |
|                                                   |            |
|  +--------------+    +--------------+             v            |
|  |  Syncer      |<---|  Writer      |    +------------------+  |
|  |  (sync_obs)  |    |  (Vault寫入) |    |  INDEX.md 更新   |  |
|  +--------------+    +--------------+    +------------------+  |
+-----------------------------------------------------------------+
```

### 2.2 元件說明

| 元件 | 負責職責 | 實作位置 |
|------|----------|----------|
| Collector | 從 Buzz 獲取頻道訊息 (指定日期/範圍) | `buzz messages get` 或現有 harvest 機制 |
| Extractor | LLM 進行對話萃取：決策、架構、待辦、雜訊過濾 | 復用 `buzz-agent` 內建 LLM (`agent.rs`) |
| Formatter | 產出標準 YAML Frontmatter + Markdown 內文 | 新增 `crates/buzz-agent/src/digest_formatter.rs` |
| Writer | 寫入 Obsidian Vault (`02-Conversation-Digest/`) | 新增 `crates/buzz-agent/src/obsidian_writer.rs` |
| Syncer | 呼叫既有 `PLANS/sync_obsidian/main.mjs` 進行雙向同步 | 復用現有 Node.js 守護程式 (工作區路徑) |
| Indexer | 更新 `INDEX.md` 索引表 | 內建於 Writer |
| Notifier | 同步完成後發布 Nostr 事件通知頻道 | 復用 `builtin::call_sign_and_send_event` (Kind:1) |

---

## 3. 詳細實作規劃

### 3.1 新增檔案清單

```
crates/buzz-agent/src/
+-- digest_collector.rs      # 訊息收集與預處理
+-- digest_extractor.rs      # LLM 摘要萃取邏輯
+-- digest_formatter.rs      # Frontmatter + Markdown 格式化
+-- obsidian_writer.rs       # Vault 寫入與 INDEX 更新
+-- digest_sync.rs           # 主協調器
+-- daemon.rs                # (修改) 掛載定時任務
```

### 3.2 核心資料結構

```rust
// digest_collector.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessages {
    pub channel_id: String,
    pub date: String,              // YYYY-MM-DD
    pub messages: Vec<BuzzMessage>,
    pub participant_count: usize,
    pub message_count: usize,
}

// digest_extractor.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestExtraction {
    pub summary: String,                    // 核心摘要 (200字內)
    pub key_decisions: Vec<String>,         // 關鍵決策清單
    pub architecture_consensus: Vec<String>, // 架構共識
    pub action_items: Vec<ActionItem>,      // 待辦落地清單
    pub topics: Vec<String>,                // 討論主題標籤
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub description: String,
    pub assignee: Option<String>,           // @mention 的代理人
    pub priority: Priority,                 // High/Medium/Low
    pub due_date: Option<String>,
}

// digest_formatter.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestFrontmatter {
    pub title: String,                      // "YYYY-MM-DD_主題"
    pub tags: Vec<String>,                  // [digest, buzz-memory, topic...]
    pub status: String,                     // "active"
    pub created: String,                    // YYYY-MM-DD
    pub channel_id: String,
    pub message_count: usize,
    pub participant_count: usize,
}

// obsidian_writer.rs
#[derive(Debug, Clone)]
pub struct DigestOutput {
    pub frontmatter: DigestFrontmatter,
    pub content: String,                    // 完整 Markdown 內文
    pub file_path: PathBuf,                 // 相對 Vault 路徑
}
```

### 3.3 LLM 摘要提示詞設計

```markdown
# Digest Extractor System Prompt

你是 Buzz 團隊的記憶清潔工 Wax。任務：從頻道對話中萃取「去蕪存菁」的結構化摘要。

## 輸入
- 頻道 ID、日期、訊息列表 (含發言者、時間、內容、@mention)

## 輸出 (嚴格 JSON)
{
  "summary": "核心摘要 (<=200 字，結論先行)",
  "key_decisions": ["決策1", "決策2"],
  "architecture_consensus": ["架構共識1", "架構共識2"],
  "action_items": [
    {"description": "任務描述", "assignee": "@Agent名", "priority": "High", "due_date": "2026-11-03"}
  ],
  "topics": ["技術債", "部署", "技能合成"]
}

## 萃取規則 (鐵律)
1. **剔除**：客套問候、閒聊廢話、重複發言、純狀態回報 (除非含決策)
2. **保留**：老闆關鍵決策、架構共識、具體待辦落地、阻礙與解決方案
3. **格式**：繁體中文、結構化、雙向連結標註核心實體 `[[實體名稱]]`
4. **安全**：偵測到 API Key/Token/私鑰/密碼 -> 替換為 `[PROTECTED_SECRET]`
5. **標籤**：從 topics 自動生成，小寫連字符 (如 `tech-debt`, `deployment`)
```

---

## 4. 部署與配置

### 4.1 環境變數 (新增)

```bash
# .env.example 追加
# Obsidian Digest Sync
BUZZ_DIGEST_SYNC_ENABLED=true
BUZZ_DIGEST_SYNC_CHANNEL=5ca019f6-1a27-592a-b015-4baf0eb33572
BUZZ_DIGEST_SYNC_SCHEDULE="0 3 * * *"        # 每日 03:00 UTC 收割前日
BUZZ_OBSIDIAN_VAULT_PATH=/c/私人/ai agent/buzz-obsidian
BUZZ_DIGEST_OUTPUT_DIR=02-Conversation-Digest
BUZZ_SYNC_OBSIDIAN_CMD=node PLANS/sync_obsidian/main.mjs
BUZZ_SYNC_WORKING_DIR=/c/Users/user/.buzz
```

### 4.2 Config 結構擴充

```rust
// crates/buzz-agent/src/config.rs
pub struct DigestSyncConfig {
    pub enabled: bool,
    pub channel_id: String,
    pub schedule: String,
    pub vault_path: PathBuf,
    pub output_dir: String,
    pub sync_command: String,
    pub working_dir: PathBuf,
}

impl Default for DigestSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            channel_id: String::new(),
            schedule: "0 3 * * *".into(),
            vault_path: PathBuf::from("/c/私人/ai agent/buzz-obsidian"),
            output_dir: "02-Conversation-Digest".into(),
            sync_command: "node PLANS/sync_obsidian/main.mjs".into(),
            working_dir: PathBuf::from("/c/Users/user/.buzz"),
        }
    }
}
```

---

## 5. 測試策略

### 5.1 單元測試

| 模組 | 測試重點 |
|------|----------|
| digest_collector | 訊息獲取、日期過濾、參與者統計 |
| digest_extractor | LLM 輸出 JSON 解析、欄位驗證、雜訊過濾邏輯 |
| digest_formatter | Frontmatter 正確性、命名規範、標籤生成 |
| obsidian_writer | 檔案寫入、INDEX.md 更新、路徑安全性 |
| digest_sync | 端到端流程 Mock |

### 5.2 整合測試

```bash
# 手動觸發單次收割
cargo run -p buzz-agent -- --digest-sync-run-once --date 2026-11-01
```

**驗收標準**：
- 成功讀取指定日期頻道訊息
- LLM 萃取輸出符合 JSON Schema
- 產出檔案命名符合 `YYYY-MM-DD_主題.md`
- Frontmatter 包含所有必要欄位
- 寫入 Vault 並更新 `INDEX.md`
- 呼叫 `sync_obsidian/main.mjs` 同步成功
- 向頻道回報沉澱筆記路徑與核心提要

---

## 6. 失敗模式推演

| # | 失敗場景 | 影響 | 偵測點 | 具體對策 |
|---|----------|------|--------|----------|
| F1 | LLM 回傳非 JSON / 缺欄位 | 單日摘要生成失敗 | serde_json::from_str 失敗 | 重試 2 次 (含 prompt 重發) -> 失敗則標記「萃取失敗」、產出最小可用摘要 (僅統計數據) |
| F2 | Vault 路徑不存在/無權限 | 檔案寫入失敗 | std::fs::write Err | 記錄錯誤、發布通知至頻道、嚴禁 panic、跳過寫入但保留記憶體結果 |
| F3 | sync_obsidian/main.mjs 執行失敗 | 雙向同步中斷 | 子進程非 0 退出碼 | 記錄錯誤輸出、標記「同步待重試」、背景任務每 30 分鐘重試 |
| F4 | INDEX.md 衝突/損壞 | 索引更新失敗 | 讀取/寫入失敗 | 載入時容錯：損壞則備份 .corrupt、重建索引 (掃描目錄所有 .md) |
| F5 | 頻道訊息獲取失敗 (Relay 斷線) | 無原料可萃取 | buzz messages get 失敗 | 記錄錯誤、標記「收割延遲」、下輪排程自動補齊 |

---

## 7. 分期交付計畫 (MVP 敏捷引導)

### Phase A：CLI 驗證期 (第 1 週，~8h)
**目標**：驗證核心萃取管線 (收集 -> LLM -> Markdown)，**不接入 Daemon/排程/同步**

| 交付物 | 內容 | 驗收 |
|--------|------|------|
| digest_collector.rs | 訊息獲取、日期過濾 | cargo test 通過；--dry-run 輸出 JSON |
| digest_extractor.rs | LLM 提示詞、JSON 解析 | Mock LLM 輸出正確解析 |
| digest_formatter.rs | Frontmatter、命名、標籤 | 產出符合規範的 .md 檔案 |
| obsidian_writer.rs | Vault 寫入、INDEX 更新 | 本地寫入測試通過 |
| digest_sync.rs (CLI) | 串聯管線、手動觸發 | --run-once --date YYYY-MM-DD 產出本地報告 |

### Phase B：Daemon 整合期 (第 2 週，~6h)
**目標**：掛載排程、自動同步、錯誤重試

| 交付物 | 內容 | 驗收 |
|--------|------|------|
| daemon.rs 掛載 | 讀取 config、啟動排程 | BUZZ_DIGEST_SYNC_ENABLED=true 即啟用 |
| 排程器 | cron 解析、準時觸發 | Daemon 啟動 24h 內準時在 03:00 UTC 執行 |
| 同步重試機制 | 背景輪詢、指數退避 | Relay 斷線 1h 後恢復，自動補發 |
| 文檔 | SKILL.md 更新、部署指南 | 新成員按文檔 10 分鐘部署完成 |

---

## 8. 里程碑與工時總表

| 階段 | 交付物 | 預估工時 | 驗收標準 |
|------|--------|----------|----------|
| M1 | digest_collector.rs | 2h | 單測全綠、真實頻道可跑通 |
| M2 | digest_extractor.rs | 3h | LLM 輸出穩定符合 Schema |
| M3 | digest_formatter.rs | 1h | 檔案命名、Frontmatter 規範 |
| M4 | obsidian_writer.rs | 2h | Vault 寫入、INDEX 更新正確 |
| M5 | digest_sync.rs (Phase A) | 2h | --run-once 端到端產出本地報告 |
| M6 | Daemon 掛載與排程 (Phase B) | 3h | 定時自動執行、同步重試生效 |
| M7 | 文檔與部署驗證 | 1h | 部署指南通過驗證 |
| 總計 | | ~14h | |

---

## 9. 代碼出處溯源

| 計畫引用 | 真實源碼路徑 | 行號區間 |
|----------|--------------|----------|
| Daemon 結構與生命週期 | crates/buzz-agent/src/daemon.rs | 12-78 |
| Config 結構與環境讀取 | crates/buzz-agent/src/config.rs | 426-800 |
| Agent 與 LLM 調用 | crates/buzz-agent/src/agent.rs | 143-420 |
| buzz-cli 訊息獲取 | crates/buzz-cli/src/messages.rs | 14-200 |
| builtin::call_sign_and_send_event | crates/buzz-agent/src/builtin.rs | 251-380 |
| sync_obsidian 守護程式 | PLANS/sync_obsidian/main.mjs | 全檔 (工作區) |

---

## 10. 風險與緩解

| 風險 | 機率 | 影響 | 緩解策略 |
|------|------|------|----------|
| LLM 萃取品質不穩 | 中 | 高 | 固定 system prompt、few-shot 範例、輸出 Schema 強制驗證 |
| Vault 路徑跨平台差異 | 低 | 中 | 使用 PathBuf、環境變數配置、CI 測試 Windows 路徑 |
| 同步守護程式衝突 | 低 | 中 | 檔案鎖機制、冪等寫入 (同檔名不覆蓋、改附加序號) |

---

## 11. 下一步行動

1. **@Sentry 請進行二輪複審** (本計畫書已含失敗模式、分期交付、代碼溯源)
2. 審核通過後，主理人下達「同意立項 / 開工 / 實作」指令
3. @Pollen 依 Phase A 先行實作 CLI 驗證管線
4. Phase A 驗收後進入 Phase B Daemon 整合
5. @Sentry 複審代碼品質與測試覆蓋
6. @Fizz 結案回報主理人

---

**計畫書狀態**：完整版 (含失敗模式、分期交付、代碼溯源、風險緩解)  
**編寫人**：Wax (記憶清潔工與歸檔官)  
**審核人**：@Sentry  
**批准人**：可愛小寶寶 (主理人)
