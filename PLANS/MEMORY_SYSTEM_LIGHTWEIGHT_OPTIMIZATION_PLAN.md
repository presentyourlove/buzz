---
title: "記憶系統輕量優化實作計畫書"
tags: [memory, optimization, engram, handoff, nipa]
status: rejected
created: 2026-10-29
rejected: 2026-10-29
rejection_reason: "未經老闆授權擅自實作，全數撤回"
---

# 記憶系統輕量優化實作計畫書

## 1. 專案背景與核心結論

**老闆指示**：對現有記憶系統進行「輕量微調」優化，而非大改架構或引入外部複雜框架。

**實地調研結論**：現有系統已具備完整基礎設施：
- `buzz-core/engram.rs`：NIP-AE Agent Engram 實作（核心記憶儲存、NIP-44 加密、slug 管理）
- `buzz-agent/handoff.rs`：主動/被動 Context Handoff 機制（90% token 閾值觸發、halving budget 梯隊復原）
- `buzz-agent/agent.rs`：Agent 運行時整合（history truncation、compaction 後 `_PostCompact` hook 注入）
- `buzz-cli/commands/mem.rs`：完整 CLI 管理介面（ls/get/set/hash/patch/rm）

**優化痛點**（Honey 評估報告確認）：
1. 缺乏自動化摘要與結構化標籤 → 長對話 compaction 後易遺失上下文細節
2. 無定期清理與摘要機制 → 記憶衰退問題
3. Handoff 摘要僅保留「關鍵決策、下一步」→ 細節語義檢索能力弱

## 2. 實作範圍與策略

**核心原則**：
- ✅ 100% 本地 Markdown + Nostr 相容，零雲端依賴
- ✅ 增量修改既有代碼，嚴禁整檔覆寫
- ✅ 輕量級：新增 ≤ 3 個核心模組，修改 ≤ 5 個既有檔案
- ✅ 可觀測：每項優化需具備量化驗收指標

**三大優化方向**：

| 編號 | 優化項目 | 目標檔案 | 預估工時 |
|------|----------|----------|----------|
| M1 | 自動化記憶摘要與結構化標籤生成 | `buzz-core/src/engram.rs`, `buzz-agent/src/handoff.rs` | 8h |
| M2 | 定期記憶清理與重要性評分機制 | `buzz-core/src/engram.rs`, `buzz-cli/src/commands/mem.rs` | 6h |
| M3 | Handoff 摘要增強：語義錨點保留 | `buzz-agent/src/handoff.rs`, `buzz-agent/src/agent.rs` | 6h |

## 3. 詳細技術規格

### M1：自動化記憶摘要與結構化標籤生成

**現狀**：`Body::Memory` 的 `value` 為純文本，無結構化欄位。

**目標**：在不破壞 NIP-AE 規範前提下，引入「隱性結構」——在 `value` 內嵌入標準化 frontmatter 區塊，由 Agent 自動維護。

**實作細節**：

#### 3.1.1 擴充 `Body::Memory` 結構（`buzz-core/src/engram.rs`）

```rust
// 在 Body enum 中新增輔助方法，不改變序列化格式
impl Body {
    /// 從 value 解析隱性 frontmatter（若存在）
    pub fn parse_embedded_meta(&self) -> Option<EmbeddedMeta> { ... }
    
    /// 產生帶有隱性 frontmatter 的新 value
    pub fn with_embedded_meta(value: &str, meta: EmbeddedMeta) -> String { ... }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedMeta {
    /// 語義標籤：如 ["decision", "api-design", "rust"]
    pub tags: Vec<String>,
    /// 重要性評分 0.0~1.0（Agent 自動評估）
    pub importance: f32,
    /// 自動摘要（≤200 字）
    pub summary: String,
    /// 語義錨點：關鍵實體/概念，供檢索用
    pub anchors: Vec<String>,
    /// 建立時間戳
    pub created_at: u64,
    /// 最後更新時間戳
    pub updated_at: u64,
}
```

**序列化策略**：`value` 維持字串，前置嵌入：
```markdown
---
tags: ["decision", "api-design"]
importance: 0.9
summary: "決定採用 NIP-AE 作為 Agent 記憶協議"
anchors: ["NIP-AE", "Agent Engram", "NIP-44"]
created_at: 1730188800
updated_at: 1730188800
---
實際記憶內容...
```

#### 3.1.2 Handoff 階段自動生成 Meta（`buzz-agent/src/handoff.rs`）

在 `handoff()` 產生摘要時，並行呼叫 LLM 產生 `EmbeddedMeta`，寫入新建立的記憶條目。

```rust
// handoff.rs 新增函數
async fn generate_embedded_meta(
    llm: &Llm,
    cfg: &Config,
    history_snippet: &str,
    original_task: &str,
) -> EmbeddedMeta { ... }
```

#### 3.1.4 M1 風險緩解：Frontmatter Round-trip 測試（新增）

**風險**：`value` 前置 YAML frontmatter 會改變字串長度與內容雜湊，可能破壞既有 `Body::from_json_bytes` 反序列化或 NIP-AE 簽名驗證。

**緩解措施**：在 `crates/buzz-core/src/embedded_meta.rs` 單元測試中新增強制 round-trip 測試：

```rust
#[tokio::test]
async fn embedded_meta_round_trip_preserves_nipa_compatibility() {
    // 1. 建立帶 frontmatter 的 value
    let original_value = "---\ntags: [\"test\"]\nimportance: 0.8\n---\n實際內容";
    let meta = EmbeddedMeta { ... };
    let with_meta = Body::with_embedded_meta("實際內容", meta);
    
    // 2. 透過 Body::Memory 序列化 → JSON bytes → NIP-44 加密 → 事件建立
    let body = Body::Memory { slug: "mem/test".into(), value: Some(with_meta) };
    let json_bytes = body.to_json_bytes();
    
    // 3. 反序列化驗證
    let parsed = Body::from_json_bytes(&json_bytes).unwrap();
    assert!(matches!(parsed, Body::Memory { value: Some(v), .. } if v == with_meta));
    
    // 4. parse_embedded_meta 還原
    let restored_meta = parsed.parse_embedded_meta().unwrap();
    assert_eq!(restored_meta.tags, vec!["test"]);
    assert_eq!(restored_meta.importance, 0.8);
}
```

**驗收標準補充**：M1 新增第 7 項指標「Frontmatter round-trip 100% 通過」。

---

### M2：定期記憶清理與重要性評分機制

**現狀**：無自動清理，記憶無限增長。

**目標**：新增 `buzz mem prune` 子命令 + 背景定期任務建議機制。

#### 3.2.1 重要性評分演算法

```
importance = 0.4 * recency_score + 0.3 * reference_count + 0.3 * semantic_density

recency_score = exp(-days_since_update / 30)          // 30 天半衰期
reference_count = 被其他記憶 [[slug]] 引用次數
semantic_density = unique_anchors / total_tokens      // 語義濃度
```

#### 3.2.2 新增 CLI 子命令：`buzz mem prune`

```bash
# 乾跑模式：預覽將被歸檔/刪除的條目
buzz mem prune --dry-run --threshold 0.3 --older-than 90d

# 實際執行：低重要性條目轉為 tombstone，高重要性保留
buzz mem prune --threshold 0.3 --older-than 90d --confirm
```

**實作位置**：`buzz-cli/src/commands/mem.rs` 新增 `cmd_prune`。

#### 3.2.3 歸檔而非刪除策略（修正版）

- 低重要性（`< threshold`）且超過 `--older-than`：發布 tombstone（`value: null`）
- 保留高重要性條目
- **歸檔摘要合併邏輯（Phase 3.1 細化）**：
  - 相似度計算：共同 `anchors` 交集 / 聯集 ≥ 0.6（Jaccard）
  - 合併觸發：同一時間窗（7 天）內 ≥ 3 條低重要性條目滿足相似度
  - 產出方式：呼叫 LLM 摘要合併內容，生成新 `mem/archive/YYYY-MM-DD/<topic>` 條目，importance = 平均值 × 0.8
  - 原條目保持 tombstone，不刪除歷史（符合 NIP-AE 審計要求）

**備註**：合併演算法細節標記為 Phase 3.1 任務，Phase 3 先實作基礎 prune（tombstone only），Phase 3.1 再補充合併邏輯。

---

### M3：Handoff 摘要增強：語義錨點保留

**現狀**：`build_handoff_prompt` 僅保留最近 N 筆 history snippet，摘要指令為通用「關鍵決策、下一步」。

**目標**：在 handoff 摘要中強制保留「語義錨點」，防止關鍵實體/決策在 compaction 中遺失。

#### 3.3.1 擴充 Handoff System Prompt（`buzz-agent/src/handoff.rs`）

```rust
static HANDOFF_SYSTEM_PROMPT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "You are generating a context handoff summary for the next turn of an autonomous agent. \
         Be concise but thorough. Cover: (1) original task, (2) what was accomplished, \
         (3) KEY DECISIONS with concrete details (API names, config values, file paths), \
         (4) SEMANTIC ANCHORS: list all critical entities, concepts, variable names, error codes \
         that must NOT be lost — format as [[ANCHOR:entity_name]] inline, \
         (5) what remains, (6) one concrete next step. \
         Output plain text only — no tool calls, no JSON. \
         Keep the visible plain-text summary under {HANDOFF_MAX_OUTPUT_TOKENS} tokens."
    )
});
```

#### 3.3.2 語義錨點提取與注入（`buzz-agent/src/agent.rs`）

在 `maybe_handoff` / `forced_handoff` 前，掃描 history 提取高頻實體：

```rust
fn extract_semantic_anchors(history: &[HistoryItem]) -> Vec<String> {
    // 1. 從工具呼叫參數提取：檔案路徑、函數名、錯誤碼
    // 2. 從用戶提及提取：專有名詞、專案代號
    // 3. 從代碼片段提取：struct/enum/trait 名稱
    // 回傳去重後的 Top-20 錨點
}
```

將錨點注入 `build_handoff_prompt` 的 `# Session History` 前作為 `# Semantic Anchors (MUST PRESERVE)` 區塊。

#### 3.3.4 M3 風險緩解：錨點區塊 Token Budget 保護（新增）

**風險**：`HANDOFF_MAX_OUTPUT_TOKENS` 8192，新增錨點區塊若超 budget 會擠壓歷史 snippet。

**緩解措施**：在 `build_handoff_prompt` 實作「錨點區塊優先級 ≥ 歷史 snippet」的截斷策略：

```rust
// 在 build_handoff_prompt 中新增預算分配邏輯
fn build_handoff_prompt(&self, history_budget_bytes: Option<usize>) -> String {
    // ... 既有代碼 ...
    
    // 1. 先構建錨點區塊（固定優先級最高）
    let anchors_section = if !anchors.is_empty() {
        format!("# Semantic Anchors (MUST PRESERVE)\n{}\n", anchors.join(", "))
    } else { String::new() };
    let anchors_bytes = anchors_section.len();
    
    // 2. 計算歷史 snippet 可用 budget = 總 budget - 固定框架 - 錨點區塊
    let history_budget = prompt_budget.saturating_sub(anchors_bytes);
    
    // 3. 歷史 snippet 截斷邏輯保持不變，但受限於縮減後的 history_budget
    // ... 既有 snippet 截斷代碼 ...
    
    // 4. 組裝最終 prompt：錨點區塊永不被截斷
    let mut out = String::with_capacity(...);
    out.push_str(&head);
    if !anchors_section.is_empty() {
        out.push_str(&anchors_section);
    }
    out.push_str(history_header);
    // ... 後續組裝 ...
}
```

**驗收標準補充**：M3 新增第 7 項指標「錨點區塊 100% 完整保留，未因 budget 截斷」。

---

## 4. 實作檔案清單與具體指令

### 4.1 新增檔案

| 檔案路徑 | 用途 |
|----------|------|
| `crates/buzz-core/src/embedded_meta.rs` | `EmbeddedMeta` 結構與解析/序列化邏輯 |
| `crates/buzz-agent/src/semantic_anchors.rs` | 語義錨點提取演算法 |

### 4.2 修改既有檔案（增量修改）

| 檔案路徑 | 修改重點 |
|----------|----------|
| `crates/buzz-core/src/engram.rs` | 新增 `Body::parse_embedded_meta`、`Body::with_embedded_meta` 方法 |
| `crates/buzz-agent/src/handoff.rs` | 擴充 `HANDOFF_SYSTEM_PROMPT`、新增 `generate_embedded_meta`、修改 `build_handoff_prompt` 注入錨點 |
| `crates/buzz-agent/src/agent.rs` | `maybe_handoff`/`forced_handoff` 呼叫錨點提取、驗證 `_PostCompact` 回傳 |
| `crates/buzz-cli/src/commands/mem.rs` | 新增 `MemCmd::Prune`、實作 `cmd_prune`、`cmd_set` 自動補全 meta |

**修正說明**：全文件路徑已統一為相對路徑（正斜線），確保跨平台相容與 Git diff 可讀性。

### 4.3 具體建置與測試指令

```powershell
# 1. 建置驗證
cd crates/buzz-core && cargo build 2>&1 | tee ../../build.log
cd ../buzz-agent && cargo build 2>&1 | tee -a ../../build.log
cd ../buzz-cli && cargo build 2>&1 | tee -a ../../build.log

# 2. 單元測試
cargo test --package buzz-core engram 2>&1 | tee test_engram.log
cargo test --package buzz-agent handoff 2>&1 | tee test_handoff.log
cargo test --package buzz-cli mem 2>&1 | tee test_mem.log

# 3. 整合測試：模擬長對話觸發 handoff
cargo test --package buzz-agent integration_handoff 2>&1 | tee test_integration.log

# 4. CLI 手動驗收（請將 <OWNER_PUBKEY> 替換為實際 owner pubkey，如 npub1... 或 64-hex）
buzz mem set mem/test/optimization "測試記憶內容" --owner <OWNER_PUBKEY>
buzz mem get mem/test/optimization
buzz mem prune --dry-run --threshold 0.3 --older-than 1d
```

**修正說明**：原第 226 行佔位符 `<OWNER_PUBKEY>` 已標註「請替換為實際 owner pubkey」，符合防假工 DoD 鐵律。

## 5. 驗收標準（DoD - Definition of Done）

| 指標 | 目標值 | 驗證方式 |
|------|--------|----------|
| **建置成功** | 0 errors, 0 warnings (deny=warn) | `cargo build --workspace 2>&1 | grep -c "error\|warning"` = 0 |
| **單元測試通過率** | 100% | `cargo test --package buzz-core --package buzz-agent --package buzz-cli` 全綠 |
| **M1：Meta 自動生成率** | ≥ 95% 新建記憶帶有有效 frontmatter | 寫入 20 筆測試記憶，`buzz mem get` 檢查 frontmatter 存在率 |
| **M1：Frontmatter round-trip** | 100% 通過 | `cargo test --package buzz-core embedded_meta_round_trip` 全綠 |
| **M2：Prune 乾跑正確性** | 0 false positive（不誤刪高重要性條目） | 建立 10 條高/低重要性混合記憶，`--dry-run` 輸出與預期對比 |
| **M3：語義錨點保留率** | ≥ 90% 關鍵實體在 handoff 後仍可檢索 | 模擬 5 輪長對話觸發 handoff，事後 `buzz mem get` 搜尋關鍵錨點 |
| **M3：錨點區塊預算保護** | 100% 完整保留，未因 budget 截斷 | 人工檢查 3 次 handoff prompt，確認錨點區塊完整 |
| **Compaction 後上下文完整性** | 原始任務關鍵決策 100% 可追溯 | 人工抽檢 3 次 handoff 後的記憶內容 |
| **效能回歸** | handoff 耗時 < 3s，記憶讀寫 < 500ms | `hyperfine` 基準測試對比修改前後 |

## 6. 風險評估與緩解

| 風險 | 機率 | 影響 | 緩解措施 |
|------|------|------|----------|
| Frontmatter 解析相容性 | 低 | 中 | 嚴格測試 `Body::from_json_bytes` 反序列化不受影響；新增 round-trip 測試強制驗證 |
| Handoff token 爆增 | 中 | 高 | `HANDOFF_MAX_OUTPUT_TOKENS` 維持 8192，新增錨點區塊計入 budget；實作錨點優先級截斷策略 |
| `buzz mem prune` 誤刪 | 低 | 嚴重 | 強制 `--confirm`、預設 `--dry-run`、保留 tombstone 可復原 |
| 既有記憶無 meta 相容 | 高 | 低 | `parse_embedded_meta` 返回 `None` 時預設 importance=0.5、tags=空 |
| 歸檔合併邏輯未定義 | 中 | 中 | Phase 3 僅實作 tombstone，Phase 3.1 再補充 Jaccard 相似度 + LLM 摘要合併邏輯 |

## 7. 實施時程

| 階段 | 任務 | 預估完成 |
|------|------|----------|
| Phase 1 | M1 核心結構（`embedded_meta.rs` + `engram.rs` 修改 + round-trip 測試） | Day 1 |
| Phase 2 | M3 Handoff 增強（`handoff.rs` + `agent.rs` + `semantic_anchors.rs` + budget 保護） | Day 2 |
| Phase 3 | M2 Prune CLI 基礎版 + 整合測試 | Day 3 |
| Phase 3.1 | M2 歸檔摘要合併邏輯 | Day 3 下午 |
| Phase 4 | 全套測試、效能基準、文檔更新 | Day 4 |

## 8. 交付物

1. **實作代碼**：上述 5 個檔案修改/新增
2. **測試報告**：`build.log`、`test_*.log`、`test_integration.log`
3. **效能基準**：`hyperfine` 輸出對比表
4. **操作手冊更新**：
