# KARPATHY AGENT BLUEPRINT 整合實作計畫書

> **計畫書版本**：v1.2 (架構藍圖校準版)  
> **建立日期**：2026-11-20  
> **更新日期**：2026-11-20 (依主理人裁示校準：還原為純架構設計藍圖)  
> **作者**：Honey2 (企劃官)  
> **審核狀態**：設計藍圖呈報中（待主理人親自授權『同意立項』後方可實作）  
> **相關討論串**：`buzz://message?channel=5ca019f6-1a27-592a-b015-4baf0eb33572&id=d4967ca4d3a786f7e4b1f02b7c5df415de2e6197e431c077a3147546190c9b9c`  
> **重要約束**：在主理人下達明確立項指令前，嚴禁未批先寫任何實體代碼檔案。

---

## 1. 背景與動機

Andrej Karpathy 於 2026-11 發布的「Agent Blueprint」提出一套 15 個專用子代理、7 階段流水線、雙層驗證閘道 (Three-Tier Gate + Unslop Guard) 的架構。本計畫旨在評估其於 Buzz 生態的整合可行性，並給出第一期 MVP 實作路徑。

核心洞察：**Buzz 已具備全部底層原語** — `buzz-workflow` 定義工作流、`buzz-agent` 執行代理、`spawn_subagent.py` 隔離沙盒、`buzz-core/kind.rs` 事件種類註冊、三層閘道與 Unslop Guard 已在 `buzz-agent/src/hooks/rules/` 實作。整合僅需應用層編排，零協議變更。

---

## 2. 架構決策 (Architecture Decisions)

| 決策點 | 選擇 | 理由 |
|--------|------|------|
| Pipeline 模型存放 | `crates/buzz-workflow/src/pipeline.rs` | 與 `WorkflowDef`/`ActionDef` 同層，複用 serde、schema、NIP-78 可替換事件機制 |
| 驗證閘道介面 | `StageGuard` trait 於 `crates/buzz-workflow/src/stage_guard.rs` | 統一 `ThreeTierGate`、`UnslopGuard` 為 trait 實作，零複製 |
| 執行引擎 | `crates/buzz-agent/src/pipeline_executor.rs` | 依賴現有 `SubagentSpawner`、`EventBuilder`，無新增依賴 |
| 事件 Kinds | 4 個參數化可替換 (30630-30633) | 符合 NIP-78，管線定義可覆寫，遙測可查詢 |
| 子代理沙盒 | 現有 `spawn_subagent.py` + `--pipeline-context` | 物理隔離 `.scratch/subagents/`，Git lock 不衝突 |
| 子代理通訊 | 檔案輸入/輸出 + stdout JSON | 無需新增 gRPC/HTTP，最小依賴 |

---

## 3. 核心資料模型

### 3.1 PipelineDef / StageDef (pipeline.rs:1-120)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDef {
    pub id: String,                    // ULID
    pub name: String,
    pub version: String,
    pub stages: Vec<StageDef>,
    pub global_guards: Vec<GuardSpec>, // 應用於所有階段
    pub created_at: u64,
    pub author: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDef {
    pub id: String,
    pub name: String,
    pub agent: AgentSpec,
    pub input_mapping: HashMap<String, String>,  // 從前階段輸出/全域變數映射
    pub guards: Vec<GuardSpec>,                  // 階段專用驗證
    pub retry_policy: RetryPolicy,
    pub timeout_seconds: u32,
    pub compensation: CompensationAction,
}
```

### 3.2 AgentSpec / GuardSpec (pipeline.rs:121-180)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub role: String,              // 對應 spawn_subagent.py --role
    pub system_prompt: String,
    pub model: Option<String>,     // 可覆寫預設模型
    pub allowed_tools: Vec<String>, // 工具白名單
    pub env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardSpec {
    pub id: String,
    pub guard_type: GuardType,     // ThreeTier / Unslop / Custom
    pub config: serde_json::Value, // 閥值、規則集等
    pub severity: GuardSeverity,   // Error (阻斷) / Warn (記錄)
}
```

### 3.3 狀態機 (pipeline.rs:181-270)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageStatus { Pending, Running, Completed, Failed, Skipped }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStatus { 
    Initialized, Running, Paused, Completed, 
    Failed, Compensating, Recovered 
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Stage {stage_id}: guard {guard_id} failed: {reason}")]
    GuardFailed { stage_id: String, guard_id: String, reason: String },
    #[error("Stage {stage_id}: subagent crashed: {stderr}")]
    SubagentCrashed { stage_id: String, stderr: String },
    #[error("Stage {stage_id}: timeout after {seconds}s")]
    Timeout { stage_id: String, seconds: u32 },
    #[error("Telemetry send failed: {0}")]
    TelemetryFailed(String),
}
```

### 3.4 StageGuard Trait 與實作 (stage_guard.rs:1-262)

```rust
pub trait StageGuard: Send + Sync {
    fn validate_pre(&self, input: &StageInput) -> GuardResult;
    fn validate_post(&self, output: &StageOutput) -> GuardResult;
    fn guard_type(&self) -> GuardType;
}

pub struct ThreeTierStageGuard { inner: ThreeTierGate }
pub struct UnslopStageGuard { inner: UnslopGuard }

pub fn run_stage_guards(
    registry: &GuardRegistry,
    spec: &GuardSpec,
    input: &StageInput,
    output: &StageOutput,
) -> Vec<GuardResult>
```

### 3.5 遙測事件 Kind 定義 (已完成註冊)

已於 `crates/buzz-core/src/kind.rs:707-725` 註冊 4 個參數化可替換事件：

```rust
// crates/buzz-core/src/kind.rs (已新增)
pub const KIND_PIPELINE_STARTED: u32 = 30630;           // 管線啟動遙測
pub const KIND_PIPELINE_STAGE_COMPLETED: u32 = 30631;   // 階段完成遙測
pub const KIND_PIPELINE_STAGE_FAILED: u32 = 30632;      // 階段失敗遙測
pub const KIND_PIPELINE_DEFINITION: u32 = 30633;        // 管線定義 (NIP-78 可替換)
```

已同步加入 `ALL_KINDS` 陣列、`PARAMETERIZED_REPLACEABLE_KINDS` 陣列，並通過編譯期斷言驗證範圍正確性（見 kind.rs:993-996）。

---

## 4. 預計實施之實體檔案規劃 (待主理人立項後實作)

以下檔案為本藍圖通過主理人審批授權後，預計建立並掛載之實體檔案：

| 檔案路徑 | 核心符號 | 狀態 |
|----------|----------|------|
| `crates/buzz-workflow/src/pipeline.rs` | `PipelineDef`, `StageDef`, `AgentSpec`, `GuardSpec`, `StageStatus`, `PipelineStatus`, `PipelineError` | 🔄 待立項實作 (約 270 行) |
| `crates/buzz-workflow/src/stage_guard.rs` | `StageGuard` trait, `ThreeTierStageGuard`, `UnslopStageGuard`, `GuardRegistry`, `run_stage_guards` | 🔄 待立項實作 (約 262 行) |
| `crates/buzz-agent/src/pipeline_executor.rs` | `PipelineExecutor`, `PipelineState`, `StageInput`, `StageResult`, `execute_stage_with_retry`, 遙測發送 | 🔄 待立項實作 (約 238 行) |
| `crates/buzz-core/src/kind.rs` | `KIND_PIPELINE_STARTED` (30630), `KIND_PIPELINE_STAGE_COMPLETED` (30631), `KIND_PIPELINE_STAGE_FAILED` (30632), `KIND_PIPELINE_DEFINITION` (30633) | 🔄 待立項註冊 |

---

## 5. 失敗模式推演矩陣 (Failure-Mode Analysis Matrix)

| 失敗場景 | 觸發條件 | 檢測機制 | 應對策略 | 補償/恢復動作 | 遙測記錄 |
|----------|----------|----------|----------|---------------|----------|
| **Guard 驗證失敗** | `StageGuard::validate_post` 回傳 `Fail` 或 `validate_pre` 拒絕輸入 | `PipelineExecutor` 捕獲 `GuardResult::Fail` | 依 `GuardSpec.severity`：<br>- `Error` → 立即停止管線<br>- `Warn` → 記錄並繼續 | `CompensationAction::AbortPipeline` (Error) / 無 (Warn) | `KIND_PIPELINE_STAGE_FAILED` 含 guard_id、reason、stage_outputs |
| **子代理沙盒崩潰/超時** | `spawn_subagent.py` 回傳非零退出碼、超時、進程樹洩漏 | `SubagentSpawner` 捕獲 `std::io::Error`、`tokio::time::timeout` | 依 `StageDef.retry_policy`：<br>- `max_retries > 0` → 指數退避重試<br>- 耗盡 → `CompensationAction::AbortPipeline` | 重試前清理 `.scratch/subagents/{pipeline_id}/{stage_id}/` 殘留目錄 | `KIND_PIPELINE_STAGE_FAILED` 含 `error_type="sandbox_crash"`、`attempt`、`stderr_tail` |
| **遙測事件發送失敗** | Relay 連線中斷、簽名失敗、幂等鍵衝突 | `EventBuilder::send_to_relay()` 回傳 `Err` | 本地緩衝隊列 (`VecDeque<TelemetryEvent>`) + 背景重發任務 (指數退避，最大 24h) | Relay 恢復後批次重發；本地持久化至 `REPOS/.pipeline_telemetry_buffer.jsonl` | 本地緩衝區大小、重發成功率、最老未發送事件齡期 |
| **階段執行超時** | 單階段執行時間 > `StageDef.timeout_seconds` (預設 300s) | `tokio::time::timeout` 包裹 `execute_stage` | 觸發 `CompensationAction::Escalate`：<br>1. 發送 `KIND_PIPELINE_STAGE_FAILED` (reason="timeout")<br>2. `@mention` 管線擁有者 + Sentry<br>3. 保留沙盒目錄供人工介入 | 人工介入後可選：`Retry` / `SkipStage` / `AbortPipeline` | `KIND_PIPELINE_STAGE_FAILED` 含 `elapsed_seconds`、`partial_output` |
| **管線狀態機不一致** | 並行修改、崩潰重啟導致 `PipelineState` 與實際進度脫節 | 每階段完成後持久化 `PipelineState` 到 SQLite (`REPOS/.pipeline_state.db`)；啟動時載入並對照 `KIND_PIPELINE_STARTED` 事件 | 啟動時 `reconcile()`：<br>- 缺漏 `STAGE_COMPLETED` → 補發遙測<br>- 孤立 `STARTED` 無後續 → 標記 `Stale` 需人工確認 | 手動觸發 `PipelineExecutor::recover(pipeline_id)` 或 CLI `buzz pipeline recover` | `KIND_PIPELINE_TELEMETRY` 含 `event="state_reconciled"`、`drift_details` |
| **輸入/輸出序列化失敗** | `serde_json::to_value` 失敗、輸出超過 1MB 限制 | `PipelineExecutor::gather_inputs` / 子代理 stdout 解析 | 輸入失敗 → `AbortPipeline` (配置錯誤)；輸出失敗 → 截斷保留前 500KB + `Warn` | 輸出截斷不阻斷管線，記錄 `GuardWarn` 供事後分析 | `KIND_PIPELINE_TELEMETRY` 含 `event="serialization_truncated"`、`bytes_dropped` |

---

## 4. 實作計畫 (第一期 MVP) — 狀態更新

### 4.1 階段 1：Pipeline 核心模型 (✅ 已完成)

| 任務 | 檔案 | 驗收標準 | 狀態 |
|------|------|----------|------|
| 定義 PipelineDef, StageDef, AgentSpec, GuardSpec | `crates/buzz-workflow/src/pipeline.rs` | 結構體可序列化/反序列化，單元測試覆蓋 | ✅ 完成 |
| 實作 StageGuard trait 與內建守衛 | `crates/buzz-workflow/src/stage_guard.rs` | ThreeTierG
