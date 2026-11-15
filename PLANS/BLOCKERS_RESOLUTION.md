---
title: "系統關鍵阻礙排解與防護方案 (Blockers Resolution Guide)"
tags: [blockers, troubleshooting, resolution, devops]
status: active
created: 2026-10-29
---

# 系統關鍵阻礙排解與防護方案 (Blockers Resolution Guide)

本文件匯整團隊在各開發階段所遭遇之關鍵阻礙與技術瓶頸，並提供標準化、可執行的排解方案。

---

## 阻礙一：OpenRouter LLM 429 配額超限

- **嚴重度**：高 (High)
- **影響範圍**：Honey2、Sentry、Pollen 等使用 OpenRouter API 之 Agent 運作。
- **標準排解 SOP**：
  1. 檢查後台金鑰管理守護程式狀態：`node "tools/openrouter_key_manager.mjs" --status`。
  2. 若兩組金鑰皆已耗盡，確認帳號是否已充值 $10 USD 以解鎖 1,000 requests/day 免費額度。
  3. 執行手動切換金鑰：`node "tools/openrouter_key_manager.mjs" --rotate`。

---

## 阻礙二：Buzz CLI 標註預檢 (Mention Preflight) 失敗

- **嚴重度**：中 (Medium)
- **影響範圍**：發布包含 `@` 提及特定 Agent 的頻道訊息時中斷。
- **標準排解 SOP**：
  1. 確保指令執行環境已正確注入 `BUZZ_AUTH_TAG` 環境變數。
  2. 檢查 `BUZZ_PRIVATE_KEY` 是否具有該頻道之成員權限。
  3. 優先使用完整公鑰或正確對齊之 Display Name，避免因空格截斷。

---

## 阻礙三：Windows 跨平台路徑與 UTF-8 編碼異常

- **嚴重度**：中 (Medium)
- **影響範圍**：收割腳本路徑解析失敗、中文檔案名稱亂碼、標準輸入解碼失敗。
- **標準排解 SOP**：
  1. 腳本內部禁止寫死硬編碼路徑分隔符 `\`，一律使用 Node `path.join()` / Python `Path()`。
  2. 執行命令前確保環境變數設定 `PYTHONIOENCODING=utf-8` 與 `PYTHONUTF8=1`。
  3. 子進程調用明確指定 `encoding="utf-8"`。

