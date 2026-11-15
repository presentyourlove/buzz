---
title: "事實查核與雙重確認機制暨聯網搜尋工具整合實作計畫書"
tags: [agent, fact-check, verification, web-search, mcp, tool-integration]
status: active
version: 1.0
author: "Honey2"
created: 2026-11-14
updated: 2026-11-14
---

# 事實查核與雙重確認機制暨聯網搜尋工具整合實作計畫書 (v1.0)

## 一、任務背景與目標

### 1.1 背景
- 現狀：Buzz 系統已具備基礎聯網搜尋能力（web-search.mjs、web-fetch.mjs 透過 MCP 聚合提供），支援 Tavily、Brave、DuckDuckGo 搜尋與 Jina Reader 網頁擷取。
- 缺口：缺乏系統化的事實查核流程、雙重確認機制、引用溯源驗證，以及搜尋結果品質評分與去偏機制。

### 1.2 核心目標
建構一套「搜尋->驗證->確認->引用」完整閉環：
1. 自動多源搜尋：並行查詢 >=3 獨立搜尋源
2. 雙重確認機制：來源可信度評分 + 交叉驗證一致性檢查
3. 引用溯源標註：結構化引用 [來源: URL, 可信度: X, 驗證狀態: 成功/失敗]
4. 衝突資訊調解：自動標記不確定性、提供加權共識或要求人工裁決

### 1.3 非目標
- 不重構現有 MCP 架構
- 不引入重型外部依賴
- 不改變 Agent 核心循環邏輯

---

## 二、系統架構設計

### 2.1 整體架構圖



### 2.2 核心組件規格

四個新增 MCP 工具：fact_check、verify_claim、cross_verify、cite_source

---

## 三、驗證中介層核心演算法

### 3.1 來源可信度評分模型
權重：domain_authority 0.30, https_valid 0.10, content_freshness 0.15, citation_density 0.15, author_expertise 0.10, factual_consistency 0.20

白名單域名：gov.tw/gov.hk 0.95, who.int/cdc.gov 0.98, reuters.com 0.92, arxiv.org 0.95 等

### 3.2 交叉驗證一致性檢查
使用關鍵詞 Jaccard 相似度進行語義聚類，計算最大簇佔比作為共識度

### 3.3 衝突資訊調解
可信度加權投票，若最高分/次高分 > 2.0 則自動解決，否則標記 REQUIRES_HUMAN_JUDGMENT

---

## 四、實作計畫與里程碑

### 4.1 第一期：MVP 輕量落地版 (Week 1-2) 優先交付

| 任務編號 | 任務名稱 | 產出物 | 驗收標準 |
|---------|---------|--------|----------|
| T1.1 | 建立 fact_check.mjs 核心工具框架 | c:\私人	oolsact_check.mjs | 返回結構化驗證結果 |
| T1.2 | 實作多源並行搜尋協調器 | 整合既有 web-search.mjs、web-fetch.mjs | 並行查詢 3 源，耗時 < 8 秒 |
| T1.3 | 實作來源可信度評分器 (啟發式版) | SourceCredibilityScorer class | 評分與人工標註相關性 > 0.8 |
| T1.4 | 實作基礎交叉驗證 (關鍵詞 Jaccard 相似度) | CrossValidator class | 一致性判定準確率 > 85% |
| T1.5 | 實作引用格式化輸出 (bracket 格式) | cite_source.mjs | 輸出：[來源: URL, 可信度: 0.87, 驗證: 成功] |
| T1.6 | 註冊至 buzz-composite-mcp.exe 工具列表 | 更新 buzz-composite-mcp.mjs | tools/list 顯示 11 個工具 |
| T1.7 | 更新 managed-agents.json 注入 API Keys | 確保 Tavily/Brave API Key 生效 | 所有受管 Agent 可呼叫 fact_check |
| T1.8 | 編寫單元測試與整合測試 | tests/test_fact_check.mjs | npm test 全綠，覆蓋率 >= 80% |

### 4.2 第二期：企業級增強版 (Week 3-4) - 視第一期驗收後再決策
嵌入模型語義相似度、衝突調解加權投票、SQLite緩存層、垂直領域適配器、自動報告生成、Sentry CI/CD整合

---

## 五、代碼庫具體修改點

### 5.1 新增檔案
c:\私人	oolsact_check.mjs, verify_claim.mjs, cross_verify.mjs, cite_source.mjs
c:\私人	oolserification\SourceCredibilityScorer.js, CrossValidator.js, ConflictResolver.js, CitationFormatter.js
c:\私人	ools	ests	est_fact_check.mjs

### 5.2 修改既有檔案
buzz-composite-mcp.mjs: 新增 4 個工具定義與路由邏輯

### 5.3 環境變數
TAVILY_API_KEY, BRAVE_API_KEY (既有), SEMANTIC_SCHOLAR_API_KEY, PUBMED_API_KEY (新增可選)

---

## 六、驗收標準

### 6.1 功能性驗收 (MVP 版)
1. 基本事實查核測試：10 秒內返回結構化結果，含 >=3 來源
2. 雙重確認機制測試：嚴格模式需 >=2 高可信度源一致
3. 衝突偵測測試：正確標記 conflict_detected: true
4. 引用溯源測試：格式符合 [來源: URL, 可信度: X, 驗證: 成功/失敗]
5. 工具鏈完整性：tools/list 返回 11 工具
6. 原有能力不中斷：既有 9 工具運作如常

### 6.2 品質指標
端到端延遲 P95 < 10 秒, 評分 Spearman ρ > 0.8, 驗證準確率 > 85%, 引用合規 100%, 覆蓋率 >= 80%

### 6.3 Sentry 四大語意門禁
1. 代碼符號真實性 2. 協議與 Relay 鑑權閉環 3. 強制失敗模式推演 4. MVP 敏捷交付引導

---

## 七、回滾機制
git checkout HEAD -- buzz-composite-mcp.mjs, 刪除新增工具檔案, 重啟服務

---

## 八、風險評估與緩解
API 配額耗盡(中/高)->指數退避+後備源+快取; 評分誤判(中/中)->啟發式白名單+第二期嵌入模型; 調解誤導(低/高)->嚴格模式+強制人工裁決

---

## 九、下一步行動
1. 立即執行 T1.1-T1.3
2. 並行 T1.4-T1.5
3. 完成 T1.6-T1.8
4. 提交 Sentry 審核

---

**計畫書已就緒，請 @Sentry 進行架構完備性與品質審核。**