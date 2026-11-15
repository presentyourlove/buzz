---
title: "Phase 3 RAG 增強 Agent 上下文 - 實施設計規格書"
tags: [architecture, rag, desktop, hook, embedding]
status: active
created: 2026-10-25
---

# Phase 3 RAG 增強 Agent 上下文 - 實施設計規格書

## 1. 核心目標
在 Desktop 端攔截 Agent 的 transcript 與 prompt 組裝過程，自動檢索向量庫中的 Top-K 相關上下文片段，並動態注入 system_prompt 中，實現零人工干預的智慧上下文增強。

## 2. 模組設計
- desktop/src/features/agents/useAgentContextEnrichment.ts: 負責解析當前 Prompt，呼叫本地向量檢索介面，並合成增強後的 system_prompt。
- 權限與 Token 隔離：僅檢索當前 Community 與 Channel 範圍內的向量資料，並限制最大 Token 預算（2000 tokens）。
