---
name: alpha-research
description: "量化 Alpha 信號挖掘與因子研究技能，負責從原始市場數據中發現統計顯著的預測性因子、設計回測框架、產出因子報告與交易信號候選池"
argument-hint: "[--universe <ticker_list>] [--horizon <days>] [--mode <lean|standard|full>] [--output <path>]"
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
tags: [quant-methods, alpha-research, factor-mining, backtesting, ect-aligned]
---

# 技能名稱 (Skill Name)

> **技能標識**：`alpha-research`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核)  
> **版本**：v1.0.0

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 用戶要求挖掘特定資產池的 Alpha 因子（如「美股大盤股 20 因子挖掘」）
- [ ] 上游特工指派因子研究子任務（如策略開發前的信號篩選）
- [ ] 定期因子庫更新與失效檢測排程觸發

> ⚠️ **非適用情境 (Out of Scope)**：實盤下單執行、風險控制參數設定、資產配置優化、基本面財報深度分析 — 此等任務屬於 `trade-execution`、`risk-control`、`portfolio-optimization` 或 `fundamental-analysis` 技能職責範圍。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `universe` | string[] | 是 | 無 | 目標標的代碼清單（如 `["AAPL","MSFT","GOOGL"]`），最少 10 檔、最多 3000 檔 |
| `horizon` | integer | 否 | `20` | 預測期限（交易日），範圍 1–252 |
| `mode` | enum | 否 | `standard` | 執行深度：`lean` (快速篩選 Top-20) / `standard` (完整 IC/回測) / `full` (含壓力測試與衰減分析) |
| `lookback` | integer | 否 | `252` | 回測樣本窗口（交易日），範圍 60–1260 |
| `output` | string | 否 | `./alpha-report/` | 產出報告目錄路徑，需為可寫入目錄 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. 檢驗 `universe` 非空且標的代碼格式合法（字母數字組合，長度 1–10）。
2. 檢查 `horizon`、`lookback` 數值落在合法區間。
3. 確認 `output` 目錄存在或可建立，且無目錄穿越風險（嚴禁 `..`）。
4. 驗證數據源連線可用（本地 Parquet/CSV 或遠端 API 金鑰）。

### 步驟 1：數據準備與清洗 (Data Preparation)
1. 讀取 `universe` 範圍內的 OHLCV、基本面、另類數據（依 `mode` 決定載入深度）。
2. 執行缺失值插補、極端值截尾（MAD 3σ）、前複權對齊。
3. 建立面板數據結構：`(date, ticker) -> features`。

### 步驟 2：因子工程與計算 (Factor Engineering)
1. 依 `mode` 載入內建因子模板（動量、反轉、波動率、流動性、基本面、微觀結構等 8 大類）。
2. `lean`：僅計算 Top-20 高 IC 歷史因子；`standard`/`full`：全量計算並生成衍生因子（跨截面標準化、時間序列 Z-score）。
3. 所有因子值輸出為 `factors.parquet`，欄位含 `date, ticker, factor_name, value`。

### 步驟 3：單因子 IC 分析與篩選 (Single-Factor IC Analysis)
1. 計算各因子的 Spearman IC、Rank IC、ICIR、IC 命中率（按 `horizon` 前瞻收益）。
2. `lean`：直接輸出 IC 前 20 名因子清單；`standard`/`full`：輸出完整 IC 統計表（含月度/年度穩定性）。
3. 依 ICIR > 0.5、命中率 > 52% 為初篩門檻（可由用戶覆蓋）。

### 步驟 4：多因子組合與正交化 (Multi-Factor Construction)
1. `standard`/`full`：對通過初篩因子執行 PCA/聚類去重、LASSO 篩選、風險模型正交化（行業、市值、風格中性化）。
2. 產出合成 Alpha 分數：`alpha_score = Σ w_i * factor_i`，權重由 IC 加權或優化器得出。
3. `full`：額外執行衰減分析（半衰期估計）、換手率約束優化。

### 步驟 5：回測與壓力測試 (Backtesting & Stress Testing)
1. 向量化回測：長短各 Top/Bottom 10%、等權/市值權、扣除滑點與手續費（可配置）。
2. 關鍵指標：年化收益、夏普、最大回撤、勝率、卡瑪比率、Alpha/Beta。
3. `full`：蒙地卡洛路徑模擬、極端市場情境（2008、2020、2022）歷史重放、參數敏感度掃描。

### 步驟 6：報告生成與歸檔 (Report Generation)
1. 產出 `factor_ic_report.md`、`backtest_report.md`、`alpha_signals.parquet`（含日期、標的、alpha_score、建議權重）。
2. 所有圖表（IC 時序、累積收益、回撤、因子熱力圖）輸出為 PNG 嵌入報告。
3. 寫入執行摘要 `execution_summary.json` 含耗時、數據筆數、關鍵統計。

---

## 🚨 四、 防呆紅旗指標 (Red Flags & Anti-Patterns)

凡遇到以下任何情況，**必須立即中止 (STOP)** 並回報錯誤，嚴禁自行猜測或強行繼續：

- 🚩 **紅旗 1（未來函數洩漏）**：因子計算或標籤構建中使用了 T 日收盤後才可知的資訊（如 T+1 開盤價、T 日收盤後財報發布）。
- 🚩 **紅旗 2（樣本選擇偏差）**：回測標的池包含僅存活者（無除權除息、退市標的），導致幸存者偏差。
- 🚩 **紅旗 3（過度擬合）**：`full` 模式下因子數量超過有效樣本數 1/10、或訓練集 IC 遠高於測試集 IC（>2 倍）。
- 🚩 **紅旗 4（交易成本忽視）**：回測報告未扣除滑點與手續費即宣稱高夏普（>3.0）。
- 🚩 **紅旗 5（大檔整讀）**：試圖一次性 `read_file` 載入 >40KB 的原始行情 CSV/Parquet，引發上下文雪球效應（必須分頁/串流）。
- 🚩 **紅旗 6（角括號濫用）**：在 Markdown 回應中裸露使用角括號 `< >` 造成解析錯誤（一律改用反引號 `` ` `` 或圓括號）。

---

## 🛡️ 五、 驗收門禁標準 (Quality Gate / Definition of Done)

在完成任務交付前，必須逐項核對以下 DoD 門禁：

- [ ] **1. 代碼與產出物真實落盤**：已透過工具實體確認 `output` 目錄下 `factor_ic_report.md`、`backtest_report.md`、`alpha_signals.parquet`、`execution_summary.json` 存在且內容完整。
- [ ] **2. 語法檢查通過**：Markdown 無未閉合標籤、Parquet 可被 `pandas.read_parquet` 正確讀取、JSON 解析無誤。
- [ ] **3. 格式遵循繁體中文**：所有產出文件、圖表標題、欄位說明均使用繁體中文 (Traditional Chinese)。
- [ ] **4. 雙向門禁無違規**：未被 `PreToolUse` 或 `PostToolUse` 攔截警告（無危險命令、無金鑰洩漏、無未來函數）。
- [ ] **5. 記憶歸檔就緒**：已備妥關鍵決策（因子選擇理由、正交化方法、回測參數）、Top-10 因子 IC 統計、回測核心指標，供 Wax 同步至 Obsidian 第二大腦。
- [ ] **6. 可重現性**：`execution_summary.json` 含隨機種子、數據版本哈希、環境依賴版本，確保同參數可 100% 複現。
