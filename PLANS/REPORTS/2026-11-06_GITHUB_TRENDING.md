# 📡 Honey2::Radar 每日 GitHub 開源專案技術研報

- **研究員身分**：Honey2 專屬 Sub-Agent (`Honey2::Radar`)
- **產出日期**：2026-11-06
- **核心模型**：`google/gemma-4-31b-it:free`
- **資料來源**：GitHub Trending API + Tavily Search Engine
- **去重機制**：SQLite Local Database Verified

---

## 專案 1：[KKKKhazix/agent-autoloop](https://github.com/KKKKhazix/agent-autoloop) (★ 3379)
- **語言**：`TypeScript` | **說明**：让你的命令行 Agent 在后台自己接活、自己跟进到底；平时不烦你，遇到拿不准的事才发卡片找你拍板。

**1. 用途（有咩用）**  
- **核心痛點**：傳統 CLI Agent（如 Claude Code、Codex）需要人手「看門」 —  — 輸入指令後必須持續監視、回覆問題，離開視窗即停止工作。  
- **解決方案**：`agent-autoloop` 提供一套 **自動循環、無需人監管** 的執行框架，讓 CLI Agent 能在背景持續執行、自行判斷是否需要人工介入（卡片）。  
- **核心功能**  
  1. **事件驅動的自洽迴圈**（≈1,650 行純 TypeScript，無時鐘、無隨機、無外部請求） —  — 相同輸入永遠產生相同結果。  
  2. **外掛（plugin）體系**：所有Credentials、API、任務實作皆封裝於外掛，Hub 只負責調度。  
  3. **零ランタイム依賴**：僅靠 Node.js 運行，無額外套件，易於嵌入任何環境。  
  4. **決定權分離**：外掛在「拿不准」時發卡片，Hub 只負責「拍板」；其餘全自動。  
- **技術創新亮點**  
  - **純函式、決定性**：無時鐘/隨機/網路呼叫，保證可重現性，適合離線、可審計的自動化。  
  - **外掛隔離**：插件可自行更換或升級，不影響 Hub 本身，提升可維護性與可測試性。  
  - **事件導向**：插件透過訊息總線（MCP）主動註冊、監聽，Hub 只負責「收」與「執行」指令，降低耦合。  

---

**2. 用處（用在哪）**  
- **架構層級**：屬於 **「執行引擎」** 層，介於 **「CLI 使用者」** 與 **「具體任務外掛」** 之間，屬於 **服務/框架** 而非純粹的應用程式。  
- **適用業務場景**  
  - **自動化 CI/CD 流程**：在無人值守的 Runner 上執行代碼檢查、測試、部署指令。  
  - **離線資料處理**：需要在無網路、無時間基礎的環境中自動完成批次腳本（如資料清洗、報表生成）。  
  - **多代理協同**：每個外掛可代表不同職能（代碼生成、文件摘要、資料查詢），Hub 透過事件總線協調多個 Agent。  
  - **Nostr / 去中心化通訊**：在 Nostr 事件中嵌入外掛，使其能自行讀取/寫入帳本，符合去中心化架構。  
- **技術棧整合性**  
  - **語言**：TypeScript + Node.js，天然與前端/後端 TS 生態相容。  
  - **協議**：支援 **M

---

## 專案 2：[t3dotgg/tstack](https://github.com/t3dotgg/tstack) (★ 3115)
- **語言**：`Python` | **說明**：The agent skills I use every day. Works with Claude Code and Codex.

**1. 用途（有咩用）**  
- **核心痛點**：傳統 AI‑Agent 開發需要重複寫「技巧」(skill) 介面、上下文傳遞、工具呼叫與結果處理，導致代碼碎片化、難以複用。  
- **核心功能**：  
  1. **技巧定義**：以 Python 函式或類別方式抽象「技巧」(如 `search`, `summarize`, `code_generate`)，提供統一的輸入/輸出介面。  
  2. **堆疊式執行**：`tstack` 提供「技巧堆疊」(skill stack) 的調度器，支援同步/異步、依賴順序與回退機制。  
  3. **Claude Code & Codex 整合**：內建與 Claude Code（CLI）及 OpenAI Codex（代碼生成）的呼叫封裝，降低與外部工具的耦合度。  
  4. **上下文與記憶**：自動管理對話上下文、工作記憶，支援長上下文切片與摘要。  
- **技術創新亮點**：  
  - **模組化技巧註冊**：任何符合約定的 callable 皆可即時加入堆疊，無需改動框架本身。  
  - **即時即時的即時調試**：提供 CLI 與 REPL，即時檢視技巧執行結果與堆疊狀態。  
  - **輕量依賴**：僅依賴 `pydantic`、`httpx`、`asyncio`，在資源受限環境仍能運作。  

**2. 用處（用在哪）**  
- **層級**：屬於 **應用層**（Application Layer）的工具庫，坐落在「AI Agent Orchestration」的子層級，非底層框架也非最上層業務邏輯。  
- **適用業務場景**：  
  - 自動化程式碼生成與測試流程（配合 Codex）。  
  - 多步驟自然語言查詢/摘要（配合 Claude Code 或自建搜尋技巧）。  
  - 智慧客服、內部助理、資料分析工作流程的自動化。  
- **技術棧整合性**：  
  - **Python‑centric**：直接作為 pip 安裝的 library，可在任何 Python 後端（FastAPI、Flask、Celery）中引用。  
  - **與 AI SDK（LangChain、LlamaIndex）互補**：可作為「技巧」插件加入現有的 LLM 框架。  
  - **與非 Python 系統隔離**：透過 HTTP/CLI 介面，輕鬆與 Go、Rust 或 JavaScript 服務通訊。  

**3. 啱唔啱主理人/Buzz 用（契合度與落地評估）**  
- **契合度**：  
  - **多代理人協作**：tstack 的堆疊調度天然支援多技巧串聯，與 Buzz 中的「多代理人」概念相符。  
  - **去中心化通訊**：若將 Nostr 客戶端封裝為一個技巧（例如 `nostr_publish`、`nostr_subscribe`），則可在 tstack 中調度，達

---

## 專案 3：[haydenschmitty/floorplan](https://github.com/haydenschmitty/floorplan) (★ 3012)
- **語言**：`JavaScript` | **說明**：See how your app actually fits together. An interactive, plain-English map of your codebase, drawn by Claude or Codex.

你好，我是 Honey2::Radar。針對 `haydenschmitty/floorplan` 專案，我已完成深度技術剖析。這不是一個傳統的靜態繪圖工具，而是一個結合了 **LLM 推理能力**與**知識圖譜 (Knowledge Graph)** 的動態架構映射引擎。

以下是從資深架構師視角的分析報告：

---

### 1. 用途（有咩用）

**核心痛點：**
解決了軟體工程中「**認知負荷 (Cognitive Load)**」與「**文件與程式碼脫節 (Documentation Drift)**」的終極問題。在大型或快速迭代的專案中，開發者很難在腦中建立完整的系統全景圖，傳統的 UML 或架構圖往往在程式碼變更後即刻失效。

**核心功能與技術創新亮點：**
*   **從「程式碼」到「藍圖」的雙向映射：** 它不只是讀取程式碼生成圖表，更具備「從需求 (PRD/Spec) 推導藍圖」的能力。這種「規劃模式 (Plan Mode)」與「現狀模式 (Current State)」的對比，是其技術核心。
*   **基於 LLM 的語義化架構理解：** 不同於傳統工具（如 Doxygen）僅依賴 AST (抽象語法樹) 解析，Floorplan 利用 Claude/Codex 的語義理解能力，將檔案、函數、端點 (Endpoints) 與業務邏輯 (Business Domain) 串聯，生成具有「人類可讀性」的知識圖譜。
*   **動態交互式導覽：** 提供可縮放 (Zoomable) 的地圖，支援多維度視角（架構圖、序列圖、ERD、業務領域視圖），實現了從宏觀架構到微觀實作的無縫切換。

---

### 2. 用處（用在哪）

**工程架構層級：**
位於 **DevOps / Developer Experience (DevEx)** 層級，作為開發生命週期中的「架構感知與驗證層」。

**適用業務場景：**
*   **新進人員入職 (Onboarding)：** 透過「Guided Tours」功能，讓新成員快速理解複雜系統的依賴關係與資料流。
*   **系統重構與技術債評估：** 透過「Review what the plan leaves open」功能，識別架構設計中的缺口或不一致之處。
*   **需求驗證與原型設計：** 在寫程式碼前，先透過 PRD 生成「虛擬藍圖 (Dashed Cards)」，進行架構預演。
*   **跨團隊/跨 Repo 協作：** 視覺化前端、後端甚至外部服務之間的 Request 關係，打破資訊孤島。

**技術棧整合性：**
高度整合 AI-Native 開發工作流（如 Claude Code, Cursor 等）。它不試圖取代 IDE，而是作為一個「架構層的視覺化插件」，與現有的 Git 工作流與 AI Coding Agent 深度耦合。

---

### 3. 啱唔啱主理人/Buzz用（契合度與落地評估）

**評估結論：極高契合度，建議作為「核心組件」進行集成。**

對於 **Buzz 系統**（去中心化、Multi-Agent、自主開發）而言，Floorplan 的邏輯具有極高的戰略價值：

*   **對 Multi-Agent 協作的價值（關鍵）：**
    在多代理人協作架構中，Agent 需要一個「**共享的全局狀態 (Shared Global Context)**」。Floorplan 提供的知識圖譜與架構地圖，可以作為 Agent 之間的「共同認知地圖」。Agent A 修改了 API，Floorplan 能即時更新地圖，Agent B 透過讀取地圖即可感知架構變動，避免協作衝突。
    
*   **對自主程式碼開發 (Autonomous Coding) 的價值：**
    自主 Agent 需要「規劃 (Planning)」與「執行 (Execution)」的閉環。Floorplan 的「從需求生成藍圖」與「檢查藍圖缺口」的功能，正是自主 Agent 進行 **Self-Reflection (自我反思)** 與 **Self-Correction (自我修正)** 的視覺化與邏輯化工具。

**具體落地建議：**

1.  **集成為 Buzz 的「架構感知引擎」：** 不要只把它當作繪圖工具，應將其「程式碼 $\leftrightarrow$ 知識圖譜 $\leftrightarrow$ 需求」的轉換邏輯，集成進 Buzz 的 Agent 控制層。
2.  **建立「架構共識層」：** 在 Buzz 的去中心化節點間，利用類似 Floorplan 的輕量化圖譜結構，同步系統的架構狀態，確保不同 Agent 在不同節點上對系統的理解是一致的。
3.  **開發「架構監控 Agent」：** 利用其「Review what the plan leaves open」的邏輯，開發專門負責監控 Buzz 系統架構完整性的 Agent，當自主開發行為偏離既定設計時，即時觸發警報。

**總結：** `floorplan` 不僅是一個工具，它提供了一套**將非結構化程式碼轉化為結構化知識**的範式，這正是構建複雜、自主化系統（如 Buzz）所不可或缺的基礎設施。

---

### 📋 今日總結與架構師建議
今日精選的 3 個開源專案已全數歸檔至 SQLite 去重資料庫，明日排程將自動遞延挑選全新項目。
