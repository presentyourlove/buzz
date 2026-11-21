# DESIGN_SPEC_B_TRIPLE_GATE.md

## 三重驗收閘門機制 — 設計規格書 v1.0

> **計畫代號**：B
> **所屬計畫**：多代理增強計畫（Queued 待命佇列）
> **版本**：v1.0
> **作者**：Honey2
> **建立日期**：2026-11-19
> **審核狀態**：待 Sentry 審核

---

## 1. 設計目標與範圍

### 1.1 核心目標
建立**三層級、可審計、可重放**的驗收閘門機制，整合升級現有 Sentry 審查流程，確保所有交付物經過：
- **Gate 1**：驗收條件先行驗證（契約測試、Schema 校驗、靜態分析）
- **Gate 2**：獨立 AI 盲審（多模型交叉驗證、對抗性測試）
- **Gate 3**：指紋比對 + 人工覆核（二進制可重現建構、簽名審計鏈）

### 1.2 非目標
- ❌ 替代人類架構師的決策權（Gate 3 保留人工覆核權）
- ❌ 處理非代碼類交付物（文檔、設計圖另行流程）
- ❌ 強制所有專案啟用（可配置啟用/停用、逐倉庫選擇）

### 1.3 設計原則
| 原則 | 說明 |
|------|------|
| **Fail-Fast** | Gate 1 在本地即可跑通，攔截低級錯誤不進 CI |
| **Deterministic** | 同一輸入必產出同一審查結果，支援重放審計 |
| **Auditable** | 每層閘門產出簽名審計日誌（Kind:30303），不可竄改 |
| **Composable** | 每層獨立可插拔，支援自定義檢查器 Plugin |
| **Zero-Trust AI** | Gate 2 多模型盲審，單一模型誤判不通過 |

---

## 2. 架構總覽

```
┌────────────────────────────────────────────────────────────────────────────┐
│                         三重驗收閘門管線                                      │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌────────────┐  │
│  │  提交觸發    │───▶│  Gate 1     │───▶│  Gate 2     │───▶│  Gate 3    │  │
│  │  (PR/Commit) │    │  契約先行   │    │  AI 盲審    │    │  指紋+人工  │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └────────────┘  │
│         │                  │                  │                  │         │
│         ▼                  ▼                  ▼                  ▼         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    審計日誌匯流                                          │   │
│  │  Kind:30303 (GateAuditLog)  ◄── 每層產出簽名事件，含：判定、證據、指紋   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 事件流與狀態機

```
提交 (PR Open / Push) 
    │
    ▼
┌─────────────────────────────────────┐
│ STATE: GATE_1_PENDING               │
│ - 觸發：GitHub Actions / 本地 pre-push  │
│ - 輸入：diff、鎖文件、Schema、OpenAPI    │
│ - 輸出：Gate1Result (PASS/FAIL) + 審計  │
└─────────────────────────────────────┘
    │ PASS
    ▼
┌─────────────────────────────────────┐
│ STATE: GATE_2_PENDING               │
│ - 觸發：Gate 1 PASS 事件             │
│ - 輸入：完整代碼庫快照、Gate1 證據     │
│ - 執行：並行調用 3+ 模型（盲審模式）    │
│ - 輸出：Gate2Result (PASS/FAIL) + 共識  │
└─────────────────────────────────────┘
    │ PASS (共識 ≥ 2/3)
    ▼
┌─────────────────────────────────────┐
│ STATE: GATE_3_PENDING               │
│ - 觸發：Gate 2 PASS 事件             │
│ - 執行：可重現建構 → 產出構建指紋      │
│ - 比對：指紋 vs 簽名記錄             │
│ - 人工：Sentry 覆核簽名              │
│ - 輸出：Gate3Result (PASS/FAIL) + 審計  │
└─────────────────────────────────────┘
    │ PASS
    ▼
STATE: APPROVED → 可合併 / 發布
```

---

## 3. 核心資料結構

### 3.1 閘門判定結果
```rust
// crates/buzz-core/src/gate.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateVerdict {
    Pass,
    Fail { reasons: Vec<GateFailureReason> },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateFailureReason {
    pub checker_id: String,           // 如 "cargo-clippy", "schema-validator"
    pub severity: Severity,           // Error | Warning | Info
    pub message: String,
    pub location: Option<CodeLocation>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: GateNumber,             // 1 | 2 | 3
    pub verdict: GateVerdict,
    pub evidence: GateEvidence,       // 可重放的證據包
    pub timestamp: DateTime<Utc>,
    pub reviewer_pubkey: PublicKey,   // 簽署者（AI 或人工）
    pub signature: String,            // Nostr 簽名
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEvidence {
    pub input_hash: String,           // 輸入代碼快照 SHA256
    pub checker_outputs: Vec<CheckerOutput>,
    pub consensus: Option<ConsensusResult>,  // Gate 2 專用
    pub build_fingerprint: Option<BuildFingerprint>, // Gate 3 專用
}
```

### 3.2 審計日誌事件（Kind:30303）
```json
{
  "kind": 30303,
  "tags": [
    ["d", "<gate_audit_id>"],           // 唯一審計 ID
    ["gate", "1|2|3"],                  // 閘門編號
    ["verdict", "pass|fail|error"],     // 判定
    ["commit", "<git_commit_sha>"],     // 關聯提交
    ["repo", "<owner>/<name>"],         // 倉庫
    ["evidence", "<base64_evidence>"],  // 證據包（壓縮）
    ["signature", "<nostr_sig>"]        // 審計員簽名
  ],
  "content": "Gate 1 PASS: 12 checkers passed, 0 failed"
}
```

---

## 4. Gate 1：驗收條件先行（契約測試優先）

### 4.1 設計哲學
> 「測試即規格，規格即契約」—— Gate 1 在**本地開發機**即可完整執行，零外部依賴。

### 4.2 檢查器清單

| ID | 檢查器 | 執行命令 | 門檻 | 失敗處理 |
|----|--------|----------|------|----------|
| G1-01 | Cargo 檢查 | `cargo check --workspace --all-targets` | 0 errors | 阻斷 |
| G1-02 | Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings | 阻斷 |
| G1-03 | Rustfmt | `cargo fmt --check --workspace` | 0 diffs | 阻斷 |
| G1-04 | 單元測試 | `cargo test --workspace --lib` | 100% pass | 阻斷 |
| G1-05 | 契約測試 | `cargo test --workspace --test contract` | 100% pass | 阻斷 |
| G1-06 | Schema 校驗 | `cargo run -p buzz-db -- migrate validate` | 0 drift | 阻斷 |
| G1-07 | OpenAPI 驗證 | `cargo run -p buzz-cli -- openapi validate` | valid | 阻斷 |
| G1-08 | 依賴審計 | `cargo audit` | 0 vulnerabilities | 阻斷 |
| G1-09 | 許可證合規 | `cargo deny check licenses` | 允許清單 | 阻斷 |
| G1-10 | 二進制大小 | `cargo bloat --release -n 20` | < 配置閾值 | 警告 |
| G1-11 | 循環複雜度 | `cargo geiger` | 無 unsafe | 阻斷 |
| G1-12 | 文檔覆蓋 | `cargo doc --document-private-items` | public API 100% | 警告 |

### 4.3 契約測試規範（Review-Proven Rule #3）

每個**生產代碼路徑**必須有對應的契約測試，綁定真實 seam：

```rust
// tests/contract/api_contract_test.rs
#[test]
fn contract_agent_status_endpoint_returns_valid_schema() {
    // 1. 啟動真實後端（或測試雙工模擬）
    let server = TestServer::spawn().await;
    
    // 2. 發送真實 HTTP 請求（非 mock handler）
    let resp = server.get("/api/v1/agents").await;
    
    // 3. 驗證 OpenAPI Schema 完全符合
    let schema = load_openapi_schema("agent_list_response");
    assert_json_matches_schema(resp.json(), &schema);
    
    // 4. 驗證語義契約：必須包含在線特工
    let agents: Vec<AgentDto> = resp.json();
    assert!(!agents.is_empty(), "至少應有一個系統特工在線");
}
```

**失敗即阻斷**：移除任何守門檢查器，對應測試必須失敗（守護有效性）。

---

## 5. Gate 2：獨立 AI 盲審

### 5.1 設計哲學
> 多模型交叉驗證，消除單一模型盲區與幻覺。盲審 = 模型**不知彼此存在**，也不知最終決策邏輯。

### 5.2 模型陣容配置

| 角色 | 模型 | 提供商 | 參數 | 權重 |
|------|------|--------|------|------|
| Architect | Claude Opus 4 | Anthropic | temp=0.1 | 1.0 |
| Security | GPT-4o | OpenAI | temp=0.1 | 1.0 |
| Performance | Gemini 1.5 Pro | Google | temp=0.1 | 1.0 |
| **備選** | DeepSeek V3 | DeepSeek | temp=0.1 | 0.5 |

### 5.3 盲審協議

```
輸入：完整代碼差異 + Gate 1 證據包 + 架構文檔片段

步驟 1：構建統一提示詞（Prompt Template）
  - 角色定義（架構師/安全/效能）
  - 評分維度（正確性、安全、效能、可維護性、測試覆蓋）
  - 輸出格式：結構化 JSON（含 verdict、score、findings、evidence_refs）

步驟 2：並行調用 3+ 模型（獨立會話，無共享上下文）
  - 每模型收到相同輸入
  - 無模型知曉其他模型參與

步驟 3：共識聚合
  - verdict 一致性：≥ 2/3 通過 → PASS
  - score 平均 ≥ 7.5/10 → PASS
  - 任一模型標記 CRITICAL security → 自動 FAIL

步驟 4：產出共識報告 + 少數派意見保留
```

### 5.4 提示詞模板（版本化管理）

```markdown
# prompts/gate2_architect_v1.md

## 角色
你是資深軟體架構師，專精 Rust、分散式系統、Nostr 協議。

## 輸入
- Git diff: {{DIFF}}
- Gate 1 證據: {{GATE1_EVIDENCE}}
- 相關架構文檔: {{ARCH_DOCS}}

## 評分維度（1-10 分）
1. 架構正確性：設計是否推進 VISION.md 方向，無反模式
2. 介面契約：API/事件幂是否穩定、向後相容
3. 錯誤處理：是否遵循「每個捕獲失敗必留重試記錄或傳播」
4. 資源邊界：是否有無界循環、記憶體、檔案描述符
5. 測試綁定：回歸測試是否綁定生產 seam、可被破壞驗證

## 輸出格式（嚴格 JSON）
{
  "verdict": "pass|fail",
  "scores": {"architecture": 8, "contracts": 9, "errors": 7, "resources": 8, "tests": 8},
  "findings": [
    {"severity": "major|minor|info", "file": "path.rs", "line": 42, "message": "...", "suggestion": "..."}
  ],
  "evidence_refs": ["G1-E-001", "G1-E-003"]
}
```

### 5.5 共識演算法

```rust
fn aggregate_consensus(results: Vec<ModelResult>) -> ConsensusResult {
    let pass_count = results.iter().filter(|r| r.verdict == Verdict::Pass).count();
    let total = results.len();
    
    // 硬性規則：任一 CRITICAL security → FAIL
    if results.iter().any(|r| r.has_critical_
