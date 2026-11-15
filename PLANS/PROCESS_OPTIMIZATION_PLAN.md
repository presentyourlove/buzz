---
title: "流程優化實作計畫書：行級雜訊過濾與 UTF-8 LF 強制編碼"
tags: [process-optimization, noise-filter, encoding-enforcement, implementation-plan, approved]
status: draft
created: 2026-11-02
version: "1.0"
author: "Honey2"
reviewer: "Sentry"
---

# 流程優化實作計畫書：行級雜訊過濾與 UTF-8 LF 強制編碼

> **授權狀態**：主理人 (可愛小寶寶) 已批准立項，Fizz 正式分派  
> **目標頻道**：`5ca019f6-1a27-592a-b015-4baf0eb33572`  
> **審查狀態**：待 Sentry 架構審查與技術驗收  
> **執行團隊**：Honey2 (規畫) ➔ Sentry (審查) ➔ Pollen (實作)

---

## 目標與範疇

落實進化建議中的兩項流程優化，使其成為標準化、自動化、可驗證的工程實踐：

1. **行級雜訊過濾器升級**：將對話萃取過濾規則由「訊息級」升級為「行級」，提升紀錄萃取精準度，保留決策/代碼/行動項目，移除心跳/表情/雜訊。
2. **交付編碼強制化**：導入自動化編碼檢查，確保所有 Patch 輸出、代碼檔案、技能文檔皆為 **UTF-8 無 BOM + LF**，徹底杜絕 Mojibake、CRLF 混雜、Big5/GBK 殘留問題。

---

## 背景與痛點

| 流程 | 現狀痛點 | 目標狀態 |
|------|----------|----------|
| 對話萃取 (`scripts/extract_conversation.py`) | 訊息級過濾：整條訊息被判定為雜訊則全棄，導致決策碎片遺漏 | 行級過濾：逐行判定，精準保留訊號行 |
| Patch/代碼輸出 | 無編碼強制機制，Windows 環境易產生 CRLF、BOM、ANSI 編碼 | 全鏈路 UTF-8 LF 強制，CI/PreToolUse 自動攔截 |
| 技能文檔/報告輸出 | 手動檢查，易疏漏 | 統一由 QA-Guard `encoding_validator` 驗證 |
| CI/部署驗證 | 僅在 CI 階段發現編碼/語法問題 | 前置於 Hook 階段 (`PreToolUse`/`PostToolUse`) |

---

## 架構設計：雙軌並行

### 軌道 A：行級雜訊過濾器 (核心模組：`noise_filter.rs`)

**部署位置**：
- Daily Digest Skill 內建 (`c:\私人\ai agent\buzz\.agents\skills\daily-digest\src\noise_filter.rs`)
- `scripts/extract_conversation.py` 升級版 (引入行級邏輯)
- 可被其他技能透過 `load_skill("daily-digest/references/noise_filter_rules.yaml")` 共享規則集

**核心資料結構**：
```rust
// 依據 crates/buzz-agent/src/hints.rs:107-150 scan_skill_dir 模式
pub struct NoiseFilter {
    noise_patterns: Vec<Regex>,      // 雜訊模式 (移除)
    signal_patterns: Vec<Regex>,     // 訊號模式 (保留)
    context_window: usize,           // 上下文窗口 (預設 2 行)
}

impl NoiseFilter {
    pub fn filter_lines(&self, content: &str) -> Vec<String> {
        let lines: Vec<&str> = content.lines().collect();
        let mut keep = vec![false; lines.len()];
        
        // 1. 首輪：標記明確訊號行
        for (i, line) in lines.iter().enumerate() {
            if self.is_signal_line(line) {
                keep[i] = true;
                // 擴展上下文窗口
                for j in i.saturating_sub(self.context_window)..=min(i + self.context_window, lines.len() - 1) {
                    keep[j] = true;
                }
            }
        }
        
        // 2. 二輪：移除未標記的雜訊行
        lines.into_iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, l)| l.to_string())
            .collect()
    }
    
    fn is_signal_line(&self, line: &str) -> bool {
        // 訊號特徵 (任一符合即保留)
        self.signal_patterns.iter().any(|re| re.is_match(line))
        // 雜訊特徵 (全符合且無訊號特徵則移除)
        && !self.noise_patterns.iter().all(|re| re.is_match(line))
    }
}
```

**預設規則集 (YAML 可配置)**：
```yaml
# .agents/skills/daily-digest/references/noise_filter_rules.yaml
noise_patterns:
  - "^\s*$"                           # 空行
  - "^(心跳|keepalive|tick|ping|pong)" # 心跳/keepalive
  - "^(已完成|完成|收到|OK|ok|好的|好|嗯|對|是)$"  # 純狀態/單字回應
  - "^[😀-🿿]+$"                        # 純表情符號
  - "^tracing::(debug|info|warn)\(.+keepalive"   # 系統日誌雜訊
  - "^\[INFO\].*heartbeat"           # 標準日誌心跳
signal_patterns:
  - "(決策|決議|採用|選擇|決定|建議|推薦)"      # 決策關鍵字
  - "(@\w+\s+請|指派|負責|執行|實作)"         # 工作指派
  - "(錯誤|Error|Exception|panic|fail)"        # 錯誤/異常
  - "(```|`[^`]+`)"                           # 代碼片段
  - "(架構|設計|API|介面|schema|protocol)"     # 架構討論
  - "(TODO|FIXME|HACK|BUG|NOTE):"              # 代碼標註
context_window: 2
```

### 軌道 B：UTF-8 LF 強制編碼 (核心模組：`encoding_validator.rs`)

**部署位置**：
- QA-Guard Skill 核心模組 (`c:\私人\ai agent\buzz\.agents\skills\qa-guard\src\encoding_validator.rs`)
- Hook 規則：`EncodingEnforceRule` (新增至 `crates/buzz-agent/src/hooks/rules/`)
- CLI：`buzz encoding check`、`buzz encoding fix`

**核心驗證邏輯**：
```rust
// 依據 QA-Guard Skill 計畫書模組 2
#[derive(Debug, Clone, PartialEq)]
pub enum DetectedEncoding {
    Utf8, Utf8Bom, Utf16Le, Utf16Be, Big5, Gbk, Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineEnding { Lf, Crlf, Cr, Mixed }

impl EncodingValidator {
    pub fn validate(&self, content: &[u8], file_path: &Path) -> EncodingReport {
        let encoding = self.detect_encoding(content);
        let line_ending = self.detect_line_ending(content);
        let has_bom = self.has_bom(content);
        
        let is_valid = matches!(encoding, DetectedEncoding::Utf8)
            && !has_bom
            && matches!(line_ending, LineEnding::Lf);
        
        EncodingReport { file_path, is_valid, encoding, line_ending, has_bom, errors: ... }
    }
    
    /// 自動修正：轉換為 UTF-8 LF (需人工確認或 --force)
    pub fn auto_fix(&self, content: &[u8]) -> Result<Vec<u8>, FixError> {
        let decoded = self.decode_lossy(content);  // 嘗試多種編碼解碼
        let normalized = decoded.replace("\r\n", "\n").replace("\r", "\n");
        Ok(normalized.into_bytes())
    }
}
```

---

## 整合點與觸發機制

### 1. Hook 規則新增 (`crates/buzz-agent/src/hooks/rules/encoding_enforce.rs`)
```rust
// 新規則：EncodingEnforceRule
pub struct EncodingEnforceRule;

#[async_trait]
impl Rule for EncodingEnforceRule {
    fn id(&self) -> &str { "encoding-enforce" }
    fn trigger(&self) -> HookTrigger { HookTrigger::PostToolUse }
    
    async fn check(&self, ctx: &HookContext) -> Result<HookResult, RuleError> {
        // 僅在 Write/Edit/MultiEdit 且目標為代碼/文檔/技能檔案時觸發
        if !matches!(ctx.tool_name.as_str(), "Write" | "Edit" | "MultiEdit") {
            return Ok(HookResult::Continue);
        }
        
        let path = ctx.tool_arguments.get("file_path").and_then(|v| v.as_str());
        let content = ctx.tool_arguments.get("content").and_then(|v| v.as_str());
        
        if let (Some(path), Some(content)) = (path, content) {
            if self.should_check_file(path) {
                let report = EncodingValidator::new().validate(content.as_bytes(), Path::new(path));
                if !report.is_valid {
                    return Ok(HookResult::Block { 
                        reason: format!("編碼驗證失敗：{} - 必須為 UTF-8 無 BOM + LF\n{}", 
                            path, report.format_errors()) 
                    });
                }
            }
        }
        Ok(HookResult::Continue)
    }
}

impl EncodingEnforceRule {
    fn should_check_file(&self, path: &str) -> bool {
        // 檢查副檔名與路徑模式
        EXTENSIONS.iter().any(|ext| path.ends_with(ext)) 
        || path.contains(".agents/skills/")
        || path.ends_with("SKILL.md")
        || path.ends_with(".patch")
    }
}
```

### 2. 現有腳本升級 (`scripts/extract_conversation.py`)
```python
# 新增行級過濾模式
class LineNoiseFilter:
    def __init__(self, rules_path: Path):
        self.rules = load_yaml(rules_path)
        self.noise_res = [re.compile(p) for p in self.rules['noise_patterns']]
        self.signal_res = [re.compile(p) for p in self.rules['signal_patterns']]
        self.context_window = self.rules.get('context_window', 2)
    
    def filter(self, text: str) -> str:
        lines = text.splitlines()
        keep = [False] * len(lines)
        
        for i, line in enumerate(lines):
            if self._is_signal(line):
                # 標記上下文窗口
                for j in range(max(0, i - self.context_window), 
                               min(len(lines), i + self.context_window + 1)):
                    keep[j] = True
        
        return "\n".join(line for i, line in enumerate(lines) if keep[i])
```

### 3. CI 整合 (`.github/workflows/encoding-check.yml`)
```yaml
name: Encoding & Line Ending Check
on: [push, pull_request]
jobs:
  encoding-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Run QA-Guard encoding check
        run: cargo run -p qa-guard -- encoding check --all
      - name: Run line noise filter test
        run: python scripts/extract_conversation.py --test-mode --strict
```

---

## 實作任務清單

### 第一期：MVP 輕量落地版 (優先交付核心價值)

| 任務編號 | 任務名稱 | 說明 | 驗收標準 | 預估工時 |
|----------|----------|------|----------|----------|
| PO-01 | 設計行級過濾規則集 YAML | 定義 `noise_filter_rules.yaml` 核心模式 | 涵蓋 6 大類雜訊、5 大類訊號、可配置上下文窗口 | 1h |
| PO-02 | 實作 `noise_filter.rs` (Rust) | 核心過濾邏輯、單元測試 ≥ 90% | 正確過濾測試語料、保留決策/代碼/
