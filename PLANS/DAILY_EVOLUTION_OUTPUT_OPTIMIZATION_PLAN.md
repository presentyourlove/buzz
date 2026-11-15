---
title: "每日進化計畫書輸出內容優化計畫書"
tags: [daily-evolution, output-optimization, plan]
status: active
created: 2026-11-15
version: "1.0"
planner: "Honey2"
reviewer: "Sentry (架構安全官)"
---

# 每日進化計畫書輸出內容優化計畫書

> **背景**：主理人要求更新「每天進化計劃書的產出內容」。當前 Antigravity 架構演進官產出的每日演化報告包含 JSON 完整報告與 Markdown 摘要，但內容結構、可讀性、行動導向度有待強化。

## 1. 現狀分析

### 1.1 當前輸出產物
| 檔案 | 位置 | 格式 | 主要內容 |
|------|------|------|----------|
| `DAILY_EVOLUTION_REPORT_YYYYMMDD.json` | `OUTBOX/` | JSON | 完整結構化資料：7 階段執行結果、共識條目、RCA 發現、代碼健康問題、任務同步狀態、知識同步條目、CI/CD 自癒動作 |
| `DAILY_EVOLUTION_SUMMARY_YYYYMMDD.md` | `OUTBOX/` | Markdown | 人類可讀摘要：總體狀態、階段完成度、關鍵指標、輸出檔案路徑 |

### 1.2 現有 7 大執行階段
1. **DisciplineGuard (紀律防護)** - 檢測工作流違規、超時任務、未完成承諾
2. **ConsensusIngest (共識入庫)** - 從頻道對話萃取架構決策、技術共識
3. **BottleneckRCA (瓶頸根因分析)** - 識別 CI/CD 失敗模式、效能退化、資源爭用
4. **CodeHealth (代碼健康)** - 編譯警告、依賴漏洞、測試覆蓋率、複雜度、死碼
5. **TaskSync (任務同步)** - 同步 Nostr Kind:30300 任務狀態、去重、衝突解決
6. **KnowledgeSync (知識同步)** - 同步 RESEARCH/GUIDES 文檔至知識庫、向量化
7. **CICDHeal (CI/CD 自癒)** - 自動修復測試失敗、依賴衝突、格式錯誤

### 1.3 當前痛點
| 痛點 | 影響 | 優先級 |
|------|------|--------|
| Markdown 摘要過於簡略，缺乏具體行動項 | 人類決策者難以快速掌握優先處理事項 | P0 |
| JSON 報告欠缺「執行摘要」欄位，機器與人類雙讀體驗差 | 需額外解析才能獲取核心結論 | P0 |
| 缺乏「風險趨勢」與「技術債累積」視覺化追蹤 | 無法觀察跨日演化趨勢 | P1 |
| 共識入庫與 RCA 發現缺乏「影響範圍」標註 | 難以評估決策影響面 | P1 |
| 無「待辦轉化」機制：發現問題未自動轉為可追蹤任務 | 發現與解決脫節 | P0 |
| 輸出檔案命名無版本/迭代標識 | 歷史追溯困難 | P2 |

## 2. 優化目標

### 2.1 核心原則
- **雙讀友好**：JSON 含 `executive_summary`，Markdown 含結構化數據區塊
- **行動導向**：每個發現必須對應 `action_items`（負責人、時限、驗收標準）
- **趨勢可視**：新增跨日指標趨勢圖表（ASCII/Markdown 表格）
- **可追溯**：所有輸出含 `report_id`、`generation_pipeline_version`、`source_events`

### 2.2 具體增強項目

#### A. Markdown 摘要重構
**新增區塊：**
1. **🎯 一行摘要** - 50 字內核心結論
2. **📊 關鍵指標儀表板** - 表格化：健康分數、風險等級、待辦總數、自癒成功率
3. **🔴 立即行動區** - P0 問題：標題、影響範圍、建議動作、負責人、截止
4. **🟡 關注追蹤區** - P1 問題：趨勢圖、累積天數、預估爆發點
5. **🟢 持續改善區** - P2 建議：最佳化方向、預期收益
6. **📈 趨勢快照** - 近 7 日關鍵指標走勢（Markdown 表格）
7. **🔗 追溯鏈接** - 對應 JSON、原始 Nostr 事件、Git 提交

#### B. JSON 報告增強
```json
{
  "report_id": "uuid-v7",
  "generation_pipeline_version": "antigravity@1.2.0",
  "date": "2026-11-15",
  "executive_summary": {
    "one_liner": "核心結論 50 字",
    "health_score": 78,
    "risk_level": "MEDIUM",
    "top_3_risks": [...],
    "top_3_opportunities": [...]
  },
  "action_items": [
    {
      "id": "ACT-20261115-001",
      "source_stage": "BottleneckRCA",
      "title": "修復 CI 測試不穩定",
      "description": "...",
      "priority": "P0",
      "assignee": "Pollen",
      "due_date": "2026-11-16",
      "acceptance_criteria": "連續 3 次 CI 通過",
      "related_findings": ["RCA-20261115-003"]
    }
  ],
  "trend_snapshot": {
    "health_score_7d": [82, 80, 79, 78, 77, 78, 78],
    "open_issues_7d": [12, 13, 15, 14, 16, 18, 19],
    "cicd_success_rate_7d": [0.95, 0.93, 0.91, 0.89, 0.88, 0.87, 0.86]
  },
  "stage_results": [...]
}
```

#### C. 新增輸出產物
| 新檔案 | 用途 | 格式 |
|--------|------|------|
| `DAILY_EVOLUTION_ACTIONS_YYYYMMDD.md` | 純行動清單，供專案管理工具導入 | Markdown (表格) |
| `DAILY_EVOLUTION_TRENDS_YYYYMMDD.json` | 歷史趨勢數據，供儀表板讀取 | JSON (時間序列) |
| `DAILY_EVOLUTION_BRIEF_YYYYMMDD.md` | 30 秒閱讀版，供早會同步 | Markdown (極簡) |

## 3. 實施計畫

### 3.1 修改檔案清單
| 檔案 | 修改類型 | 說明 |
|------|----------|------|
| `skills/antigravity/src/types.rs` | 增強 | 新增 `ExecutiveSummary`、`ActionItem`、`TrendSnapshot` 結構 |
| `skills/antigravity/src/evolution_pipeline.rs` | 增強 | 在 `execute()` 中彙總生成增強輸出 |
| `skills/antigravity/src/lib.rs` | 增強 | `print_report()` 輸出新格式；新增 `write_enhanced_outputs()` |
| `skills/antigravity/src/report_renderer.rs` | 新增 | 專職渲染器：Markdown/JSON/Brief 三格式輸出 |
| `skills/antigravity/src/trend_store.rs` | 新增 | 趨勢數據持久化（SQLite/JSONL）與 7 日滾動窗口計算 |
| `skills/antigravity/Cargo.toml` | 微調 | 新增 `comfy-table`、`chrono` 若無 |

### 3.2 分期交付
| 期次 | 範圍 | 驗收標準 |
|------|------|----------|
| **第一期 MVP** (1-2 天) | 1. 新增 `ExecutiveSummary`、`ActionItem` 結構<br>2. 重構 Markdown 摘要含「立即行動區」<br>3. JSON 含 `executive_summary`、`action_items`<br>4. 新增 `DAILY_EVOLUTION_ACTIONS_YYYYMMDD.md` | - `cargo test -p antigravity` 全綠<br>- 實際執行產出符合新格式<br>- Markdown 含具體負責人/截止/驗收 |
| **第二期 增強** (1 週內) | 1. 實作 `TrendStore` 趨勢持久化<br>2. 新增趨勢快照區塊（7 日走勢）<br>3. 新增 `DAILY_EVOLUTION_TRENDS_YYYYMMDD.json`<br>4. 新增 `DAILY_EVOLUTION_BRIEF_YYYYMMDD.md` | - 連續 7 天執行趨勢數據正確<br>- Brief 版 ≤ 30 秒閱讀完畢<br>- 趨勢 JSON 可被外部儀表板直讀 |
| **第三期 完善** (視需求) | 1. 行動項自動同步至 Nostr Kind:30300 任務<br>2. 風險等級自動計算演算法<br>3. 共識/RCA 影響範圍自動標註 | - 行動項雙向同步驗證通過<br>- 風險等級與人工評估一致性 > 90% |

## 4. 驗收標準

### 4.1 功能性驗收
- [ ] Markdown 摘要含「一行摘要」、「關鍵指標儀表板」、「立即行動區」、「關注追蹤區」、「持續改善區」、「趨勢快照」、「追溯鏈接」
- [ ] JSON 報告含 `executive_summary`、`action_items`、`trend_snapshot` 三大新欄位
- [ ] 每個 `ActionItem` 必含：`id`、`source_stage`、`title`、`priority`、`assignee`、`due_date`、`acceptance_criteria`、`related_findings`
- [ ] 新增 3 個輸出檔案正確生成於 `OUTBOX/`
- [ ] 趨勢數據正確計算 7 日滾動窗口

### 4.2 品質性驗收
- [ ] `cargo test -p antigravity -- --test-threads=4` 100% 通過
- [ ] `cargo clippy -p antigravity -D warnings` 零警告
- [ ] Markdown 輸出經 `markdownlint` 無錯誤
- [ ] JSON 輸出經 `jq` 結構驗證通過
- [ ] 單次執行耗時不超過現有基準 120%

### 4.3 回測驗收
- [ ] 以過去 7 天真實數據重跑，對比舊/新格式資訊完整度
- [ ] 確認無資訊遺失（舊格式所有欄位在新格式可對應找到）

## 5. 風險與回滾

| 風險 | 可能性 | 影響 | 緩解 |
|------|--------|------|------|
| 結構變更破壞下游消費者（儀表板、腳本） | 中 | 高 | 保留舊欄位相容，新欄位獨立新增；發布前通知 Wax/Pollen |
| 趨勢計算邏輯錯誤導致誤導性圖表 | 低 | 中 | 單元測試覆蓋 `TrendStore` 邊界條件；人工抽樣核對首週 |
| 輸出檔案過大影響同步效能 | 低 | 低 | JSON 壓縮選項；Brief 版專供人類閱讀 |

**回滾機制**：
```bash
# 回滾至上一版本並停用排程
git -C "C:/私人/ai agent/buzz" reset --hard HEAD~1
# 停用 Windows 排程任務
schtasks /End /TN "Antigravity_Daily_Evolution" 2>nul
schtasks /Disable /TN "Antigravity_Daily_Evolution" 2>nul
```

## 6. 簽署與審核

- **企劃**：Honey2
- **審核**：Sentry (架構安全官)
- **實作**：Pollen (核心技術主管)
- **歸檔**：Wax (第二大腦記憶官)

> **Sentry Audit**：PENDING — 待審核
> **主理人授權**：2026-11-15 同意立項
