---
title: "5 項 GitHub 開源專案技術參考與整合實作計畫書"
tags: [github-integration, tech-reference, implementation-roadmap, sentry-review]
status: pending-sentry-review
created: 2026-11-02
author: "Honey2 (專案架構企劃官)"
target_channel: "5ca019f6-1a27-592a-b015-4baf0eb33572"
reviewer: "Sentry (安全與架構總監)"
implementer: "Pollen (執行工程師)"
approver: "可愛小寶寶 (老闆)"
source_report: "PLANS/REPORTS/2026-11-01_GITHUB_TRENDING.md"
projects_analyzed:
  - openinterpreter/open-interpreter
  - microsoft/OmniParser
  - simular-ai/Agent-S
  - bytedance/UI-TARS-desktop
  - OpenAdaptAI/OpenAdapt
---

# 5 項 GitHub 開源專案技術參考與整合實作計畫書

> **來源**：Honey2::Radar 專題深度研報 (2026-11-01_GITHUB_TRENDING.md)
> **觸發指示**：主理人於 2026-11-02 晚間 6 點交辦整理 5 個 GitHub 專案
> **研報路徑**：`C:\Users\user\.buzz\PLANS\REPORTS\2026-11-01_GITHUB_TRENDING.md`

---

## 一、專案背景與目標

### 1.1 老闆指示
主理人於 2026-11-02 晚間下達指示：
> 「我11/2晚上6點嘅時候叫咗honey2去咗整理5個github嘅專案。佢分析咗有部分內容我哋可以參考。哩個未有計劃書。」

### 1.2 五大專案綜合評估矩陣 (來源：研報第 507-513 行)

| 維度 | Open Interpreter | OmniParser | Agent-S | UI-TARS Desktop | OpenAdapt |
|------|------------------|------------|---------|-----------------|-----------|
| **核心定位** | Model-Agnostic Agent Harness Platform | Pure Vision Perception Engine | Cognitive Architecture (Memory/Evolution) | Product-Grade Agent Desktop App | Verified Execution Layer |
| **關鍵創新** | Harness Abstraction + ACP/MCP Native | YOLOv9-E + Florence-2 Two-Stage Parsing | Narrative Memory + Interface Evolution | Event Stream + MCP Kernel + Local/Remote Operator | Explore-Compile-Run-Repair + Read-back Verification |
| **技術成熟度** | ★★★★★ (生產就緒) | ★★★★☆ (核心感知 SOTA) | ★★★★★ (學術 SOTA、商業化驗證) | ★★★★★ (產品級、用戶體驗標杆) | ★★★★☆ (核心邏輯證明) |
| **對 Buzz 戰略價值** | **架構標杆**：Harness、協議、技能標準 | **基建組件**：視覺感知標準工具 | **認知藍圖**：記憶、路由、技能合成 | **產品參考**：UX、Event Stream、雙模運算 | **信任基石**：驗證、確定性、合規 |
| **建議採納優先級** | **P0 (立即對標架構)** | **P0 (集成視覺工具鏈)** | **P1 (重構記憶與路由)** | **P1 (重構 Agent 面板 UX)** | **P0 (引入寫入驗證門檻)** |

### 1.3 三大技術攻關方向 (來源：研報第 515-534 行)

基於五大專案綜合研判，Buzz 下一階段三大技術攻關方向：

1. **方向一：Harness 抽象化與多模型統一運行時** (參考 Open Interpreter)
2. **方向二：可信執行環境：感知-決策-驗證閉環** (參考 OmniParser + OpenAdapt + Agent-S)
3. **方向三：產品級 Agent 桌面體驗與遠程協作** (參考 UI-TARS Desktop)

---

## 二、分專案整合規劃

### 2.1 專案 1：Open Interpreter - Harness 抽象層與多模型統一運行時 (P0)

#### 核心價值對齊
- Buzz 目前 `buzz-agent` 綁定特定推理邏輯，需引入 **Harness Abstraction Layer**
- 實現**同一套 Agent 核心，支援任意模型後端** (OpenAI/Anthropic/DeepSeek/Kimi/GLM/Qwen/本地 GGUF)
- 原生支援 **ACP (Agent Client Protocol)** + **MCP (Model Context Protocol)** 雙軌協議

#### 具體落地任務

| 任務代號 | 任務名稱 | 交付物 | 優先級 | 預估工期 |
|---------|---------|--------|--------|----------|
| **OI-1** | `BuzzHarness` Trait 定義 | `buzz/agent/harness/trait.rs` - 定義 `Harness`、`HarnessRegistry`、`ModelBackend` 抽象介面 | P0 | 1 天 |
| **OI-2** | Harness 註冊表實作 | `buzz/agent/harness/registry.rs` - 支援熱插拔：`NativeHarness`、`ClaudeCodeHarness`、`KimiHarness`、`LocalLLMHarness` | P0 | 1 天 |
| **OI-3** | ACP Server 內建 | `buzz/agent/acp/server.rs` - 使 Buzz Agent 可被任何 ACP Client (VS Code, Zed, Cursor) 直接調用 | P0 | 1.5 天 |
| **OI-4** | `.agents/skills` 目錄標準化 | 遷移既有技能系統至 Open Interpreter 推動的共享技能目錄標準 | P0 | 0.5 天 |
| **OI-5** | MCP Client 完善 | 完善 MCP Client 支援，接入生態工具 (參考 Open Interpreter MCP 整合) | P0 | 1 天 |
| **OI-6** | 本地優先模式 | 實現 `LocalHarness` 支援 GGUF/llama.cpp 本地推理，零外網依賴 | P1 | 1 天 |

#### 風險與防範
- **風險**：現有 `buzz-agent` 架構耦合度高，重構範圍大
- **防範**：採用 Strangler Fig 模式漸進式重構，新舊 Harness 並行運行，灰度切換

---

### 2.2 專案 2：OmniParser - 純視覺感知引擎集成 (P0)

#### 核心價值對齊
- Buzz Agent 缺乏「看懂螢幕」能力，特別是遠程桌面/RDP/Citrix 無 Accessibility API 場景
- OmniParser 提供**跨平台、跨應用、跨系統**的純視覺定位能力 (Windows/macOS/Linux/Android/Web)

#### 具體落地任務

| 任務代號 | 任務名稱 | 交付物 | 優先級 | 預估工期 |
|---------|---------|--------|--------|----------|
| **OP-1** | `VisionPerceptionTool` 封裝 | `buzz/agent/tools/vision_perception.py` - 封裝 OmniParser 推理：`parse_screen(screenshot_bytes) -> Vec<UIElement>` | P0 | 1 天 |
| **OP-2** | 模型權重部署 | 整合 Hugging Face `microsoft/OmniParser-v2.0` (YOLOv9-E + Florence-2)，支援本地 GPU/CPU 推理 | P0 | 0.5 天 |
| **OP-3** | 混合感知策略實作 | `buzz/agent/perception/hybrid.rs` - 優先級：Platform Native API > OmniParser > Raw VLM Grounding | P0 | 1 天 |
| **OP-4** | 遠程桌面感知適配 | 驗證 RDP/Citrix/遠程桌面場景下純視覺定位可用性，作為唯一可行方案 | P0 | 0.5 天 |
| **OP-5** | 訓練數據飛輪 | 整合 OmniTool 自動生成「螢幕→元素→動作」軌跡數據，微調 Buzz 專用小型視覺定位模型 (Qwen2.5-VL-3B/7B LoRA) | P1 | 2 天 |

#### 風險與防範
- **風險**：模型推理延遲較高 (YOLOv9-E + Florence-2 雙模型)
- **防範**：支援模型量化 (INT4/INT8)、批次推理優化、快取機制；提供降級策略

---

### 2.3 專案 3：Agent-S - 認知架構：記憶、路由、技能合成 (P1)

#### 核心價值對齊
- Buzz 目前 Agent 記憶較簡單 (上下文窗口 + 簡單 KV)，需引入 **雙層記憶架構**
- Multi-Agent 本質是 Generalist + Specialists，需正式定義 **Agent Router** 動態路由機制
- Agent S4 的 **Interface Evolution** 直接對應 Buzz 的 **Skill Synthesis** 願景

#### 具體落地任務

| 任務代號 | 任務名稱 | 交付物 | 優先級 | 預估工期 |
|---------|---------|--------|--------|----------|
| **AS-1** | `NarrativeMemory` 敘事長期記憶 | `buzz/agent/memory/narrative.py` - 將完成任務抽象為「情境-行動-結果」三元組，存入向量庫 (Chroma/FAISS) | P1 | 1.5 天 |
| **AS-2** | `ExperienceRetriever` 經驗檢索器 | 新任務啟動時，檢索相似敘事作為 Few-shot/RAG Context 注入 | P1 | 1 天 |
| **AS-3** | `AgentRouter` 動態路由機制 | 根據任務嵌入向量、歷史成功率、成本估算，動態路由至最合適的 Specialist Agent | P1 | 1.5 天 |
| **AS-4** | `SkillSynthesizer` 技能合成管線 | Agent 完成任務後：分析軌跡 → 識別高頻模式 → 生成新 Skill → 驗證 → 註冊技能庫 | P1 | 2 天 |
| **AS-5** | 評估體系建設 | 引入 OSWorld / WindowsAgentArena / AndroidWorld 作為標準化回歸測試基準 | P1 | 1 天 |

#### 風險與防範
- **風險**：記憶系統引入向量資料庫增加部署複雜度
- **防範**：支援嵌入式向量庫 (sqlite-vec) 作為輕量選項，生產環境可升級 Chroma

---

### 2.4 專案 4：UI-TARS Desktop - 產品級 Agent 桌面體驗與遠程協作 (P1)

#### 核心價值對齊
- Buzz Desktop 內建 Agent 面板需對標 UI-TARS Desktop 水平
- **Event Stream Architecture** 可支援前端即時渲染、時光機回溯、人工介入分支
- **Local/Remote Operator 雙模** 將 Nostr 遠程協作優勢具體化：Agent 可跨設備操作

#### 具體落地任務

| 任務代號 | 任務名稱 | 交付物 | 優先級 | 預估工期 |
|---------|---------|--------|--------|----------|
| **UT-1** | `AgentEventStream` 不可變事件日誌 | `buzz/agent/events/stream.rs` - `ThoughtEvent`、`ToolCallEvent`、`ObservationEvent`、`ScreenshotEvent`、`ModelOutputEvent` | P1 | 1.5 天 |
| **UT-2** | 前端 Agent 面板重構 | Buzz Desktop React 面板：事件流即時渲染、時光機回溯、分支視覺化、人工介入修正 | P1 | 2 天 |
| **UT-3** | `RemoteComputerOperator` 遠程運算子 | 被控端 `buzz-agent-daemon` (WebRTC/NATS/HTTP) 提供螢幕流、鼠鍵注入、文件系統訪問 | P1 | 2 天 |
| **UT-4** | `MCPKernel` 工具即 MCP Server 架構 | 內建工具 (文件、終端、Git、Nostr) 即內建 MCP Server；外部工具掛載標準 MCP Server | P1 | 1.5 天 |
| **UT-5** | `AIO Sandbox` 安全隔離執行環境 | 引入 `agent-infra/sandbox`：Shell 流式、多文件結構化、網絡隔離、資源限制 | P1 | 1 天 |
| **UT-6** | UI-TARS 模型集成 | 整合 `ByteDance-Seed/UI-TARS-1.5-7B` 作為本地視覺定位模型首選權重 | P1 | 1 天 |

#### 風險與防範
- **風險**：WebRTC/NAT 穿透在企業內網環境複雜
- **防範**：支援 TURN Server、NATS JetStream 作為傳輸備選；提供純 HTTP 輪詢降級模式

---

### 2.5 專案 5：OpenAdapt - 可信執行驗證層：讀回驗證與確定性重放 (P0)

#### 核心價值對齊
- 解決「Agent 寫入不可信」核心信任危機：**讀回驗證**實證壞保存 30/30 全部攔截，零誤報
- **Explore-Compile-Run-Repair** 工作流：將非確定性 Agent 行為轉化為確定性、零成本、可驗證的程序行為
- 適用於高風險寫入場景：資料庫、API POST/PUT、文件保存、區塊鏈交易

#### 具體落地任務

| 任務代號 | 任務名稱 | 交付物 | 優先級 | 預估工期 |
|---------|---------|--------|--------|----------|
| **OA-1** | `VerifiedWrite` Trait 強制門檻 | `buzz/agent/traits/verified_write.rs` - `pre_check()`、`execute()`、`post_verify()` (讀回驗證)、`outcome` 枚舉 | P0 | 1 天 |
| **OA-2** | `TransactionOutcome` 枚舉 | `VERIFIED` / `HALTED_BEFORE_EFFECT` / `RECONCILIATION_REQUIRED` / `FAILED` | P0 | 0.5 天 |
| **OA-3** | `Explore-Compile-Run-Repair` 工作流引擎 | `buzz/agent/workflow/openadapt_engine.py` - 優先檢查已編譯程序庫，有則 `Run` (零模型成本) | P0 | 1.5 天 |
| **OA-4** | 多表面統一適配器 | `SurfaceAdapter` 抽象：`WebSurface`、`WindowsSurface`、`MacOSSurface`、`LinuxSurface`、`RDPSurface` | P0 | 1.5 天 |
| **OA-5** | MCP Tool 化部署 | 將編譯後工作流作為標準 MCP Server 部署：`openadapt://workflow/...` | P0 | 1 天 |
| **OA-6** | Reward Model 整合 | `openadapt-flow serve-reward` 將讀回驗證轉化為獎勵信號，用於 Agent 訓練 (RLHF/RLAIF) | P1 | 1 天 |

#### 風險與防範
- **風險**：讀回驗證增加寫入操作延遲 (需額外查詢/讀取)
- **防範**：非關鍵寫入可選擇性啟用；關鍵寫入 (資料庫、鏈上、醫療/金融) 強制啟用

---

## 三、整合依賴關係與實施路線圖

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Buzz 技術演進整合路線圖 (2026 Q4 - 2027 Q1)                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Phase 0: 基礎設施就緒 (Week 1-2)                                          │
│  
