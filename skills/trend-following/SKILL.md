---
name: trend-following
description: "CTA 式趨勢跟隨、動量突破、ATR 波動率倉位管理與海龜交易系統實作技能"
argument-hint: "<market_data> [--system <donchian|ma_cross|turtle>] [--risk_budget <pct>]"
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
tags: [strategies, trend-following, cta, momentum, ect-aligned]
---

# Trend Following 趨勢跟隨與動量交易策略技能

> **技能標識**：`trend-following`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核) / [[Wax]] (歸檔)  
> **版本**：v1.0.0  
> **架構標準**：遵循 `ARCHITECTURE_SPEC_ECT.md` 與 `_TEMPLATE/SKILL.md`

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 構建 CTA (Commodity Trading Advisor) 風格的跨品種中長期趨勢跟隨系統。
- [ ] 設計雙均線交叉 (MA Crossover) 或唐奇安通道突破 (Donchian Breakout) 模型。
- [ ] 實作海龜交易法則 (Turtle Trading Rules) 或基於 ATR (Average True Range) 的部位規模動態縮放。
- [ ] 利用 ADX (Average Directional Index) 指標進行趨勢強弱過濾，在盤整期抑制無效頻繁停損。

> ⚠️ **非適用情境 (Out of Scope)**：
> - 嚴禁在 ADX < 20 或明顯高頻震盪區間內強制啟動趨勢突破開倉（避免頻繁磨損本金）。
> - 嚴禁用於固定點數硬停損而未按 ATR 波動率調整口數的策略（缺乏波動率中性）。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `market_data_path` | string | 是 | 無 | 歷史行情 OHLCV 資料路徑，嚴格驗證路徑安全 |
| `system_type` | enum | 否 | `donchian_breakout` | 趨勢策略類型：`donchian_breakout` / `ma_cross` / `turtle_n` |
| `trend_period` | integer | 否 | `20` | 趨勢判定週期（如 20 日突破或 50/200 均線），有效範圍：`5 <= trend_period <= 250` |
| `atr_period` | integer | 否 | `14` | 平均真實波幅 (ATR) 計算週期 |
| `risk_per_trade` | number | 否 | `0.01` | 單筆交易風險預算（佔總淨值百分比，如 `0.01` 代表 1%） |
| `adx_threshold` | number | 否 | `25.0` | 趨勢確認門檻，低於此值視為無趨勢盤整 |
| `output_dir` | string | 否 | `reports/strategies/` | 策略回測、交易明細與績效報告輸出目錄 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. **輸入路徑安全檢驗**：檢查 `market_data_path` 嚴禁包含路徑穿越字元。
2. **大檔串流讀取**：多品種長週期行情資料檔案大於 40KB 時，必須分塊或分頁載入。
3. **無偏資料驗證**：確保時間戳記單調遞增，無重複 K 線或時間倒流現象。

### 步驟 1：核心邏輯執行 (Core Execution Phase)
1. **指標體系構建**：
   - 計算真實波幅 $\text{TR} = \max(\text{High}_t - \text{Low}_t, |\text{High}_t - \text{Close}_{t-1}|, |\text{Low}_t - \text{Close}_{t-1}|)$ 與 $N$ 日 ATR。
   - 計算趨勢指標：唐奇安高低軌 ($\text{Highest High}_N$, $\text{Lowest Low}_N$) 或雙指數移動均線 (EMA)。
   - 計算定向指標：+DI, -DI 與趨勢強度 ADX。
2. **海龜/波動率部位規模計算 (Position Sizing)**：
   - 單元口數公式：$\text{Unit Size} = \frac{\text{Total Equity} \times \text{Risk Per Trade}}{\text{ATR} \times \text{Contract Multiplier}}$。
   - 確保不同波動率資產對組合風險貢獻維持等權（Risk Parity 特性）。
3. **信號觸發與加碼/停損管理**：
   - 突破上軌且 $\text{ADX} > 25$ 時開倉 1 個 Unit。
   - 價格每順行 $0.5 \times \text{ATR}$ 加碼 1 個 Unit（最多 4 個 Units）。
   - 停損點設於最後一次成交價回撤 $2.0 \times \text{ATR}$ 處。
4. **績效與危機 Alpha (Crisis Alpha) 評估**：
   - 計算夏普比率 (Sharpe Ratio)、索提諾比率 (Sortino Ratio)、最大回撤 (Max Drawdown) 與勝率。

### 步驟 2：後置語法與完整性驗證 (Integrity Check)
1. **回測報告落盤**：生成包含淨值曲線、回撤週期、持倉熱力圖的 Markdown 報告。
2. **DoD 驗收簽核**：由 Sentry 門禁對產出腳本與報告進行一致性審驗。

---

## 🚩 四、 防呆紅旗指標 (Red Flags)

遇下列任一情況，特工必須**立即中止流程並發出警報**：
- 🚩 **單一品種風險暴露 > 5% 淨值**：部位口數計算失誤，違反風控上限。
- 🚩 **沒有計入過夜費與滑點 (Slippage & Funding Fee)**：回測未設定交易成本，導致虛胖高收益。
- 🚩 **無條件硬扛逆向趨勢**：未嚴格執行 $2 \times \text{ATR}$ 停損規則，導致回撤失控。

---

## 🏁 五、 完成定義 (Definition of Done, DoD)

本技能執行完畢時，必須達成以下所有準則：
- [ ] 產出標準 Markdown 策略研發報告，包含完整風控與部位配比邏輯。
- [ ] 策略 Python 腳本實作完整的進出場與 ATR 部位計算函式，具備 100% 單元測試覆蓋。
- [ ] 通過 Sentry 門禁審查，確保無過擬合及交易成本漏洞。
- [ ] 由 Wax 同步歸檔至第二大腦知識庫 `wiki/01-Cold-Memory/`。
