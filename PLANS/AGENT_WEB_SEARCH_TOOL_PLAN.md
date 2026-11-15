---
title: "Agent 外部 Web 搜尋與網頁擷取工具整合實施計畫書 (升級版)"
tags: [agent, tool, web-search, jina-reader, tavily, duckduckgo, mcp]
status: completed
version: 2.1
author: "Buzz Architecture Team"
created: 2026-10-27
updated: 2026-10-27
---

# Agent 外部 Web 搜尋與網頁擷取工具整合實施計畫書 (v2.1 全數竣工)

## 一、任務概述

本計畫書旨在為 Buzz 平台上的 AI 代理（Honey, Pollen, Fizz）導入即時網頁搜尋（Web Search）與網頁內容擷取（Web Scraper）能力。
核心目標是在**不破壞現有 `buzz-dev-mcp` 代碼編輯與本機 Shell 工具鏈**的前提下，提供「零成本免金鑰預設方案」與「高品質專業 AI 搜尋方案」的雙軌架構，並建立嚴格的 Context 防爆截斷機制，徹底杜絕 429 與 Context Overflow。

---

## 二、架構分析與關鍵問題解決

### 1. 解決 MCP 單一指令覆蓋危機 (Multi-MCP Coexistence)
* **現狀盲點**：Buzz 的 `managed-agents.json` 中每個 Agent 僅支援單一 `mcp_command` 欄位（原本為 `buzz-dev-mcp`）。若直接將其置換為獨立的 Web MCP，Agent 將失去 `shell`、`read_file`、`str_replace` 等核心能力。
* **解決架構**：
  * **階段一（CLI 輕量模式）**：不變更 `mcp_command`，透過現成的 `buzz-dev-mcp` 內建 `shell` 工具，直接呼叫本機專屬 CLI 搜尋腳本。
  * **階段二（MCP 聚合代理模式）**：已實作完成！透過 Win32 原生包裝器 `buzz-composite-mcp.exe`（繼承句柄無縫轉發 Stdio），串接 `buzz-composite-mcp.mjs`，同時掛載原生 `buzz-dev-mcp`（7 個代碼工具）與 Web 工具（`web_search`、`web_fetch`），對外統一暴露 9 大原生工具。

### 2. 雙軌搜尋與擷取引擎 (Dual-Engine Fallback)
* **自由瀏覽／常規搜尋**：採用 **Tavily API**（已注入 `TAVILY_API_KEY`）作為首選，支援 DuckDuckGo 自動回退。
* **網頁讀取 (Web Reader)**：接入 **Jina Reader API** (`https://r.jina.ai/<URL>`)，免金鑰、抗 Cloudflare、自動濾除導航廣告，轉為乾淨 Markdown。
* **專題爬蟲 (Topic Scraping)**：維持 **Apify Actors** 不變，專職精準抓取推特、Reddit 等社群垂直內容。

### 3. 三層防禦性工程規範 (Defense-in-Depth)
1. **Context 截斷防護（防 429 惡夢）**：
   * 搜尋結果預設僅保留前 5 筆，每筆限制 200 字摘要。
   * 網頁全文抓取設有硬上限：**單篇最多 15,000 字元（約 3,000 Tokens）**，超出部分自動截斷並附註提示。
2. **格式與防吞字規範**：
   * 網址與搜尋關鍵字一律採用反引號（` `）包裹，嚴禁使用角括號（`< >`），杜絕 Markdown 標籤被 UI 吞噬。
3. **超時與彈性容錯**：
   * 搜尋請求 10 秒逾時、爬蟲請求 20 秒逾時；若存取失敗回傳簡明錯誤碼，絕不阻塞 Agent 主迴圈。

---

## 三、實施成果 (Implementation Summary)

* `C:\私人\tools\buzz-composite-mcp.exe`：Win32 原生可執行檔（C# P/Invoke 透過 `csc.exe` 編譯），解決 Windows Rust `Command::new` 對 `.bat` 返回 193/EINVAL 異常。
* `C:\私人\tools\buzz-composite-mcp.mjs`：Stdio JSON-RPC 2.0 聚合代理，將 `buzz-dev-mcp` 的 7 大核心工具與 `web_search`、`web_fetch` 合併輸出。
* `managed-agents.json`：全數 15 筆 Agent 記錄之 `mcp_command` 已切換為 `C:\私人\tools\buzz-composite-mcp.exe`。
* `global-agent-config.json`：已注入 `TAVILY_API_KEY`、`OPENROUTER_API_KEY` 與 Thought Proxy 端點。

---

## 四、驗收標準 (Definition of Done)

1. [x] **基本搜尋測試**：Agent 透過 `web_search` 或 CLI `web-search.mjs`，能於 3 秒內返回有效 Tavily 搜尋摘要。
2. [x] **爬蟲閱讀測試**：Agent 透過 `web_fetch` 或 CLI `web-fetch.mjs`，能透過 Jina Reader 獲取乾淨 Markdown 內文（15,000 字元截斷）。
3. [x] **代碼能力不中斷**：確認在具備搜尋能力後，Agent 原有的 `read_file`、`str_replace`、`shell`、`todo` 等 7 大開發工具運作如常。
4. [x] **防吞字合規**：所有輸出結果均無角括號造成的吞字現象。
5. [x] **原生 MCP 聚合驗收**：`buzz-composite-mcp.exe` 經測試 `tools/list` 正常返回 9 大工具，`tools/call web_search` 實測成功。
