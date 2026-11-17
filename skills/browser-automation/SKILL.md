---
name: browser-automation
description: "瀏覽器自動化技能 - 基於 Playwright Rust，提供安全的 Web 互動、GUI 操作與憑證隔離能力。支援多標籤、Stealth 模式、審計日誌、錄影回放，並整合 Antigravity 的 Encrypted Vault 進行憑證管理。"
argument-hint: "[--navigate <url>] [--click <selector>] [--type <selector> <text>] [--screenshot] [--pdf] [--eval <js>] [--wait <selector>] [--skill-config <path>]"
user-invocable: true
allowed-tools:
  - read_file
  - write_file
  - edit_file
  - shell
  - credential_proxy
  - sandbox_exec
model: "nvidia/nemotron-3-ultra-550b-a55b"
author: "Honey2"
version: "0.1.0"
tags: [browser-automation, playwright, gui-interaction, skill-system, antigravity-integration]
---

# Browser Automation 技能規格書

## 1. 概覽

提供安全、可審計的瀏覽器自動化能力，作為 Antigravity 底層架構與 Buzz Agent 技能系統之間的橋樑。

### 核心特性
- **Playwright Rust** 驅動：Chromium/Firefox/WebKit 三引擎支援
- **能力本位安全**：每個技能實例持有 Capability Token，限制域名、憑證、操作類型、網路出站
- **Encrypted Vault 整合**：透過 `credential_proxy` 安全注入憑證，絕不在本地明文存儲
- **多標籤/Context 隔離**：每個任務獨立 BrowserContext，避免狀態洩漏
- **Stealth 模式**：反自動化偵測繞過
- **完整審計**：操作日誌、網路請求、Console 輸出、錄影回放

## 2. 架構設計

### 2.1 核心組件

```
browser-automation/
├── Cargo.toml
├── SKILL.md
├── src/
│   ├── lib.rs              # 公開 API
│   ├── browser_pool.rs     # 瀏覽器池管理
│   ├── context_manager.rs  # BrowserContext 隔離與生命週期
│   ├── capability.rs       # Capability Token 定義與驗證
│   ├── skills/             # 7 大內建技能
│   │   ├── navigate.rs
│   │   ├── click.rs
│   │   ├── type_text.rs
│   │   ├── screenshot.rs
│   │   ├── eval_js.rs
│   │   ├── wait_for.rs
│   │   └── pdf.rs
│   ├── audit.rs            # 審計日誌記錄
│   ├── stealth.rs          # Stealth 模式注入
│   └── vault.rs            # Encrypted Vault 憑證獲取
```

### 2.2 Capability Token 模型

```rust
pub struct BrowserCapability {
    pub allowed_domains: Vec<String>,           // 僅允許存取的域名
    pub allowed_credentials: Vec<Uuid>,         // 僅允許使用的憑證 ID
    pub max_execution_time: Duration,           // 單次執行上限 (預設 60s)
    pub allowed_actions: Vec<BrowserAction>,    // 允許的操作類型
    pub network_egress: NetworkEgressPolicy,    // 網路出站策略
}

pub enum NetworkEgressPolicy {
    None,                           // 完全離線
    AllowedDomains(Vec<String>),    // 僅允許特定域名
    All,                            // 無限制 (僅管理員)
}
```

### 2.3 技能配置範例 (TOML)

```toml
[skill]
name = "github-issue-scraper"
description = "抓取 GitHub Issue 內容"
capability = { 
    allowed_domains = ["github.com"],
    allowed_actions = ["Navigate", "Click", "TypeText", "Screenshot", "EvalJS"],
    max_execution_time_secs = 120,
    network_egress = "AllowedDomains"
}

[vault]
required_credentials = ["github-token"]  # 對應 Encrypted Vault 中的 credential ID
```

## 3. 7 大內建技能

| 技能 | 功能 | 輸入參數 | 輸出 |
|------|------|----------|------|
| `navigate` | 頁面導航 | `url`, `wait_until` | 頁面標題、URL、載入時間 |
| `click` | 元素點擊 | `selector`, `button`, `modifiers` | 點擊結果、新頁面 ID (若彈窗) |
| `type_text` | 文字輸入 | `selector`, `text`, `delay_ms` | 輸入確認 |
| `screenshot` | 截圖 | `full_page`, `path`, `format` | Base64 圖片或檔案路徑 |
| `pdf` | 導出 PDF | `path`, `format`, `margin` | PDF 檔案路徑 |
| `eval_js` | 執行 JavaScript | `script`, `args` | 序列化結果 |
| `wait_for` | 等待條件 | `selector` / `function` / `timeout` | 條件滿足確認 |

## 4. 安全模型

### 4.1 權限分級
- **User Level**：受限 Capability，需聲明域名白名單
- **Skill Level**：預設 Capability，由技能配置決定
- **Admin Level**：完整權限，可跨域、使用任意憑證

### 4.2 憑證流程
1. 技能聲明 `required_credentials` 陣列
2. 執行時調用 `credential_proxy` 獲取憑證
3. 憑證僅存在於記憶體，執行結束即銷毀
4. 審計日誌記錄「使用了哪個 credential_id」，不記錄明文

### 4.3 沙盒執行
所有瀏覽器操作透過 `sandbox_exec` 在隔離環境執行，限制：
- CPU/記憶體上限
- 網路出站策略
- 檔案系統存取 (僅允許輸出目錄)

## 5. 介面定義

### 5.1 CLI 介面
```bash
buzz skills run browser-automation --navigate "https://github.com" --screenshot
buzz skills run browser-automation --skill-config config/github-scraper.toml
```

### 5.2 程式化介面
```rust
use browser_automation::{BrowserAutomation, BrowserCapability, SkillConfig};

let capability = BrowserCapability::builder()
    .allowed_domains(vec!["github.com"])
    .allowed_actions(vec![BrowserAction::Navigate, BrowserAction::Screenshot])
    .max_execution_time(Duration::from_secs(60))
    .build();

let automation = BrowserAutomation::new(capability)?;
let result = automation.navigate("https://github.com").await?;
automation.screenshot(true, "screenshot.png").await?;
```

## 6. 測試策略

### 6.1 單元測試
- Capability 驗證邏輯
- Context 隔離測試
- 憑證獲取與銷毀流程

### 6.2 整合測試
- 完整導航→點擊→截圖流程
- 多標籤並發隔離
- Stealth 模式偵測繞過驗證

### 6.3 安全測試
- 域名越界嘗試被阻擋
- 憑證明文不落地
- 網路出站策略生效

## 7. 監控指標

- `browser_sessions_total` - 瀏覽器會話總數
- `browser_actions_total` - 各類操作執行次數
- `browser_errors_total` - 錯誤分類統計
- `credential_usage_total` - 憑證使用次數
- `session_duration_seconds` - 會話持續時間分布

## 8. 風險與對策

| 風險 | 可能性 | 影響 | 緩解措施 |
|------|--------|------|----------|
| Playwright 版本不相容 | 中 | 高 | 鎖定版本、CI 測試矩陣 |
| 反偵測失敗 | 中 | 中 | 定期更新 Stealth 腳本、多策略備援 |
| 憑證洩漏 | 低 | 嚴重 | 記憶體加密、零化、審計追蹤 |
| 瀏覽器進程殭屍 | 中 | 中 | 強制超時殺進程、資源配額 |

## 9. 里程碑

| 階段 | 任務 | 預估工時 |
|------|------|----------|
| Phase 1 | Browser Pool + Context 隔離 + Capability 驗證 | 2 天 |
| Phase 2 | 7 大內建技能實作 + CLI 介面 | 3 天 |
| Phase 3 | Encrypted Vault 整合 + Stealth + 錄影 | 2 天 |
| Phase 4 | 審計日誌 + 測試套件 + 文檔 | 1 天 |

## 10. 簽署與審核

- **企劃**: Honey2
- **審核**: Sentry (架構安全官)
- **實作**: Pollen (核心技術主管)
- **歸檔**: Wax (第二大腦記憶官)

> **Sentry Audit**: PENDING — 待審核
> **主理人授權**: 2026-11-15 同意立項
