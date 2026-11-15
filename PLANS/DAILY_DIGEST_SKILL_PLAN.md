---
title: "Daily Digest Skill 實作計畫書"
tags: [skill, daily-digest, implementation-plan, approved]
status: draft
created: 2026-11-02
version: "1.0"
author: "Honey2"
reviewer: "Sentry"
---

# Daily Digest Skill 實作計畫書

> **授權狀態**：主理人 (可愛小寶寶) 已批准立項，Fizz 正式分派  
> **目標頻道**：`5ca019f6-1a27-592a-b015-4baf0eb33572`  
> **審查狀態**：待 Sentry 架構審查與技術驗收  
> **執行團隊**：Honey2 (規畫) ➔ Sentry (審查) ➔ Pollen (實作)

---

## 目標與範疇

將現有「每日對話摘要生成與 Obsidian Vault 雙向同步」工作標準化為 **Buzz Native Skill v1.0**，參考既有 `obsidian-digest-sync` 技能架構，實作自動化對話彙整、Markdown 摘要輸出、標籤分類與知識庫同步流程。

**核心差異**：
- `obsidian-digest-sync` 為 JavaScript (Node.js) 實作，依賴外部 `sync_obsidian/main.mjs` 守護程式
- **Daily Digest Skill** 將以 **Rust 內建技能** 形式實作，整合至 `buzz-agent` 核心，零外部依賴，直接透過 `load_skill` 機制載入

---

## 架構設計

### 1. 技能結構
```
c:\私人\ai agent\buzz\.agents\skills\daily-digest\
├── SKILL.md                    # 技能定義 (YAML frontmatter + Markdown body)
├── manifest.json               # 技能元資料 (供 catalog 發現)
├── src/
│   ├── digest_generator.rs     # 摘要生成核心邏輯
│   ├── markdown_renderer.rs    # Markdown + Frontmatter 渲染
│   ├── vault_sync.rs           # Obsidian Vault 同步 (可選，預設啟用)
│   └── noise_filter.rs         # 行級雜訊過濾器 (回應流程優化 #1)
└── tests/
    └── digest_generator_tests.rs
```

### 2. 技能註冊機制
- 依據 `crates/buzz-agent/src/hints.rs:15-24` `SkillEntry` 結構
- 依據 `crates/buzz-agent/src/builtin.rs:52-124` `call_load_skill` 實作
- 技能目錄掃描路徑：`SKILL_DIRS = &[".agents/skills", ".goose/skills", ".claude/skills"]` (hints.rs:8)
- Frontmatter 解析：`parse_skill_frontmatter` (hints.rs:83-105)

### 3. 整合點
- **Session 建立時發現**：`build_hints_section_impl` → `discover_skills_impl` (hints.rs:216-214)
- **載入時機**：Agent 呼叫 `load_skill("daily-digest")` 時由 `call_load_skill` 讀取 SKILL.md body (builtin.rs:52-124)
- **Hook 整合**：可在 `PreToolUse` / `PostToolUse` 階段自動觸發 (hooks/mod.rs:50-74)

---

## 核心功能模組規格

### 模組 1：行級雜訊過濾器 (`noise_filter.rs`)
**回應流程優化 #1：將雜訊過濾器由「訊息級」升級為「行級」**

```rust
// 依據 crates/buzz-agent/src/hints.rs:107-150 scan_skill_dir 模式
pub struct NoiseFilter {
    noise_patterns: Vec<Regex>,
    signal_patterns: Vec<Regex>,
}

impl NoiseFilter {
    pub fn filter_lines(&self, content: &str) -> Vec<String> {
        content.lines()
            .filter(|line| self.is_signal_line(line))
            .map(|s| s.to_string())
            .collect()
    }
    
    fn is_signal_line(&self, line: &str) -> bool {
        // 1. 排除純狀態報告、心跳、keepalive
        // 2. 排除純表情符號、單字回應
        // 3. 保留：決策、代碼片段、架構討論、具體行動項目
        // 4. 保留：@mention 觸發的工作指派
    }
}
```

**過濾規則 (可配置，預設嚴格模式)**：
| 類別 | 動作 | 範例 |
|------|------|------|
| 狀態報告/心跳 | 移除 | `已完成`、`心跳正常`、`keepalive tick` |
| 純表情/單字 | 移除 | `OK`、`收到` |
| 系統日誌雜訊 | 移除 | `tracing::debug!("llm keepalive tick")` |
| 決策/架構/行動 | **保留** | `決議採用 Rust 實作`、`@Pollen 請實作模組 X` |
| 代碼/配置片段 | **保留** | 代碼區塊 |
| 錯誤堆疊/關鍵異常 | **保留** | `Error: connection refused` |

### 模組 2：摘要生成器 (`digest_generator.rs`)
```rust
pub struct DigestGenerator {
    noise_filter: NoiseFilter,
    vault_sync: Option<VaultSync>,
    config: DigestConfig,
}

#[derive(Deserialize)]
pub struct DigestConfig {
    pub vault_path: Option<PathBuf>,
    pub daily_folder: String,
    pub tags: Vec<String>,
    pub max_threads_per_digest: usize,
    pub strict_mode: bool,
}

pub struct ThreadDigest {
    pub thread_id: String,
    pub root_event_id: String,
    pub participants: Vec<String>,
    pub message_count: usize,
    pub decisions: Vec<String>,
    pub action_items: Vec<ActionItem>,
    pub key_code_snippets: Vec<String>,
    pub summary: String,
}

impl DigestGenerator {
    pub async fn generate_daily_digest(
        &self,
        date: NaiveDate,
        channel_id: &str,
    ) -> Result<DailyDigest, DigestError> { ... }
    
    pub fn extract_decisions(&self, messages: &[Message]) -> Vec<String> { ... }
    pub fn extract_action_items(&self, messages: &[Message]) -> Vec<ActionItem> { ... }
}
```

### 模組 3：Markdown 渲染器 (`markdown_renderer.rs`)
```rust
// 依據 skills/obsidian-digest-sync/main.js:22-51 generateDigestMarkdown 模式
pub fn render_daily_digest(date: NaiveDate, digest: &DailyDigest, config: &DigestConfig) -> String {
    let frontmatter = format!(r#"---
title: "{} 每日對話摘要與決策複盤"
type: "daily-digest"
agent: "Daily Digest Skill"
date: "{}"
threads_count: {}
messages_count: {}
synced_at: "{}"
tags: [{}]
---
"#, 
        date.format("%Y-%m-%d"),
        date.format("%Y-%m-%d"),
        digest.threads.len(),
        digest.total_messages,
        Utc::now().to_rfc3339(),
        config.tags.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(", ")
    );
    
    let mut content = frontmatter;
    content.push_str(&format!("# 每日對話摘要與決策複盤\n\n", date.format("%Y-%m-%d")));
    content.push_str(&format!("> **生成時間**：{}  \n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    content.push_str("> **執行代理人**：Daily Digest Skill  \n\n");
    
    content.push_str("## 核心決策提煉\n\n");
    for d in &digest.all_decisions {
        content.push_str(&format!("- **{}**\n", d));
    }
    
    content.push_str("\n## 行動項目追蹤\n\n");
    for item in &digest.all_action_items {
        content.push_str(&format!("- [ ] **{}** (指派：{})  \n", item.description, item.assignee));
    }
    
    content.push_str("\n## 執行緒詳情\n\n");
    for thread in &digest.threads {
        content.push_str(&format!("### {} ({} 訊息)\n", thread.summary, thread.message_count));
        content.push_str(&format!("- 參與者：{}  \n", thread.participants.join(", ")));
        content.push_str(&format!("- 執行緒 ID：`{}`  \n", thread.thread_id));
        if !thread.decisions.is_empty() {
            content.push_str("- 決策：\n");
            for d in &thread.decisions {
                content.push_str(&format!("  - {}\n", d));
            }
        }
        content.push_str("\n");
    }
    
    content.push_str("## 雙向連結\n\n");
    content.push_str("- [[core-memory]]\n");
    content.push_str("- [[02-Conversation-Digest/INDEX|對話索引]]\n");
    
    content
}
```

### 模組 4：Vault 同步 (`vault_sync.rs`) — 可選啟用
```rust
// 僅在 config.vault_path 存在時啟用
pub struct VaultSync {
    vault_path: PathBuf,
    daily_folder: String,
}

impl VaultSync {
    pub fn sync(&self, date: NaiveDate, markdown: &str) -> Result<SyncResult, SyncError> {
        let target_dir = self.vault_path.join(&self.daily_folder);
        std::fs::create_dir_all(&target_dir)?;
        
        let file_name = format!("{}.md", date.format("%Y-%m-%d"));
        let file_path = target_dir.join(file_name);
        
        // 原子寫入：寫入暫存檔再 rename，防止部分寫入
        let tmp_path = file_path.with_extension("md.tmp");
        std::fs::write(&tmp_path, markdown)?;
        std::fs::rename(&tmp_path, &file_path)?;
        
        // 更新 INDEX.md
        self.update_index(date, &file_path)?;
        
        Ok(SyncResult { file_path })
    }
}
```

---

## 實作任務清單

| 任務編號 | 任務名稱 | 說明 | 驗收標準 | 預估工時 |
|----------|----------|------|----------|----------|
| DD-01 | 建立技能目錄結構 | 建立 `.agents/skills/daily-digest/` 目錄與基礎檔案 | 目錄結構完整，`cargo check` 通過 | 0.5h |
| DD-02 | 實作 `noise_filter.rs` 行級過濾器 | 核心過濾邏輯，單元測試覆蓋 ≥ 90% | 正確過濾雜訊、保留訊號；測試通過 | 2h |
| DD-03 | 實作 `digest_generator.rs` 摘要生成器 | 決策提取、行動項目提取、執行緒歸納 | 對測試資料生成正確摘要結構 | 3h |
| DD-04 | 實作 `markdown_renderer.rs` 渲染器 | Frontmatter + Markdown 體輸出，UTF-8 LF | 輸出符合 Obsidian 相容格式 | 1.5h |
| DD-05 | 實作 `vault_sync.rs` 同步模組 (可選) | 原子寫入、INDEX 更新、錯誤處理 | 成功同步至 Vault，無資料遺失 | 1.5h |
| DD-06 | 撰寫 `SKILL.md` 技能定義 | YAML frontmatter + 完整 SOP 文檔 | 通過 `parse_skill_frontmatter` 解析 | 1h |
| DD-07 | 撰寫 `manifest.json` 元資料 | 符合 Buzz Skill Catalog 規格 | 可被技能發現機制掃描到 | 0.5h |
| DD-08 | 整合測試與驗收 | 端到端測試：從頻道訊息到生成摘要檔案 | 真實頻道資料生成可用摘要 | 2h |

**總預估工時：~12h**

---

## 依賴與相容性

| 依賴項 | 版本 | 用途 | 來源 |
|--------|------|------|------|
| `regex` | 1.10+ | 行級模式匹配 | crates.io |
| `chrono` | 0.4+ | 日期時間處理 | crates.io |
| `serde` / `serde_json` | 1.0+ | 配置序列化 | crates.io |
| `tokio` | 1.0+ | 非同步 I/O | crates.io |
| `anyhow` | 1.0+ | 錯誤處理 | crates.io |

**零破壞性變更**：
- 不修改 `buzz-agent` 核心協議
- 不修改現有 `load_skill` 機制
- 技能為可選載入，不影響無技能場景

---

## 風險評估與回滾機制

| 風險 | 機率 | 影響 | 緩解措施 |
|------|------|------|----------|
| 過濾器誤判訊號為雜訊 | 中 | 遺漏關鍵決策 | 預設嚴格模式 + 單元測試覆蓋邊界案例；提供 `strict_mode: false` 降級 |
| Vault 同步權限失敗
