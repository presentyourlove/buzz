---
name: obsidian-digest-sync
description: >
  自動化對話摘要提取、標籤分類與 Obsidian 第二大腦雙向知識庫同步流程。
version: 1
---

# Obsidian 對話摘要與第二大腦同步技能

> **提煉來源**：自近期 6 份對話紀錄與知識庫摘要中高頻提煉 (觸發詞頻: 1987)  
> **狀態**：候選中 (Pending Approval)  
> **規格標準**：Buzz Native Skill v1.0

---

## 🎯 觸發場景 (Trigger Conditions)
- 日常對話結束或定時收割時需要沉澱知識
- 使用者指示將重大決策或技術架構整理至 Obsidian
- 需要維護 02-Conversation-Digest 目錄之 Markdown 結構時

---

## 📋 輸入與前置需求 (Prerequisites)
- 本機已安裝並啟動 sync_obsidian/main.mjs 背景同步守護程式
- 知識庫目錄 C:/私人/ai agent/buzz-obsidian 具備讀寫權限
- 內容必須使用 UTF-8 (no BOM) 編碼與繁體中文

---

## 🛠️ 標準作業程序 (Standard Operating Procedure)
1. 檢查今日對話紀錄並過濾純機器狀態報告雜訊
2. 依據時間戳與主題進行歸納，產生符合 YYYY-MM-DD_主題.md 之摘要檔
3. 自動添加 Frontmatter 標籤 (tags: [digest, buzz-memory])
4. 透過 sync_obsidian 寫入 Obsidian Vault 並更新 INDEX.md 索引表
5. 向頻道回報沉澱筆記之相對路徑與核心提要

---

## 🔒 安全與授權約束 (Security & Authorization Constraints)
- 不得覆蓋已存在之歷史封存筆記
- 禁止將敏感金鑰或 Token 寫入公開筆記中
- 嚴格遵循 Buzz 代理人通訊紀律與老闆審批關卡 (Boss Approval Gatekeeper)
- 禁止私自執行未授權的高風險系統命令
