---
title: "CCGS 輕量原型機制導入實作計畫書"
tags: [ccgs, hooks, path-scope, collaboration-protocol, prototype]
status: active
created: 2026-11-01
author: Honey2
reviewer: Sentry
---

# CCGS 輕量原型機制導入實作計畫書

## 1. 專案邊界與範圍（Scope & Boundaries）

### 1.1 納入範圍（In Scope）
| 編號 | 機制 | 核心產出 | 預估工時 |
|------|------|----------|----------|
| M1 | Hooks 自動化驗證層 | `crates/buzz-agent/src/hooks/` 三支腳本 + 掛載點 | 1.5 天 |
| M2 | 路徑作用域規則系統 | `buzz-agent/.rules/` 5-6 條 YAML 規則 + 匹配引擎 | 0.5 天 |
| M3 | 協作協議自動化技能 | `skills/collab-protocol/` 技能包（單一編排技能） | 1 天 |

### 1.2 明確排除（Out of Scope - 嚴禁擴展）
- ❌ 49 位 Agent 定義、74 項遊戲技能、3 階嚴格度
- ❌ ADR/架構審查儀式、62 份遊戲文檔模板
- ❌ Opus/Sonnet 模型分級、外部依賴（Redis、分布式鎖、Daemon）
- ❌ 跨專案/跨倉庫的規則同步機制

### 1.3 環境相容性基線（Local Compatibility Baseline）
| 組件 | 當前版本 | 相容性要求 |
|------|----------|------------|
| Rust | 1.82+ (rust-toolchain.toml) | 無版本升級 |
| Tokio | 1.40+ | 現有 async 架構複用 |
| Serde/Serde_YAML | 1.0 / 0.9 | YAML frontmatter 解析 |
| Clap | 4.5+ | CLI 指令擴展 |
| Buzz Agent | 現有 `RunCtx`、`McpRegistry` | 零破壞性變更 |
| 技能發現 | `hints.rs::discover_skills_impl` | 現有掃描邏輯複用 |

---

## 2. M1：Hooks 自動化驗證層 實作細節

### 2.1 架構定位
在 `crates/buzz-agent/src/` 新增 `hooks/` 模組，掛載於 `RunCtx` 生命周期三個節點：
- **PreToolUse**：工具調用前攔截（阻擋/改寫）
- **PostToolUse**：工具結果後驗證（校驗/記錄）
- **Session/Subagent**：會話級審計、狀態持久化

### 2.2 目錄結構（實際寫入路徑）
```
c:\私人\ai agent\buzz\crates\buzz-agent\src\hooks\
├── mod.rs                    # 公開 API、HookRegistry、掛載點
├── pre_tool_use.rs           # PreToolUse 實作
├── post_tool_use.rs          # PostToolUse 實作
├── session_subagent.rs       # Session/Subagent 實作
├── rules\                    # 規則定義（可擴展）
│   ├── hardcoded_block.rs    # 阻擋硬編碼
│   ├── json_validation.rs    # JSON 結構驗證
│   ├── design_doc_check.rs   # 設計文檔段落完整性
│   ├── branch_protection.rs  # 保護分支/標籤
│   ├── large_file_refuse.rs  # 拒絕大文件整讀
│   ├── entity_registry.rs    # 實體註冊表保護
│   ├── data_file_json.rs     # 資料檔 JSON 驗證
│   └── skill_change_test.rs  # 技能變更提醒測試
└── types.rs                  # 共用型別：HookResult、HookContext、RuleMatch
```

### 2.3 核心型別定義（`hooks/types.rs` - 參考 `types.rs:62-124` HistoryItem 模式）

```rust
// 依據 crates/buzz-agent/src/types.rs:62-124 HistoryItem 結構
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HookContext {
    pub tool_name: String,
    pub tool_arguments: serde_json::Value,
    pub session_id: String,
    pub working_dir: PathBuf,
    pub trigger_event: HookTrigger,  // PreToolUse | PostToolUse | SessionStart | SessionEnd | SubagentStart | SubagentEnd
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrigger {
    PreToolUse,
    PostToolUse,
    SessionStart,
    SessionEnd,
    SubagentStart,
    SubagentEnd,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HookResult {
    Continue,                    // 繼續執行
    Block { reason: String },    // 阻擋並回報原因
    Modify {                     // 改寫參數/結果
        new_arguments: Option<serde_json::Value>,
        new_result: Option<ToolResultContent>,
    },
    Warn { message: String },    // 僅警告不阻擋
}

#[derive(Debug, Clone)]
pub struct RuleMatch {
    pub rule_id: String,
    pub severity: Severity,      // Error | Warn | Info
    pub message: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity { Error, Warn, Info }
```

### 2.4 HookRegistry 與掛載點（`hooks/mod.rs`）

```rust
// 依據 crates/buzz-agent/src/agent.rs:143-244 RunCtx 結構
pub struct HookRegistry {
    pre_tool_use: Vec<Box<dyn PreToolUseHook>>,
    post_tool_use: Vec<Box<dyn PostToolUseHook>>,
    session: Vec<Box<dyn SessionHook>>,
    subagent: Vec<Box<dyn SubagentHook>>,
}

impl HookRegistry {
    pub fn new() -> Self { /* 從配置載入規則 */ }
    
    // 在 agent.rs run() 循環中調用
    pub async fn run_pre_tool_use(&self, ctx: &HookContext) -> HookResult { ... }
    pub async fn run_post_tool_use(&self, ctx: &HookContext, result: &ToolResult) -> HookResult { ... }
    pub async fn run_session_start(&self, ctx: &HookContext) -> HookResult { ... }
    pub async fn run_session_end(&self, ctx: &HookContext) -> HookResult { ... }
    pub async fn run_subagent_start(&self, ctx: &HookContext) -> HookResult { ... }
    pub async fn run_subagent_end(&self, ctx: &HookContext) -> HookResult { ... }
}
```

**掛載位置**（已由 `grep -n` 在真實代碼庫中逐一驗證，修正以下三處錯誤引用）：

| 計畫書原標註 | 修正後（真實代碼庫） | 說明 |
|-------------|-------------------|------|
| `WorkflowEngine`（不存在） | **無此組件** — 全代碼庫不存在 | 刪除；改為既有 `McpRegistry::call_hooks()` |
| 第 1320 行工具結果處理點 | **不存在** — 工具結果相關代碼分散於 `agent.rs:776/860/894/1076/1142/1301` | 已修正為各實際行號 |
| `glob_matches` 在 `hints.rs` | **實際在 `crates/buzz-agent/src/config.rs:491`** | 已修正 |

**實際掛載點（經 `grep -n` 確認）**：
- `RunCtx::run()` 函數定義：`agent.rs:324`
- `run()` 循環起點：`agent.rs:378` — `loop {`
- 工具列表載入：`agent.rs:406` — `let mut tools = self.mcp.tools();`
- 工具調用統一執行入口：`agent.rs:846` — `async fn execute_calls()`
- 單一工具調用核心：`agent.rs:1095` — `async fn invoke_tool_inner()`
- 工具結果合成：`agent.rs:860`（取消）、`agent.rs:894`（失敗）、`agent.rs:1076`（取消）、`agent.rs:1142`（InvokeOutcome::Failed）
- `_Stop` Hook 調用點：`agent.rs:756` — `self.mcp.call_hooks("_Stop", ...)`
- 子代理 handoff：`handoff.rs:176` — `async fn handoff()`

**既有 Hook 基礎設施（無需重建，直接複用）**：
- `McpRegistry::call_hooks()`：`crates/buzz-agent/src/mcp.rs:345` — 已有 Hook 調度機制（平行執行、超時控制、fail-open）
- `McpRegistry::is_hook()`：`mcp.rs:309` — Hook 工具識別（bare name 開頭為 `_`）
- `McpRegistry::has()`：`mcp.rs:302` — 工具存在性檢查
- `HookServers` 列舉：`crates/buzz-agent/src/config.rs:1119` — None / All / Only(Vec<String>)
- `HookServers::allows()`：`config.rs:1128` — 伺服器允許檢查
- `HookServers::is_disabled()`：`config.rs:1136` — 停用檢查
- `hook_timeout` 設定：`config.rs:593` — Hook 超時 Duration（預設 2500ms）
- `hook_servers` 設定：`config.rs:610` — Hook 伺服器允許清單（由 `MCP_HOOK_SERVERS` 環境變數控制）
- `push_hook_outputs_as_tool_results()`：`agent.rs:1260` — Hook 輸出轉為工具結果

**擴充方式**：不修改 `RunCtx` 結構體，透過 `McpRegistry` 已有的 `call_hooks()` 機制與 `_Stop` Hook 點注入新 Hook 伺服器。新增 Hook 伺服器以 `_` 開頭的工具名稱注冊，由 `is_hook()` 自動識別並對 LLM 隱藏。

### 2.5 8 條核心規則實作規格

| 規則 ID | 觸發點 | 邏輯摘要 | 阻擋/警告 | 測試案例 |
|---------|--------|----------|-----------|----------|
| `hardcoded-block` | PreToolUse | 掃描 `Write`/`Edit` 參數中的 `password`、`api_key`、`secret`、硬編碼 IP/URL | Block | 含 `sk-` 前綴字串的 Write 被阻擋 |
| `json-validation` | PreToolUse | `arguments` 必須為有效 JSON；`Write` 內容若為 `.json` 驗證語法 | Block | 非 JSON 字串觸發 Block |
| `design-doc-check` | PreToolUse | 專案根目錄下 `PLANS/*.md`、`GUIDES/*.md` 必含 `## 設計決策`、`## 驗收標準` 段落 | Warn | 缺段落發出 Warn |
| `branch-protection` | PreToolUse | `git push`/`git commit` 指令檢查分支名是否為 `main`/`master`/`release/*` | Block | 直接推 main 分支被阻擋 |
| `large-file-refuse` | PreToolUse | `Read`/`cat` 目標文件 > 500KB 時拒絕整讀，建議用 `head`/`rg` | Block | 1MB 文件 Read 被阻擋 |
| `entity-registry` | PreToolUse | `crates/buzz-agent/src/types.rs` 新增 struct/enum 需同步更新 `ENTITY_REGISTRY.md` | Warn | 新類型無註冊表條目發 Warn |
| `data-file-json` | PostToolUse | `Write` 完成後，若路徑匹配 `*.json`/`*.toml`/`*.yaml` 執行語法驗證 | Warn | 寫入無效 TOML 發 Warn |
| `skill-change-test` | PostToolUse | 修改 `skills/*/SKILL.md` 或 `*.rs` 技能相關代碼時，提醒運行 `cargo test -p buzz-agent` | Warn | 技能文件變更未跑測試發 Warn |

### 2.6 配置啟用機制
- 環境變數 `BUZZ_AGENT_HOOKS=1` 啟用（預設關閉，MVP 階段可控）
- 配置文件 `buzz-agent/hooks.toml`（可選，Phase 2 引入）

### 2.7 DoD 驗收標準（量化、可執行）
```bash
# 1. 編譯通過
cargo build -p buzz-agent 2>&1 | grep -q "Compiling buzz-agent" && echo "✅ Build OK"

# 2. 單元測試通過（新增 hooks 模組測試）
cargo test -p buzz-agent hooks:: 2>&1 | grep -q "test result: ok" && echo "✅ Unit Tests OK"

# 3. 整合測試：模擬 PreToolUse 阻擋硬編碼
printf '{"tool_name":"Write","arguments":{"path":"test.rs","content":"const API_KEY = \"sk-123\""}}' \
  | cargo run --bin test-hook-pre 2>&1 | grep -q "Block.*hardcoded" && echo "✅ Hardcoded Block Works"

# 4. 整合測試：PostToolUse JSON 驗證
printf '{"tool_name":"Write","arguments":{"path":"test.json","content":"{invalid}"}}' \
  | cargo run --bin test-hook-post 2>&1 | grep -q "Warn.*json" && echo "✅ JSON Validation Works"

# 5. 無破壞現有功能
cargo test -p buzz-agent 2>&1 | grep -q "test result: ok" && echo "✅ No Regression"
```

---

## 3. M2：路徑作用域規則系統 實作細節

### 3.1 架構定位
在專案根目錄 `c:\私人\ai agent\buzz\` 建立 `.rules/` 目錄，每條規則為獨立 YAML 文件，由 `HookRegistry` 在 `PreToolUse` 階段根據文件路徑自動匹配套用。

### 3.2 目錄結構（實際寫入路徑）
```
c:\私人\ai agent\buzz\.rules\
├── rust-backend.yaml          # Rust 後端代碼規則
├── cli-commands.yaml          # CLI 指令定義規則
├── agent-definitions.yaml     # Agent 定義規則
├── workflow-steps.yaml        # Workflow 步驟規則
├── config-files.yaml          # 配置文件規則
└── test-files.yaml            # 測試文件規則
```

### 3.3 規則 Schema（YAML Frontmatter - 參考 `hints.rs:83-105` 解析邏輯）

```yaml
# .rules/rust-backend.yaml
---
name: "rust-backend"
description: "Rust 後端代碼必須遵循錯誤處理模式與編碼規範"
paths:                    # glob 模式匹配（相對專案根目錄）
  - "crates/**/*.rs"
  - "!crates/**/tests/**"  # 排除測試文件
severity: "error"         # error | warn | info
rules:
  - id: "error-handling"
    pattern: "unwrap\(|expect\(|\.unwrap\(|\.expect\("
    message: "禁止使用 unwrap()/expect()，請使用 ? 操作符或 Result 處理"
    fix: "將 unwrap() 改為 ? 或 match 錯誤處理"
  - id: "todo-must-have-owner"
    pattern: "// TODO:|// FIXME:"
    message: "TODO/FIXME 必須標註負責人：// TODO(@name): 描述"
    fix: "添加 @負責人 標籤"
  - id: "no-panic-in-production"
    pattern: "panic\!\("
    message: "生產代碼禁止 panic!，請用 Result/Error 傳遞"
    fix: "改為返回 Err(...)"
  - id: "public-api-docs"
    pattern: "^pub (fn|struct|enum|trait|type|const|mod)"
    message: "Public API 必須有文檔註釋 ///"
    fix: "添加 /// 文檔註釋"
---
```

### 3.4 匹配引擎實作（`hooks/rules/path_scope.rs`）

```rust
// 依據 crates/buzz-agent/src/config.rs:491 glob_matches 函數複用
// 該函數為字元級 glob 匹配（支援 * 和 ?），已廣泛用於 model_id 過濾
pub fn path_matches_scope(path: &str, patterns: &[String]) -> bool {
    let mut matched = false;
    let mut excluded = false;
    for pattern in patterns {
        let is_exclude = pattern.starts_with('!');
        let pat = if is_exclude { &pattern[1..] } else { pattern };
        if glob_matches(pat, path) {
            if is_exclude {
                excluded = true;
                break;
            } else {
                matched = true;
            }
        }
    }
    matched && !excluded
}
```

**函式引用驗證**：
- `glob_matches()`：`crates/buzz-agent/src/config.rs:491` — 字元級 glob 匹配，支援 `*`（任意字元）和 `?`（單一字元）
- `glob_matches()` 呼叫點：`config.rs:484` — 用於 model_id 過濾
- `glob_matches()` 測試：`crates/buzz-dev-mcp/src/rg.rs:470`

### 3.5 規則載入與 YAML 解析

```rust
// 依據 crates/buzz-agent/src/hints.rs:249 strip_frontmatter 函數
// 複用既有 frontmatter 剝離邏輯
pub fn load_rule_file(path: &PathBuf) -> Result<RuleFile, RuleError> {
    let content = std::fs::read_to_string(path)?;
    let body = strip_frontmatter(&content);  // hints.rs:249
    let rule: RuleFile = serde_yaml::from_str(body)?;
    Ok(rule)
}

pub fn load_all_rules(rules_dir: &PathBuf) -> Result<Vec<RuleFile>, RuleError> {
    let mut rules = Vec::new();
    if !rules_dir.exists() {
        return Ok(rules);
    }
    for entry in std::fs::read_dir(rules_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "yaml") {
            match load_rule_file(&path) {
                Ok(rule) => rules.push(rule),
                Err(e) => tracing::warn!("Failed to load rule {:?}: {}", path, e),
            }
        }
    }
    Ok(rules)
}
```

### 3.6 錯誤處理與隔離

| 失敗模式 | 處理方式 |
|---------|---------|
| YAML 解析失敗 | `tracing::warn!` 記錄並跳過該規則，不中止加載 |
| 檔案不存在 | `load_all_rules()` 直接返回空 Vec |
| 規則匹配 panic | 規則引擎運行於 `spawn_blocking`，panic 不會傳播至主 async 循環 |
| glob 模式無效 | `glob_matches()` 純函式，無副作用，無效模式 Simply 不匹配 |

### 3.7 單元測試規格

```rust
// 測試路徑：crates/buzz-agent/tests/path_scope_rules.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_matches_scope_basic() {
        let patterns = vec!["crates/**/*.rs".to_string()];
        assert!(path_matches_scope("crates/buzz-agent/src/main.rs", &patterns));
        assert!(!path_matches_scope("docs/README.md", &patterns));
    }

    #[test]
    fn test_path_matches_scope_exclusion() {
        let patterns = vec![
            "crates/**/*.rs".to_string(),
            "!crates/**/tests/**".to_string(),
        ];
        assert!(path_matches_scope("crates/buzz-agent/src/main.rs", &patterns));
        assert!(!path_matches_scope("crates/buzz-agent/tests/foo.rs", &patterns));
    }

    #[test]
    fn test_glob_matches_function() {
        // 複用 config.rs:491 的 glob_matches
        assert!(glob_matches("crates/**/*.rs", "crates/buzz-agent/src/main.rs"));
        assert!(!glob_matches("crates/**/*.rs", "docs/README.md"));
    }
}
```

### 3.8 DoD 驗收標準（量化、可執行）

```bash
# 1. 單元測試通過
cargo test -p buzz-agent path_scope 2>&1 | grep -q "test result: ok" && echo "✅ Path Scope Tests OK"

# 2. 整合測試：規則載入
printf '{"tool_name":"Read","arguments":{"path":"crates/buzz-agent/src/main.rs"}}' \
  | cargo run --bin test-rule-load 2>&1 | grep -q "rust-backend" && echo "✅ Rule Loading Works"

# 3. 整合測試：路徑匹配
printf '{"path":"crates/buzz-agent/src/main.rs","patterns":["crates/**/*.rs"]}' \
  | cargo run --bin test-glob-match 2>&1 | grep -q "true" && echo "✅ Path Matching Works"

# 4. 無破壞現有功能
cargo test -p buzz-agent 2>&1 | grep -q "test result: ok" && echo "✅ No Regression"
```

---

## 4. M3：協作協議自動化技能 實作細節

### 4.1 架構定位
將現有「7 步公文閉環（老闆 ➔ Fizz ➔ Honey/Honey2 ➔ Sentry ➔ 老闆 ➔ Pollen ➔ Sentry ➔ Fizz）」編碼為單一技能 `collab-protocol`，複用現有技能發現機制（`hints.rs:201` `discover_skills_impl`）與技能載入（`builtin.rs:50` `call_load_skill`），不引入新執行引擎。

### 4.2 目錄結構（實際寫入路徑）
```
c:\私人\ai agent\buzz\.agents\skills\collab-protocol\
├── SKILL.md                    # 技能定義與 frontmatter
├── protocol.yaml               # 狀態機定義（7 步節點、轉移、嚴格度參數）
├── references/
│   ├── step_mapping.md         # 7 步節點對應表
│   └── strictness_levels.md    # minimal/standard 兩階嚴格度說明
└── scripts/
    └── validate_transition.py  # 可選：狀態轉移驗證腳本
```

### 4.3 SKILL.md 定義（YAML Frontmatter - 參考 `hints.rs:83-105`）
```yaml
---
name: "collab-protocol"
description: "將 7 步公文閉環編碼為狀態機的協作協議自動化技能。支援 minimal（僅 Fizz+Honey2）與 standard（含 Sentry 審核）兩階嚴格度。"
version: "1.0.0"
author: "Honey2"
tags: [collaboration, protocol, state-machine, automation]
---
# 協作協議自動化技能

## 用途
將人工執行的「7 步公文閉環」固化為可重複、可審計、可自動化的技能調用。

## 支援嚴格度
- **minimal**：僅 Fizz（分派）+ Honey2（規劃）執行，跳過 Sentry 審核與 Pollen 實作
- **standard**：完整 7 步（Fizz → Honey2 → Sentry → 老闆 → Pollen → Sentry → Fizz）

## 參數
```json
{
  "task_id": "string",           // 任務識別碼
  "strictness": "minimal|standard", // 嚴格度，預設 standard
  "initiator": "pubkey_hex",     // 發起人（老闆或代理）
  "payload": {}                  // 任務具體內容
}
```

## 狀態機節點
| 步驟 | 角色 | 動作 | 輸出 |
|------|------|------|------|
| 1 | 老闆 | 下達授權指令 | `authorization` event |
| 2 | Fizz | 接收分派、指派 Honey2 | `dispatch` event |
| 3 | Honey/Honey2 | 產出計畫書 | `plan.md` + `@Sentry` |
| 4 | Sentry | 四大門禁審核 | `review_result` (PASS/REJECT) |
| 5 | 老闆 | 門禁批示（同意/駁回） | `approval` event |
| 6 | Pollen | 實作代碼/產出物 | `implementation` + PR |
| 7 | Sentry | 複審驗收 | `acceptance` event |
| 8 | Fizz | 結案回報 | `closure` event |
```

### 4.4 狀態機定義（`protocol.yaml`）
```yaml
# .agents/skills/collab-protocol/protocol.yaml
---
version: "1.0"
initial_state: "PENDING_AUTHORIZATION"
strictness_levels:
  minimal:
    steps: [1, 2, 3, 5, 8]  # 跳過 Sentry 審核、Pollen 實作、Sentry 複審
    required_roles: ["owner", "fizz", "honey2"]
  standard:
    steps: [1, 2, 3, 4, 5, 6, 7, 8]  # 完整 7 步
    required_roles: ["owner", "fizz", "honey2", "sentry", "pollen"]

states:
  - id: "PENDING_AUTHORIZATION"
    description: "等待老闆授權"
    transitions:
      - event: "authorization_received"
        target: "DISPATCHED"
        action: "record_authorization"

  - id: "DISPATCHED"
    description: "Fizz 已分派給 Honey2"
    transitions:
      - event: "plan_submitted"
        target: "AWAITING_REVIEW"
        action: "record_plan"

  - id: "AWAITING_REVIEW"
    description: "等待 Sentry 審核（standard 模式）或直接進入老闆批示"
    transitions:
      - event: "review_passed"
        target: "PENDING_APPROVAL"
        action: "record_review"
      - event: "review_rejected"
        target: "REJECTED"
        action: "record_rejection"
      # minimal 模式跳過審核
      - event: "minimal_skip_review"
        target: "PENDING_APPROVAL"
        condition: "strictness == 'minimal'"
        action: "record_minimal_skip"

  - id: "PENDING_APPROVAL"
    description: "等待老闆門禁批示"
    transitions:
      - event: "approved"
        target: "IMPLEMENTING"
        action: "record_approval"
      - event: "rejected"
        target: "REJECTED"
        action: "record_rejection"

  - id: "IMPLEMENTING"
    description: "Pollen 實作中"
    transitions:
      - event: "implementation_submitted"
        target: "AWAITING_ACCEPTANCE"
        action: "record_implementation"

  - id: "AWAITING_ACCEPTANCE"
    description: "等待 Sentry 複審驗收（standard 模式）或直接結案"
    transitions:
      - event: "acceptance_passed"
        target: "CLOSING"
        action: "record_acceptance"
      - event: "acceptance_failed"
        target: "IMPLEMENTING"
        action: "record_acceptance_failure"
      # minimal 模式跳過複審
      - event: "minimal_skip_acceptance"
        target: "CLOSING"
        condition: "strictness == 'minimal'"
        action: "record_minimal_skip"

  - id: "CLOSING"
    description: "Fizz 結案回報"
    transitions:
      - event: "closure_reported"
        target: "COMPLETED"
        action: "record_closure"

  - id: "COMPLETED"
    description: "流程完成"
    terminal: true

  - id: "REJECTED"
    description: "被駁回，需重新開始"
    terminal: true
```

### 4.5 執行入口（技能載入後由 Agent 調用）
```rust
// 依據 crates/buzz-agent/src/builtin.rs:50 call_load_skill 模式
// 技能不包含 Rust 代碼，透過 load_skill 載入 protocol.yaml 後，
// 由 Agent 在對話中解析 YAML 並按狀態機推進
//
// 使用方式：
// 1. Agent 執行 load_skill("collab-protocol") 載入協議
// 2. Agent 根據當前狀態與嚴格度，判斷下一步驟與負責角色
// 3. Agent 在頻道中發送對應事件（dispatch、review、approval 等）
// 4. 透過 Nostr 事件記錄狀態轉移，形成可審計軌跡
```

### 4.6 7 步節點映射表（`references/step_mapping.md`）
| 我方流程步驟 | CCGS 對應機制 | 自動化方式 |
|-------------|--------------|------------|
| 老闆授權 | Vertical Authorization | 老闆發送 `authorization` 事件啟動 |
| Fizz 分派 | Vertical Delegation | Fizz 發送 `dispatch` 事件指派 Honey2 |
| Honey2 規劃 | Planning Agent | Honey2 產出計畫書並 `@Sentry` |
| Sentry 審核 | Architecture Review | Sentry 執行四大門禁檢查 |
| 老闆批示 | Gate Decision | 老闆發送 `approval` 事件 |
| Pollen 實作 | Implementation Agent | Pollen 開 PR、產出代碼 |
| Sentry 複審 | Quality Gate | Sentry 執行代碼審查與測試驗證 |
| Fizz 結案 | Closure Report | Fizz 發送 `closure` 事件完成 |

### 4.7 嚴格度參數化（`references/strictness_levels.md`）
```markdown
# 嚴格度定義

## minimal（輕量模式）
- 適用場景：微小改動、文檔更新、配置調整、風險極低任務
- 執行角色：老闆 → Fizz → Honey2 → 老闆 → Fizz（5 步）
- 跳過：Sentry 審核、Pollen 實作、Sentry 複審
- 預估時間：分鐘級

## standard（標準模式，預設）
- 適用場景：功能開發、架構變更、有風險的代碼修改
- 執行角色：完整 7 步 + 結案（8 步）
- 包含：所有審核與驗收環節
- 預估時間：小時至天級
```

### 4.8 錯誤處理與降級策略
| 失敗模式 | 處理方式 |
|---------|---------|
| 狀態機 YAML 解析失敗 | `tracing::error!` 記錄，回退至人工流程（不阻塞 Agent） |
| 角色未在線/無響應 | 發送提醒事件，超時後標記為 `STALLED`，老闆可介入 |
| 狀態轉移條件不滿足 | 記錄失敗原因，保持當前狀態，等待正確事件 |
| 嚴格度參數無效 | 預設為 `standard`，記錄警告 |
| 技能文件缺失 | `load_skill` 返回錯誤，Agent 提示用戶 |

### 4.9 DoD 驗收標準（量化、可執行）
```bash
# 1. 技能可被 load_skill 發現並載入
cargo run --bin buzz-agent -- load_skill collab-protocol 2>&1 | grep -q "collab-protocol" && echo "✅ Skill Discoverable"

# 2. 狀態機 YAML 解析無誤
python3 -c "import yaml; yaml.safe_load(open('.agents/skills/collab-protocol/protocol.yaml'))" && echo "✅ Protocol YAML Valid"

# 3. minimal 模式狀態轉移驗證
cargo run --bin test-collab-protocol -- --strictness minimal --simulate 2>&1 | grep -q "COMPLETED" && echo "✅ Minimal Flow Works"

# 4. standard 模式狀態轉移驗證
cargo run --bin test-collab-protocol -- --strictness standard --simulate 2>&1 | grep -q "COMPLETED" && echo "✅ Standard Flow Works"

# 5. 拒絕流程驗證
cargo run --bin test-collab-protocol -- --reject-at review 2>&1 | grep -q "REJECTED" && echo "✅ Rejection Flow Works"

# 6. 無破壞現有功能
cargo test -p buzz-agent 2>&1 | grep -q "test result: ok" && echo "✅ No Regression"
```

---

## 5. 整體交付時程與里程碑

| 階段 | 任務 | 預估工時 | 依賴 |
|------|------|----------|------|
| Phase 1 | M1 Hooks 模組骨架 + 掛載點接線 | 0.5 天 | 無 |
| Phase 2 | M1 8 條核心規則實作 | 1 天 | Phase 1 |
| Phase 3 | M2 路徑作用域規則系統 | 0.5 天 | 無 |
| Phase 4 | M3 協作協議技能包 | 1 天 | 無 |
| Phase 5 | 整合測試與驗收 | 0.5 天 | Phase 1-4 |
| **總計** | | **3.5 天** | |

---

## 6. 四大門禁自查表（提交 Sentry 審核前必過）

| 門禁 | 檢查項目 | 狀態 | 備註 |
|------|---------|------|------|
| **代碼符號真實性** | 所有 Struct、函式、行號在代碼庫確認存在 | ✅ | 已修正 WorkflowEngine、1320 行、glob_matches 路徑 |
| **協議鑑權閉環** | Hook 掛載點複用既有 McpRegistry::call_hooks，無需修改 RunCtx | ✅ | 掛載點：agent.rs:756 (_Stop)、agent.rs:846 (execute_calls) |
| **強制失敗模式推演** | M1/M2/M3 各掛載點均有錯誤分支、隔離機制、降級策略 | ✅ | M1: spawn_blocking 隔離；M2: warn+skip；M3: 回退人工 |
| **MVP 敏捷交付引導** | 範圍鎖定 3 機制，總工時 3.5 天，Phase 1 可獨立驗證 | ✅ | 無外部依賴、零破壞性變更 |

---

## 7. 回滾機制（可執行指令）

```bash
# M1 回滾：移除 hooks 模組
rm -rf c:/私人/ai agent/buzz/crates/buzz-agent/src/hooks/
git -C "c:/私人/ai agent/buzz" checkout -- crates/buzz-agent/src/agent.rs

# M2 回滾：移除 .rules 目錄
rm -rf c:/私人/ai agent/buzz/.rules/

# M3 回滾：移除協作協議技能
rm -rf c:/私人/ai agent/buzz/.agents/skills/collab-protocol/

# 完整回滾（單一指令）
git -C "c:/私人/ai agent/buzz" reset --hard HEAD
```

---

**計畫書版本：v2（修正版）**  
**狀態：就緒提交 @Sentry 進行第 2 輪審核**  
**修正記錄：補全 M2 完整區段、新增 M3 完整區段、修正所有代碼引用至真實行號、補充 M2/M3 DoD 驗收指令、新增各機制失敗模式分析、確認擴充方式為複用既有 McpRegistry 機制**
