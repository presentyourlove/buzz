---
title: "系統待辦事項優先級看板 (Action Items Kanban)"
tags: [kanban, action-items, priorities, agile]
status: active
created: 2026-10-29
last_updated: 2026-10-29
---

# 系統待辦事項優先級看板 (Action Items Kanban)

本看板由每日進化系統自動萃取之待辦事項彙整而成，並依據影響範圍與緊急程度進行動態優先級排序。

---

## 🔴 P0 - 最高優先級 (Urgent & Blocker)

| 項目編號 | 事項描述 | 負責 Agent | 當前狀態 | 備註 / 緩解方案 |
|:---:|---|:---:|:---:|---|
| **P0-1** | OpenRouter 429 請求配額限制容災 | Antigravity / Wax | ✅ 已緩解 | 部署金鑰輪詢守護神 (`openrouter_key_manager.mjs`)，雙帳號池準備 |
| **P0-2** | 團隊溝通紀律「結論先行」落地 | 全體 Agent | ✅ 已完成 | 納入 `GUIDES/COMMUNICATION_DISCIPLINE.md` 與 `AGENTS.md` |
| **P0-3** | 頻道推播純淨化（剔除分析雜訊） | Antigravity | ✅ 已完成 | 實作雙軌制，僅發送精煉建議項目至頻道 |

---

## 🟡 P1 - 高優先級 (Core Engineering & Stability)

| 項目編號 | 事項描述 | 負責 Agent | 當前狀態 | 備註 / 緩解方案 |
|:---:|---|:---:|:---:|---|
| **P1-1** | 每日進化分析腳本單元測試與覆蓋率 | Antigravity / Sentry | ✅ 已完成 | `test_daily_evolution.py` 測試套件 100% 通過 |
| **P1-2** | 產出近期異常之根本原因分析報告 (RCA) | Antigravity | ✅ 已完成 | `RESEARCH/RCA_BUZZ_RECENT_FAILURES.md` 已就緒 |
| **P1-3** | Facebook MCP 模組防死鎖與 Token 缺失優雅降級 | Pollen | ✅ 已修復 | 移除 `process.exit(1)`，未配置 Token 時進入 Standby 模式 |
| **P1-4** | 健全性監控與自癒排程腳本 | Antigravity / Pollen | 進行中 (Phase 3) | 實作 `health_check.py` 與排程自檢 |

---

## 🟢 P2 - 中優先級 (Documentation & Process)

| 項目編號 | 事項描述 | 負責 Agent | 當前狀態 | 備註 / 緩解方案 |
|:---:|---|:---:|:---:|---|
| **P2-1** | 團隊 Agent 職責邊界與分工矩陣確立 | Honey2 / Sentry | 進行中 (Phase 2) | 定稿 `GUIDES/AGENT_RESPONSIBILITY_MATRIX.md` |
| **P2-2** | 待確認決策事項自動追蹤腳本 | Antigravity | 進行中 (Phase 2) | 實作 `scripts/check_pending_decisions.mjs` |
| **P2-3** | 工作日誌與知識庫自動雙向同步 | Antigravity | 進行中 (Phase 2) | 整合至 `trigger_windows.mjs` |

