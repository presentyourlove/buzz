# Multi-Agent Architecture & Engineering Spec (Sprint 1)
# 蜂群多特工協同架構總綱設計規格書（第 1 衝刺階段）

> **編制角色**：Honey2（蜂群首席企劃官）& 後台工程團隊  
> **審查狀態**：APPROVED & INTEGRATED  
> **關聯版本**：Buzz Agent v0.1.0-alpha / Proactive Triggers Phase 1~4  
> **所屬模組**：`PLANS/MULTI_AGENT_SPEC_SPRINT1.md`

---

## 1. 執行摘要 (Executive Summary)

本規格書為 Buzz 蜂群多代理協同架構（Multi-Agent System）Sprint 1 之總綱設計。
系統從單一 Agent 轉型為「企劃官（Honey/Honey2）+ 工程官（Pollen）+ 監控官（Watchdog）」之多軌非同步工作體系。
本期衝刺完成四大核心子領域之完整規格定義：

1. **子規格 A：可視化監控面板（Dashboard Spec）** -> 參見 `PLANS/DESIGN_SPEC_A_DASHBOARD.md`
   - 提供實時特工狀態、任務佇列排程、系統指標與 Nostr 通訊事件視覺化。
2. **子規格 B：三重閘門安全防護（Triple Gate Spec）** -> 參見 `PLANS/DESIGN_SPEC_B_TRIPLE_GATE.md`
   - 建立「代碼靜態稽核 + 實體沙盒隔離 (Git Worktree) + 熔斷器自動防禦 (Circuit Breaker)」三層縱深防禦體系。
3. **子規格 C：特工專屬模組化擴展（Mods Spec）** -> 參見 `PLANS/DESIGN_SPEC_C_MODS.md`
   - 規範特工技能動態載入、MCP 服務掛載與自訂擴展生命週期管理。
4. **子規格 D：smolagents 輕量級多特工技術儲備（smolagents Spec）** -> 參見 `PLANS/DESIGN_SPEC_D_SMOLAGENTS.md`
   - 評估並對接輕量級 CodeAgent / ToolCallingAgent 架構，支援極致性能與低 Token 開銷之特工調度。

---

## 2. 雙軌特工分工與通訊協定 (Agent Division of Labor & Protocol)

| 特工代號 | 核心職能 | 權限範圍 | 產出標準 |
| :--- | :--- | :--- | :--- |
| **Honey / Honey2** | 首席企劃與架構設計官 | `PLANS/`、Markdown 規格書、業務需求分解 | 產出符合 SSDLC 與 ADR 規範之設計文檔，嚴禁越界修改 Rust 代碼庫 |
| **Pollen** | 首席工程與代碼實作官 | `crates/`、`scripts/`、Rust 原始碼 | 透過實體 Git Worktree 沙盒撰寫程式碼，需達成 100% `cargo check` 與單元測試通過 |
| **Watchdog / Guardian** | 蜂群守護與自我修復官 | 心跳監控、熔斷器（Circuit Breaker）、進程解鎖 | 監控特工假死、逾時重試，自動重設或通知主理人介入 |

---

## 3. 三重安全防護閘門 (Triple Gate Integration)

```
[使用者指令] 
     │
     ▼
┌───────────────────────────────────────────────┐
│ 第一道門：語法與靜態結構檢查 (Syntax & Lint Gate)   │
│ - Codebase Verifier / AST 語意審查             │
└──────────────────────┬────────────────────────┘
                       ▼
┌───────────────────────────────────────────────┐
│ 第二道門：實體 Git Worktree 隔離沙盒 (Sandbox Gate)  │
│ - pollen_worktree.py 獨立目錄與 .target 快取   │
│ - 多任務平行執行，主工作區零污染               │
└──────────────────────┬────────────────────────┘
                       ▼
┌───────────────────────────────────────────────┐
│ 第三道門：Circuit Breaker 熔斷器保護 (Defense Gate) │
│ - 連續失敗 5 次自動 Open 阻斷，保護 API 預算  │
│ - 半開探針 (HalfOpen) 探測恢復                 │
└───────────────────────────────────────────────┘
```

---

## 4. 驗收標準與交付清單 (Sprint 1 Acceptance Criteria)

- [x] **Phase 1~3 觸發器核心**：Condition DSL、Event Bus、Runtime 排程器 100% 綠燈。
- [x] **Phase 4 協調器層**：
  - `crates/buzz-agent/src/bin/proactive_trigger_orchestrator.rs` REST API 服務端完成。
  - `crates/buzz-agent/src/triggers/orchestrator/` 包含完整 CRUD、狀態機與 Circuit Breaker 熔斷機制。
  - `cargo test -p buzz-agent --test test_orchestrator` 7/7 通過。
- [x] **Git Worktree 沙盒調度器**：`scripts/pollen_worktree.py` 與 `scripts/pollen_task_queue.py` 整合通過。
- [x] **Sprint 1 規格體系**：A、B、C、D 四大設計規格書與本總綱文件完備就緒。
