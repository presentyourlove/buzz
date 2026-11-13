---
name: var-models
description: "風險價值模型構建與驗證技能，負責建立參數/非參數/混合 VaR/ES 模型、執行回測與壓力測試、產出合規級風險報告與模型驗證文檔"
argument-hint: "[--portfolio <path>] [--confidence <0.95|0.99|0.995>] [--method <historical|parametric|monte-carlo|evt|garch>] [--horizon <days>] [--output <path>]"
user-invocable: true
allowed-tools:
  - read_file
  - write_file
  - edit_file
  - shell
  - ask_user
model: "nvidia/nemotron-3-ultra-550b-a55b"
author: "Honey2"
version: "1.0.0"
tags: [quant-methods, var-models, risk-management, expected-shortfall, backtesting, basel-compliance, ect-aligned]
---

# 技能名稱 (Skill Name)

> **技能標識**：`var-models`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核)  
> **版本**：v1.0.0

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 用戶要求為投資組合計算 VaR/ES（如「計算 99% 置信度 10 日 VaR」）
- [ ] 上游特工指派風險限額設定前的模型驗證子任務
- [ ] 監管報告週期觸發（巴塞爾協議 III、溢利能力 II、內部模型法 IMA 申報）
- [ ] 新策略/新資產類別納入組合前的風險度量基礎建設

> ⚠️ **非適用情境 (Out of Scope)**：實時交易風控熔斷、保證金計算、對手方信用風險（CVA/DVA）、流動性風險建模 — 此等任務屬於 `risk-control`、`margin-engine`、`credit-risk` 或 `liquidity-risk` 技能職責範圍。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `portfolio` | string | 是 | 無 | 投資組合持倉檔案路徑（CSV/Parquet，欄位：`ticker, quantity, avg_price, currency`） |
| `confidence` | float | 否 | `0.99` | 置信度水位，允許值：`0.95`、`0.99`、`0.995`（對應 95%、99%、99.5%） |
| `method` | enum | 否 | `historical` | 計算方法：`historical` (歷史模擬) / `parametric` (參數法/方差協方差) / `monte-carlo` (蒙地卡洛) / `evt` (極值理論) / `garch` (GARCH 條件異方差) |
| `horizon` | integer | 否 | `10` | 風險期限（交易日），範圍 1–252，監管常用 10 日 |
| `lambda` | float | 否 | `0.94` | EWMA 衰減因子（`parametric`/`garch` 適用），範圍 0.90–0.99 |
| `n_sim` | integer | 否 | `10000` | 蒙地卡洛/歷史模擬路徑數，範圍 1000–500000 |
| `lookback` | integer | 否 | `252` | 歷史樣本窗口（交易日），範圍 60–2520 |
| `output` | string | 否 | `./var-report/` | 產出報告目錄路徑，需為可寫入目錄 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. 檢驗 `portfolio` 檔案存在且欄位完整（持倉數量非零、幣別合法 ISO 4217）。
2. 檢查 `confidence`、`method`、`horizon` 參數合法性。
3. 確認 `output` 目錄存在或可建立，且無目錄穿越風險（嚴禁 `..`）。
4. 驗證市場數據源可用（需覆蓋組合所有標的的 `lookback` 期間 OHLCV）。

### 步驟 1：持倉映射與風險因子提取 (Position Mapping & Risk Factor Extraction)
1. 讀取持倉檔，展開至單一標的層級，計算各標的市值權重。
2. 將各標的映射至系統風險因子：股票→市場/行業/風格因子；債券→久期/曲線/信用利差；衍生品→Delta/Gamma/Vega 展開至標的因子。
3. 輸出 `risk_factors.parquet`：`(date, factor_name, exposure)` 面板。

### 步驟 2：模型擬合與參數估計 (Model Fitting)
1. **historical**：直接使用歷史收益率分布（等權或 EWMA 加權）。
2. **parametric**：估計協方差矩陣（EWMA `lambda` 或 Ledoit-Wolf 收縮），假設多元正態/學生 t 分布（自由度由 MLE 估計）。
3. **monte-carlo**：擬合邊際分布（參數或核密度）+ 科普拉相依結構（高斯/學生 t/Clayton/Gumbel），生成 `n_sim` 條路徑。
4. **evt**：擬合廣義帕累托分布（GPD）至尾部超臨值（臨值由 Hill 圖/平均超額圖選擇），計算 VaR/ES 解析解。
5. **garch**：對組合收益率擬合 GARCH(1,1)/EGARCH/GJR-GARCH，滾動預測條件方差，生成條件 VaR/ES。
6. 所有模型輸出 `model_params.json` 含估計參數、收斂狀態、AIC/BIC。

### 步驟 3：VaR/ES 計算 (VaR/ES Computation)
1. 對每個交易日 `t`，基於 `t-1` 可得資訊計算 `horizon` 日前瞻 VaR/ES。
2. 輸出 `var_series.parquet`：`date, var_95, var_99, var_995, es_95, es_99, es_995, method, horizon`。
3. `method=historical/monte-carlo` 時並行輸出完整損益分布樣本供後續分析。

### 步驟 4：回測與模型驗證 (Backtesting & Model Validation)
1. **Kupiec PO 檢定**：失效率是否與置信度一致（p > 0.05 為通過）。
2. **Christoffen 獨立性檢定**：失效事件是否獨立分布（p > 0.05 為通過）。
3. **條件/無條件覆蓋檢定**：聯合檢定 PO + 獨立性。
4. **ES 回測**：Fissler-Ziegel 損失函數檢定、Acerbi-Szekely ES 回測。
5. **巴塞爾交通燈**：綠區（≤4 次失效）、黃區（5–9 次）、紅區（≥10 次，250 日窗口）。
6. 輸出 `backtest_results.json` 含所有檢定統計量、p-value、交通燈狀態。

### 步驟 5：壓力測試與情境分析 (Stress Testing)
1. 歷史情境：1987 黑色星期一、1997 亞洲金融風暴、2000 互聯網泡沫、2008 雷曼兄弟、2020 新冠、2022 通膨加息。
2. 假設情境：利率平移 ±200bps、曲線陡峭/平坦化、股市 -20%/波動率 +100%、信用利差擴大 200bps、流動性枯竭（Bid-Ask 擴大 5 倍）。
3. 反向壓力測試：尋找導致組合虧損超過資本 50% 的最小衝擊向量。
4. 輸出 `stress_test_report.md` 含各情境 P&L、風險因子敏感度分解。

### 步驟 6：報告生成與合規歸檔 (Report Generation & Compliance Filing)
1. 產出 `var_report.md`：模型選擇理由、參數估計詳情、VaR/ES 時序圖、回測交通燈、壓力測試摘要、模型局限性聲明。
2. 產出 `model_validation_dossier.md`：完整驗證記錄供監查/稽核調閱（含數據版本、代碼版本、審核人簽署欄）。
3. 寫入 `execution_summary.json` 含耗時、樣本數、關鍵指標、合規狀態。

---

## 🚨 四、 防呆紅旗指標 (Red Flags & Anti-Patterns)

凡遇到以下任何情況，**必須立即中止 (STOP)** 並回報錯誤，嚴禁自行猜測或強行繼續：

- 🚩 **紅旗 1（持倉映射遺漏）**：衍生品未展開至標的風險因子（如期權僅計 Delta 忽略 Gamma/Vega）、貨幣敞口未換算至基準幣別。
- 🚩 **紅旗 2（協方差矩陣不正定）**：`parametric` 法估計協方差矩陣特徵值 ≤ 0，未施加收縮或正定修正直接用於 Cholesky 分解。
- 🚩 **紅旗 3（歷史模擬靜態權重）**：`historical` 法使用等權歷史收益率而組合權重在樣本期內顯著變動（換手率 > 50%/月），導致風險度量與實際持倉脫節。
- 🚩 **紅旗 4（極值理論臨值主觀）**：`evt` 臨值選擇未經 Hill 圖/平均超額圖/穩定性圖三重驗證，或尾部樣本數 < 30 仍強行擬合 GPD。
- 🚩 **紅旗 5（回測樣本不足）**：回測窗口 < 250 觀測值仍宣稱通過巴塞爾交通燈（統計效力極低，屬濫用）。
- 🚩 **紅旗 6（大檔整讀）**：試圖一次性載入 >40KB 的歷史行情/持倉檔，引發上下文雪球效應（必須分頁/串流）。
- 🚩 **紅旗 7（角括號濫用）**：在 Markdown 回應中裸露使用角括號 `< >` 造成解析錯誤（一律改用反引號 `` ` `` 或圓括號）。

---

## 🛡️ 五、 驗收門禁標準 (Quality Gate / Definition of Done)

在完成任務交付前，必須逐項核對以下 DoD 門禁：

- [ ] **1. 代碼與產出物真實落盤**：已透過工具實體確認 `output` 目錄下 `var_report.md`、`model_validation_dossier.md`、`stress_test_report.md`、`var_series.parquet`、`model_params.json`、`backtest_results.json`、`execution_summary.json` 存在且內容完整。
- [ ] **2. 語法檢查通過**：Markdown 無未閉合標籤、Parquet/JSON 可被標準庫正確讀取、數值精度無溢位。
- [ ] **3. 格式遵循繁體中文**：所有產出文件、圖表標題、欄位說明、合規聲明均使用繁體中文 (Traditional Chinese)。
- [ ] **4. 雙向門禁無違規**：未被 `PreToolUse` 或 `PostToolUse` 攔截警告（無危險命令、無金鑰洩漏、無協方差矩陣不正定、無未來函數）。
- [ ] **5. 記憶歸檔就緒**：已備妥關鍵決策（模型選擇理由、臨值選擇依據、壓力情境設計邏輯）、回測交通燈狀態、ES 回測 p-value，供 Wax 同步至 Obsidian 第二大腦。
- [ ] **6. 合規可追溯性**：`model_validation_dossier.md` 含數據版本哈希、代碼 Git commit SHA、模型參數完整記錄、獨立驗證人簽署欄位，滿足監管查核追溯要求。
- [ ] **7. 可重現性**：`execution_summary.json` 含隨機種子（蒙地卡洛）、數據版本、環境依賴版本，確保同參數可 100% 複現。
