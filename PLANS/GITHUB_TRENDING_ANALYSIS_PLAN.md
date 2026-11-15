---
title: "GitHub Trending 分析與 janestreet/agent_skill 整合實作計畫書"
tags: [github, trending, plan, agent_skill, daily-evolution]
status: approved
created: 2026-11-12
reviewed_at: 2026-11-12T05:30:00+08:00
version: "1.0"
verifier: "Sentry (架構安全官)"
---

# GitHub Trending 分析與 janestreet/agent_skill 整合實作計畫書

> **授權來源**：主理人（可愛小寶寶）親自授權 @Pollen 開工
> **審核狀態**：Sentry 架構完備性與品質審核已通過

## 📋 專案背景

依據 2026-11-11 與 2026-11-12 每日 GitHub 開源技術研報分析，識別出 **janestreet/agent_skill** 為高價值整合目標：
- 輕量級 AI 代理框架，模組化插件設計、即時學習、跨平台 Rust 實作、低延遲
- 透過 Nostr 事件流 (Kind:30300) 與 Buzz 討論串同步
- 利用 Buzz SQLite 去重機制持久化任務歷史
- 可作為 Agent 角色直接調用，強化 Buzz 蜂群系統自主能力

## 🎯 實作目標

### 階段一：janestreet/agent_skill 深度整合（優先級：P0）
1. **建立 agent_skill 技能模組** - 在 `skills/agent-skill/` 建立 Rust 技能
2. **Nostr 事件橋接** - 實作 Kind:30300 事件流同步機制
3. **SQLite 任務持久化** - 對接 Buzz 去重資料庫
4. **Agent 角色註冊** - 使 agent_skill 可被 Buzz Agent 系統直接調用

### 階段二：每日系統進化自動化（優先級：P1）
1. **完善 daily-digest 技能** - 落實決策提取、Markdown 渲染、Vault 同步
2. **建立自動化排程腳本** - `scripts/03_daily_evolution.py` 完整化
3. **整合 qa-guard / env-guardian** - CI 門禁與環境健康檢查自動化
4. **Obsidian 雙向同步** - `obsidian-digest-sync` 生產級部署

### 階段三：趨勢雷達持續優化（優先級：P2）
1. **Honey2::Radar 子代理強化** - 多源聚合、去重精度提升
2. **技術評估報告模板化** - 標準化輸出格式供架構師決策
3. **知識圖譜建構** - 專案關聯分析與技術棧映射

## 📦 交付物清單

| 路徑 | 類型 | 說明 |
|------|------|------|
| `skills/agent-skill/` | 新增目錄 | janestreet/agent_skill 整合技能（Rust） |
| `skills/agent-skill/Cargo.toml` | 新增 | 依賴：buzz-agent, buzz-core, agent_skill crate |
| `skills/agent-skill/manifest.json` | 新增 | 技能清單：nostr_bridge, task_persistence, agent_runtime |
| `skills/agent-skill/src/nostr_bridge.rs` | 新增 | Kind:30300 事件發布/訂閱 |
| `skills/agent-skill/src/task_store.rs` | 新增 | SQLite 任務歷史持久化 |
| `skills/agent-skill/src/agent_runtime.rs` | 新增 | Agent 執行環境封裝 |
| `skills/agent-skill/main.rs` | 新增 | 技能入口點 |
| `scripts/03_daily_evolution.py` | 修改/完善 | 每日進化自動化主腳本 |
| `scripts/extract_conversation.py` | 新增 | 對話萃取精準度升級 |
| `tests/regression_tests.py` | 新增 | 18 項自動化回歸測試 |

## 🔧 技術架構決策

1. **技能隔離**：agent-skill 以獨立 cdylib 編譯，透過 buzz-agent plugin 系統動態載入
2. **事件流向**：Buzz Channel → Kind:30300 → agent_skill → SQLite → Buzz Thread
3. **配置管理**：manifest.json 定義能力，環境變數注入敏感配置
4. **錯誤處理**：熔斷器模式（參考 env-guardian），優雅降級不拖垮主進程

## 📊 驗收標準 (Definition of Done)

- [ ] `skills/agent-skill/` 完整編譯通過 `cargo build --package buzz-agent`
- [ ] Kind:30300 事件可在 Buzz Desktop 正確顯示與同步
- [ ] 任務歷史可在 SQLite 查詢且去重機制生效
- [ ] 每日進化腳本可無人值守執行並產出報告
- [ ] 所有新增代碼通過 qa-guard 語法與 cargo 檢查
- [ ] Sentry 門禁簽核通過

## ⚠️ 風險評估與緩解

| 風險 | 等級 | 緩解方案 |
|------|------|----------|
| agent_skill 上游 API 變更 | 中 | 釘選版本，建立介面適配層 |
| Nostr 事件衝突 | 低 | 使用獨立 Kind 範圍，命名空間隔離 |
| SQLite 並發寫入 | 中 | WAL 模式 + 連接池，參考 buzz-db 設計 |

## 🚀 開工順序

1. **Step 1**：建立 `skills/agent-skill/` 目錄結構與 Cargo.toml
2. **Step 2**：實作 nostr_bridge.rs（核心橋接）
3. **Step 3**：實作 task_store.rs（持久化）
4. **Step 4**：實作 agent_runtime.rs（執行環境）
5. **Step 5**：整合測試並產出 Git Patch
6. **Step 6**：@Sentry 實測驗收
