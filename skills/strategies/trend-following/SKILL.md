---
name: trend-following
description: "趨勢跟踪策略執行與訊號生成技能，負責多時間框架趨勢識別、進出場邏輯執行、倉位動態管理、實盤訊號推送與績效歸因分析"
argument-hint: "[--universe <ticker_list>] [--timeframes <list>] [--entry <breakout|pullback|dual-ma>] [--exit <trailing|fixed-r|time-stop>] [--position-sizing <vol-target|kelly|fixed-frac>] [--mode <paper|live>] [--output <path>]"
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
tags: [strategies, trend-following, signal-generation, position-management, execution, ect-aligned]
---

# 技能名稱 (Skill Name)

> **技能標識**：`trend-following`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核)  
> **版本**：v1.0.0

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 用戶要求部署趨勢策略於指定標的池（如「美股期貨 20 檔雙均線突破策略」）
- [ ] 上游特工指派實盤/模擬盤訊號生成子任務（每日開盤前/收盤後批次）
- [ ] 現有策略參數優化或新時間框架擴展需求
- [ ] 策略績效歸因與歸因報告產出需求（月度/季度複盤）

> ⚠️ **非適用情境 (Out of Scope)**：Alpha 因子挖掘、風險模型構建、資產配置優化、高頻做市、期權波動率交易 — 此等任務屬於 `alpha-research`、`var-models`、`portfolio-optimization`、`market-making` 或 `volatility-trading` 技能職責範圍。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `universe` | string[] | 是 | 無 | 目標標的代碼清單（如 `["ES","NQ","CL","GC"]`），最少 1 檔、最多 100 檔 |
| `timeframes` | string[] | 否 | `["D","W"]` | 趨勢判定時間框架清單（支援 `15m`、`1h`、`4h`、`D`、`W`、`M`），至少 1 個、最多 4 個 |
| `entry` | enum | 否 | `breakout` | 進場邏輯：`breakout` (唐奇安/唐奇安雙向) / `pullback` (回調入場) / `dual-ma` (雙均線黃金交叉) / `supertrend` (SuperTrend 翻轉) |
| `entry_params` | object | 否 | 見說明 | 進場參數：`breakout`→`{lookback:20}`；`dual-ma`→`{fast:20,slow:50}`；`supertrend`→`{period:10,mult:3}` |
| `exit` | enum | 否 | `trailing` | 出場邏輯：`trailing` (ATR 尾隨停損) / `fixed-r` (固定 R 倍數停利停損) / `time-stop` (時間停損) / `trend-reversal` (趨勢反轉離場) |
| `exit_params` | object | 否 | 見說明 | 出場參數：`trailing`→`{atr_period:14,mult:2.5}`；`fixed-r`→`{stop_r:2,target_r:3}`；`time-stop`→`{max_hold_days:60}` |
| `position_sizing` | enum | 否 | `vol-target` | 倉位管理：`vol-target` (波動率目標) / `kelly` (凱利準則/半凱利) / `fixed-frac` (固定分數) / `risk-parity` (風險平價) |
| `risk_params` | object | 否 | 見說明 | 風控參數：`vol-target`→`{target_vol:0.15,max_leverage:2}`；`kelly`→`{win_rate:0.45,avg_win_loss:2.5,fraction:0.5}`；`fixed-frac`→`{frac:0.02}`；`risk-parity`→`{target_vol:0.1}` |
| `mode` | enum | 否 | `paper` | 執行模式：`paper` (模擬盤/訊號輸出) / `live` (實盤下單 — 需額外授權) |
| `session` | enum | 否 | `rth` | 交易時段：`rth` (常規交易時段) / `eth` (延長交易時段) / `24h` (全天候，加密/外匯適用) |
| `output` | string | 否 | `./trend-signals/` | 產出目錄路徑，需為可寫入目錄 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. 檢驗 `universe` 非空且標的代碼在交易所合約主檔中存在。
2. 檢查 `timeframes` 格式合法且數量 ≤ 4；驗證 `entry`/`exit`/`position_sizing` 參數組合邏輯相容（如 `kelly` 需 `win_rate`、`avg_win_loss`）。
3. 確認 `output` 目錄存在或可建立，且無目錄穿越風險（嚴禁 `..`）。
4. `mode=live` 時：必須驗證券商 API 憑證、風控額度、交易權限；未通過嚴禁進入實盤邏輯。

### 步驟 1：多時間框架數據同步與指標計算 (Multi-TF Data Sync & Indicators)
1. 並行讀取各 `timeframes` 的 OHLCV 數據（`lookback` = max 入場參數 × 3，至少 200 根 K 線）。
2. 計算各時間框架核心指標：
   - 趨向：EMA/SMA、ADX、SuperTrend、唐奇安通道、Donchian 雙向突破位
   - 波動：ATR(14)、實現波動率(20)、GARCH(1,1) 條件波動率
   - 成交量：OBV、VWAP、成交量加權動量
3. 將高頻時間框架指標向上重採樣對齊至最低頻框架（`D` 或 `W`），產出 `indicators.parquet`。

### 步驟 2：趨勢狀態識別 (Trend State Classification)
1. 依 `entry` 邏輯定義趨勢狀態機：
   - `breakout`：價格突破唐奇安上/下軌 → `LONG`/`SHORT`/`FLAT`
   - `pullback`：主趨勢向上（MA 快>慢、ADX>25）+ 回調至支撐位 → `LONG_PULLBACK`/`SHORT_PULLBACK`
   - `dual-ma`：快線上穿/下穿慢線 → `LONG`/`SHORT`
   - `supertrend`：SuperTrend 翻紅/翻綠 → `LONG`/`SHORT`
2. 多時間框架共識機制：需 ≥ 50% 時間框架方向一致才生成訊號；衝突時輸出 `CONFLICT` 狀態並降低倉位權重。
3. 輸出 `trend_states.parquet`：`date, ticker, timeframe, state, strength`。

### 步驟 3：訊號生成與過濾 (Signal Generation & Filtering)
1. 在 `trend_states` 狀態變化點生成原始訊號（`ENTRY_LONG`、`ENTRY_SHORT`、`EXIT_LONG`、`EXIT_SHORT`）。
2. 過濾規則：
   - 最小持倉間隔：同方向訊號間隔 < 3 根 K 線者合併。
   - 偽突破過濾：突破當根 K 線成交量 < 近 20 期均量 × 1.2 者剔除（可配置）。
   - 流動性過濾：日均成交額 < 閾值（股票 $50M、期貨 $100M、加密 $10M）者剔除。
3. 輸出 `raw_signals.parquet`、`filtered_signals.parquet`。

### 步驟 4：動態倉位計算 (Dynamic Position Sizing)
1. 依 `position_sizing` 計算目標倉位：
   - `vol-target`：`target_position = target_vol / realized_vol × equity × leverage_cap`
   - `kelly`：`f* = (p×b - q)/b × fraction`，其中 `p=win_rate, b=avg_win_loss`
   - `fixed-frac`：固定權重 `frac`，按信號強度線性縮放
   - `risk-parity`：反波動率加權，使各標的風險貢獻相等
2. 套用組合級風控覆蓋層：
   - 單一標的上限：`max_single_weight`（預設 15%）
   - 相關性聚類上限：同一聚類總權重 ≤ 40%
   - 總槓桿上限：`max_leverage`
   - 最大回撤熔斷：當前回撤 > `dd_limit`（預設 10%）時線性縮減至 0
3. 輸出 `target_positions.parquet`：`date, ticker, target_weight, target_qty, risk_budget`。

### 步驟 5：執行與訊號推送 (Execution & Signal Dispatch)
1. `mode=paper`：生成 `daily_signals.csv`（含 `date, ticker, action, target_qty, limit_price, stop_price, reason`）供人工/下游系統執行。
2. `mode=live`：透過券商 API 送單（市價/限價可配）、追蹤成交回報、異常處理（拒單、部分成交、斷線重試）。
3. 實時推送：WebSocket/HTTP 回調推送訊號變更至下游消費者（風控、PMS、前端看板）。

### 步驟 6：績效歸因與複盤報告 (Performance Attribution & Review)
1. 計算策略級指標：年化收益、夏普、索提諾、卡瑪、最大回撤、勝率、盈虧比、年化換手率。
2. 歸因分解：Brinson 模型（配置效應、選股效應、交互效應）、因子暴露回歸（動量、價值、規模、質量、波動率）。
3. 交易成本分析：滑點分布、手續費占比、市場衝擊成本、實際 vs 理論倉位偏離。
4. 輸出 `performance_report.md`、`attribution_report.md`、`trade_log.parquet`、`execution_summary.json`。

---

## 🚨 四、 防呆紅旗指標 (Red Flags & Anti-Patterns)

凡遇到以下任何情況，**必須立即中止 (STOP)** 並回報錯誤，嚴禁自行猜測或強行繼續：

- 🚩 **紅旗 1（未來函數訊號）**：訊號生成使用了當根 K 線收盤後才可知的資訊（如 `close` 判斷突破卻用 `high`/`low` 當根確認、或用 T 日收盤價計算 T 日訊號卻在 T 日開盤前推送）。
- 🚩 **紅旗 2（參數過度優化）**：回測參數搜尋空間 > 10^4 組合且未經走樣外/走樣前驗證，或單一參數組合在樣本外夏普 < 0.5 仍宣稱有效。
- 🚩 **紅旗 3（忽略交易成本）**：回測報告未扣除滑點（≥ 1 tick/0.5 bps）、手續費、資金成本、稅費即宣稱高頻策略年化 > 30%。
- 🚩 **紅旗 4（倉位邏輯缺漏）**：`mode=live` 時缺少單標的上限、相關性聚類上限、總槓桿上限、回撤熔斷任一層防護。
- 🚩 **紅旗 5（數據對齊錯誤）**：多時間框架指標重採樣使用 `forward-fill` 導致高頻資訊洩漏至低頻決策（必須用 `previous-close` 對齊）。
- 🚩 **紅旗 6（大檔整讀）**：試圖一次性載入 >40KB 的逐筆/分鐘級行情檔，引發上下文雪球效應（必須分頁/串流/資料庫查詢）。
- 🚩 **紅旗 7（角括號濫用）**：在 Markdown 回應中裸露使用角括號 `< >` 造成解析錯誤（一律改用反引號 `` ` `` 或圓括號）。

---

## 🛡️ 五、 驗收門禁標準 (Quality Gate / Definition of Done)

在完成任務交付前，必須逐項核對以下 DoD 門禁：

- [ ] **1. 代碼與產出物真實落盤**：已透過工具實體確認 `output` 目錄下 `daily_signals.csv`、`target_positions.parquet`、`filtered_signals.parquet`、`performance_report.md`、`attribution_report.md`、`trade_log.parquet`、`execution_summary.json` 存在且內容完整。
- [ ] **2. 語法檢查通過**：Markdown 無未閉合標籤、CSV/Parquet/JSON 可被標準庫正確讀取、數值精度無溢位、日期格式 ISO 8601。
- [ ] **3. 格式遵循繁體中文**：所有產出文件、圖表標題、欄位說明、訊號理由欄位均使用繁體中文 (Traditional Chinese)。
- [ ] **4. 雙向門禁無違規**：未被 `PreToolUse` 或 `PostToolUse` 攔截警告（無危險命令、無金鑰洩漏、無未來函數、無實盤未授權執行）。
- [ ] **5. 記憶歸檔就緒**：已備妥關鍵決策（入出場邏輯選擇理由、倉位模型參數校準過程、過濾規則閾值設定依據）、月度績效核心指標、歸因分解結果，供 Wax 同步至 Obsidian 第二大腦。
- [ ] **6. 可審計追蹤**：`trade_log.parquet` 含完整訂單生命週期（送單時間、成交時間、成交價、成交量、手續費、滑點、訊號 ID）、`execution_summary.json` 含代碼 Git commit SHA、數據版本、參數完整快照。
- [ ] **7. 可重現性**：同 `execution_summary.json` 參數、同數據版本、同隨機種子（如涉及蒙地卡洛模擬滑點）可 100% 複現訊號序列與績效指標。
