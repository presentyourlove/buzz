---
title: "Alpha Research MVP 技能規格書"
tags: [alpha-research, quant-methods, mvp, specification]
status: active
created: 2026-11-06
source: "C:\Users\user\.buzz\REPOS\everything-claude-trading\skills\quant-methods\alpha-research\SKILL.md"
---

# Alpha Research MVP 技能規格書

## 1. 技能身份
- **技能名稱**: alpha-research
- **分類**: quant-methods / alpha-research
- **來源文件**: `C:\Users\user\.buzz\REPOS\everything-claude-trading\skills\quant-methods\alpha-research\SKILL.md`
- **版本**: 基於現有 SKILL.md（201 行，約 8.7 KB）

## 2. 功能定義與啟用條件

### 2.1 核心功能
Alpha 訊號的發掘、測試、評估與管理。提供系統化的研究流程，確保訊號具備真實預測力而非數據挖掘產物。

### 2.2 啟用觸發條件（When to Activate）
- 使用者探索資產報酬的新預測訊號
- 評估交易想法是否具備真實預測力
- 將多個 Alpha 訊號組合為綜合訊號
- 診斷 Alpha 訊號衰減的原因
- 設計系統化訊號發掘研究管線
- 對訊號庫套用多重測試修正

### 2.3 首輪澄清問題
1. Universe 與資產類別（股票、期貨、外匯、加密貨幣）？
2. 是橫斷面訊號（選股排序）還是時間序列訊號（單一資產計時）？
3. 預期持有期（日內、天、週、月）？
4. 已測試多少訊號（需作多重測試調整）？
5. 是否有 point-in-time 資料，還是存在 look-ahead 風險？

## 3. 核心概念與理論基礎

### 3.1 假設驅動研究
最強的 Alpha 訊號源於經濟假設，而非數據挖掘。有效假設必須回答：
- **為何**此訊號能預測報酬？（行為偏誤、結構性限制、資訊不對稱）
- **誰**在交易另一邊？（識別虧損方）
- **為何**此機會尚未被套利消失？

### 3.2 關鍵指標體系
| 指標 | 定義 | 基準值 |
|------|------|--------|
| **IC (Information Coefficient)** | 訊號與未來報酬的 Spearman/IC 相關 | > 0.02 為可觀 |
| **ICIR (IC Information Ratio)** | IC_mean / IC_std | > 0.10 值得關注；> 0.25 為強健 |
| **Quintile Spread** | Q1-Q5 長短腳報酬差 | 需單調遞增 |
| **Break-even Cost** | gross_alpha / turnover | 必須 > 實際交易成本 |

### 3.3 主動管理基本法
**IR = IC × √BR × TC**
- IR: Information Ratio（超額報酬/追蹤誤差）
- IC: 訊號品質
- BR: Breadth（年度獨立押注數）
- TC: Transfer Coefficient（投組實施效率，0-1）

含義：弱訊號（低 IC）若應用於眾多資產（高 BR），仍可產出強策略。

## 4. 詳細方法論

### 4.1 IC 衰減分析
```
計算多期限 IC：1日、5日、10日、21日、63日
繪製 IC vs Horizon 衰減曲線：
- 快速衰減 (< 5 天)：高頻交易訊號
- 中速衰減 (5-21 天)：系統化策略最佳區
- 慢速衰減 (21-63+ 天)：低換手、容量友善
- 非單調衰減：特定期限可能反轉
```
最佳持有期 = IC × √horizon 最大化處。

### 4.2 換手率分析
```
單期換手率 = sum(|w_t - w_{t-1}|) / 2
年化換手率 = 單期換手率 × 年期數
成本調整後 Alpha = gross_alpha - (turnover × round_trip_cost)
損益平衡成本 = gross_alpha / turnover
```
若損益平衡成本 < 實際交易成本 → 訊號無利可圖。

### 4.3 多重測試修正
- **Bonferroni**：顯著門檻除以 N（保守）
- **Holm-Bonferroni**：分步改良版 Bonferroni
- **Benjamini-Hochberg (FDR)**：控制虛假發現率，較不保守，常用
- **Harvey-Liu-Zhu haircut**：根據文獻測試因子數調整 t-stat，門檻 t > 3.0
- **Sharpe Ratio haircut (Bailey-Lopez de Prado)**：SR_adj = SR × √(1 - N_tests/N_obs)

### 4.4 樣本外驗證協定
```
步驟 1：資料切分 — 60% 內樣、20% 驗證、20% 保留
步驟 2：僅在內樣開發訊號
步驟 3：驗證集調參
步驟 4：保留集最終評估 — 僅一次機會，不可迭代
步驟 5：比較保留集 vs 內樣表現：
         - OOS/IS SR 比 > 0.5：強健
         - 0.3-0.5：邊緣，謹慎推進
         - < 0.3：疑似過擬合，捨棄或重設計
```

### 4.5 訊號組合方法
| 方法 | 特性 |
|------|------|
| 等權重 | 簡單、強健、無估計誤差 |
| IC 加權 | 依追蹤 IC 或 ICIR 加權 |
| 優化權重 | 訊號報酬均值-方差優化，但估計誤差風險高 |
| 排名平均 | 各訊號排名 → 平均排名 → 重新排名，非參數強健 |
| 機器學習 | 訊號作特徵，配合 purged CV 的樹模型/線性模型 |

### 4.6 Alpha 衰減管理
**衰減指標**：
- 滾動 12 個月 IC 下降
- 總換手率不變而總夏普下降
- 擁擠度指標上升
- 學術發表後典型衰減 30-50%

**應對策略**：
- 多訊號分散，單一衰減不致命
- 持續研究新訊號（研究管線）
- 修改訊號構建（已知主題的專有變體）
- 動態降低衰減訊號的部位權重

## 5. 模板與範例

### 5.1 Alpha 訊號評估報告模板
```
Signal Name: [如 Earnings Revision Breadth]
Universe: S&P 500
Signal Frequency: Monthly
Test Period: 2005-01-01 to 2024-12-31 (IS: 2005-2017, OOS: 2018-2024)

--- In-Sample ---
IC Mean:            0.042
IC Std:             0.085
ICIR:               0.49
Monthly Turnover:   18%
Quintile Spread:    65 bps/month (Q1 - Q5)
Hit Rate:           58%
Max DD (L/S):       -12.3%
Sharpe (L/S):       1.25

--- Out-of-Sample ---
IC Mean:            0.031
IC Std:             0.092
ICIR:               0.34
Sharpe (L/S):       0.88
OOS/IS SR Ratio:    0.70

--- Correlation with Existing Alphas ---
Alpha_momentum:     0.15
Alpha_value:        -0.08
Alpha_quality:      0.22

Verdict: PASS — 強健 OOS 表現，與既有訊號相關性低
```

### 5.2 研究管線檢核表
```
[ ] 假設文件化並記錄經濟論據
[ ] 資料來源具 point-in-time 時間戳
[ ] 處理倖存者偏誤（含除牌證券）
[ ] 每次再平衡日計算訊號
[ ] 全樣本計算 IC 與 ICIR
[ ] 繪製多期限 IC 衰減曲線
[ ] 計算換手率與損益平衡成本
[ ] 五分位分析：要求單調展開
[ ] 產業與市值中性化測試
[ ] 套用多重測試修正（記錄總測試數）
[ ] 保留期樣本外驗證
[ ] 與既有訊號相關性 < 0.40
[ ] 容量分析：能承接目標 AUM？
[ ] 含真實成本的實施模擬
```

## 6. 品質門檻
將 Alpha 訊號納入生產前必須通過：

- [ ] IC 為正且經多重測試修正後統計顯著
- [ ] 樣本外 ICIR > 0.10
- [ ] OOS/IS Sharpe 比 > 0.50（非過擬合）
- [ ] 換手率與實際交易成本相容（損益平衡成本 > 實際成本）
- [ ] 不與既有生產訊號高度相關（> 0.40）
- [ ] IC 衰減曲線符合預期持有期
- [ ] 訊號具備目標配置所需容量
- [ ] 訊號構建無 look-ahead bias
- [ ] 經濟論據文件化且可辯護
- [ ] 建立持續 IC 追蹤監控儀表板

## 7. MVP 實作範圍與驗收標準

### 7.1 MVP 範圍（第一期交付）
| 項目 | 內容 | 驗收標準 |
|------|------|----------|
| 核心計算 | IC、ICIR、Quintile Spread、Turnover | 單元測試覆蓋 ≥ 90%，數值與參考實作一致 |
| 衰減分析 | 多期限 IC 計算與繪圖 | 能輸出 1/5/10/21/63 日 IC 曲線 |
| 多重測試 | Bonferroni、Holm、BH-FDR 三種修正 | 給定 p-values 輸出修正後門檻 |
| 樣本外驗證 | 60/20/20 切分與 OOS/IS 比率 | 自動化流程，禁止迭代保留集 |
| 報告輸出 | 評估報告模板自動填充 | Markdown 格式，含所有關鍵指標 |

### 7.2 非功能性需求
- **效能**：單一訊號全樣本 IC 計算 < 500ms（10 年日頻、500 檔股票）
- **可重現**：固定隨機種子、記錄完整參數與資料版本
- **擴充性**：模組化設計，支援自訂指標與修正方法

## 8. 相依性與整合點
- **上游**：需 point-in-time 價量基礎資料、財報修正資料
- **下游**：訊號輸出供 Portfolio Construction、Risk Management 使用
- **並行**：與 factor-models、regime-detection 技能協作

## 9. 風險與限制
- var-models 技能目前僅有 frontmatter，需補齊波動率模型才能完整支援成本調整
- 實際部署需對接實盤交易成本模型（滑價、手續費、市場衝擊）

## 10. 後續擴充建議（第二期）
- 引入機器學習訊號組合（purged CV + 特徵重要度）
- 線上學習 / 適應性 IC 加權
- 替代資料整合管線（衛星圖像、信用卡消費、爬蟲資料）

---

**規格書狀態**：草稿完成，待 @Sentry 架構審核
**提交時間**：2026-11-06
**撰寫者**：Honey2
