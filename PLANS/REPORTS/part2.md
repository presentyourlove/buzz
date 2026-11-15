- **語言**：`TypeScript` (Electron/React) | **說明**：The Open-Source Multimodal AI Agent Stack: Connecting Cutting-Edge AI Models and Agent Infra

### 1. 用途（核心價值主張）

**核心痛點：**
現有 GUI Agent 要麼是「雲端黑盒」 (Operator, Computer Use API)，要麼是「本地簡陋腳本」。缺乏一個**開箱即用、本地優先、可擴展、支援多模態模型熱切換**的桌面級 Agent 應用。

**核心功能與技術亮點：**
*   **雙產品架構**：
    *   **Agent TARS (CLI/Web UI)**：通用多模態 Agent 堆疊，終端/瀏覽器/桌面/產品四端融合。核心特性：**Event Stream 驅動的 Context Engineering**、**MCP 內核**、**混合瀏覽器策略** (Visual Grounding + DOM + Hybrid)。
    *   **UI-TARS Desktop (Electron App)**：原生桌面 GUI Agent，基於 **UI-TARS 模型** (ByteDance 開源的專用 GUI Agent VLM，7B/72B)。
*   **本地/遠程雙模運算子**：
    *   **Local Operator**：本機截圖 + 本地/雲端模型推理 + 本機鼠鍵控制。隱私優先，零數據上傳。
    *   **Remote Operator**：一鍵連接遠程電腦/瀏覽器，無需配置。解決「幫家人修電腦」、「雲端 CI/CD 操作」場景。
*   **Event Stream Architecture (事件流架構)**：
    *   所有 Agent 內部狀態 (思考、工具調用、觀察、截圖、模型輸出) 均為不可變事件流。
    *   驅動 **Context Engineering**：動態構建最優上下文 (截圖壓縮、歷史摘要、工具結果裁剪)。
    *   驅動 **Agent UI**：前端即時渲染事件流，支援「時光機」回溯、分支、人工介入修正。
*   **MCP 原生內核**：Agent 核心即建立在 MCP 之上，工具即 MCP Server。支援動態掛載任意 MCP Server。
*   **AIO Sandbox 整合**：獨立隔離執行環境，支援 Shell、多文件結構化展示、流式輸出。

### 2. 用處（架構定位）

**工程架構層級：**
位於 **Agent Application Layer (代理應用層)** 與 **Agent Infrastructure Layer (代理基礎設施層)** 交界。它是一個 **Reference Implementation (參考實現)**，展示了如何將模型、感知、動作、記憶、UI、協議組裝成產品級體驗。

**適用業務場景：**
*   **開發者生產力工具**：本地 IDE 伴隨、終端自動化、代碼庫導航。
*   **QA 自動化測試**：本地/遠程瀏覽器與桌面應用的端到端測試。
*   **遠程運維/協助**：IT 支援、跨設備協作。
*   **Agent 產品原型驗證**：快速驗證新模型、新工具、新交互範式。

**技術棧整合性：**
*   **前端**：Electron, React, TypeScript, Monaco Editor
*   **後端/核心**：Node.js, `@agent-tars/cli`, `mcp` SDK, `agent-infra/sandbox`
*   **模型**：UI-TARS (HF: ByteDance-Seed/UI-TARS-1.5-7B/72B), Seed-1.5-VL, GPT-4o, Claude, DeepSeek, Qwen 等任意 OpenAI 兼容 API
*   **協議**：MCP (核心), ACP (規劃中), WebSocket (Event Stream)

### 3. 啱唔啱主理人/Buzz用（契合度與落地評估）

**評估結論：高度契合，UI-TARS Desktop 是「產品級 Agent 桌面應用」的標杆，Buzz Desktop 的 Agent 面板應對標其 UX 與架構。**

**具體落地建議：**

1.  **Event Stream Architecture 全面採用**
    *   Buzz 目前的消息流主要基於 Nostr Event。Agent 內部執行循環缺乏結構化事件流。
    *   **行動項**：在 `buzz-agent` 內部引入 **Agent Event Stream** (內存/持久化)：`ThoughtEvent`, `ToolCallEvent`, `ObservationEvent`, `ScreenshotEvent`, `ModelOutputEvent`。
    *   **效用**：支援前端即時渲染 Agent 思維鏈、時光機回溯、人工介入分支、Context Engineering 自動優化。

2.  **Local/Remote Operator 雙模能力**
    *   Buzz 的 Nostr 天然支援遠程協作。應實現 **Remote Computer Operator**：
        *   被控端運行輕量 `buzz-agent-daemon` (WebRTC/NATS/HTTP) 提供螢幕流、鼠鍵注入、文件系統訪問。
        *   主控端 Agent 調用 `remote_computer` 工具，體驗與本地一致。
    *   這是 Buzz 「去中心化協作」獨特優勢的具體化：**Agent 可跨設備、跨網絡、跨用戶操作**。

3.  **MCP 內核化**
    *   參考 Agent TARS「內核建立在 MCP 上」。Buzz Agent 的工具系統應遷移至 **MCP Server 架構**：內建工具 (文件、終端、Git、Nostr) 即內建 MCP Server；外部工具掛載標準 MCP Server。Agent 核心僅作為 MCP Client 與 LLM 交互。

4.  **UI-TARS 模型集成**
    *   將 `ByteDance-Seed/UI-TARS-1.5-7B` 作為 Buzz **本地視覺定位模型** 的首選開源權重。配合 OmniParser 邏輯，實現完全離線的 GUI 操作能力。

5.  **AIO Sandbox 引入**
    *   引入 `agent-infra/sandbox` 作為 Buzz Agent 的**安全隔離執行環境**，支援 Shell 流式、多文件結構化、網絡隔離、資源限制。

---

## 專案 5：[OpenAdaptAI/OpenAdapt](https://github.com/OpenAdaptAI/OpenAdapt) (★ 1,860)
- **語言**：`Python` | **說明**：For EHRs, payer portals, and any app your team clicks through. A person or an agent shows the task; OpenAdapt turns it into a local program that repeats the steps with no model calls, checks each save, and stops when something doesn't match.

### 1. 用途（核心價值主張）

**核心痛點：**
Computer Use Agent (CUA) 每次運行都「重新看屏幕、重新決定點哪裡」，導致：**成本高、延遲高、不可靠 (同一任務每次路徑不同)、無法驗證保存是否真正成功 (屏幕顯示「已保存」≠數據庫寫入成功)**。傳統 RPA 腳本雖確定性強，但極其脆弱，選擇器一變即斷。

**核心功能與技術亮點：**
*   **三模式閉環：Explore → Run → Repair**
    1.  **Explore (探索/錄製)**：人類或 Agent 執行一次任務，OpenAdapt 記錄**所有層面**：螢幕像素、DOM/UIA/Accessibility/AT-SPI 結構、網絡請求 (API 調用)。
    2.  **Compile (編譯)**：將錄製軌跡編譯為**確定性程序**。編譯器優先選擇最可靠層：API > 結構 (DOM/UIA) > 像素模板匹配。生成可讀、可審計、可版本控制的程序代碼。
    3.  **Run (執行)**：程序**零模型調用**重放。關鍵創新：**每次寫入前驗證記錄、寫入後讀回驗證 (Read-back Verification)**。若讀回不符，立即 HALT，拒絕猜測。
    4.  **Repair (修復)**：應用變更導致運行停止時，Agent 提出修復，程序試運行至寫入點，**人類審批**後熱更新程序。
*   **Effect Verification (效果驗證/讀回機制)**：這是核心護城河。實證表明：
    *   Agent 單獨運行：壞保存 30/30 被誤判為成功。
    *   OpenAdapt `verify-run`：壞保存 30/30 全部攔截，良性 10/10 確認。零誤報。
*   **多表面統一**：Web (DOM+像素)、Windows (UIA+像素)、macOS (Accessibility+像素)、Linux (AT-SPI+像素)、RDP/Citrix (純像素) —— 同一套 編譯/運行/修復 流程。
*   **MCP / Agent Skill / CLI 多介面**：編譯後的工作流即 MCP Tool、Agent Skill、CLI 命令。Agent 決定「輸入什麼」，OpenAdapt 負責「怎麼填、驗證、讀回」。
*   **本地優先、隱私保護**：健康運行零模型調用、患者數據脫敏、簽名收據。

### 2. 用處（架構定位）

**工程架構層級：**
位於 **Execution Verification Layer (執行驗證層)** 與 **Deterministic Replay Layer (確定性重放層)**。它是 **Computer Use Agent 的「安全閥」與「加速器」**：將非確定性、高成本、不可驗證的 Agent 行為，轉化為確定性、零成本、可驗證的程序行為。

**適用業務場景：**
*   **高風險寫入操作**：醫療 EHR、保險理賠、銀行核心、ERP/CRM 關鍵數據錄入。**「寫錯不可逆、驗證必須徹底」**場景。
*   **大批量重複性任務**：同一表單提交 1000+ 次。Agent 做一次，程序跑 1000 次，成本降為 0。
*   **合規審計要求**：每次操作需留存可審計軌跡、讀回證據、簽名收據。
*   **Agent 安全防護層**：任何 Agent 的寫入動作，經過 OpenAdapt 程序代理，獲得「讀回驗證」保護。

**技術棧整合性：**
*   **核心**：Python, `openadapt-flow` (編譯/運行/驗證引擎), `openadapt-capture` (多後端錄製)
*   **後端**：Playwright (Web), UI Automation (Windows), Accessibility (macOS), AT-SPI (Linux), RDP/Citrix (遠程)
*   **協議**：MCP Tool Server (`openadapt-agent serve`), Agent Skill (`openadapt-agent emit-skill`), CLI
*   **驗證**：`openadapt-flow verify-run` (獨立於 Agent 的裁判)

### 3. 啱唔啱主理人/Buzz用（契合度與落地評估）

**評估結論：戰略級關鍵基建。OpenAdapt 解決了「Agent 寫入不可信」這個阻礙 Agent 落地生產環境的核心信任危機。Buzz 若要支撐真實業務自動化，必須集成其驗證邏輯。**

**具體落地建議：**

1.  **引入「讀回驗證」作為 Buzz Agent 寫入操作的強制門檻**
    *   所有涉及**持久化寫入** (資料庫、API POST/PUT、文件保存、區塊鏈交易) 的 Agent 工具，必須實現 `VerifyableWrite` Trait：
        *   `pre_check()`：寫入前驗證狀態
        *   `execute()`：執行寫入
        *   `post_verify()`：**讀回驗證** (查詢 DB、GET API、讀文件、鏈上查詢)
        *   `outcome`：`VERIFIED` / `HALTED_BEFORE_EFFECT` / `RECONCILIATION_REQUIRED` / `FAILED`
    *   參考 OpenAdapt 的 `transaction_outcome` 枚舉設計。

2.  **Explore-Compile-Run-Repair 工作流集成**
    *   Buzz Agent 獲得新任務時，優先檢查**已編譯程序庫**。有則直接 `Run` (零模型成本)；無則 `Explore` (Agent 或人類演示) → `Compile` → 人類/策略審批 → `Admit` 入庫。
    *   這將 Buzz Agent 從「每次即興表演」升級為「劇本化專業演員」。

3.  **多表面統一適配器**
    *   參考 OpenAdapt 的 Surface 抽象，為 Buzz 實現統一的 `SurfaceAdapter`：`WebSurface`, `WindowsSurface`, `MacOSSurface`, `LinuxSurface`, `RDPSurface`。Agent 只調用 `surface.click(selector)`、`surface.read_back(query)`，底層自動選擇最佳層 (API > 結構 > 像素)。

4.  **MCP Tool 化部署**
    *   將 OpenAdapt 編譯後的工作流作為 **Buzz 標準 MCP Server** 部署。任何 Buzz Agent (甚至外部 Agent) 均可調用 `openadapt://workflow/patient_intake` 完成醫療掛號，享受零模型、驗證保障、可審計。

5.  **作為 Agent 訓練/評估的 Reward Model**
    *   `openadapt-flow serve-reward` 將讀回驗證轉化為獎勵信號。Buzz 可用於 **Agent 訓練 (RLHF/RLAIF)** 與 **自動化評估基準**，解決「如何自動評分 Agent 操作質量」的難題。

---

# 📊 綜合對比分析與 Buzz 技術路線圖建議

| 維度 | Open Interpreter | OmniParser | Agent-S | UI-TARS Desktop | OpenAdapt |
|------|------------------|------------|---------|-----------------|-----------|
| **核心定位** | Model-Agnostic Agent Harness Platform | Pure Vision Perception Engine | Cognitive Architecture (Memory/Evolution) | Product-Grade Agent Desktop App | Verified Execution Layer |
| **關鍵創新** | Harness Abstraction + ACP/MCP Native | YOLOv9-E + Florence-2 Two-Stage Parsing | Narrative Memory + Interface Evolution | Event Stream + MCP Kernel + Local/Remote Operator | Explore-Compile-Run-Repair + Read-back Verification |
| **技術成熟度** | ★★★★★ (生產就緒、企業採用) | ★★★★☆ (核心感知 SOTA、工程化進行中) | ★★★★★ (學術 SOTA、商業化驗證) | ★★★★★ (產品級、用戶體驗標杆) | ★★★★☆ (核心邏輯證明、生態擴展中) |
| **對 Buzz 戰略價值** | **架構標杆**：Harness、協議、技能標準 | **基建組件**：視覺感知標準工具 | **認知藍圖**：記憶、路由、技能合成 | **產品參考**：UX、Event Stream、雙模運算 | **信任基石**：驗證、確定性、合規 |
| **建議採納優先級** | **P0 (立即對標架構)** | **P0 (集成視覺工具鏈)** | **P1 (重構記憶與路由)** | **P1 (重構 Agent 面板 UX)** | **P0 (引入寫入驗證門檻)** |

## 🎯 Buzz 下一階段三大技術攻關方向 (基於五大專案綜合研判)

### 方向一：Harness 抽象化與多模型統一運行時 (參考 Open Interpreter)
*   **目標**：`buzz-agent` 支援任意模型後端 (OpenAI/Anthropic/DeepSeek/Kimi/GLM/Qwen/本地 GGUF) 熱切換，統一 ACP/MCP 協議。
*   **交付物**：`BuzzHarness` Trait + `HarnessRegistry` + `ACP Server` 內建 + `.agents/skills` 目錄標準化。

### 方向二：可信執行環境：感知-決策-驗證閉環 (參考 OmniParser + OpenAdapt + Agent-S)
*   **目標**：Agent 操作 GUI/API 時，默認開啟「感知驗證 (OmniParser) + 執行驗證 (OpenAdapt Read-back) + 經驗學習 (Agent-S Memory)」。
*   **交付物**：
    *   `VisionPerceptionTool` (OmniParser 集成)
    *   `VerifiedWrite` Trait + `TransactionOutcome` 枚舉 (OpenAdapt 邏
