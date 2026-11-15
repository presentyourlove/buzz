---
title: "QA-Guard Skill 實作計畫書"
tags: [skill, qa-guard, implementation-plan, approved]
status: draft
created: 2026-11-02
version: "1.0"
author: "Honey2"
reviewer: "Sentry"
---

# QA-Guard Skill 實作計畫書

> **授權狀態**：主理人 (可愛小寶寶) 已批准立項，Fizz 正式分派  
> **目標頻道**：`5ca019f6-1a27-592a-b015-4baf0eb33572`  
> **審查狀態**：待 Sentry 架構審查與技術驗收  
> **執行團隊**：Honey2 (規畫) ➔ Sentry (審查) ➔ Pollen (實作)

---

## 目標與範疇

將「Git Patch 自動語法、編碼與執行預檢」工作標準化為 **Buzz Native Skill v1.0**，徹底杜絕補丁駁回、Mojibake 編碼問題與語法錯誤，提升部署品質。

**核心痛點**：
- Patch 提交前缺乏自動化語法檢查
- 編碼不一致 (UTF-8 vs UTF-8 BOM vs Big5, LF vs CRLF) 導致 Mojibake
- Rust/Cargo/TypeScript/JSON 等語法錯誤僅在 CI 階段發現，回饋循環過長

---

## 架構設計

### 1. 技能結構
```
c:\私人\ai agent\buzz\.agents\skills\qa-guard\
├── SKILL.md
├── manifest.json
├── src/
│   ├── patch_parser.rs       # Git Patch 解析與結構化
│   ├── syntax_checker.rs     # 多語言語法檢查器
│   ├── encoding_validator.rs # 編碼/換行符驗證器 (回應流程優化 #2)
│   ├── cargo_check_runner.rs # Rust 專用 cargo check 執行器
│   └── report_generator.rs   # 檢查報告生成 (Markdown + JSON)
└── tests/
    ├── patch_parser_tests.rs
    ├── syntax_checker_tests.rs
    └── encoding_validator_tests.rs
```

### 2. 核心檢查流程
```
Patch Input → 解析 → 編碼驗證 (UTF-8 LF 強制) → 語法檢查 → cargo check (Rust) → 報告輸出
                                    ↓
                            失敗即阻擋，回報具體行號與錯誤
```

### 3. 整合點
- **Hook 階段**：`PostToolUse` 於 `Write`/`Edit` 寫入 `.patch` 或代碼檔案時自動觸發 (hooks/mod.rs:63-74)
- **CLI 整合**：可作為 `buzz pr open` / `buzz issues create` 的前置檢查
- **Skill 載入**：`load_skill("qa-guard")` 提供完整檢查 API

---

## 核心功能模組規格

### 模組 1：Patch 解析器 (`patch_parser.rs`)
```rust
// 依據 git diff --unified=3 格式解析
#[derive(Debug, Clone)]
pub struct ParsedPatch {
    pub files: Vec<PatchFile>,
    pub is_binary: bool,
    pub total_additions: usize,
    pub total_deletions: usize,
}

#[derive(Debug, Clone)]
pub struct PatchFile {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
    pub hunks: Vec<Hunk>,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_renamed: bool,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<HunkLine>,
}

#[derive(Debug, Clone)]
pub enum HunkLine {
    Context(String),
    Addition(String),
    Deletion(String),
}

impl ParsedPatch {
    pub fn parse(patch_content: &str) -> Result<Self, ParseError> { ... }
    pub fn get_modified_files(&self) -> Vec<&PatchFile> { ... }
    pub fn get_rust_files(&self) -> Vec<&PatchFile> { ... }
}
```

### 模組 2：編碼驗證器 (`encoding_validator.rs`) — **回應流程優化 #2 核心**
```rust
// 強制 UTF-8 LF，拒絕 BOM、CRLF、Big5、GBK 等
#[derive(Debug, Clone)]
pub struct EncodingReport {
    pub file_path: PathBuf,
    pub is_valid: bool,
    pub encoding: DetectedEncoding,
    pub line_ending: LineEnding,
    pub has_bom: bool,
    pub errors: Vec<EncodingError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DetectedEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Big5,
    Gbk,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineEnding {
    Lf,      // Unix/Linux/macOS (強制要求)
    Crlf,    // Windows (拒絕)
    Cr,      // 經典 Mac (拒絕)
    Mixed,   // 混雜 (拒絕)
}

impl EncodingValidator {
    pub fn new() -> Self { ... }
    
    /// 核心驗證：必須為 UTF-8 無 BOM + LF
    pub fn validate(&self, content: &[u8], file_path: &Path) -> EncodingReport {
        let encoding = self.detect_encoding(content);
        let line_ending = self.detect_line_ending(content);
        let has_bom = self.has_bom(content);
        
        let is_valid = matches!(encoding, DetectedEncoding::Utf8)
            && !has_bom
            && matches!(line_ending, LineEnding::Lf);
        
        EncodingReport { ... }
    }
    
    /// 自動修正：轉換為 UTF-8 LF (可選，需人工確認)
    pub fn auto_fix(&self, content: &[u8]) -> Result<Vec<u8>, FixError> { ... }
}
```

**驗收標準**：
- 所有 Patch 輸出 **必須** 為 UTF-8 無 BOM + LF
- 檢測到 CRLF/CR/Mixed/BOM/非 UTF-8 即判定失敗
- 報告需包含：檔案路徑、行號、具體違規類型、建議修正指令

### 模組 3：語法檢查器 (`syntax_checker.rs`)
```rust
#[derive(Debug, Clone)]
pub struct SyntaxReport {
    pub file_path: PathBuf,
    pub language: Language,
    pub is_valid: bool,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<SyntaxWarning>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Json,
    Toml,
    Yaml,
    Markdown,
    Shell,
    Unknown,
}

impl SyntaxChecker {
    pub fn new() -> Self { ... }
    
    pub fn check_file(&self, path: &Path) -> SyntaxReport {
        let lang = Language::from_extension(path);
        match lang {
            Language::Rust => self.check_rust(path),
            Language::TypeScript | Language::JavaScript => self.check_ts_js(path),
            Language::Json => self.check_json(path),
            Language::Toml => self.check_toml(path),
            Language::Yaml => self.check_yaml(path),
            Language::Markdown => self.check_markdown(path),
            Language::Shell => self.check_shell(path),
            _ => SyntaxReport::unknown(path),
        }
    }
    
    /// Rust: 使用 `cargo check --message-format=json` 解析結構化錯誤
    fn check_rust(&self, path: &Path) -> SyntaxReport { ... }
    
    /// TS/JS: 使用 `tsc --noEmit` 或 `eslint --format=json`
    fn check_ts_js(&self, path: &Path) -> SyntaxReport { ... }
    
    /// JSON: `serde_json::from_str` 解析驗證
    fn check_json(&self, path: &Path) -> SyntaxReport { ... }
}
```

### 模組 4：Cargo Check 執行器 (`cargo_check_runner.rs`)
```rust
// 專為 Rust 專案優化，支援增量檢查
pub struct CargoCheckRunner {
    project_root: PathBuf,
    target_dir: Option<PathBuf>,
}

impl CargoCheckRunner {
    pub fn new(project_root: PathBuf) -> Self { ... }
    
    /// 僅檢查受影響的 crate (依據 patch 解析結果)
    pub async fn check_incremental(&self, modified_files: &[PathBuf]) -> CargoCheckReport { ... }
    
    /// 完整專案檢查
    pub async fn check_full(&self) -> CargoCheckReport { ... }
    
    /// 解析 cargo JSON 輸出為結構化報告
    fn parse_cargo_output(&self, output: &str) -> CargoCheckReport { ... }
}

#[derive(Debug, Clone)]
pub struct CargoCheckReport {
    pub success: bool,
    pub errors: Vec<CargoError>,
    pub warnings: Vec<CargoWarning>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct CargoError {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub code: Option<String>,  // e.g., "E0308"
    pub level: ErrorLevel,
}
```

### 模組 5：報告生成器 (`report_generator.rs`)
```rust
// 輸出格式：Markdown (人類閱讀) + JSON (機器解析)
pub fn generate_report(
    patch: &ParsedPatch,
    encoding_reports: &[EncodingReport],
    syntax_reports: &[SyntaxReport],
    cargo_report: Option<&CargoCheckReport>,
) -> QaGuardReport {
    let mut md = String::new();
    md.push_str("# QA-Guard 檢查報告\n\n");
    md.push_str(&format!("> **檢查時間**：{}  \n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    md.push_str(&format!("> **Patch 檔案**：{} 個  \n", patch.files.len()));
    md.push_str(&format!("> **新增/刪除行**：+{} / -{}  \n\n", patch.total_additions, patch.total_deletions));
    
    // 編碼檢查結果
    md.push_str("## 編碼與換行符檢查\n\n");
    for r in encoding_reports {
        let status = if r.is_valid { "✅ 通過" } else { "❌ 失敗" };
        md.push_str(&format!("- **{}** `{} ({:?}, {:?})`\n", status, r.file_path.display(), r.encoding, r.line_ending));
        for e in &r.errors {
            md.push_str(&format!("  - `{}`: {}\n", e.line, e.message));
        }
    }
    
    // 語法檢查結果
    md.push_str("\n## 語法檢查\n\n");
    for r in syntax_reports {
        let status = if r.is_valid { "✅ 通過" } else { "❌ 失敗" };
        md.push_str(&format!("- **{}** `{} ({:?})`\n", status, r.file_path.display(), r.language));
        for e in &r.errors {
            md.push_str(&format!("  - 第 {} 行: {}\n", e.line, e.message));
        }
    }
    
    // Cargo check 結果
    if let Some(c) = cargo_report {
        md.push_str("\n## Cargo Check (Rust)\n\n");
        md.push_str(&format!("- **{}** (耗時 {}ms)\n", if c.success { "✅ 通過" } else { "❌ 失敗" }, c.duration_ms));
        for e in &c.errors {
            m
