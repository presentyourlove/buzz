---
title: "YYYY-MM-DD Evolution Proposal"
tags: [evolution, auto-generated]
status: pending-approval
created: YYYY-MM-DD
harvest_log: "WORK_LOGS/YYYY-MM-DD_Harvest.jsonl"
---

## 執行摘要

今日頻道對話與工作紀錄的自動化分析摘要，包含關鍵決策、行動項目、阻礙與優化建議。

---

## 風險評估

| 風險項目 | 等級 | 說明 | 緩解措施 |
|----------|------|------|----------|
| 自動化腳本失敗 | 高 | 連續 2 次失敗觸發熔斷 | 人工介入、清除失敗日誌、手動重跑 |
| API Key 缺失 | 高 | 環境變數未配置導致腳本無法執行 | 部署前驗證、優雅降級並記錄錯誤 |
| 資料遺漏 | 中 | 增量抓取游標錯誤導致訊息遺漏 | 使用 Nostr 事件 ID 去重、保留完整日誌 |

---

## 具體變更清單（含檔案路徑、增量 diff 方向）

| 項次 | 檔案路徑 | 變更類型 | Diff 方向 | 說明 |
|------|----------|----------|-----------|------|
| 1.1 | `scripts/daily-harvest.ts` | 新增 | + | 每日訊息抓取腳本（TypeScript） |
| 1.2 | `scripts/daily-evolution-trigger.sh` | 新增 | + | 觸發器整合腳本（含熔斷機制） |
| 1.3 | `scripts/daily_evolution/daily_evolution_workflow.yaml` | 修改 | ~ | 更新時區為 Asia/Taipei、Webhook 指向 trigger 腳本 |
| 1.4 | `PLANS/EVOLUTION_PROPOSAL_TEMPLATE.md` | 新增 | + | 符合計畫規格的提案模板 |

---

## 驗收測試步驟

1. **環境變數驗收**
   - 設定 `BUZZ_RELAY_URL`、`BUZZ_PRIVATE_KEY`、`BUZZ_AUTH_TAG`
   - 執行 `npx ts-node scripts/daily-harvest.ts --channel <UUID> --since <TS> --out-dir WORK_LOGS`
   - 驗證輸出 `WORK_LOGS/YYYY-MM-DD_Harvest.jsonl` 為有效 NDJSON

2. **觸發器整合驗收**
   - 設定相同環境變數
   - 執行 `bash scripts/daily-evolution-trigger.sh`
   - 驗證生成 `PLANS/DAILY_EVOLUTION_PROPOSAL_YYYYMMDD.md`
   - 驗證頻道收到摘要通知

3. **熔斷機制驗收**
   - 連續製造 2 次失敗（如移除 API Key）
   - 驗證第 3 次執行被阻擋並發送告警到頻道
   - 清除 `WORK_LOGS/daily_evolution_failure.log` 後驗證恢復執行

4. **Windows 相容性驗收**
   - 在 PowerShell 環境執行上述步驟
   - 驗證長路徑（`\?\` 前綴）正常運作
   - 驗證路徑分隔符 `path.join()` 正確處理

5. **Workflow 部署驗收**
   - 部署 Webhook 端點接收 `daily-evolution-trigger.sh`
   - 執行 `buzz workflows create --channel <UUID> --yaml scripts/daily_evolution/daily_evolution_workflow.yaml`
   - 驗證排程於 22:30 Asia/Taipei 觸發

---

## 回滾方案

1. **腳本層級回滾**
   ```bash
   git checkout HEAD -- scripts/daily-harvest.ts scripts/daily-evolution-trigger.sh
   git checkout HEAD -- scripts/daily_evolution/daily_evolution_workflow.yaml
   ```

2. **Workflow 停用**
   ```bash
   buzz workflows disable --id <workflow-id>
   ```

3. **資料清理**
   - 刪除生成的收割檔：`WORK_LOGS/YYYY-MM-DD_Harvest.jsonl`
   - 刪除生成的提案：`PLANS/DAILY_EVOLUTION_PROPOSAL_YYYYMMDD.md`
   - 清除失敗日誌：`> WORK_LOGS/daily_evolution_failure.log`

4. **Cron 作業移除**
   ```bash
   crontab -l | grep -v "daily-evolution-trigger.sh" | crontab -
   ```
