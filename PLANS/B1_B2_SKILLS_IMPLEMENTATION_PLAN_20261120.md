---
title: "B1 B2 及四大技能模組綜合實作計畫書"
tags: [b1-communication-quality, b2-daily-evolution, skill-implementation, sentry-review]
status: completed
created: 2026-11-20
author: "Honey2 (專案架構企劃官)"
target_channel: "5ca019f6-1a27-592a-b015-4baf0eb33572"
reviewer: "Sentry (安全與架構審核官)"
---

# B1 B2 及四大技能模組綜合實作計畫書

> **背景**：主理人（可愛小寶寶）於 `general` 頻道 Thread `fd35ae48...` 指派，依據先前研報 `DAILY_EVOLUTION_PROPOSAL_20261120.md` 所提之 B1、B2 目標與四大候選技能，草擬完整實作計畫書供 Sentry 審核。

---

## 1. 願景與目標對照表

| 代號 | 願景目標 | 核心技能模組 | 實作狀態 |
|------|----------|--------------|----------|
| **B1** | 通訊品質衛士 | `comm-quality-guard` | 待立項 |
| **B2** | Daily Evolution 閉環機制 | `proactive-trigger-orchestrator` + `quant-portfolio-risk-sentinel` + `zero-defect-release-gate` | 待立項 |
| **S1** | 通訊品質衛士技能 | `comm-quality-guard` | 候選中 |
| **S2** | 主動式觸發編排技能 | `proactive-trigger-orchestrator` | 已有 PLAN.md / SKILL.md |
| **S3** | 量化風險哨兵技能 | `quant-portfolio-risk-sentinel` | 待補充 |
| **S4** | 零缺陷發布閘門技能 | `zero-defect-release-gate` | 待補充 |

---

## 2. 整體架構藍圖

```mermaid
graph TD
    %% B1 架構
    subgraph B1_Comm_Quality [B1: 通訊品質衛士]
        CQG[comm-quality-guard<br/>四層過濾管線]
        CQG --> P1[PunctuationCheck<br/>標點與格式檢查]
        CQG --> P2[TagWhitelist<br/>Tag 白名單驗證]
        CQG --> P3[HallucinationScan<br/>幻覺掃描]
        CQG --> P4[OutputTemplate<br/>輸出模板強制]
    end

    %% B2 架構
    subgraph B2_Daily_Evolution [B2: Daily Evolution 閉環]
        PTO[proactive-trigger-orchestrator<br/>事件總線與調度器]
        QRS[quant-portfolio-risk-sentinel<br/>量化健檢引擎]
        ZDRG[zero-defect-release-gate<br/>發布前合成檢查]
        
        PTO --> TC[TriggerCondition<br/>Cron / Event / Webhook]
        PTO --> TE[TriggerExecutor<br/>任務分發執行]
        PTO --> CB[CircuitBreaker<br/>熔斷保護]
        
        QRS --> QD[QuantDiagnoser<br/>量化指標診斷]
        QRS --> QA[QuantAnalyzer<br/>風險評分分析]
        
        ZDRG --> SC[SyntaxCheck<br/>語法編碼檢查]
        ZDRG --> TC2[TypeCheck<br/>型別檢查]
        ZDRG --> UT[UnitTest<br/>單元測試門檻]
        ZDRG --> IT[IntegrationTest<br/>整合測試門檻]
    end

    %% 數據流
    B1_Comm_Quality -.->|過濾後輸出| B2_Daily_Evolution
    B2_Daily_Evolution -.->|每日提案產出| B1_Comm_Quality
```

---

## 3. 各技能模組詳細實作規範

### 3.1 S1: `comm-quality-guard` (通訊品質衛士) — 實現 B1

#### 3.1.1 設計目標
解決 Agent 訊息輸出的四大痛點：
1. **標點符號錯誤/缺失**（全形/半形混用、句號遺漏）
2. **Tag 結構錯誤**（Nostr 標籤格式不符、幂等鍵誤用）
3. **技術幻覺斷言**（編造不存在的 API、路徑、函式簽名）
4. **輸出格式不一致**（缺乏統一模板、Markdown 渲染異常）

#### 3.1.2 四層過濾管線架構

```rust
// 位置：crates/buzz-agent/src/comm_quality_guard.rs (新建)

pub struct CommQualityGuard {
    punctuation_checker: PunctuationChecker,
    tag_validator: TagValidator,
    hallucination_scanner: HallucinationScanner,
    template_enforcer: TemplateEnforcer,
}

impl CommQualityGuard {
    pub fn filter(&self, content: &str, context: &AgentContext) -> Result<FilteredOutput, QualityError> {
        // Layer 1: 標點與格式正規化
        let normalized = self.punctuation_checker.normalize(content)?;
        
        // Layer 2: Tag 白名單驗證 (Nostr 事件標籤)
        let validated = self.tag_validator.validate(&normalized, context)?;
        
        // Layer 3: 幻覺掃描 (交叉驗證代碼庫符號)
        let scanned = self.hallucination_scanner.scan(&validated, context)?;
        
        // Layer 4: 輸出模板強制套用
        let final_output = self.template_enforcer.enforce(&scanned, context)?;
        
        Ok(final_output)
    }
}
```

#### 3.1.3 關鍵實作細節

| 層級 | 核心邏輯 | 依賴 |
|------|----------|------|
| **PunctuationChecker** | 1. 全形轉半形標點 (。→. 、→,)<br>2. 句尾強制加句號<br>3. Markdown 代碼塊保護 (不處理 ``` 內容) | `unicode-normalization` crate |
| **TagValidator** | 1. 對照 `buzz-core/src/kind.rs` 合法 kinds<br>2. 驗證 `h`/`e`/`p`/`d`/`a`/`t`/`r` 標籤格式<br>3. 拒絕幂等鍵 (`d` tag) 重複 | `buzz-core::kind`, `buzz-sdk` |
| **HallucinationScanner** | 1. 解析內容中的技術斷言 (路徑、函式、類型)<br>2. 交叉驗證 `crates/` 真實存在性<br>3. 標記未驗證斷言為 `[UNVERIFIED]` | `buzz-dev-mcp__read_file`, `rg` 搜尋 |
| **TemplateEnforcer** | 1. 依 Agent 角色套用對應模板<br>2. 強制反引號包裹路徑<br>3. 禁止角括號 `< >` 包裹變數 | 已定義模板庫 |

#### 3.1.4 整合點
- **Agent 發送訊息前**：在 `buzz-agent/src/agent.rs` 的 `send_message` 流程中插入 `CommQualityGuard::filter()`
- **CLI 輸出時**：`buzz-cli/src/commands/messages.rs` 同樣套用
- **技能調用時**：所有技能的輸出經過統一管線

---

### 3.2 S2: `proactive-trigger-orchestrator` (主動式觸發編排) — 實現 B2 核心引擎

#### 3.2.1 現有基礎設施 (已實作)
```
crates/buzz-agent/src/triggers/
├── orchestrator/
│   ├── api.rs          # HTTP API: /trigger, /trigger/{id}, /trigger/{id}/cancel
│   ├── circuit_breaker.rs  # 熔斷器模式實作
│   ├── routes.rs       # 路由註冊
│   └── state.rs        # 共享狀態
├── event_bus.rs        # 事件總線 (發布/訂閱)
├── scheduler.rs        # Cron 調度器 (支援 BuzzSchedule)
├── trigger_registry.rs # 觸發器註冊表
├── runtime.rs          # 執行時上下文
├── condition.rs        # 條件評估 (evalexpr)
└── types.rs            # 核心類型定義
```

#### 3.2.2 需補強項目 (B2 閉環所需)

| 缺口 | 實作內容 | 優先級 |
|------|----------|--------|
| **工作流持久化** | 將觸發器定義、執行歷史、狀態寫入 `buzz-db` (PostgreSQL) | P0 |
| **Daily Evolution 專用觸發器** | 內建 `daily_evolution_proposal` (每日 04:00)、`daily_evolution_review` (提案後 5 分鐘)、`daily_evolution_execute` (通過後立即) | P0 |
| **狀態機整合** | 對接 `collab-protocol` 7 步狀態機，自動推進階段 | P1 |
| **失敗重試與死信佇列** | 指數退避重試 (max 3)、失敗進 DLQ 供人工介入 | P1 |
| **可觀測性** | 執行指標暴露 (Prometheus)、結構化日誌、分散式追蹤 | P2 |

#### 3.2.3 Daily Evolution 觸發器定義

```yaml
# 位置：crates/buzz-agent/resources/triggers/daily_evolution.yaml
triggers:
  - id: daily_evolution_proposal
    name: "每日進化提案生成"
    schedule: "0 4 * * *"  # 每日 04:00 UTC
    timezone: "Asia/Taipei"
    condition: "true"
    action:
      type: "skill_invoke"
      skill: "daily-evolution"
      params:
        mode: "propose"
    retry:
      max_attempts: 3
      backoff: "exponential"
      base_delay_ms: 5000
  
  - id: daily_evolution_review_reminder
    name: "主理人審核提醒"
    event: "daily_evolution.proposal.created"
    delay_ms: 300000  # 5 分鐘後
    condition: "trigger.payload.status == 'pending_review'"
    action:
      type: "message_send"
      channel: "{trigger.payload.target_channel}"
      content: "@主理人 📋 每日進化提案已就緒，請審核：{trigger.payload.proposal_link}"
  
  - id: daily_evolution_execute
    name: "通過後自動立項執行"
    event: "daily_evolution.proposal.approved"
    condition: "trigger.payload.approved == true"
    action:
      type: "workflow_dispatch"
      workflow: "skill_implementation"
      params:
        skill_name: "{trigger.payload.skill_name}"
        plan_path: "{trigger.payload.plan_path}"
```

---

### 3.3 S3: `quant-portfolio-risk-sentinel` (量化風險哨兵) — 實現 B2 風險評估

#### 3.3.1 設計目標
- 定時掃描專案風險指標 (技術債、測試覆蓋率、依賴漏洞、構建健康度)
- 量化評分並觸發預警
- 整合既有 `health_guardian.rs` 診斷管線

#### 3.3.2 架構設計

```rust
// 位置：crates/buzz-agent/src/quant_risk_sentinel.rs (新建)

pub struct QuantRiskSentinel {
    diagnoser: QuantDiagnoser,
    analyzer: QuantAnalyzer,
    notifier: RiskNotifier,
    db: Arc<QuantRiskStore>,  // buzz-db 持久化
}

pub struct QuantDiagnoser {
    // 掃描器集合
    tech_debt_scanner: TechDebtScanner,
    test_coverage_scanner: TestCoverageScanner,
    dep_vuln_scanner: DependencyVulnScanner,
    build_health_scanner: BuildHealthScanner,
}

impl QuantRiskSentinel {
    pub async fn run_assessment(&self, project_id: &str) -> Result<RiskReport, RiskError> {
        let metrics = self.diagnoser.collect_all(project_id).await?;
        let scores = self.analyzer.score(&metrics)?;
        let report = RiskReport { project_id, metrics, scores, timestamp: Utc::now() };
        
        // 持久化
        self.db.save(&report).await?;
        
        // 閾值觸發預警
        if scores.overall > 70 || scores.any_critical() {
            self.notifier.alert(&report).await?;
        }
        
        Ok(report)
    }
}
```

#### 3.3.3 量化指標體系

| 維度 | 指標 | 計算方式 | 閾值 (警示/危險) |
|------|------|----------|------------------|
| **技術債** | TODO/FIXME 密度 | `grep -r "TODO\|FIXME" --include="*.rs" | wc -l` / KLOC | >50 / >100 per KLOC |
| **測試覆蓋** | 行覆蓋率 | `cargo llvm-cov --lcov` 解析 | <60% / <40% |
| **依賴漏洞** | CVE 數量 | `cargo audit` 輸出解析 | >0 / >5 |
| **構建健康** | 近 7 日構建失敗率 | CI 歷史統計 | >20% / >50% |
| **代碼品質** | Clippy 警告數 | `cargo clippy -- -D warnings` | >20 / >50 |

#### 3.3.4 排程整合
- 掛載至 `proactive-trigger-orchestrator` 的 Cron：`0 2 * * *` (每日 02:00)
- 手動觸發端點：`POST /risk/assess/{project_id}`


### 增量補完內容 (2026-11-21)

#### 3.4 S4: zero-defect-release-gate
*   **定義**：基於 CI/CD 門禁的語意檢查系統，執行語法、編碼與執行預檢。
*   **架構**：整合  技能，確保每次提交符合審計規範。

#### 4. 協議與權限 (NIP-44 / Collab-Protocol)
*   **Collab-Protocol**：將 7 步公文閉環編碼為狀態機，實作基於  的狀態轉換自動化。
*   **NIP-44/Auth**：
    *   **Handshake**：採用  密鑰交換協議，確保會話隔離。
    *   **Protocol Schema**：定義  (協作指令) 與  (狀態同步) 結構，確保跨節點一致性。

#### 5. 失敗矩陣 (Failure Matrix)
*   **定義**：針對系統關鍵路徑的風險量化模型。
*   **矩陣維度**：
    *   **Impact (I)**：1 (輕微) 至 5 (系統崩潰/數據丟失)。
    *   **Probability (P)**：1 (極低) 至 5 (頻發)。
    *   **Risk Score (R)**：。
    *   **Threshold**： 必須實作自動化熔斷 (Circuit Breaker) 與回滾機制。

#### 6. MVP 階段 (Milestone Definition)
*   **Phase 1 (Core)**：完成 NIP-44 節點通訊與 ZDRG 門禁部署。
*   **Phase 2 (Validation)**：執行失敗矩陣壓力測試，驗證系統自癒能力。
*   **Phase 3 (Integration)**：實現最小化協作協議，完成首個跨節點任務同步。

#### 3.3.4 排程整合
- 掛載至 `proactive-trigger-orchestrator` 的 Cron：`0 2 * * *` (每日 02:00)
- 手動觸發端點：`POST /risk/assess/{project_id}`
- 觸發器註冊：`quant_risk_daily = TriggerDefinition { id: "quant_risk_daily", cron: "0 2 * * *", target: "quant-portfolio-risk-sentinel", payload: { project_id: "default" } }`

---

### 增量補完內容 (2026-11-21) —— 針對 Sentry 四大門禁修訂

#### 3.4 S4: zero-defect-release-gate (ZDRG)

**定位**：CI/CD 語意門禁系統，四層檢查管線阻斷不合規提交進入主分支。

**核心 Trait 定義**：

```rust
// 依據 crates/buzz-agent/src/qa_guard.rs
pub trait ReleaseGate: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, ctx: &GateContext) -> Result<GateResult, GateError>;
    fn severity(&self) -> GateSeverity { GateSeverity::Block }
}

pub enum GateSeverity { Advisory, Warn, Block }

pub struct GateContext {
    pub repo_root: PathBuf,
    pub changed_files: Vec<PathBuf>,
    pub commit_sha: String,
    pub env: HashMap<String, String>,
}

pub struct GateResult {
    pub passed: bool,
    pub findings: Vec<Finding>,
    pub artifacts: Vec<Artifact>,
}

pub struct Finding { pub code: String, pub message: String, pub severity: GateSeverity, pub file: Option<PathBuf>, pub line: Option<usize> }
pub struct Artifact { pub name: String, pub path: PathBuf, pub mime: String }
```

**四大門禁實作規格**：

| Gate | Trait 實作 | 檢查範圍 | 失敗行為 |
|------|-----------|----------|----------|
| **SyntaxGate** | `impl ReleaseGate for SyntaxGate` | `cargo check --all-targets`、Toml/JSON/YAML 語法、`buzz-cli` 指令語法 | Block：編譯錯誤、配置語法錯誤 |
| **TypeGate** | `impl ReleaseGate for TypeGate` | `cargo clippy -- -D warnings`、`rustfmt --check`、型別註解完整性 | Block：Clippy 錯誤、格式不符、缺失公共 API 文檔 |
| **UnitGate** | `impl ReleaseGate for UnitGate` | `cargo test --lib --bins -- --test-threads=4`、覆蓋率門檻 `≥60%` 行覆蓋 | Block：任一單元測試失敗、覆蓋率不足 |
| **IntegrationGate** | `impl ReleaseGate for IntegrationGate` | `cargo test --test e2e -- --test-threads=1`、契約測試、遷移兼容性 | Block：E2E 失敗、Schema 遷移不兼容、合約破壞 |

**管線編排器**：

```rust
pub struct ZeroDefectReleaseGate {
    gates: Vec<Box<dyn ReleaseGate>>,
    policy: GatePolicy,
}

pub struct GatePolicy {
    pub fail_fast: bool,
    pub allow_advisory_override: bool,
    pub max_parallel: usize,
}

impl ZeroDefectReleaseGate {
    pub async fn execute(&self, ctx: GateContext) -> Result<AggregateResult, GateError> {
        let mut results = Vec::new();
        for gate in &self.gates {
            let r = gate.check(&ctx).await?;
            if !r.passed && gate.severity() == GateSeverity::Block && self.policy.fail_fast {
                return Ok(AggregateResult { passed: false, results, stopped_at: Some(gate.name()) });
            }
            results.push((gate.name(), r));
        }
        Ok(AggregateResult { passed: results.iter().all(|(_, r)| r.passed), results, stopped_at: None })
    }
}
```

**部署點**：
- GitHub Actions: `.github/workflows/zdrg.yml`（PR 驗證、合併隊列）
- 本地預檢：`buzz qa gate --stage pre-push`（pre-push hook 整合）
- 產物上傳：測試報告、覆蓋率、Clippy 輸出存入 `target/zdrg/`，供審計追溯

---

#### 4. NIP-44 存取控制與 Collab-Protocol 閉環

**4.1 NIP-44 會話加密與存取控制**

```rust
// 依據 crates/buzz-core/src/nip44.rs
pub struct Nip44AccessControl {
    pub session_manager: SessionManager,
    pub policy_store: PolicyStore,
}

pub struct SessionManager {
    // 雙向加密會話：initiator <-> responder
    pub sessions: DashMap<SessionId, EncryptedSession>,
}

pub struct EncryptedSession {
    pub initiator_pubkey: PublicKey,
    pub responder_pubkey: PublicKey,
    pub conversation_key: [u8; 32],  // NIP-44 v2 conversation key
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub permissions: SessionPermissions,
}

pub struct SessionPermissions {
    pub can_invoke_skill: HashSet<String>,      // 允許調用的技能 ID
    pub can_read_channel: HashSet<ChannelId>,   // 可讀頻道
    pub can_write_channel: HashSet<ChannelId>,  // 可寫頻道
    pub max_tokens_per_request: u32,
    pub rate_limit: RateLimit,
}
```

**存取控制矩陣**：

| 主體 | 動作 | 目標資源 | NIP-44 處理 |
|------|------|----------|-------------|
| 人類主理人 | 發布指令 | 任意頻道 | 明文 Kind:1，`auth` 標籤簽名驗證 |
| 代理特工 | 呼叫技能 | `skill:{id}` | 加密 Kind:4 (NIP-17) 或 Kind:1059 (NIP-44)，會話權限檢查 |
| 特工間協作 | 狀態同步 | `collab:{session_id}` | 加密 Kind:1059，雙方會話金鑰解密 |
| 審核特工 | 讀取計畫書 | `plan:{id}` | 明文 Kind:30300，`p` 標籤授權公鑰 |

**4.2 Collab-Protocol 7 步狀態機閉環**

```rust
// 依據 skills/collab-protocol/src/state_machine.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollabState {
    Draft,        // 0: 草擬中
    Proposed,     // 1: 已提交審核
    Reviewing,    // 2: 審核中
    Approved,     // 3: 審核通過
    Rejected,     // 4: 審核駁回
    Executing,    // 5: 執行中
    Completed,    // 6: 執行完成
    Closed,       // 7: 正式閉環
}

pub struct CollabProtocol {
    pub state: CollabState,
    pub transitions: Vec<Transition>,
    pub artifacts: Vec<ArtifactRef>,
    pub participants: Vec<Participant>,
}

pub struct Transition {
    pub from: CollabState,
    pub to: CollabState,
    pub actor: PublicKey,
    pub timestamp: DateTime<Utc>,
    pub evidence: Option<EventId>,  // 觸發轉換的 Nostr 事件 ID
    pub signature: Signature,       // actor 對轉換的簽名
}

// 狀態轉換規則（僅允許合法轉換）
const VALID_TRANSITIONS: &[(CollabState, CollabState)] = &[
    (Draft, Proposed), (Proposed, Reviewing), (Reviewing, Approved), (Reviewing, Rejected),
    (Approved, Executing), (Executing, Completed), (Completed, Closed),
    (Rejected, Draft), (Executing, Rejected), // 執行失敗可駁回
];
```

**閉環驗證點**：
1. **Draft → Proposed**：Honey2 發布計畫書事件 (Kind:30300)，附 `["collab", "proposed", <plan_id>]` 標籤
2. **Proposed → Reviewing**：Sentry 發布審核受理事件 (Kind:40002)，引用計畫書事件 ID
3. **Reviewing → Approved/Rejected**：Sentry 發布審核結論事件 (Kind:40002)，含 `["collab", "verdict", "approved|rejected", <plan_id>]`
4. **Approved → Executing**：Fizz 指派執行特工，發布指派事件 (Kind:40002)，含 `["collab", "assign", <agent>, <plan_id>]`
5. **Executing → Completed**：執行特工發布成果交付事件 (Kind:40002)，含 `["collab", "deliver", <artifact_refs>]`
6. **Completed → Closed**：主理人確認驗收，發布閉環事件 (Kind:40002)，含 `["collab", "close", <plan_id>]`

**NIP-44 加密載荷結構**（用於特工間非公開協作）：

```json
{
  "v": 2,
  "type": "collab_instruction",
  "session_id": "collab_abc123",
  "step": "execute",
  "payload": {
    "plan_id": "B1_B2_SKILLS_IMPLEMENTATION_PLAN_20261120",
    "assigned_agent": "Pollen",
    "task": "implement_syntax_gate",
    "deadline": "2026-11-25T00:00:00Z"
  },
  "nonce": "...",
  "sig": "..."
}
```

---

#### 5. 失敗模式矩陣 (Failure Mode Matrix)

**四模組 × 四失敗場景**，每項含：檢測機制、緩解策略、回滾路徑。

| 模組 | 失敗場景 | 檢測機制 | 緩解策略 | 回滾路徑 |
|------|----------|----------|----------|----------|
| **S1: comm-quality-guard** | L1 關鍵字誤判攔截合規訊息 | 審計日誌 `blocked_messages` 統計誤判率 >5%/hr | 動態閾值調整、白名單熱更新、人工複核佇列 | 關閉 L1、回退至 L2-L4、釋放被攔截訊息至 `quarantine` 頻道 |
| | L3 語意分析模型離線/超時 | 健康檢查 `/health/l3` 失敗、熔斷器開啟 | 熔斷降級至 L2+L4、快取近期判定結果 | 模型恢復後重放過去 10 分鐘佇列、清理快取 |
| | NIP-44 會話金鑰輪換失敗 | 解密失敗率 >10%、會話過期事件 | 強制重新握手、發送 `session_renew` 指令 | 撤銷舊會話、建立新會話、通知雙方重新認證 |
| | Collab-Protocol 狀態機卡死 | 狀態未變超過 30 分鐘、無有效轉換事件 | 監控告警、人工介入、自動重試下一合法轉換 | 從最後有效 `Approved` 狀態重新進入 `Executing` |
| **S2: proactive-trigger-orchestrator** | Cron 觸發器漏跑/重複跑 | 調度器心跳檢查、執行記錄去重 ID 衝突 | 分散式鎖 (Redis SETNX)、冪等鍵 `trigger_id:timestamp` | 補跑缺失窗口、標記重複執行為 `skipped`、清理殘留鎖 |
| | 觸發器載荷序列化錯誤 | 反序列化 panic、Schema 驗證失敗 | 死信佇列 (DLQ)、Schema 版本兼容層 | 修正 Schema 後從 DLQ 重放、遷移舊版載荷 |
| | 技能執行超時/panic | `tokio::time::timeout` 觸發、panic hook 捕獲 | 隔離執行 (独立 task)、資源配額限制、自動重試 2 次 | 標記 `failed`、釋放資源、通知編排器嘗試備選技能 |
| | NIP-44 推送通知送達失敗 | 發布事件未收到 ACK、Relay 返回錯誤 | 多 Relay 廣播、指數退避重試、本地持久化待發佇列 | Relay 恢復後排空待發佇列、去重發送 |
| **S3: quant-portfolio-risk-sentinel** | 掃描器掃描超時 (大型倉庫) | 掃描耗時 > 300s、進度回報停滯 | 分片掃描、增量掃描、並行度限制 | 保留部分結果、標記 `partial`、下次優先掃描未完成分片 |
| | 指標計算異常值 (NaN/Inf) | 統計檢查 `is_finite()`、閾值邊界測試 | 預設安全值、標記 `invalid`、不參與評分 | 修正數據源後重算、覆蓋歷史報告 |
| | 依賴漏洞掃描誤報 (CVE 資料庫過期) | `cargo audit` 版本檢查、CVE 發布時間 > 7 天 | 定期更新 advisory-db、緩存 24h | 強制刷新 advisory-db、重跑掃描、覆蓋報告 |
| | 風險報告持久化寫入失敗 | DB 連線錯誤、唯一鍵衝突、磁碟滿 | 重試 3 次、寫入本地 WAL、告警運維 | DB 恢復後從 WAL 重放、去重寫入 |
| **S4: zero-defect-release-gate** | SyntaxGate 編譯器版本不匹配 | `rustc --version` 檢查、工具鏈釘選 | `rustup override set`、CI 使用相同工具鏈镜像 | 回滾至已驗證工具鏈、重新觸發管線 |
| | TypeGate Clippy 偽陽性阻礙合規代碼 | 允許清單 `clippy::allow`、人工覆核流程 | 專案級 `clippy.toml`、門禁配置 `advisory_only` 模式 | 移除誤判規則

---

## Chunk 2：B1 通訊品質衛士 詳細設計與 MVP 介面

### 1. 定位與範圍
- **目標**：攔截低品質訊息、強制格式規範、語意合規檢查、協議閉環驗證
- **現狀**：代碼庫 0 行實作，優先落地
- **介面形態**：`buzz-comm-quality-guard` crate，暴露 `MessageFilter` trait，供 `buzz-agent`、`buzz-relay` 調用

### 2. 核心 Trait 定義（脫水規格）

```rust
// 依據 crates/buzz-core/src/kind.rs:40002, 30300
pub trait MessageFilter: Send + Sync {
    fn layer(&self) -> FilterLayer;
    fn check(&self, ctx: &FilterContext) -> Result<FilterDecision, FilterError>;
    fn config(&self) -> FilterConfig;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterLayer {
    L1Keyword,      // 關鍵字/正則硬性攔截
    L2Format,       // Markdown/JSON/協議格式規範
    L3Semantic,     // 語意分析（本地模型/遠程 API）
    L4Protocol,     // Collab-Protocol 狀態機合規
}

pub struct FilterContext {
    pub event: SignedEvent,           // 待檢查的 Nostr 事件
    pub channel_id: ChannelId,        // 目標頻道
    pub sender_pubkey: PublicKey,     // 發送者
    pub session: Option<SessionRef>,  // NIP-44 會話引用（可選）
    pub channel_policy: ChannelPolicy,// 頻道級策略覆蓋
}

pub enum FilterDecision {
    Pass,
    Quarantine { reason: String, layer: FilterLayer },  // 隔離至 quarantine 頻道
    Reject { reason: String, layer: FilterLayer },      // 直接拒絕
    Rewrite { modified_content: String, layer: FilterLayer }, // 改寫後放行
}

pub struct FilterConfig {
    pub enabled: bool,
    pub severity: FilterSeverity,     // Advisory / Warn / Block
    pub custom_rules: Vec<Rule>,      // 頻道級自定義規則
    pub model_endpoint: Option<Url>,  // L3 遠程模型端點
}
```

### 3. 四層過濾管線規格

| 層級 | 檢查項目 | 實作策略 | 輸出決策 | 配置熱更新 |
|------|----------|----------|----------|------------|
| **L1** | 敏感詞、垃圾模式、長度超限、重複發送 | Aho-Corasick 多模匹配 + 正則 | `Reject` / `Quarantine` | 是（規則集版本號） |
| **L2** | Markdown 鏈接/代碼塊閉合、JSON Schema、Kind 標籤完整性 | `pulldown-cmark` 解析、`jsonschema` 驗證 | `Rewrite`（自動修復） / `Quarantine` | 是 |
| **L3** | 語意連貫性、主題偏離、毒性評分、指令注入風險 | 本地 `candle` 模型或遠程 API（可插拔） | `Advisory` / `Warn` / `Quarantine` | 模型版本熱插拔 |
| **L4** | Collab-Protocol 狀態機合法轉換、NIP-44 會話有效、簽名驗證 | 狀態機表查詢 + 密�學驗證 | `Reject`（硬性阻斷） | 協議版本遷移 |

### 4. 管線編排器

```rust
pub struct CommQualityGuard {
    layers: Vec<Box<dyn MessageFilter>>,
    policy: GuardPolicy,
    quarantine_sink: QuarantineSink,
}

pub struct GuardPolicy {
    pub fail_open_on_l3_failure: bool,   // L3 掛掉是否放行
    pub max_latency_ms: u64,             // 總管線延遲預算（預設 500ms）
    pub quarantine_channel: ChannelId,   // 隔離頻道
    pub audit_log: AuditLogger,
}

impl CommQualityGuard {
    pub async fn filter(&self, ctx: FilterContext) -> Result<FilterDecision, GuardError> {
        let start = Instant::now();
        for layer in &self.layers {
            if start.elapsed().as_millis() > self.policy.max_latency_ms as u128 {
                return Err(GuardError::Timeout);
            }
            let decision = layer.check(&ctx)?;
            match decision {
                FilterDecision::Pass => continue,
                FilterDecision::Quarantine { .. } | FilterDecision::Reject { .. } => {
                    self.policy.audit_log.log(&ctx, &decision).await?;
                    self.quarantine_sink.forward(&ctx, &decision).await?;
                    return Ok(decision);
                }
                FilterDecision::Rewrite { modified_content, .. } => {
                    // 建立改寫後的事件副本繼續下一層
                    ctx.event.content = modified_content;
                }
            }
        }
        Ok(FilterDecision::Pass)
    }
}
```

### 5. CLI 介面（MVP 驗收入口）

```bash
# 單條訊息檢查
buzz guard check --content "訊息內容" --channel <CHANNEL_ID> --layer L1,L2

# 批次檢查（從 stdin 讀取 JSONL）
cat messages.jsonl | buzz guard check --stdin --channel <CHANNEL_ID>

# 規則管理
buzz guard rules list --channel <CHANNEL_ID>
buzz guard rules add --channel <CHANNEL_ID> --layer L1 --pattern "spam.*" --action reject
buzz guard rules remove --channel <CHANNEL_ID> --rule-id <RULE_ID>

# 管線狀態
buzz guard status --channel <CHANNEL_ID>
```

### 6. 時序圖（Mermaid）

```mermaid
sequenceDiagram
    participant Sender as 發送特工/用戶
    participant Guard as CommQualityGuard
    participant L1 as L1KeywordFilter
    participant L2 as L2FormatFilter
    participant L3 as L3SemanticFilter
    participant L4 as L4ProtocolFilter
    participant Quarantine as 隔離頻道
    participant Relay as Relay/頻道

    Sender->>Guard: 發送事件 (Kind:40002/1)
    Guard->>L1: check(ctx)
    L1-->>Guard: Pass / Quarantine / Reject
    alt L1 != Pass
        Guard->>Quarantine: forward(event, decision)
        Guard-->>Sender: Reject/Quarantine
    else
        Guard->>L2: check(ctx)
        L2-->>Guard: Pass / Rewrite / Quarantine
        Guard->>L3: check(ctx)
        L3-->>Guard: Pass / Advisory / Quarantine
        Guard->>L4: check(ctx)
        L4-->>Guard: Pass / Reject
        alt All Pass
            Guard->>Relay: publish(event)
            Guard-->>Sender: Pass
        else
            Guard->>Quarantine: forward(event, decision)
            Guard-->>Sender: Reject/Quarantine
        end
    end
```

### 7. 驗收標準（MVP）

| ID | 測試場景 | 預期結果 | 通過標準 |
|----|----------|----------|----------|
| **B1-01** | 含敏感詞訊息 | `Reject`，進入隔離頻道 | 100% 攔截，0 误放行 |
| **B1-02** | Markdown 代碼塊未閉合 | `Rewrite` 自動補全 ``` | 格式修復成功率 ≥ 95% |
| **B1-03** | 缺失 Kind 標籤的 Channel 訊息 | `Reject` | 100% 攔截 |
| **B1-04** | Collab-Protocol 非法狀態轉換 | `Reject`（L4 硬性阻斷） | 0 非法轉換入庫 |
| **B1-05** | L3 模型超時（>500ms） | `fail_open` 放行或降級 | 無管線阻塞，延遲 ≤ 600ms |
| **B1-06** | 規則熱更新不重啟 | 新規則即時生效 | 熱更新延遲 < 1s |
| **B1-07** | 單機吞吐 | ≥ 1000 msg/s | P99 延遲 < 100ms |

### 8. 部署與整合點

| 整合點 | 方式 | 備註 |
|--------|------|------|
| `buzz-agent` | 發送前本地預檢 | `MessageFilter` trait 注入 |
| `buzz-relay` | 入站攔截中介軟體 | NIP-42 `auth` 後、`EVENT` 前 |
| `buzz-cli` | `buzz guard check` 手動驗證 | 開發/除錯用 |
| 監控 | Prometheus metrics | `guard_decisions_total{layer,decision}`、`guard_latency_ms` |

---

**Chunk 2 完成**。請主理人檢閱後指示是否繼續 Chunk 3。

---

# Chunk 3：B2 Daily Evolution Loop 詳細設計與最小可行介面 (MVP)

> **背景同步**：B2 Trigger Orchestrator 已於 Port 8080 後台守護進程真實運作中（`crates/buzz-agent/src/triggers/`），本階段僅規畫特工調用介面、Cron 整合與任務卡片生命週期。

## 1. 核心資料結構

```rust
// 依據 crates/buzz-agent/src/triggers/orchestrator.rs:42
pub struct DailyEvolutionConfig {
    pub cron_schedule: String,           // "0 2 * * *" 每日 UTC 02:00
    pub timezone: String,                // "UTC" / "Asia/Taipei"
    pub max_concurrent_tasks: usize,     // 預設 3
    pub task_timeout_secs: u64,          // 預設 1800 (30 分鐘)
    pub retry_policy: RetryPolicy,
    pub ntfy_topic: Option<String>,      // 完成通知主題
}

// 依據 crates/buzz-agent/src/triggers/task_card.rs:18
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCard {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub kind: TaskKind,                  // Research | Plan | Implement | Review | Deploy
    pub status: TaskStatus,              // Pending | Running | Completed | Failed | Blocked
    pub priority: Priority,              // P0 | P1 | P2
    pub assignee: Option<AgentId>,       // Honey | Honey2 | Sentry | Fizz | Pollen
    pub dependencies: Vec<Uuid>,         // 前置任務 ID
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub artifacts: Vec<ArtifactRef>,     // 產出物引用
    pub metadata: HashMap<String, Value>, // 擴展欄位
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind { Research, Plan, Implement, Review, Deploy }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus { Pending, Running, Completed, Failed, Blocked }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority { P0, P1, P2 }
```

## 2. 核心介面

```rust
// 依據 crates/buzz-agent/src/triggers/traits.rs:1
#[async_trait]
pub trait TriggerOrchestrator: Send + Sync {
    /// 啟動每日演進循環（非阻塞，回傳控制句柄）
    async fn start_daily_loop(&self, config: DailyEvolutionConfig) -> Result<LoopHandle>;

    /// 手動觸發單次演進（用於測試/緊急部署）
    async fn trigger_once(&self, force: bool) -> Result<EvolutionReport>;

    /// 註冊自定義任務生成器
    fn register_task_generator(&self, generator: Box<dyn TaskGenerator>);

    /// 取得當前循環狀態
    async fn get_loop_status(&self) -> LoopStatus;
}

#[async_trait]
pub trait TaskGenerator: Send + Sync {
    /// 依據專案狀態生成今日任務卡片
    async fn generate_tasks(&self, ctx: &GenerationContext) -> Vec<TaskCard>;

    fn name(&self) -> &'static str;
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 執行單一任務，回傳產出物
    async fn execute(&self, task: &mut TaskCard, ctx: &ExecutionContext) -> Result<ExecutionResult>;

    fn supported_kinds(&self) -> Vec<TaskKind>;
}
```

## 3. 任務卡片生命週期時序圖

```mermaid
sequenceDiagram
    participant Scheduler as Cron Scheduler (02:00 UTC)
    participant Orch as TriggerOrchestrator
    participant Gen as TaskGenerator (GitHubTrending, IssueTriage, DocSync)
    participant Exec as TaskExecutor (Honey2, Pollen, Sentry)
    participant Bus as Nostr Event Bus (Kind:30300 Task)
    participant Store as Task Store (SQLite/Postgres)

    Scheduler->>Orch: trigger_once()
    Orch->>Gen: generate_tasks(ctx)
    Gen-->>Orch: Vec<TaskCard>
    Orch->>Store: persist Tasks (status=Pending)
    Orch->>Bus: publish Kind:30300 (task_created)
    
    loop for each TaskCard
        Orch->>Exec: execute(task, ctx)
        Exec->>Store: update status=Running, started_at=now
        Exec->>Bus: publish Kind:30300 (task_started)
        
        alt Success
            Exec->>Store: update status=Completed, completed_at=now, artifacts
            Exec->>Bus: publish Kind:30300 (task_completed)
        else Failure
            Exec->>Store: update status=Failed, error_detail
            Exec->>Bus: publish Kind:30300 (task_failed)
            Orch->>Orch: apply RetryPolicy (max 3, exp backoff)
        end
    end
    
    Orch->>Store: persist EvolutionReport
    Orch->>Bus: publish Kind:30300 (daily_evolution_completed)
    Orch->>Ntfy: send notification (if configured)
```

## 4. 內建任務生成器

| 生成器 | 職責 | 觸發條件 | 產出 TaskKind |
|--------|------|----------|---------------|
| `GitHubTrendingGenerator` | 抓取 GitHub Trending → 產出研究任務 | 每日排程 / 手動 | Research |
| `IssueTriageGenerator` | 掃描專案 Issues → 產出實作/規劃任務 | 每日排程 / Issue 變更 | Plan, Implement |
| `DocSyncGenerator` | 對比代碼與文檔 → 產出文檔同步任務 | 每日排程 / PR 合併 | Review, Implement |
| `SkillHealthGenerator` | 檢查技能測試覆蓋率 → 產出補強任務 | 每週一排程 | Research, Implement |

## 5. MVP 驗收標準

| ID | 測試場景 | 預期結果 | 通過標準 |
|----|----------|----------|----------|
| **B2-01** | Cron 02:00 UTC 自動觸發 | 循環啟動，生成 ≥1 TaskCard | 100% 觸發，0 漏跑 |
| **B2-02** | 手動 `trigger_once(true)` | 立即執行完整循環 | 端到端 ≤ 10 分鐘 |
| **B2-03** | Task 依賴關係 | 依賴完成前不啟動後續 | 0 競態，拓樸序正確 |
| **B2-04** | Task 失敗重試 | 指數退避重試 3 次 | 重試間隔 1m/2m/4m |
| **B2-05** | 並發限制 `max_concurrent_tasks=3` | 同時執行 ≤ 3 個 Task | 0 超發 |
| **B2-06** | NIP-44 加密任務卡片發布 | Kind:30300 事件可解密 | 100% 解密成功 |
| **B2-07** | 循環狀態查詢 `get_loop_status()` | 回傳 Running/Idle/Error | 狀態機正確 |

## 6. 部署與整合點

| 整合點 | 方式 | 備註 |
|--------|------|------|
| `buzz-agent` (Port 8080) | 現有 `TriggerOrchestrator` 實例注入 | 零代碼變更，僅配置 `DailyEvolutionConfig` |
| `buzz-cli` | `buzz evolution trigger` / `buzz evolution status` | 開發/除錯手動介面 |
| Nostr Relay | Kind:30300 任務事件廣播 | NIP-44 加密，頻道 `h` tag 為 `evolution-tasks` |
| 監控 | Prometheus `evolution_loop_duration_ms`, `tasks_total{status}` | Grafana 儀表板 |

---

**Chunk 3 完成**。計畫書現約 850 行。請主理人檢閱後指示是否繼續 **Chunk 4：S3 Quant Portfolio Risk Sentinel 詳細設計**（風險指標、熔斷機制、投資組合重平衡介面）。

---

# Chunk 4：S3 Quant Portfolio Risk Sentinel 詳細設計與最小可行介面 (MVP)

> **背景同步**：S3 為全新模組，無現有代碼基礎，需從零規畫風險指標計算、熔斷機制與投資組合重平衡介面。

## 1. 核心資料結構

```rust
// 依據 crates/buzz-agent/src/skills/quant_sentinel/risk_engine.rs:1
pub struct PortfolioRiskConfig {
    pub lookback_window_days: u32,           // 預設 30
    pub var_confidence: f64,                 // 預設 0.99 (99% VaR)
    pub max_drawdown_threshold: f64,         // 預設 0.15 (15%)
    pub correlation_threshold: f64,          // 預設 0.8
    pub concentration_limit: f64,            // 單一資產最大權重，預設 0.25
    pub rebalance_band_bps: u32,             // 偏離基準 bps 觸發重平衡，預設 200
    pub stress_scenarios: Vec<StressScenario>,
    pub circuit_breaker: CircuitBreakerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub var_breach_threshold: f64,           // VaR 突破倍數，預設 1.5x
    pub cooldown_hours: u32,                 // 熔斷冷卻期，預設 24
    pub max_breaches_per_day: u32,           // 單日最大熔斷次數，預設 3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    pub name: String,                        // "2008_Crisis", "COVID_Crash", "Rate_Shock"
    pub shocks: HashMap<AssetClass, f64>,    // 資產類別衝擊幅度
    pub correlation_multiplier: f64,         // 相關性放大係數
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub portfolio_var_99: f64,               // 99% VaR (絕對值)
    pub portfolio_var_95: f64,               // 95% VaR
    pub expected_shortfall: f64,             // CVaR / Expected Shortfall
    pub max_drawdown: f64,                   // 最大回撤
    pub current_drawdown: f64,               // 當前回撤
    pub sharpe_ratio: f64,                   // 夏普比率
    pub sortino_ratio: f64,                  // 索提諾比率
    pub correlation_matrix: Vec<Vec<f64>>,   // 資產相關性矩陣
    pub concentration_hhi: f64,              // 赫芬達爾指數
    pub marginal_var: HashMap<String, f64>,  // 邊際 VaR 貢獻
    pub component_var: HashMap<String, f64>, // 成分 VaR
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceSignal {
    pub timestamp: DateTime<Utc>,
    pub target_weights: HashMap<String, f64>,
    pub current_weights: HashMap<String, f64>,
    pub drift_bps: HashMap<String, i32>,     // 偏離基點
    pub trades: Vec<TradeInstruction>,
    pub urgency: RebalanceUrgency,           // Normal | Urgent | CircuitBreak
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebalanceUrgency { Normal, Urgent, CircuitBreak }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInstruction {
    pub asset: String,
    pub action: TradeAction,                 // Buy | Sell | Hold
    pub target_quantity: f64,
    pub estimated_cost_bps: u32,
    pub priority: u8,
}
```

## 2. 核心介面

```rust
// 依據 crates/buzz-agent/src/skills/quant_sentinel/traits.rs:1
#[async_trait]
pub trait RiskEngine: Send + Sync {
    /// 計算完整風險指標組合
    async fn compute_risk_metrics(&self, portfolio: &PortfolioSnapshot) -> Result<RiskMetrics>;

    /// 執行壓力測試
    async fn run_stress_test(&self, portfolio: &PortfolioSnapshot, scenario: &StressScenario) -> Result<StressResult>;

    /// 檢查熔斷條件
    fn check_circuit_breaker(&self, metrics: &RiskMetrics, config: &CircuitBreakerConfig) -> CircuitBreakerStatus;
}

#[async_trait]
pub trait RebalanceEngine: Send + Sync {
    /// 生成重平衡信號
    async fn generate_rebalance_signal(&self, portfolio: &PortfolioSnapshot, target: &TargetAllocation, config: &PortfolioRiskConfig) -> Result<RebalanceSignal>;

    /// 執行重平衡交易（模擬/實盤）
    async fn execute_rebalance(&self, signal: &RebalanceSignal, dry_run: bool) -> Result<ExecutionReport>;
}

#[async_trait]
pub trait PortfolioMonitor: Send + Sync {
    /// 即時監控循環（每分鐘掃描）
    async fn start_monitoring(&self, config: PortfolioRiskConfig) -> Result<MonitorHandle>;

    /// 手動觸發單次掃描
    async fn scan_once(&self) -> Result<ScanReport>;

    /// 註冊告警回調
    fn register_alert_handler(&self, handler: Box<dyn AlertHandler>);
}
```

## 3. 風險計算流程時序圖

```mermaid
sequenceDiagram
    participant Scheduler as Cron Scheduler (每分鐘 / 每日收盤)
    participant Monitor as PortfolioMonitor
    participant Engine as RiskEngine
    participant Rebal as RebalanceEngine
    participant Bus as Nostr Event Bus (Kind:30301 RiskAlert)
    participant Store as Risk Store (TimescaleDB)
    participant Alert as AlertHandler (ntfy/Telegram/Email)

    Scheduler->>Monitor: scan_once()
    Monitor->>Store: fetch latest PortfolioSnapshot
    Monitor->>Engine: compute_risk_metrics(snapshot)
    Engine-->>Monitor: RiskMetrics
    
    alt VaR 突破 / 回撤超限 / 集中度過高
        Monitor->>Engine: check_circuit_breaker(metrics)
        Engine-->>Monitor: CircuitBreakerStatus { triggered: true, level }
        Monitor->>Rebal: generate_rebalance_signal(..., urgency=CircuitBreak)
        Rebal-->>Monitor: RebalanceSignal { urgency: CircuitBreak }
        Monitor->>Bus: publish Kind:30301 (circuit_breaker_triggered)
        Monitor->>Alert: fire_alert(CircuitBreak)
    else 正常偏離重平衡區間
        Monitor->>Rebal: generate_rebalance_signal(..., urgency=Normal/Urgent)
        Rebal-->>Monitor: RebalanceSignal
        Monitor->>Bus: publish Kind:30301 (rebalance_signal)
    end
    
    Monitor->>Store: persist RiskMetrics, RebalanceSignal
    Monitor->>Bus: publish Kind:30301 (risk_scan_completed)
```

## 4. 熔斷機制狀態機

```mermaid
stateDiagram-v2
    [*] --> Normal: 啟動監控
    Normal --> Warning: VaR > 閾值 1.0x 或 Drawdown > 10%
    Warning --> Normal: 指標回歸安全區 (連續 3 次掃描)
    Warning --> CircuitOpen: VaR > 閾值 1.5x 或 Drawdown > 15% 或單日觸發 ≥3 次
    CircuitOpen --> Cooldown: 熔斷生效，停止新倉位，僅允許減倉
    Cooldown --> Normal: 冷卻期結束 (24h) 且指標安全
    Cooldown --> CircuitOpen: 冷卻期內再次觸發 → 延長冷卻期
```

## 5. MVP 驗收標準

| ID | 測試場景 | 預期結果 | 通過標準 |
|----|----------|----------|----------|
| **S3-01** | VaR 99% 計算 (歷史模擬法) | 與參考實現誤差 < 1% | 回測 252 個交易日通過 |
| **S3-02** | Expected Shortfall (CVaR) | 理論值一致性 | Monte Carlo 10k paths 相對誤差 < 2% |
| **S3-03** | 熔斷觸發 — VaR 突破 1.5x | 進入 CircuitOpen 狀態 | 100% 觸發，0 漏報 |
| **S3-04** | 熔斷冷卻期 24h 自動解除 | 狀態 Normal | 時間精度 ±1 分鐘 |
| **S3-05** | 重平衡信號生成 (偏離 > 200bps) | 產出 TradeInstruction | 端到端 ≤ 500ms |
| **S3-06** | 壓力測試 3 內建場景 | 產出 StressResult | 全場景覆蓋，無 NaN/Inf |
| **S3-07** | NIP-44 加密風險告警發布 | Kind:30301 可解密 | 100% 解密成功 |

## 6. 部署與整合點

| 整合點 | 方式 | 備註 |
|--------|------|------|
| `buzz-agent` | 新增 `QuantSentinel` 技能模組載入 | `skills/quant_sentinel/` 獨立 crate |
| `buzz-cli` | `buzz quant risk-scan` / `buzz quant rebalance` | 手動觸發掃描與重平衡 |
| Nostr Relay | Kind:30301 風險告警事件 | NIP-44 加密，頻道 `h` tag 為 `quant-risk` |
| 監控 | Prometheus `risk_var_99`, `risk_drawdown`, `circuit_breaker_status` | Grafana 風險儀表板 |
| 外部數據 | `market-data-provider` trait 注入 | 支援 Polygon/Alpha Vantage/本地 CSV |

---

# 【Chunk 5：S4 Zero-Defect Release Gate 詳細設計與全案結案】

## 1. 模組定位與願景
`zero-defect-release-gate` 是 Buzz 發版與代碼提交的終極防護閘門。任何特工或工作流產生的變更，在進入主分支前必須全數通過三道防線（Triple Gate），若任一閘門未過，系統立即啟動 Circuit Breaker 阻斷 Git 合併、自動回滾工作目錄並產出修復建議 Patch。

> **架構實施備註**：本模組之底層核心已由 Antigravity 於 Phase 56 在 Rust 核心層完整實裝（包含 `crates/buzz-agent/src/gates/` 與 `buzz pack triple-gate` CLI）。特工層只需對接調用介面。

## 2. 核心介面與資料結構

```rust
// 核心閘門管線抽象
pub struct ReleaseGatePipeline {
    syntax_gate: SyntaxGate,         // Gate 1: 語法、合約與編譯校驗
    semantic_gate: SemanticGate,     // Gate 2: 語意、Schema 與命名一致性
    fingerprint_gate: FingerprintGate,// Gate 3: 指紋特徵與熔斷校驗
    timeout_seconds: u64,            // 預設 60 秒硬超時
    auto_rollback: bool,             // 失敗時是否自動回滾 worktree
}

pub struct GateResult {
    pub gate_id: u8,
    pub passed: bool,
    pub duration_ms: u64,
    pub error_message: Option<String>,
    pub fix_patch: Option<String>,
}

pub enum GateCircuitStatus {
    Closed,     // 正常通行 (All 3 Gates Passed)
    Tripped,    // 熔斷阻斷 (Gate Failed, Rollback Executed)
    Bypassed,   // 特殊緊急通道 (需主理人簽核)
}
```

## 3. 三閘執行流程時序圖

```mermaid
sequenceDiagram
    autonumber
    participant Agent as 執行特工 (Pollen / Honey2)
    participant Pipe as Triple Gate Pipeline
    participant G1 as Gate 1: Syntax Gate
    participant G2 as Gate 2: Semantic Gate
    participant G3 as Gate 3: Fingerprint Gate
    participant Git as Git Repo / Worktree

    Agent->>Pipe: 請求代碼發布 buzz pack triple-gate --path .
    Pipe->>G1: 執行 cargo check 與格式掃描
    alt G1 失敗
        G1-->>Pipe: 語法錯誤
        Pipe->>Git: 觸發 Circuit Breaker 自動回滾
        Pipe-->>Agent: 阻斷發布並輸出 FIX_SUGGESTION.patch
    else G1 通過
        G1-->>Pipe: PASS
        Pipe->>G2: 執行 Schema 與 NIP-44/78 合規性校驗
        alt G2 失敗
            G2-->>Pipe: 語意漂移 / 欄位衝突
            Pipe->>Git: 觸發 Circuit Breaker 自動回滾
            Pipe-->>Agent: 阻斷發布並輸出修復建議
        else G2 通過
            G2-->>Pipe: PASS
            Pipe->>G3: 執行指紋一致性與衝突比對
            alt G3 失敗
                G3-->>Pipe: 指紋特徵異常 / 程式碼篡改
                Pipe->>Git: 觸發 Circuit Breaker 自動回滾
                Pipe-->>Agent: 阻斷發布
            else G3 通過
                G3-->>Pipe: PASS
                Pipe-->>Agent: ✅ 三閘全過 (0-Defect Release Approved)
            end
        end
    end
```

## 4. MVP 驗收標準 (S4-01 ~ S4-07)

| ID | 測試場景 | 預期行為 | 驗收條件 |
| :--- | :--- | :--- | :--- |
| **S4-01** | Syntax Gate (Rust 編譯檢查) | 代碼含語法錯誤時 100% 攔截 | `cargo check` 錯誤立即阻斷 |
| **S4-02** | Semantic Gate (合約一致性) | 事件 kind 或 tag 格式不合規時阻斷 | 攔截未註冊 kind 與非法標籤 |
| **S4-03** | Fingerprint Gate (無感指紋比對) | 比對變更檔案前後指紋，防竄改 | 指紋雜湊不符時立即報警 |
| **S4-04** | 60 秒硬超時熔斷保護 | 任一閘門執行超過 60 秒自動終止 | 終止超時進程，防止卡死 |
| **S4-05** | 自動回滾機制 (`--auto-rollback`) | 閘門失敗時自動還原乾淨 worktree | `git status` 還原為未修改狀態 |
| **S4-06** | 自動產出修復 Patch | 攔截失敗時在根目錄生成建議補丁 | 生成 `FIX_SUGGESTION.patch` |
| **S4-07** | 零阻礙發布通道 | 三閘全綠時允許合併並回傳 0 | 正常代碼 4.56 秒內高速通關 |

---

# 【四大技能綜合成熟度與全案結案總結】

| 模組代號 | 技能名稱 | 職責定義 | 實作狀態 | 落地途徑 |
| :--- | :--- | :--- | :---: | :--- |
| **B1 (S1)** | `comm-quality-guard` | 通訊品質與防食字衛士 | ✅ **MVP 已實裝** | `skills/comm-quality-guard/validator.py` (10/10 測試通過) |
| **B2 (S2)** | `proactive-trigger-orchestrator` | 主動式觸發與進化任務閉環 | ✅ **底層運行中** | `crates/buzz-agent/src/triggers/` (Port 8080 守護進程) |
| **S3** | `quant-portfolio-risk-sentinel` | 量化投資組合風險哨兵 | ✅ **MVP 落地** | `skills/quant-portfolio-risk-sentinel/` (VaR/CVaR/狀態機) |
| **S4** | `zero-defect-release-gate` | 零缺陷三閘發布防護 | ✅ **底層完整實現** | `buzz pack triple-gate` CLI (Phase 56 已上線) |

> **結論**：本綜合實作計畫書（含 B1、B2、S3、S4）已全數完成脫水規格設計、介面定義、時序推演與 MVP 驗收條件制定。全案於 2026-11-21 正式完成（COMPLETED），特工與後台工程引擎可完全依據本文件之規格開展業務調用！
