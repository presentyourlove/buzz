---
title: "Buzz 常駐代理工作流與安全審核規範（草案）"
tags: [always-on-agents, workflow, security, governance, buzz]
status: draft
created: 2026-11-15
---

# Buzz 常駐代理工作流與安全審核規範（草案）

## 1. 概述
本規範依據 Anthropic《Building always-on agents》設計指南之核心精神，結合 Buzz 平台特性（Nostr 協議、線程隔離與權限審核機制），為 Buzz 系統中的常駐代理（Always-on Agents）確立應用層的業務規則、事件觸發條件、溝通分寸及安全審核邊界。

## 2. 收件匣與事件觸發規則 (Inbox & Triggers)
- **事件過濾**：代理僅透過 Nostr 事件串流接收專屬提及或指派工單，嚴禁無差別廣播響應。
- **單一執行緒 (Single-threaded Execution)**：同一時間對同一工作單僅允許一個主動代理進程運行，避免並發寫入衝突。

## 3. 寄件匣與溝通分寸守則 (Outbox & Boundaries)
- **拒絕無意義回覆**：嚴禁發布「收到」、「好」、「了解」等單純確認性質的 bare acknowledgement，減少討論串洗版。
- **有結論才發話**：代理在完成具體分析、實作或遭遇阻礙需要求助時，必須透過 `--reply-to` 準確回覆至對應討論串。

## 4. 權限與安全審核邊界 (Permissions & Governance)
- **Owner-reviewed 審核機制**：涉及系統敏感修改、代理 drafts 建立或高風險檔案寫入時，必須通過主理人（Owner）的簽署審核。
- **沙盒隔離**：所有高風險或第三方代工模組，必須於獨立沙盒環境（如 `.scratch/subagents/`）中運作，嚴禁直接污染核心代碼庫。
