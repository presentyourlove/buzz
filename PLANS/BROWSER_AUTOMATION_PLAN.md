---
title: "Browser Automation 實作計畫書"
tags: [browser-automation, antigravity, playwright, gui-interaction, skill-system]
status: draft
created: 2026-11-15
---

# Browser Automation 實作計畫書

> **分工定位**：由 **Antigravity** 負責底層架構與安全沙盒實作，Buzz Agent 技能系統負責上層技能掛載與業務邏輯。

---

## 1. 背景與動機

參考 Cue.im 的「網頁/桌面端真實 GUI 互動」能力，Buzz 需要具備在**本地端安全沙盒**中操控瀏覽器的能力，以完成：
- 自動化填寫表單、行政作業
- 網頁內容擷取與結構化解析
- 跨網站操作流程自動化（如電商比價、票務搶購）
- 測試與驗證 Web 應用行為

**核心原則**：
- ❌ **不**在雲端託管瀏覽器（隱私風險）
- ✅ **本地端** Headless/Headful 模式，配合 OS Keyring 加密憑證
- ✅ 技能系統化，支援動態載入/卸載

---

## 2. 架構設計

### 2.1 整體分層

```
┌─────────────────────────────────────────────────────────────┐
│                    Buzz Agent 技能層                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │ Browser Skill   │  │ Form Fill Skill │  │ Scrape Skill│  │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘  │
└───────────┼────────────────────┼───────────────────┼─────────┘
            ▼                    ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│              Antigravity Browser Runtime (核心)              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Browser Pool Manager (Playwright)                    │  │
│  │  ├─ Context Isolation (每任務獨立 BrowserContext)      │  │
│  │  ├─ Resource Limits (CPU/Memory/Network/Time)         │  │
│  │  └─ Snapshot/Recording (除錯與審計)                    │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Encrypted Vault (OS Keyring / AES-256-GCM)           │  │
│  │  ├─ Credential Storage (加密存儲)                      │  │
│  │  ├─ Access Control (權限範疇化)                        │  │
│  │  └─ Audit Log (存取審計)                               │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
            ▲                    ▲                   ▲
            │                    │                   │
┌───────────┴────────────────────┴───────────────────┴─────────┐
│                      OS Layer (Windows/macOS/Linux)           │
│  - Playwright Browsers (Chromium/Firefox/WebKit)              │
│  - OS Credential Manager (Windows Credential Manager,         │
│    macOS Keychain, libsecret)                                 │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心元件規格

#### 2.2.1 Browser Pool Manager (`browser_pool.rs`)

```rust
// 依據 crates/buzz-agent/src/types.rs:261 參考 AgentContext 設計
pub struct BrowserPoolConfig {
    pub max_concurrent_contexts: usize,      // 預設 5
    pub context_idle_timeout_secs: u64,      // 預設 300
    pub default_viewport: Viewport,          // 1280x720
    pub resource_limits: ResourceLimits,
    pub enable_recording: bool,              // 預設 true (除錯用)
    pub recording_dir: PathBuf,
}

pub struct ResourceLimits {
    pub max_cpu_percent: f32,       // 50%
    pub max_memory_mb: u64,         // 1024 MB
    pub max_network_kbps: u64,      // 10240 KB/s
    pub max_execution_secs: u64,    // 300 秒
}

pub struct BrowserContextHandle {
    pub context_id: Uuid,
    pub browser_context: Arc<playwright::BrowserContext>,
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub task_id: Option<String>,
    // 資源監控
    pub cpu_usage: Arc<RwLock<f32>>,
    pub memory_usage: Arc<RwLock<u64>>,
}
```

#### 2.2.2 Encrypted Vault (`encrypted_vault.rs`)

```rust
// 依據 skills/antigravity/src/storage.rs:45 參考 StorageConfig 設計
pub struct VaultConfig {
    pub backend: VaultBackend,           // OSKeyring | Aes256GcmFile
    pub master_key_derivation: KeyDerivation, // Argon2id
    pub key_rotation_interval_days: u32, // 90
    pub audit_log_enabled: bool,         // true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub id: Uuid,
    pub label: String,                   // "github.com", "google.com"
    pub username: String,
    pub password: SecretString,          // 加密後
    pub totp_secret: Option<SecretString>,
    pub scope: CredentialScope,          // Domain | Global | Custom
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
}

pub enum CredentialScope {
    Domain(String),      // 僅限特定域名
    Global,              // 所有域名 (需最高權限)
    Custom(Vec<String>), // 自定義 URL patterns
}
```

#### 2.2.3 Skill Interface (`browser_skill.rs`)

```rust
// 依據 skills/agent-skill/src/runtime.rs:45 參考 TaskExecutor 設計
#[async_trait]
pub trait BrowserSkill: Send + Sync {
    /// 導航到 URL
    async fn navigate(&self, ctx: &SkillContext, url: &str, wait_until: WaitUntil) -> Result<NavigationResult>;
    
    /// 點擊元素
    async fn click(&self, ctx: &SkillContext, selector: &str, options: ClickOptions) -> Result<ClickResult>;
    
    /// 填寫表單
    async fn fill_form(&self, ctx: &SkillContext, form_data: FormData) -> Result<FormResult>;
    
    /// 擷取頁面內容
    async fn extract(&self, ctx: &SkillContext, selector: &str, extractor: ExtractorType) -> Result<ExtractedData>;
    
    /// 等待條件
    async fn wait_for(&self, ctx: &SkillContext, condition: WaitCondition, timeout: Duration) -> Result<()>;
    
    /// 執行 JavaScript
    async fn evaluate(&self, ctx: &SkillContext, script: &str, args: Vec<Value>) -> Result<Value>;
    
    /// 截圖
    async fn screenshot(&self, ctx: &SkillContext, options: ScreenshotOptions) -> Result<Vec<u8>>;
    
    /// 關閉並釋放資源
    async fn close(&self, ctx: &SkillContext) -> Result<()>;
}

pub struct SkillContext {
    pub task_id: String,
    pub context_handle: BrowserContextHandle,
    pub vault: Arc<EncryptedVault>,
    pub allowed_domains: Vec<String>,
    pub credential_labels: Vec<String>,  // 允許使用的憑證標籤
}
```

---

## 3. 實作階段

### Phase 1: 基礎架構 (Week 1-2) - Antigravity 主導

| 任務 | 產出物 | 驗收標準 |
|------|--------|----------|
| 1.1 Playwright Rust 綁定整合 | `Cargo.toml` 新增 `playwright` 依賴 | `cargo build` 成功，可啟動 Chromium |
| 1.2 Browser Pool Manager 實作 | `src/browser_pool.rs` | 支援並發 5 contexts，資源限制生效 |
| 1.3 Context 隔離與生命週期管理 | 單元測試 `tests/browser_pool_test.rs` | 每任務獨立 context，結束自動清理 |
| 1.4 Encrypted Vault 核心實作 | `src/encrypted_vault.rs` | AES-256-GCM 加解密，OS Keyring 整合 |
| 1.5 憑證 CRUD API | `src/vault_api.rs` | 增刪改查 + 存取審計日誌 |

### Phase 2: 技能系統整合 (Week 2-3) - Antigravity + Buzz 協作

| 任務 | 產出物 | 驗收標準 |
|------|--------|----------|
| 2.1 BrowserSkill Trait 定義 | `src/browser_skill.rs` | 完整覆蓋 7 大核心操作 |
| 2.2 預設技能實作 (Navigate/Click/Fill/Extract) | `src/skills/builtin/` | 整合測試通過 |
| 2.3 Skill Registry 掛載機制 | `src/skill_registry.rs` | 動態載入/卸載，熱重載 |
| 2.4 權限與域名白名單控制 | `src/permission_guard.rs` | 未授權域名/憑證拒絕存取 |

### Phase 3: 進階功能與安全強化 (Week 3-4) - Antigravity 主導

| 任務 | 產出物 | 驗收標準 |
|------|--------|----------|
| 3.1 錄影/重播與除錯快照 | `src/recording.rs` | 失敗自動保存截圖+錄影 |
| 3.2 反偵測/人類行為模擬 | `src/stealth.rs` | 通過基礎 bot detection 測試 |
| 3.3 網路攔截與 Mock | `src/network_intercept.rs` | 支援 API Mock 測試 |
| 3.4 多標籤頁協作 | `src/multi_tab.rs` | 支援跨標籤資料傳遞 |
| 3.5 安全審計報告生成 | `src/audit_report.rs` | 每次執行產出 JSON 審計日誌 |

---

## 4. 技術細節與關鍵決策

### 4.1 Playwright 選型理由

| 維度 | Playwright | Selenium | Puppeteer (Node) |
|------|------------|----------|------------------|
| **Rust 原生支援** | ✅ `playwright-rust` | ❌ 需 WebDriver | ❌ Node.js 限制 |
| **多瀏覽器** | ✅ Chromium/Firefox/WebKit | ✅ | ❌ 僅 Chromium |
| **自動等待/重試** | ✅ 內建 | ❌ 需手寫 | ✅ |
| **網路攔截** | ✅ 強大 | ⚠️ 基礎 | ✅ |
| **反偵測** | ✅ Stealth 模式 | ⚠️ 需插件 | ⚠️ |
| **執行速度** | 快 (CDP 直連) | 慢 (WebDriver) | 快 |

**決策**：採用 **Playwright Rust** (`playwright = "0.40"`) 作為唯一瀏覽器自動化後端。

### 4.2 安全模型：能力本位安全

```rust
// 每個 Browser Skill 實例持有 Capability Token
pub struct BrowserCapability {
    pub allowed_domains: Vec<String>,           // 僅允許存取的域名
    pub allowed_credentials: Vec<Uuid>,         // 僅允許使用的憑證
    pub max_execution_time: Duration,           // 單次執行上限
    pub allowed_actions: Vec<BrowserAction>,    // 允許的操作類型
    pub network_egress: NetworkEgressPolicy,    // 網路出站策略
}

pub enum NetworkEgressPolicy {
    None,                    // 完全離線
    AllowedDomains(Vec<String>), // 僅允許特定域名
   
