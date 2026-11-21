# DESIGN_SPEC_C_MODS.md
## 2. Patch 格式規範
### 2.1 通用結構

```yaml
# 範例：guard_wrapper_no_rm.mod.yaml
mod_id: "guard.no_rm_rf"
mod_type: "GUARD_WRAPPER"
version: "1.0.0"
target:
  kind: "tool_call"
  match:
    tool_name: "shell"
    args_pattern: "rm\\s+-rf"
conditions:
  - "context.phase != 'emergency'"
actions:
  - type: "block"
    message: "⛔ 禁止使用 rm -rf，請改用安全刪除流程"
    severity: "critical"
metadata:
  author: "Honey2"
  created: "2026-11-19"
  tags: ["safety", "filesystem"]
```

### 2.3 TODO_BAND 專屬欄位

```yaml
mod_type: "TODO_BAND"
target:
  kind: "task_lifecycle"         # 綁定任務生命週期
  match:
    task_type: "regex"           # 任務類型過濾（可選）
    agent_id: "regex"            # 特工過濾（可選）
actions:
  - type: "enforce_structure"
    require_todo_before: true    # 強制要求執行前建立 Todo
    require_progress_updates: true  # 強制定期更新進度
    min_update_interval_sec: 30  # 最小更新間隔
    progress_format: "percentage|steps"  # 進度顯示格式
  - type: "visual_band"
    position: "top|bottom|sidebar"  # 進度條顯示位置
    show_eta: true               # 顯示預估剩餘時間
    show_token_budget: true      # 顯示 Token 預算消耗
```
```

### 2.4 TOKEN_AUDIT 專屬欄位

```yaml
mod_type: "TOKEN_AUDIT"
target:
  kind: "agent_execution"        # 綁定特工執行上下文
  match:
    agent_id: "regex"            # 特工過濾（可選）
    model: "regex"               # 模型過濾（可選）
actions:
  - type: "budget_enforce"
    max_input_tokens: 100000     # 單次輸入上限
    max_output_tokens: 50000     # 單次輸出上限
    max_total_tokens: 500000     # 任務總額度
    alert_threshold_pct: 80      # 警告閾值百分比
    hard_stop_at_pct: 100        # 硬性熔斷百分比
  - type: "usage_log"
    log_interval_sec: 10         # 記錄間隔
    include_prompt_hash: true    # 包含提示詞哈希（去識別化）
    destination: "audit_bus"     # 寫入審計總線
```


## 3. MOD 載入器與生命週期

### 3.1 掃描與載入流程
```
掃描目錄 -> 解析 YAML -> 驗證 Schema -> 註冊至擴充點 -> 熱載入完成
  |           |           |             |              |
mods/      mod_id      JSON Schema   Extension      ModRegistry
目錄        唯一性       嚴格校驗      Point 映射       單例管理
```

### 3.2 載入器介面 (ModLoader)
```rust
// 位置：crates/buzz-agent/src/mods/loader.rs
pub struct ModLoader {
    scan_dirs: Vec<PathBuf>,
    registry: Arc<ModRegistry>,
    event_bus: Arc<EventBus>,
}

impl ModLoader {
    pub async fn scan_and_load(&self) -> Result<Vec<ModId>, ModError>;
    pub async fn load_mod(&self, path: &Path) -> Result<ModId, ModError>;
    pub async fn unload_mod(&self, mod_id: &ModId) -> Result<(), ModError>;
    pub async fn reload_mod(&self, mod_id: &ModId) -> Result<(), ModError>;
    pub fn get_mod(&self, mod_id: &ModId) -> Option<Arc<dyn Mod>>;
}
```

### 3.3 熱重載機制
- 監聽 mods/ 目錄變更
- 變更時：卸舊 -> 載新 -> 原子切換
- 失敗自動回滾，發出 ModReloadFailed 事件


## 4. 擴充點介面

### 4.1 三大標準擴充點
| 擴充點 | 掛載時機 | 輸入 | 輸出 |
|--------|----------|------|------|
| pre_tool_call | 工具調用前 | ToolCallContext | ModAction (block/warn/transform/continue) |
| task_lifecycle | 任務建立/更新/完成 | TaskEvent | ModAction (enforce/visualize/log) |
| agent_execution | 特工每輪推理 | ExecutionContext | ModAction (budget_check/log/alert) |

### 4.2 擴充點執行順序
```
多個 MOD 同一擴充點 -> 依 priority 升序執行 -> 任一 block 即短路阻斷
priority 規則：核心內建(0) < 系統級(1-99) < 使用者(100-999) < 實驗性(1000+)
```

### 4.3 擴充點介面定義
```rust
// 位置：crates/buzz-agent/src/mods/extension_points.rs
#[async_trait]
pub trait ExtensionPoint: Send + Sync {
    async fn execute(&self, ctx: &ExtensionContext) -> Result<ModAction, ModError>;
    fn priority(&self) -> u32;
    fn mod_id(&self) -> &ModId;
}

pub enum ModAction {
    Continue,                           // 放行
    Block { reason: String, severity: Severity },  // 阻斷
    Warn { message: String },           // 警告放行
    Transform { spec: TransformSpec },  // 參數轉換
    Enforce { config: EnforceConfig },  // 強制結構
    Visualize { config: VisualConfig }, // 視覺化綁定
    Log { data: serde_json::Value },    // 審計記錄
}
```
## 5. 事件總線與審計整合
### 5.1 MOD 事件 Kind 定義
| Kind | 名稱 | 用途 |
|------|------|------|
| 40090 | mod.loaded | MOD 載入成功 |
| 40091 | mod.unloaded | MOD 卸載 |
| 40092 | mod.action_executed | MOD 動作執行記錄 |
| 40093 | mod.budget_alert | Token 預算告警 |
| 40094 | mod.budget_exhausted | Token 預算耗盡熔斷 |
| 40095 | mod.reload_failed | 熱重載失敗 |
### 5.2 事件結構範例 (Kind: 40092)
```json
{
  "mod_id": "guard.no_rm_rf",
  "mod_type": "GUARD_WRAPPER",
  "extension_point": "pre_tool_call",
  "action": "block",
  "target_tool": "shell",
  "matched_args": "delete command",
  "severity": "critical",
  "agent_id": "abc123...",
  "task_id": "task-001",
  "timestamp": "2026-11-19T10:30:00Z"
}
```
### 5.3 審計鏈整合
MOD 系統所有關鍵操作必須寫入 **buzz-audit** 哈希鏈，確保不可篡改與完整可追溯。

#### 審計條目結構
```json
{
  "audit_id": "mod-audit-20261119-001",
  "mod_id": "guard.no_rm_rf",
  "operation": "block",
  "input_hash": "sha256:abc123...",
  "output_hash": "sha256:def456...",
  "prev_chain_hash": "sha256:prev_hash...",
  "timestamp": "2026-11-19T10:30:00Z",
  "agent_pubkey": "abc123...",
  "verifiable": true
}
```

## 6. 測試與驗收標準
### 6.1 單元測試
- Patch 解析器：正則匹配、YAML 反序列化、版本相容性、錯誤處理
- 載入器：依賴拓撲排序、循環依賴檢測、優先級排序、熱重載狀態機
- 執行器：各 ModAction 類型的正確性、條件求值 (evalexpr)、中斷傳播
- TOKEN_AUDIT：預算計算精度、告警/熔斷觸發閾值、多 Agent 隔離計數
- 審計鏈：哈希鏈連續性、驗證邏輯、防篡改測試

### 6.2 整合測試
- 端到端 MOD 生命週期：載入 → 執行 → 卸載 → 熱重載 → 回滾
- GUARD_WRAPPER 阻斷真實 shell 呼叫（含危險刪除指令、磁碟操作、格式化等高危指令）
- TODO_BAND 插入/更新/完成流程與事件發射驗證
- TOKEN_AUDIT 多輪對話累計、預算耗盡熔斷行為、恢復機制
- 事件匯流排：Kind 40090-40095 正確發射、訂閱端接收、重試與背壓

### 6.3 驗收標準
| 項目 | 指標 | 驗收方式 |
|------|------|----------|
| Patch 載入成功率 | 100% (無語法錯誤 Patch) | 自動化測試套件 50+ Patch 範例 |
| GUARD_WRAPPER 阻斷率 | 100% 攔截高危指令、0% 誤攔正常指令 | 紅隊測試 20+ 攻擊向量、綠隊測試 50+ 正常操作 |
| TODO_BAND 可見性 | 任務清單即時顯示於 Agent 狀態、UI 同步延遲 < 500ms | 整合測試 + 手動驗證 |
| TOKEN_AUDIT 精度 | Token 計數誤差 < 1%、預算告警觸發準確率 100% | 模擬 10 萬 Token 對話、邊界值測試 |
| 熱重載零停機 | 重載過程現有任務不中斷、狀態平滑遷移 | 壓力測試 100 次連續熱重載 |
| 審計鏈完整性 | 鏈式哈希驗證 100% 通過、任意條目可獨立驗證 | 自動化驗證腳本全量跑 |

### 6.4 效能基準
| 指標 | 目標 | 測試條件 |
|------|------|----------|
| Patch 解析延遲 | < 5ms / Patch | 1000 Patch 併發解析 |
| 擴充點攔截延遲 | < 1ms (同步)、< 5ms (異步) | 10000 次呼叫統計 P99 |
| 熱重載完成時間 | < 200ms (無依賴變更)、< 500ms (有依賴變更) | 50 MOD 同時重載 |
| 記憶體佔用增量 | < 50 MB (50 MOD 載入) | 基線對比 |
