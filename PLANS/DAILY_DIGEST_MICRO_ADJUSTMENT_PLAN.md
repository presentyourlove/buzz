---
title: "Daily Digest Micro-Adjustment Implementation Plan"
tags: [daily-digest, enhancement, noise-filter, spec-validation, categorization]
status: active
created: 2026-11-06
---

# Daily Digest Micro-Adjustment Implementation Plan

## 1. Scope & Objectives

### 1.1 Project Context
- **Trigger**: User (可愛小寶寶) approved evolution of `DAILY_EVOLUTION_PROPOSAL` after Honey's benefit analysis
- **Current State**: Daily Digest skill exists at `skills/daily-digest/src/` with 4 modules (noise_filter, digest_generator, markdown_renderer, vault_sync)
- **Pipeline**: fetch → line-filter → extract (decisions/action_items/code) → aggregate → render → sync

### 1.2 Evolution Points (from Honey's analysis, user-approved)

| # | Evolution Point | Current Gap |
|---|----------------|-------------|
| 1 | **Enhance Noise Filtering** | Line-level filter exists but Q&A/chit-chat detection is weak; only basic patterns for "OK", "收到", heartbeat |
| 2 | **Add Spec Pre-Check** | No front-matter validation or DoD checklist enforcement for MVP specs |
| 3 | **Improve Summary Presentation** | Flat thread list; no categorization (hardware consult vs engineering tasks) |

### 1.3 Success Criteria (DoD)

- [ ] **Noise Filter v2**: ≥90% precision on Q&A/chit-chat lines in test corpus; ≤5% false positive on decisions/action_items
- [ ] **Spec Validator**: New `spec_validator.rs` module validates front-matter (title, type, tags, date) + DoD checklist; fails build on missing required fields
- [ ] **Categorized Renderer**: Output groups threads by `category` field (engineering/hardware/consult/ops) with section headers
- [ ] **Tests**: Unit tests for each new function + integration test covering full pipeline
- [ ] **Rollback**: `git revert` plan documented; feature flags for each evolution point

---

## 2. Technical Design

### 2.1 Module Impact Map

```
skills/daily-digest/src/
├── lib.rs                    ← re-export new spec_validator
├── noise_filter.rs           ← ENHANCE: Q&A patterns, heuristic scoring
├── digest_generator.rs       ← INTEGRATE: spec_validator + category extraction
├── markdown_renderer.rs      ← ENHANCE: categorized sections, category-aware template
├── spec_validator.rs         ← NEW: front-matter + DoD validation
└── vault_sync.rs             ← NO CHANGE (atomic write + INDEX update unchanged)
```

### 2.2 Evolution 1: Noise Filter v2 (`noise_filter.rs`)

#### 2.2.1 Current State (lines 54-193)
```rust
// Signal patterns (lines 54-72): @mention, 決議/決策/建議, Error/, ```code, 完成.*審查, 任務.*:
// Noise patterns (lines 74-98): ^OK$, ^收到$, ^好的?$, ^done$, heartbeat, tracing::debug, 已完成$
// is_signal_line() priority: signal > noise > defaults (len<=2, heartbeat, tracing)
```

#### 2.2.2 Required Changes

| Change | Location | Details |
|--------|----------|---------|
| Add Q&A/chit-chat regex patterns | `noise_patterns` vec (after line 98) | `^Q:`, `^A:`, `^問[：:]`, `^答[：:]`, `^\?`, `^為什麼`, `^如何`, `^請問`, `^謝謝`, `^感謝`, `^沒問題`, `^沒關係`, `^隨意`, `^無所謂` |
| Add heuristic scoring | New `score_line()` method | Weight: +2 for @mention, +2 for 決議/決策/行動, +1 for code block, -2 for Q&A pattern, -1 for pure ack |
| Configurable threshold | Add `signal_threshold: i32` field | Default 0; lines with score ≥ threshold kept |
| Preserve backwards compat | `Default` impl unchanged | New fields have sensible defaults |

#### 2.2.3 New Method Signature
```rust
// In NoiseFilter impl
pub fn score_line(&self, line: &str) -> i32;
pub fn filter_lines_with_scores(&self, content: &str) -> Vec<(String, i32)>;
```

### 2.3 Evolution 2: Spec Validator (`spec_validator.rs` — NEW FILE)

#### 2.3.1 Purpose
Validate that generated daily digest Markdown conforms to MVP spec requirements before vault sync.

#### 2.3.2 Validation Rules

| Check | Required | Severity | Description |
|-------|----------|----------|-------------|
| Front-matter exists | Yes | Error | `---` delimiters at file start |
| `title` field | Yes | Error | Must match `^\d{4}-\d{2}-\d{2} 每日對話摘要與決策複盤$` |
| `type` field | Yes | Error | Must equal `daily-digest` |
| `agent` field | Yes | Error | Must equal `Daily Digest Skill` |
| `date` field | Yes | Error | Valid ISO date, matches filename |
| `tags` array | Yes | Warning | At least `["digest", "daily"]` |
| `threads_count` | Yes | Warning | Integer ≥ 0 |
| `messages_count` | Yes | Warning | Integer ≥ 0 |
| DoD checklist section | No | Warning | `## DoD 檢查清單` with `- [ ]` items |

#### 2.3.3 API Design
```rust
// spec_validator.rs
pub struct SpecValidator {
    strict_mode: bool,
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug)]
pub enum ValidationError {
    MissingFrontmatter,
    MissingField(String),
    InvalidFieldValue { field: String, expected: String, actual: String },
    DateMismatch { frontmatter: String, filename: String },
}

#[derive(Debug)]
pub enum ValidationWarning {
    MissingRecommendedField(String),
    DodChecklistMissing,
    TagMissing(String),
}

impl SpecValidator {
    pub fn new(strict_mode: bool) -> Self;
    pub fn validate(&self, markdown: &str, filename: &str) -> ValidationResult;
}
```

#### 2.3.4 Integration Point
In `digest_generator.rs::generate_daily_digest()` (after line 176, before vault_sync):
```rust
let markdown = markdown_renderer::render_daily_digest(date, &digest, &self.config);
let validator = SpecValidator::new(self.config.strict_mode);
let result = validator.validate(&markdown, &format!("{}.md", date.format("%Y-%m-%d")));
if !result.valid {
    return Err(DigestError::ValidationError(result.errors.join("; ")));
}
// log warnings, then proceed to vault_sync
```

### 2.4 Evolution 3: Categorized Renderer (`markdown_renderer.rs`)

#### 2.4.1 Current State (lines 46-130)
Flat sections: `## 決策與行動項目彙總` → `## 執行緒詳情` (all threads sequential)

#### 2.4.2 Required Changes

| Change | Location | Details |
|--------|----------|---------|
| Add `category` field to `ThreadDigest` | `digest_generator.rs` line ~70 | Derive from thread content: keywords → `engineering`/`hardware`/`consult`/`ops`/`other` |
| Categorize in `aggregate_digest_info` | `digest_generator.rs` line ~290 | Group threads by category before rendering |
| New render method | `markdown_renderer.rs` after line 130 | `render_categorized_digest()` with section per category |
| Category-aware template | `markdown_renderer.rs` lines 17-20 | Section order: Engineering → Hardware → Consult → Ops → Other |

#### 2.4.3 Category Detection Heuristic (in `digest_generator.rs`)
```rust
fn categorize_thread(&self, thread: &ThreadDigest) -> ThreadCategory {
    let content = thread.decisions.join(" ") + &thread.action_items.iter().map(|a| &a.description).collect::<Vec<_>>().join(" ");
    let lower = content.to_lowercase();
    
    if lower.contains("硬體") || lower.contains("hardware") || lower.contains("設備") || lower.contains("裝置") {
        ThreadCategory::Hardware
    } else if lower.contains("諮詢") || lower.contains("consult") || lower.contains("建議") || lower.contains("推薦") {
        ThreadCategory::Consult
    } else if lower.contains("部署") || lower.contains("deploy") || lower.contains("運維") || lower.contains("監控") || lower.contains("維護") {
        ThreadCategory::Ops
    } else {
        ThreadCategory::Engineering  // default: code, arch, bug, feature
    }
}
```

#### 2.4.4 New Render Output Structure
```markdown
# 2026-11-06 每日對話摘要與決策複盤

## 📊 統計概覽
- 執行緒數：5 | 訊息數：23 | 決策數：8 | 行動項目：12

## ⚙️ 工程類
### 執行緒 #1: 決策和行動項目的討論 (3 訊息)
- 決策：決議採用 Rust 實作
- 行動項目：[ ] 撰寫 API 文檔 (指派：Pollen)

## 🔧 硬體諮詢類
### 執行緒 #3: 硬體選型討論 (2 訊息)
- 決策：建議採用 AMD EPYC 系列
- 行動項目：[ ] 整理規格比較表 (指派：Honey)

## 💬 諮詢類
### 執行緒 #2: 發布計劃的討論 (2 訊息)
- 決策：建議延後發布

## 📎 雙向連結
- [[core-memory]]
- [[wiki/02-Conversation-Digest/INDEX|對話索引]]
```

---

## 3. File-Level Change Summary

| File | Change Type | Lines Affected | Est. LoC Delta |
|------|-------------|----------------|----------------|
| `noise_filter.rs` | Enhance | 54-193, +new methods | +80 |
| `spec_validator.rs` | **New** | entire file | +180 |
| `digest_generator.rs` | Integra

### 1.4 Extreme Boundary Failure-Mode Analysis

| Scenario | Expected Behavior / Edge Case |
|----------|-------------------------------|
| When `signal_threshold = i32::MAX`, all lines are filtered, user receives empty digest | System handles gracefully: renders digest with 0 threads/messages, no crash, UI shows "no content filtered" notice |
| When front-matter is damaged (missing `---` delimiters, malformed YAML), spec validator downgrades to warnings only and proceeds with default values | Validator logs `MissingFrontmatter` error as warning, uses fallback defaults for required fields, does not block vault sync |
| When all thread decisions and action items match no category keywords, default `Engineering` category assigned | Every thread receives `Engineering` categorization, section order follows Engineering → Hardware → Consult → Ops → Other, no data loss |
| When `tags` field contains non-string elements or null values, validator issues warning but does not reject | Validator emits `TagMissing` warning for invalid entries, sanitizes tags to `["digest", "daily"]` before proceeding |
| When date in front-matter does not match filename, validator flags `DateMismatch` error but allows strict_mode=off to proceed with warning | In strict_mode: validation error, digest generation halted. In non-strict_mode: warning logged, proceeds with filename date as fallback |
