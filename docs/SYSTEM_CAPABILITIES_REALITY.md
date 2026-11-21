# 系統底層真實能力與資產清冊 (System Reality Manifest)

> **版本**：1.0.0 (Phase 58)  
> **更新日期**：2026-11-21  
> **維護者**：Antigravity (系統架構與後台執行引擎)  
> **適用對象**：全體特工 (Buzz Agents) 與主理人  
> **用途**：作為代碼庫「單一事實來源」(Single Source of Truth)，終結特工對已存在設施的重複發明輪子與未實裝技能的虛假幻覺。

---

## 1. 核心狀態總覽矩陣 (State Matrix)

| 模組 / 工具名稱 | 實作狀態 | 所在位置 / 調用方式 | 功能說明與調用約束 |
| :--- | :---: | :--- | :--- |
| **Proactive Trigger Orchestrator** | ✅ **已實裝且運行中** | `crates/buzz-agent/src/triggers/`<br>常駐背景服務：Port `8080` | 事件總線 (EventBus)、Cron 觸發器、條件 DSL 排程器。<br>**嚴禁重新規劃排程底層，直接透過 HTTP 或 CLI 調用。** |
| **Triple Gate 防線** | ✅ **已實裝且運行中** | `crates/buzz-agent/src/gates/`<br>CLI 指令：`buzz pack triple-gate` | 包含 Syntax Gate (語法合約)、Semantic Gate (語義 Schema)、Fingerprint Gate (電路熔斷)。<br>任一失敗即阻斷發布並自動回滾與產出 Patch。 |
| **ACP RBAC 沙箱** | ✅ **已實裝且運行中** | `crates/buzz-acp/src/rbac.rs` | 角色權限矩陣：區分 Architect, Engineer, Reviewer, Planner。<br>針對高風險檔案修改與終端指令自動進行安全邊界攔截。 |
| **即時動態儀表板** | ✅ **已實裝且運行中** | `c:\私人\ai agent\buzz\dashboard`<br>本地預覽：Port `4173` | Vite 4173 提供即時動態 Telemetry 活力圖表、6 大特工連線狀態環。 |
| **B1: 通訊品質衛士 (`comm-quality-guard`)** | ✅ **MVP 已實裝** | `skills/comm-quality-guard/`<br>`.agents/skills/comm-quality-guard/` | 提供 `sanitize_message`，自動將角括號轉全形、保護行首特殊標點，10/10 測試通過。 |
| **B2: 進化任務生成引擎 (`daily-evolution-generators`)** | ✅ **MVP 已實裝** | `skills/daily-evolution-generators/`<br>`.agents/skills/daily-evolution-generators/` | 包含 HealthCard、EvoTask、RiskCard、GateCard 4 大任務卡片生成器，4/4 測試通過。 |
| **S3: 量化風險哨兵 (`quant-portfolio-risk-sentinel`)** | ✅ **MVP 已實裝** | `skills/quant-portfolio-risk-sentinel/`<br>`.agents/skills/quant-portfolio-risk-sentinel/` | 提供 VaR 99% / CVaR / Max Drawdown 計算引擎與 4 態自動熔斷狀態機，9/9 測試通過。 |

---

## 2. 特工行為約束守則 (Behavioral Guardrails)

1. **強制引用既有能力**：
   - 當需要設定定期任務、Webhook 監聽或事件排程時，**一律宣告使用 Port 8080 的 Trigger Orchestrator**，嚴禁草擬新的排程器。
   - 當需要執行代碼審核或發版防護時，**一律宣告執行 `buzz pack triple-gate`**，嚴禁重複設計 Gate 1/2/3。
2. **禁止宣稱未實作功能**：
   - 對於標記為 ❌ 之項目（如 `comm-quality-guard`），特工不得在對話中宣稱「已透過四層管線過濾」，必須誠實標註為「待 Antigravity 於後台落地之 MVP 規格」。
3. **消除食字格式規範**：
   - 特工輸出代碼、路徑、識別碼時，必須以 Markdown 反引號（\`code\`）包裹。
   - 嚴禁裸露角括號 `< >` 以免被渲染器誤認為未閉合標籤吞掉。
