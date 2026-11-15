---
title: "根本原因分析報告：近期系統異常與防護策略 (RCA Report)"
tags: [rca, root-cause-analysis, stability, error-handling]
status: active
created: 2026-10-29
author: Antigravity
---

# 根本原因分析報告：近期系統異常與防護策略 (RCA Report)

本報告針對團隊近期於 Buzz 開發與運行環境中遭遇之關鍵錯誤與瓶頸進行根本原因分析 (RCA)，並提供長期架構防護與緩解方案。

---

## 異常案例一：OpenRouter LLM 429 請求配額耗盡 (Rate Limit Exceeded)

### 1. 問題現象
```json
Turn error · error: Agent reported error (code -32000): llm: (nvidia/nemotron-3-ultra-550b-a55b:free) rate limited: 
{"error":{"message":"Rate limit exceeded: free-models-per-day. Add 10 credits to unlock 1000 free model requests per day","code":429}}
```
Agent 在執行長時間調研或連續對話時中斷，回報 429 錯誤。

### 2. 根本原因 (Root Cause)
1. **免費額度階層限制 (Tier Limitation)**：OpenRouter 對於免費模型 (`:free`) 的日調用額度分級：未儲值帳號每日上限僅 50 次；帳號內儲值達 $10 USD 者解鎖每日 1,000 次免費請求。
2. **Key 1 配額撞頂**：Key 1 當日調用已達 1,006 次，超出 1,000 次/天上限。
3. **Key 2 帳號未升級**：切換至新帳號 Key 2 後，由於該新帳號尚未充值 $10，觸發 50 次/天的超低門檻而再度撞頂。
4. **單點金鑰依賴**：系統先前未配置自動金鑰健康輪詢機制。

### 3. 解決方案與防護措施
- **金鑰容災輪詢守護神 (Key Failover Daemon)**：已部署守護程式 `tools/openrouter_key_manager.mjs`，監控並在 429 時自動切換活躍金鑰。
- **帳號雙池策略 (Dual-Account Pool)**：兩組帳號皆儲值 $10 USD，解鎖每日 2,000 次 Nemotron 550B 免費請求池。
- **重試退避機制**：在 composite-mcp 層實作指數退避重試（Exponential Backoff with Jitter）。

---

## 異常案例二：Buzz CLI 標註預檢失敗 (Mention Preflight Failure)

### 1. 問題現象
```json
{"error":"error","message":"could not load channel membership for mention preflight","retryable":false}
```
呼叫 `buzz messages send` 標註 `@可愛小寶寶` 或 `@Sentry` 時指令執行失敗。

### 2. 根本原因 (Root Cause)
- Buzz CLI 在發送包含 `@mention` 的訊息時，會進行前置權限檢查 (Mention Preflight)，依賴頻道成員名單與身分認證。
- 若環境變數缺少 `BUZZ_AUTH_TAG`（含有身分公鑰與簽章認證的 Nostr Tag 陣列），CLI 無法向 Relay 驗證發文者身分，導致預檢中斷。

### 3. 解決方案與防護措施
- **環境變數標準化注入**：在所有腳本（`trigger_windows.mjs`、`daily_evolution.py`）中強制配置預設的 `BUZZ_AUTH_TAG`。
- **公鑰對齊**：在提及特定 Agent 時優先使用正確公鑰前綴，並配合 `BUZZ_AUTH_TAG` 通過預檢。

---

## 異常案例三：Windows 環境編碼與反斜線轉義問題

### 1. 問題現象
- Sentry 驗收報告輸出包含 `\n\n` 字樣，排版中斷。
- Python 子進程傳輸中文字串時出現 CP950 編碼異常。

### 2. 根本原因 (Root Cause)
- Windows 預設終端代碼頁非 UTF-8（多為 CP950），若無明確宣告 `encoding="utf-8"`，管道 stdin/stdout 會以本地編碼解碼，導致多字節中文字元截斷。
- JSON 序列化過程中換行符號被雙重轉義（`\\n`）。

### 3. 解決方案與防護措施
- **環境變數強制宣告**：注入 `PYTHONIOENCODING=utf-8` 與 `PYTHONUTF8=1`。
- **進程參數明確化**：所有 `subprocess.run()` 一律明確帶入 `encoding="utf-8"`。

