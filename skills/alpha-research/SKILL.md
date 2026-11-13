---
name: alpha-research
description: "量化因子與 Alpha 預測信號研究、IC 衰減分析與複合信號評估技能"
argument-hint: "<signal_formula> [--universe <universe>] [--horizon <days>] [--holding <period>]"
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
tags: [quant-methods, alpha, research, ect-aligned]
---

# Alpha Research 量化因子信號研發技能

> **技能標識**：`alpha-research`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核) / [[Wax]] (歸檔)  
> **版本**：v1.0.0  
> **架構標準**：遵循 `ARCHITECTURE_SPEC_ECT.md` 與 `_TEMPLATE/SKILL.md`

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 探索針對特定資產池的全新預測因子（如動量、波動率倒數、量價背離等）。
- [ ] 評估現有 Alpha 假說是否具備統計顯著的預測能力（資訊係數 IC 與 ICIR）。
- [ ] 分析信號半衰期與 IC 衰減曲線（IC Decay），以決定最佳持倉週期（Holding Period）。
- [ ] 計算信號周轉率（Turnover）與扣除交易成本後的淨 Alpha。
- [ ] 結合多個低相關弱信號（Multi-Alpha）構建複合預測模型。

> ⚠️ **非適用情境 (Out of Scope)**：
> - 嚴禁用於未經回測驗證的即時高頻實盤下單指令。
> - 嚴禁在未處理前視偏差（Look-ahead Bias）或存活偏差（Survivorship Bias）的資料集上執行評估。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `signal_name` | string | 是 | 無 | 信號因子名稱，如 `momentum_12_1`、`vol_reversal_5d` |
| `universe` | string | 否 | `crypto_top50` | 標的資產池代號 (`crypto_top50`, `sp500`, `futures_liquid`) |
| `data_path` | string | 是 | 無 | 本地行情特徵資料檔路徑（Parquet 或 CSV），必須經過路徑安全驗證 |
| `horizon` | integer | 否 | `5` | 預測收益週期（天數），有效範圍：`1 <= horizon <= 63` |
| `target_ic` | number | 否 | `0.03` | 預期最低平均資訊係數（IC Mean 門檻） |
| `output_dir` | string | 否 | `reports/alpha/` | 因子分析報告與評估數據落盤目錄 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. **路徑安全檢驗**：檢查 `data_path` 是否包含非法目錄穿越字元（如 `..`、`C:\Windows`），防範 Path Traversal。
2. **大檔防爆檢驗**：若資料檔案大於 40KB，嚴禁以未分頁方式一次性載入記憶體，必須使用 Chunk / Parquet 欄位串流讀取。
3. **經濟學假說檢視**：確認該信號是否具備明確的經濟學機制（行為偏差、流動性約束或資訊不對稱），杜絕無意義之數據挖掘（Data Mining）。

### 步驟 1：核心邏輯執行 (Core Execution Phase)
1. **信號值標準化**：
   - 截面排序化（Rank Normalization）或 Winsorization 縮尾處理（去除 3 倍標準差外極端值）。
   - 中性化處理（若適用：行業中性化、市值中性化）。
2. **IC 與 ICIR 計算**：
   - 計算每一週期的 Spearman 秩相關係數：$\text{IC}_t = \text{corr}(\text{rank}(\text{Signal}_t), \text{rank}(\text{Return}_{t+h}))$。
   - 計算長期 IC 均值 ($\text{IC Mean}$) 與標準差 ($\text{IC Std}$)。
   - 計算資訊比率：$\text{ICIR} = \frac{\text{IC Mean}}{\text{IC Std}}$。
3. **IC 衰減曲線（IC Decay）分析**：
   - 計算 1-day、5-day、10-day、21-day 滾動 IC，繪製衰減圖譜，評估信號半衰期。
4. **周轉率與扣費檢驗**：
   - 計算單期周轉率 $\text{Turnover} = \frac{1}{2} \sum |w_t - w_{t-1}|$。
   - 計算扣除滑點與手續費後之淨年化 Alpha。

### 步驟 2：後置語法與完整性驗證 (Integrity Check)
1. **報告完整性校驗**：產出 Markdown 因子研究評估報告，包含 IC 分佈圖、分組收益單調性表格。
2. **DoD 驗收簽核**：由 Sentry 門禁對產出腳本與報告進行一致性審驗。

---

## 🚩 四、 防呆紅旗指標 (Red Flags)

遇下列任一情況，特工必須**立即中止流程並發出警報**：
- 🚩 **IC > 0.20 且回測近乎直線上升**：高度疑似發生「前視偏差 (Look-ahead Bias)」或使用未來資料。
- 🚩 **多重檢定無校正 (Multiple Testing)**：若已手動測試超過 50 個因子變形，未經 Bonferroni 或 Holm 校正直接採用者一律無效。
- 🚩 **周轉率過高**：年化周轉率超過 4000%，毛收益完全被交易摩擦成本吞噬。
- 🚩 **資料量不足**：歷史有效觀測樣本少於 250 個交易日。

---

## 🏁 五、 完成定義 (Definition of Done, DoD)

本技能執行完畢時，必須達成以下所有準則：
- [ ] 產出標準 Markdown 評估報告至指定路徑，包含完整度量指標（IC Mean、IC Std、ICIR、t-stat）。
- [ ] 信號計算代碼包含單元測試，測試覆蓋率 $\ge 80\%$。
- [ ] 通過 Sentry 四大語意門禁審核（無前視資料、無硬編碼金鑰、無路徑穿越）。
- [ ] 由 Wax 同步歸檔至第二大腦知識庫 `wiki/01-Cold-Memory/`。
