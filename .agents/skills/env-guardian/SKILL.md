---
name: "env-guardian"
description: "執行前環境變數與依賴缺失健檢，預防執行期錯誤，提升系統穩健度的 Rust 內建技能"
version: "1.0.0"
author: "Honey2"
tags: [env, guardian, validation, automation, dependency, service]
status: draft
created: 2026-11-02
---

# Env-Guardian Skill 實作計畫書

## 目標與範疇

將「執行前環境變數與依賴缺失健檢」工作標準化為 **Buzz Native Skill v1.0**，在工具執行/子代理啟動/腳本運行前自動檢測環境完整性，預防執行期錯誤，提升系統穩健度。

### 核心痛點

- 環境變數缺失導致運行時崩潰 (如 `BUZZ_RELAY_URL`、`BUZZ_PRIVATE_KEY`、`BUZZ_AUTH_TAG`)
- 依賴工具版本不符 (Rust/cargo、Node/npm、Python/pip、Git) 導致建構失敗
- 背景服務 (資料庫、Redis、Relay 連線) 不可用導致測試/部署假陰性
- 缺乏統一的「執行前檢查」機制，錯誤發現過晚，回饋循環過長

---

## 架構設計

### 1. 技能結構
```
.c:\私人\ai agent\buzz\.agents\skills\env-guardian\
├── SKILL.md                    # 技能定義 (YAML frontmatter + Markdown body)
├── manifest.json               # 技能元資料 (供 catalog 發現)
├── src/
│   ├── env_checker.rs          # 環境變數檢查器 (必填/可選/驗證規則)
│   ├── dependency_checker.rs   # 依賬工具版本檢查器
│   ├── service_probe.rs        # 背景服務健康探針 (TCP/HTTP/gRPC)
│   ├── circuit_breaker.rs      # 三態熔斷器 (參考 system-health-guardian)
│   ├── health_check_runner.rs  # 整合健檢執行器 (CLI 介面)
│   └── report_generator.rs     # 檢查報告生成 (Markdown + JSON)
└── tests/
    ├── env_checker_tests.rs
    ├── dependency_checker_tests.rs
    └── circuit_breaker_tests.rs
```

### 2. 技能註冊機制
- 依據 `crates/buzz-agent/src/hints.rs:15-24` `SkillEntry` 結構
- 依據 `crates/buzz-agent/src/builtin.rs:52-124` `call_load_skill` 實作
- 技能目錄掃描路徑：`SKILL_DIRS = &[\".agents/skills\", \".goose/skills\", \".claude/skills\"]` (hints.rs:8)
- Frontmatter 解析：`parse_skill_frontmatter` (hints.rs:83-105)

### 3. 整合點
- **Hook 階段**：`PreToolUse` 於 `Write`/`Edit`/`Task` (子代理啟動) 時自動觸發 (hooks/mod.rs:50-61)
- **SessionStart Hook**：Session 啟動時執行完整環境健檢 (hooks/mod.rs:76-87)
- **SubagentStart Hook**：子代理啟動前檢查其所需環境 (hooks/mod.rs:102-113)
- **CLI 整合**：`buzz env check`、`buzz env doctor` 子命令
- **Skill 載入**：`load_skill(\"env-guardian\")` 提供完整檢查 API

---

## 核心功能模組規格

### 模組 1：環境變數檢查器 (`env_checker.rs`)
```rust
// 依據 crates/buzz-agent/src/config.rs 實際需求定義
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EnvVarSpec {
    pub name: String,
    pub required: bool,
    pub description: String,
    pub validator: Option<EnvValidator>,  // 可選：格式驗證 (URL、Hex、路徑等)
    pub default: Option<String>,          // 可選：預設值
    pub secret: bool,                     // 是否為敏感資訊 (報告中遮罩)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
pub enum EnvValidator {
    Url { schemes: Vec<String> },         // 如 ["https", "wss"]
    Hex { length: Option<usize> },        // 如 64 (nostr pubkey)
    Path { must_exist: bool },            // 檔案/目錄路徑
    Enum { values: Vec<String> },         // 列舉值
    Regex { pattern: String },            // 自訂正則
}

#[derive(Debug, Clone)]
pub struct EnvCheckReport {
    pub var_name: String,
    pub status: CheckStatus,
    pub value_preview: Option<String>,    // 非敏感時顯示前 8 字元
    pub error: Option<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Ok,
    MissingRequired,
    MissingOptional,
    InvalidFormat,
    ValidationFailed,
}

impl EnvChecker {
    pub fn new(specs: Vec<EnvVarSpec>) -> Self { ... }
    
    /// 核心檢查：支援 .env 檔案載入、系統環境變數、預設值回退
    pub fn check_all(&self) -> Vec<EnvCheckReport> { ... }
    
    /// 僅檢查必填項 (快速模式)
    pub fn check_required_only(&self) -> Vec<EnvCheckReport> { ... }
    
    /// 產生修正建議腳本 (export 陳述句)
    pub fn generate_fix_script(&self, reports: &[EnvCheckReport]) -> String { ... }
}
```

**Buzz 核心必檢環境變數 (依據 config.rs 與 agent.rs)**：
| 變數名 | 必填 | 驗證規則 | 說明 |
|--------|------|----------|------|
| `BUZZ_RELAY_URL` | ✅ | `Url { schemes: ["wss", "ws"] }` | Nostr Relay 連線位址 |
| `BUZZ_PRIVATE_KEY` | ✅ | `Hex { length: 64 }` | Agent 簽名私鑰 (hex) |
| `BUZZ_AUTH_TAG` | ⚠️ 可選 | `Hex { length: 32 }` | Owner-reviewed agent drafts 授權碼 |
| `BUZZ_GIT_IDENTITY` | ⚠️ 可選 | `Enum { ["agent", "user"] }` | Git 提交身份模式 |
| `HOME` / `USERPROFILE` | ✅ | `Path { must_exist: true }` | 使用者主目錄 |

### 模組 2：依賬工具版本檢查器 (`dependency_checker.rs`)
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub command: String,              // 如 "cargo --version"
    pub version_arg: String,          // 如 "--version"
    pub min_version: Option<String>,  // 語義化版本下限
    pub max_version: Option<String>,  // 語義化版本上限 (可選)
    pub required: bool,
    pub install_hint: String,         // 安裝指引
}

#[derive(Debug, Clone)]
pub struct DependencyReport {
    pub tool_name: String,
    pub status: CheckStatus,
    pub detected_version: Option<String>,
    pub required_range: String,
    pub error: Option<String>,
    pub install_command: Option<String>,
}

impl DependencyChecker {
    pub fn new(specs: Vec<ToolSpec>) -> Self { ... }
    
    pub async fn check_all(&self) -> Vec<DependencyReport> { ... }
    
    /// 解析版本字串 (支援 "cargo 1.82.0", "node v20.11.0", "Python 3.12.3" 等)
    fn parse_version_output(&self, output: &str, tool: &str) -> Option<String> { ... }
    
    /// 語義化版本比較
    fn version_satisfies(&self, detected: &str, min: Option<&str>, max: Option<&str>) -> bool { ... }
}
```

**核心依賬檢查清單**：
| 工具 | 指令 | 最低版本 | 必填 | 安裝提示 |
|------|------|----------|------|----------|
| Rust/cargo | `cargo --version` | 1.75+ | ✅ | `rustup update` |
| Node.js | `node --version` | 18+ | ⚠️ | `nvm install 20` |
| npm/pnpm | `pnpm --version` | 8+ | ⚠️ | `corepack enable pnpm` |
| Git | `git --version` | 2.40+ | ✅ | 系統套件管理器 |
| Python | `python --version` | 3.11+ | ⚠️ | `pyenv install 3.12` |
| protoc | `protoc --version` | 25+ | ⚠️ | `brew install protobuf` |

### 模組 3：背景服務健康探針 (`service_probe.rs`)
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServiceProbe {
    pub name: String,
    pub kind: ProbeKind,
    pub target: String,           // host:port 或 URL
    pub timeout_ms: u64,
    pub required: bool,
    pub expected_response: Option<String>, // HTTP: 狀態碼/body 關鍵字
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ProbeKind {
    Tcp { port: u16 },
    Http { path: String, method: String, headers: HashMap<String, String> },
    Grpc { service: String },
    Custom { command: String },   // 自訂探針腳本
}

#[derive(Debug, Clone)]
pub struct ServiceProbeReport {
    pub service_name: String,
    pub status: CheckStatus,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

impl ServiceProbe {
    pub async fn probe(&self) -> ServiceProbeReport { ... }
    
    async fn tcp_probe(&self, host: &str, port: u16) -> ServiceProbeReport { ... }
    async fn http_probe(&self, url: &str) -> ServiceProbeReport { ... }
}
```

**預設探針目標 (可配置)**：
| 服務 | 類型 | 目標 | 必填 | 說明 |
|------|------|------|------|------|
| Nostr Relay | WebSocket | `wss://presentyourlove.communities.buzz.xyz` | ✅ | 核心通訊骨幹 |
| 本地資料庫 | TCP | `127.0.0.1:5432` | ⚠️ | PostgreSQL (如啟用) |
| Redis | TCP | `127.0.0.1:6379` | ⚠️ | 快取/佇列 (如啟用) |
| HTTP API | HTTP GET | `http://127.0.0.1:8080/health` | ⚠️ | 本地服務健康端點 |

### 模組 4：三態熔斷器 (`circuit_breaker.rs`) — 參考 `system-health-guardian`
```rust
// 依據 skills/system-health-guardian 三態熔斷器設計
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CircuitState {
    Closed,   // 正常：允許請求通過，統計失敗率
    Open,     // 熔斷：拒絕請求，快速失敗
    HalfOpen, // 半開：允許少量請求測試恢復
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub name: String,
    pub state: Arc<RwLock<CircuitState>>,
    pub failure_threshold: u32,      // 連續失敗閾值 (預設 5)
    pub success_threshold: u32,      // 半開態恢復所需成功數 (預設 3)
    pub timeout: Duration,           // 開啟狀態持續時間 (預設 30s)
    pub failure_count: Arc<AtomicU32>,
    pub success_count: Arc<AtomicU32>,
    pub last_state_change: Arc<RwLock<Instant>>,
}

impl CircuitBreaker {
    pub fn new(name: &str) -> Self { ... }
    
    /// 執行受保護操作
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Future<Output = Result<T, E>>,
    { ... }
    
    /// 手動重置 (管理員操作)
    pub fn
```

### 模組 5：報告生成器 (`report_generator.rs`)
```rust
// 輸出格式：Markdown (人類閱讀) + JSON (機器解析)
pub fn generate_report(
    patch: &ParsedPatch,
 
