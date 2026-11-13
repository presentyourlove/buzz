---
name: var-models
description: "風險價值 (VaR) 與預期虧損 (CVaR/ES) 模型評估、壓力測試與 Kupiec 回測檢驗技能"
argument-hint: "<portfolio_data> [--method <parametric|historical|monte_carlo>] [--confidence <0.95|0.99>]"
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
tags: [risk, var-models, quantitative, ect-aligned]
---

# Value at Risk (VaR) 風險價值模型技能

> **技能標識**：`var-models`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核) / [[Wax]] (歸檔)  
> **版本**：v1.0.0  
> **架構標準**：遵循 `ARCHITECTURE_SPEC_ECT.md` 與 `_TEMPLATE/SKILL.md`

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 計算投資組合或單一持倉在特定信賴水準（95% 或 99%）與持有期下的最大可能潛在損失。
- [ ] 對現有部位進行三種主流 VaR 模型交叉比對（參數法 Parametric、歷史模擬法 Historical、蒙地卡羅法 Monte Carlo）。
- [ ] 執行歷史回測檢驗（Kupiec 測試與 Christoffersen 條件覆蓋測試），驗證風控模型是否低估極端損失。
- [ ] 評估肥尾（Fat-tail）、偏態與非線性期權資產組合在極端市場情境下的條件風險價值（CVaR / Expected Shortfall）。

> ⚠️ **非適用情境 (Out of Scope)**：
> - 嚴禁將常態分佈假設下的參數法 VaR 直接套用於具備深度凸性（Gamma/Vega）的非線性期權組合。
> - 嚴禁在流動性枯竭、無法實施停損的極端黑天鵝事件中將 VaR 視為「最大損失上限」。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `portfolio_path` | string | 是 | 無 | 持倉權重與資產收益率歷史資料檔路徑，嚴格驗證路徑安全 |
| `confidence` | number | 否 | `0.99` | 信賴水準，標準選項：`0.95` 或 `0.99` |
| `horizon_days` | integer | 否 | `1` | 持有期間（天數），預設 1 日，可選 10 日（巴塞爾規範） |
| `method` | enum | 否 | `historical` | 計算模型：`parametric` / `historical` / `monte_carlo` / `all` |
| `simulations` | integer | 否 | `10000` | 蒙地卡羅模擬次數，有效範圍：`1000 <= simulations <= 100000` |
| `output_path` | string | 否 | `reports/risk/var_report.md` | 風險評估結果報告輸出路徑 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. **輸入與路徑檢查**：確認 `portfolio_path` 存在且無目錄穿越字元。
2. **資產權重完整性檢查**：校驗投資組合權重總和必須等於 $1.0$ ($\pm 10^{-4}$)。
3. **大檔處理規範**：若多資產歷史資料超過 40KB，必須採用欄位串流方式讀取收盤價矩陣。

### 步驟 1：核心邏輯執行 (Core Execution Phase)
1. **三種模型並行計算**：
   - **參數法 (Parametric VaR)**：
     - 計算資產協方差矩陣 $\Sigma$ 與組合波動率 $\sigma_p = \sqrt{w^T \Sigma w}$。
     - $\text{VaR}_\alpha = W \times (z_\alpha \sigma_p - \mu_p)$。
   - **歷史模擬法 (Historical Simulation VaR)**：
     - 重現歷史 $T$ 個交易日的組合 P&L，排序後取 $(1 - \alpha) \times T$ 分位數損失。
     - 計算條件風險價值 $\text{CVaR}_\alpha = E[L \mid L > \text{VaR}_\alpha]$。
   - **蒙地卡羅法 (Monte Carlo VaR)**：
     - 擬合多元分佈（或 Copula 關聯架構），生成 $N$ 條價格路徑並重新評價組合。
2. **Kupiec 回測檢驗 (Backtesting)**：
   - 統計歷史實際損失超出 VaR 的超額次數（Exceptions）。
   - 進行似然比率檢驗：$\text{LR}_{uc} = -2 \ln \left[ \frac{(1-p)^{T-x} p^x}{(1-x/T)^{T-x} (x/T)^x} \right]$，判斷模型是否落在巴塞爾綠燈區（0~4 次違規）。

### 步驟 2：後置語法與完整性驗證 (Integrity Check)
1. **生成風控報告**：彙整各模型 VaR 數值、CVaR 數值、Kupiec 檢驗統計量及應急停損建議。
2. **DoD 驗收簽核**：由 Sentry 門禁對產出腳本與報告進行一致性審驗。

---

## 🚩 四、 防呆紅旗指標 (Red Flags)

遇下列任一情況，特工必須**立即中止流程並發出警報**：
- 🚩 **Kupiec 檢驗落入紅燈區 (Exceptions $\ge 10$)**：模型嚴重低估真實市場風險，禁止用於生產風控。
- 🚩 **共變異數矩陣非正定 (Non-positive Definite)**：資產相關性矩陣出現奇異性，需使用 Ledoit-Wolf 收縮法修正。
- 🚩 **忽視肥尾跳躍**：歷史報酬偏態（Skewness）或峰態（Kurtosis）極大時，嚴禁單獨採信參數法 VaR。

---

## 🏁 五、 完成定義 (Definition of Done, DoD)

本技能執行完畢時，必須達成以下所有準則：
- [ ] 產出標準 Markdown 風險度量報告，包含 95% 與 99% 下的 VaR 與 CVaR (Expected Shortfall)。
- [ ] 包含完整的 Kupiec 回測檢驗結果與巴塞爾燈號標記（綠/黃/紅）。
- [ ] 提供對應的 Python 計算腳本與單元測試，測試通過率 100%。
- [ ] 由 Wax 同步歸檔至第二大腦知識庫 `wiki/01-Cold-Memory/`。
