---
title: "Apify 社群網頁抓取與自動化記憶實作計畫書"
tags: [apify, scraper, plan, automation]
status: completed
completed_at: 2026-10-26
---

# Apify 社群網頁抓取與自動化記憶實作計畫書

## 1. 專案目標
本計畫旨在建立一套透過 **Apify Actors**（如 Threads / Instagram / Facebook Scraper）自動抓取社群媒體連結內容，並將抓取結果自動轉存至 Buzz 代理人 engram 記憶庫（`buzz mem set`）的自動化整合方案。

---

## 2. 實作架構與路徑
- **腳本存放目錄**：`C:\Users\user\.buzz\.scratch\` 或專屬工具腳本區。
- **核心依賴**：
  - Python `requests` (或 `apify-client` 官方 Python SDK)
  - Buzz CLI (`buzz mem set`)
- **配置變數（待用戶提供後填入）**：
  - `APIFY_API_TOKEN`: 用戶提供的 Apify API 授權金鑰。
  - `ACTOR_ID`: 目標社群平台的 Apify Actor ID 或名稱（例如 `apify/instagram-scraper`, `apify/threads-scraper` 等）。

---

## 3. 具體實作步驟

### 步驟一：環境與依賴確認
1. 確認 Python 環境及必要套件安裝 (`pip install apify-client requests`)。
2. 驗證 Buzz CLI 記憶指令可用性 (`buzz mem set <slug> <value>`)。

### 步驟二：編寫 Apify 呼叫與資料抓取腳本
建立 Python 腳本（例如 `C:\Users\user\.buzz\.scratch\fetch_social.py`）：
- 接收目標網址（URL）。
- 呼叫 Apify API 啟動對應 Actor 進行爬取。
- 擷取回傳的結構化資料（內文、作者、發布時間等）。

### 步驟三：轉存為 Engram 記憶
- 將抓取的內容整理為標準 Markdown 格式。
- 自動生成合適的 slug 名稱（如 `social_post_<timestamp>`）。
- 透過指令執行：
  `buzz mem set <slug> "<content>"`

### 步驟四：驗證與回報
- 測試讀取記憶 (`buzz mem get <slug>`) 確認寫入成功。
- 向使用者（@可愛小寶寶）回報並展示成果。

---

## 4. 風險與應對
- **API 額度消耗**：社群平台爬蟲較消耗資源，需提醒用戶注意 Apify 免費額度。
- **動態變更**：社群平台介面常改版，需依賴 Apify 官方維護 Actor 的穩定性。

---

## 5. 實作落地紀錄 (2026-10-26 完工)
- **配置檔案**：`C:\Users\user\.buzz\config\apify.json` (API Token 已驗證通過)。
- **核心工具套件**：`C:\Users\user\.buzz\tools\apify-scraper\`
  - `apify-client.mjs`：Apify REST API 客戶端（自動路由、任務輪詢、資料集下載）。
  - `content-formatter.mjs`：社群結構化轉譯器（YAML + 雙向連結 + 多媒體）。
  - `obsidian-archiver.mjs`：第二大腦自動歸檔、INDEX 維護與 GitHub 自動推送。
  - `cli.mjs`：CLI 主入口。
  - `scrape-social.bat`：一鍵調用批次檔。
- **第二大腦歸檔目錄**：`c:\私人\ai agent\buzz-obsidian\04-Social-Scraps\`
- **重要避坑注意**：`prog-party/apify-actors` 需要帳號 Full Access 權限，首次調用前請於瀏覽器開啟 `https://console.apify.com/actors/fiUEOdTO2bbgJAtDQ?approvePermissions=true` 核准。日常社群貼文（Threads / IG / FB / X）可使用預設官方 Actors 免授權直連爬取。
