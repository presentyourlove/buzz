---
title: "System Health Guardian 技能實作計畫書"
tags: [system-health-guardian, skill, implementation, monitoring, circuit-breaker]
status: draft
created: 2026-11-02
version: "1.0"
---

# System Health Guardian 技能實作計畫書

> **技能名稱**：`system-health-guardian`
> **來源提案**：每日系統進化提案 - 2026-11-02 (技能合成候選 #2)
> **狀態**：待老闆批准立項
> **編寫人**：Wax (記憶清潔工與歸檔官)
> **審核人**：@Sentry
> **批准人**：可愛小寶寶（主理人）

---

## 1. 專案背景與核心需求

### 1.1 來自技能候選規範 (SKILL.md)
- **描述**：自動化執行 CLI 狀態診斷、環境變數檢驗、熔斷日誌排查與常駐守護重啟標準作業流程
- **觸發詞頻**：5 (維運高頻需求)
- **觸發場景**：
  - 日常進化流程啟動前之例行前置體檢
  - 背景守護行程異常退出或通訊失敗時
  - 熔斷器跳脫時之故障排查

### 1.2 核心目標
- **自動化體檢**：純 Rust 實作健康檢查 (CLI 版本、環境變數驗證、進程狀態檢測)，**不依賴外部 Python 腳本**
- **失敗日誌分析**：掃描 `WORK_LOGS/` 下失敗日誌與錯誤堆疊
- **常駐行程監控**：檢視 3 大背景常駐行程 (OpenRouter:8080, SyncObsidian:PID, Proxy:3000) 運作狀態，**使用 `std::process::Command` + `sysinfo` crate 原生實作**
- **熔斷自癒**：發現熔斷日誌，排查根因並於確認修復後重置，**熔斷狀態檔案：`WORK_LOGS/circuit_breaker.json`**
- **健全性報告**：彙整報告呈報至專屬頻道
- **告警通知**：異常時透過 Nostr Kind:1 事件發布至頻道，含 `e` 標籤引用根事件、`p` 標籤 @mention @Sentry/@Pollen

---

## 2. 架構設計

### 2.1 整體架構圖

```
+-----------------------------------------------------------------+
|                   System Health Guardian Skill                  |
+-----------------------------------------------------------------+
|  +--------------+    +--------------+    +------------------+  |
|  |  Diagnoser   |--->|  Analyzer    |--->|  Remediator      |  |
|  |  (CLI檢查)   |    |  (日誌/熔斷) |    |  (重置/重啟)     |  |
|  +--------------+    +--------------+    +--------+---------+  |
|                                                   |            |
|  +--------------+    +--------------+             v            |
|  |  Reporter    |<---|  Notifier    |    +------------------+  |
|  |  (報告生成)  |    |  (頻道通報)  |    |  Daemon 監控     |  |
|  +--------------+    +--------------+    +------------------+  |
+-----------------------------------------------------------------+
```

### 2.2 元件說明

| 元件 | 負責職責 | 實作位置 |
|------|----------|----------|
| Diagnoser | 執行純 Rust 健康檢查、環境變數驗證、CLI 版本檢查、進程掃描 | 新增 `crates/buzz-agent/src/health_diagnoser.rs` |
| Analyzer | 掃描失敗日誌、解析錯誤堆疊、檢測熔斷狀態 (讀取 `WORK_LOGS/circuit_breaker.json`) | 新增 `crates/buzz-agent/src/health_analyzer.rs` |
| Remediator | 熔斷重置 (寫入 `circuit_breaker.json`)、常駐行程重啟 (`std::process::Command`) | 新增 `crates/buzz-agent/src/health_remediator.rs` |
| Reporter | 產出結構化健全性報告 (Markdown + JSON) | 新增 `crates/buzz-agent/src/health_reporter.rs` |
| Notifier | 發布報告/告警至 Buzz 頻道 (Nostr Kind:1) | 復用 `builtin::call_sign_and_send_event` |
| Daemon Monitor | 週期性排程執行、異常即時告警 | 掛載於 `daemon.rs` |

---

## 3. 詳細實作規劃

### 3.1 新增檔案清單

```
crates/buzz-agent/src/
+-- health_diagnoser.rs      # CLI/環境/變數診斷
+-- health_analyzer.rs       # 日誌/熔斷/堆疊分析
+-- health_remediator.rs     # 熔斷重置/行程重啟
+-- health_reporter.rs       # 報告生成
+-- health_guardian.rs       # 主協調器
+-- daemon.rs                # (修改) 掛載排程任務
```

### 3.2 核心資料結構

```rust
// health_diagnoser.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub cli_version: String,
    pub env_vars: EnvVarStatus,
    pub background_processes: Vec<ProcessStatus>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarStatus {
    pub buzz_relay_url: VarCheck,
    pub buzz_private_key: VarCheck,
    pub buzz_auth_tag: VarCheck,
    pub openrouter_key: VarCheck,
    pub github_token: VarCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarCheck {
    pub name: String,
    pub present: bool,
    pub valid_format: bool,
    pub masked_value: Option<String>,
}

// health_analyzer.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureLogEntry {
    pub timestamp: String,
    pub component: String,
    pub error_type: String,
    pub stack_trace: Option<String>,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    pub tripped: bool,
    pub tripped_at: Option<String>,
    pub reason: Option<String>,
    pub failure_count: u32,
    pub last_failure: Option<String>,
}

// health_remediator.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationAction {
    pub action_type: RemediationType,
    pub target: String,
    pub reason: String,
    pub success: bool,
    pub output: Option<String>,
}

// health_reporter.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub check_id: String,
    pub timestamp: String,
    pub overall_status: OverallStatus,
    pub checks: HealthCheckResult,
    pub failures: Vec<FailureLogEntry>,
    pub circuit_breakers: Vec<CircuitBreakerStatus>,
    pub remediations: Vec<RemediationAction>,
    pub recommendations: Vec<String>,
}
```

### 3.3 檢查項目清單 (Checklist)

| 類別 | 檢查項目 | 正常標準 | 異常處理 |
|------|----------|----------|----------|
| CLI | buzz --version | 可執行、版本匹配 | 記錄錯誤、標記 Critical |
| 環境變數 | BUZZ_RELAY_URL | 非空、有效 URL 格式 | 記錄缺失、標記 Critical |
| 環境變數 | BUZZ_PRIVATE_KEY | 非空、十六進位 64 字元 | 記錄缺失、標記 Critical |
| 環境變數 | BUZZ_AUTH_TAG | 非空 | 記錄缺失、標記 Warning |
| 環境變數 | OPENROUTER_API_KEY | 非空、sk- 開頭 | 記錄缺失、標記 Warning |
| 背景行程 | openrouter-gateway | 進程存活、端口監聽 | 嘗試重啟、標記 Critical |
| 背景行程 | sync_obsidian | 進程存活、最近同步 < 10min | 嘗試重啟、標記 Warning |
| 背景行程 | buzz-proxy | 進程存活、Relay 連線正常 | 嘗試重啟、標記 Critical |
| 日誌 | WORK_LOGS/*.log | 無 ERROR/CRITICAL 近 1h | 解析堆疊、關聯熔斷 |
| 熔斷器 | circuit_breaker.log | tripped=false | tripped=true 觸發 Remediator |

---

## 4. 部署與配置

### 4.1 環境變數 (新增)

```bash
# .env.example 追加
# System Health Guardian
BUZZ_HEALTH_GUARDIAN_ENABLED=true
BUZZ_HEALTH_GUARDIAN_CHANNEL=5ca019f6-1a27-592a-b015-4baf0eb33572
BUZZ_HEALTH_GUARDIAN_SCHEDULE="*/15 * * * *"     # 每 15 分鐘
BUZZ_HEALTH_GUARDIAN_DAILY_DEEP="0 4 * * *"      # 每日 04:00 深度掃描
BUZZ_WORK_LOGS_PATH=.buzz/WORK_LOGS
BUZZ_CIRCUIT_BREAKER_PATH=.buzz/WORK_LOGS/circuit_breaker.json
BUZZ_OPENROUTER_PORT=8080
BUZZ_SYNC_PID_FILE=.buzz/sync_obsidian.pid
BUZZ_PROXY_PORT=3000
```

### 4.2 Config 結構擴充

```rust
// crates/buzz-agent/src/config.rs
pub struct HealthGuardianConfig {
    pub enabled: bool,
    pub channel_id: String,
    pub schedule: String,
    pub daily_deep_schedule: String,
    pub work_logs_path: PathBuf,
    pub circuit_breaker_path: PathBuf,
    pub openrouter_port: u16,
    pub sync_pid_file: PathBuf,
    pub proxy_port: u16,
    pub alert_threshold: AlertThreshold,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertThreshold {
    pub max_consecutive_failures: u32,
    pub circuit_breaker_auto_reset: bool,
    pub process_restart_cooldown_secs: u64,
}

impl Default for HealthGuardianConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            channel_id: String::new(),
            schedule: "*/15 * * * *".into(),
            daily_deep_schedule: "0 4 * * *".into(),
            work_logs_path: PathBuf::from(".buzz/WORK_LOGS"),
            circuit_breaker_path: PathBuf::from(".buzz/WORK_LOGS/circuit_breaker.json"),
            openrouter_port: 8080,
            sync_pid_file: PathBuf::from(".buzz/sync_obsidian.pid"),
            proxy_port: 3000,
            alert_threshold: AlertThreshold {
                max_consecutive_failures: 3,
                circuit_breaker_auto_reset: true,
                process_restart_cooldown_secs: 300,
            },
        }
    }
}
```

---

## 5. 測試策略

### 5.1 單元測試

| 模組 | 測試重點 |
|------|----------|
| health_diagnoser | CLI 執行、環境變數解析、進程狀態檢測 |
| health_analyzer | 日誌解析、錯誤分類、熔斷狀態讀取 |
| health_remediator | 熔斷重置邏輯、VBS 行程重啟、冷卻期機制 |
| health_reporter | 報告結構、Markdown 渲染、JSON 序列化 |
| health_guardian | 端到端流程 Mock |

### 5.2 整合測試

```bash
# 手動觸發單次健康檢查
cargo run -p buzz-agent -- --health-check-run-once

# 手動觸發深度掃描
cargo run -p buzz-agent -- --health-check-deep-scan
```

**驗收標準**：
- 純 Rust 健康檢查成功執行並輸出結構化 JSON (無外部腳本依賴)
- 3 大背景行程狀態正確檢測 (端口掃描 + PID + 進程名驗證)
- 失敗日誌正確解析並分級
- 熔斷狀態正確讀取/寫入 (`WORK_LOGS/circuit_breaker.json`)，自動重置驗證
- 報告發布至頻道，格式可讀
- 告警機制：連續 3 次失敗觸發 @mention 通知 (Nostr Kind:1)

---

## 6. 失敗模式推演

| # | 失敗場景 | 影響 | 偵測點 | 具體對策 |
|---|----------|------|--------|----------|
| F1 | `sysinfo` crate 無法獲取進程列表 | 進程檢測失效 | `sysinfo::System::new_all()` 失敗 | 記錄 Critical、降級至 `std::process::Command` 執行 `tasklist`/`ps` 解析 |
| F2 | WORK_LOGS 目錄不存在/權限不足 | 日誌分析失效 | `std::fs::read_dir` 失敗 | 記錄 Warning、跳過日誌分析、僅執行 CLI/環境/行程檢查 |
| F3 | `std::process::Command` 重啟行程失敗 | 行程重啟失效 | 子進程非 0 退出碼 | 記錄錯誤、標記「需人工介入」、發送 @mention 至 @Sentry (Kind:1) |
| F4 | 熔斷器狀態檔案損壞/遺失 | 無法判斷熔斷狀態 | 檔案讀取/解析失敗 | 預設視為未熔斷 (tripped=false)、記錄 Warning、備份損壞檔案為 `.corrupt` |
| F5 | 背景行程 PID 檔案不一致 | 狀態誤判 | PID 檔案 vs 實際進程 | 以實際進程掃描為準 (`sysinfo` + 端口驗證)、忽略 PID 檔案 |
| F6 | Relay 斷線導致無法發報 | 告警無法送達 | `sign_and_send_event` 失敗 | 本地暫存佇列 (`.buzz/health_guardian_outbox/`)、背景重試、恢復後補發 |

---

## 7. 分期交付計畫 (MVP 敏捷引導)

### Phase A：CLI 驗證期 (第 1 週，~10h)
**目標**：驗證核心診斷管線 (CLI -> 分析 -> 報告)，**不接入 Daemon/排程/自動重啟**

| 交付物 | 內容 | 驗收 |
|--------|------|------|
| health_diagnoser.rs | CLI/環境/行程檢查 (純 Rust) | cargo test 通過；--dry-run 輸出 JSON |
| health_analyzer.rs | 日誌/熔斷分析 | Mock 日誌正確解析分級 |
| health_reporter.rs | 報告生成 | 產出可讀 Markdown + JSON |
| health_guardian.rs (CLI) | 串聯管線、手動觸發 | --run-once 產出本地報告 |

### Phase B：自動化與自癒期 (第 2 週，~8h)
**目標**：掛載排程、熔斷自動重置、行程自動重啟、告警機制

| 交付物 | 內容 | 驗收 |
|--------|------|------|
| health_remediator.rs | 熔斷重置、行程重啟 (std::process::Command) | 熔斷觸發 -> 自動重置驗證 |
| daemon.rs 掛載 | 讀取 config、雙排程 | 15min 定期 + 每日深度準時執行 |
| 告警機制 | 連續失敗 @mention (Nostr Kind:1) | 連續 3 次失敗觸發通知 |
| 文檔 | SKILL.md 更新、部署指南 | 新成員 10 分鐘部署完成 |

---

## 8. 里程碑與工時總表

| 階段 | 交付物 | 預估工時 | 驗收標準 |
|------|--------|----------|----------|
| M1 | health_diagnoser.rs | 3h | 單測全綠、真實環境可跑通 |
| M2 | health_analyzer.rs | 3h | 日誌解析、熔斷讀取正確 |
| M3 | health_reporter.rs | 2h | 報告格式、Markdown 可讀 |
| M4 | health_guardian.rs (Phase A) | 2h | --run-once 端到端產出本地報告 |
| M5 | health_remediator.rs (Phase B) | 3h | 熔斷重置、行程重啟驗證 |
| M6 | Daemon 掛載與雙排程 | 2h | 定時自動執行、告警生效 |
| M7 | 文檔與部署驗證 | 1h | 部署指南通過驗證 |
| 總計 | | ~16h | |

---

## 9. 代碼出處溯源

| 計畫引用 | 真實源碼路徑 | 行號區間 |
|----------|--------------|----------|
| Daemon 結構與生命週期 | crates/buzz-agent/src/daemon.rs | 12-78 |
| Config 結構與環境讀取 | crates/buzz-agent/src/config.rs | 426-800 |
| builtin::call_sign_and_send_event | crates/buzz-agent/src/builtin.rs | 251-380 |
| sysinfo crate (進程掃描) | 外部依賴 `sysinfo` | Cargo.toml 新增 |
| std::process::Command (行程重啟) | Rust 標準庫 | N/A |

---

## 10. 風險與緩解

| 風險 | 機率 | 影響 | 緩解策略 |
|------|------|------|----------|
| 自動重啟導致衝突/資料遺失 | 低 | 高 | 冷卻期機制 (5min)、單次重啟限制、需人工確認 Critical 級 |
| 熔斷誤判自動重置 | 中 | 中 | 僅在 failure_count 歸零且無新錯誤 10min 後允許重置 |
| sysinfo 跨平台相容性 | 低 | 中 | Windows/Linux/Mac 均支援、CI 矩陣測試 |
| 告警風暴 | 中 | 中 | 連續失敗閾值 (3)、抑制窗口 (30min)、分級告警 |

---

## 11. 下一步行動

1. **@Sentry 請進行二輪複審** (本計畫書已含失敗模式、分期交付、代碼溯源)
2. 審核通過後，主理人下達「同意立項 / 開工 / 實作」指令
3. @Pollen 依 Phase A 先行實作 CLI 驗證管線
4. Phase A 驗收後進入 Phase B 自動化與自癒
5. @Sentry 複審代碼品質與測試覆蓋
6. @Fizz 結案回報主理人

---

**計畫書狀態**：完整版 (含失敗模式、分期交付、代碼溯源、風險緩解)  
**編寫人**：Wax (記憶清潔工與歸檔官)  
**審核人**：@Sentry  
**批准人**：可愛小寶寶 (主理人)
