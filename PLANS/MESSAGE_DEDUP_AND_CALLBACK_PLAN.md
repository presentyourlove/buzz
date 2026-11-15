---
title: "訊息去重與回報閉環最小可行實作計畫書"
tags: [messaging, idempotency, callback, mvp]
status: active
created: 2026-11-01
---

# 計畫書 v2.1 完整版

## 計畫概覽

| 項目 | 內容 |
|------|------|
| 目標 | 在 send_channel_message 植入本地去重、client 標籤、同步回報 |
| 範圍 | 僅修改 4 個檔案：commands/messages.rs、events.rs、relay/submit.rs、app_state.rs |
| 原則 | KISS/YAGNI/MVP，利用既有 client_tags 參數 |
| 門禁 | 對齊 @Sentry 四大門禁：符號真實性、協議閉環、失敗推演、MVP 引導 |
| 狀態 | v2.1 完整版，門禁 1、2、4 通過，門禁 3 已修正 6 項缺陷 |

## 真實代碼錨點

- commands/messages.rs:409 send_channel_message
- events.rs:285 build_message_with_client_tags 原生支援 client_tags
- message_tags.rs:105 append_client_tags
- relay/submit.rs:100 submit_event_at_created_at
- commands/messages.rs:616 has_managed_agent_channel_message_marker
- app_state.rs:19 AppState struct

## 關鍵修正（對應 Sentry 6 項缺陷）

1. 快取寫入時機：提交成功後 confirm，失敗 mark_failed
2. 原子 try_reserve：Mutex 內完成，Pending 防重入
3. catch_unwind 保護 callback
4. 重啟先查 Relay 標籤（二層去重）
5. LRU 僅清 Confirmed/Failed
3. 佔位 event_id 在 confirm 階段填入真實值

## 三狀態機

Pending -> Confirmed / Failed -> LRU 清除（僅後兩者）

## 五步實作

### 步驟1：DedupCache + compute_idempotency_key + try_reserve/confirm/mark_failed
### 步驟2：build_message 新增 client_tags 參數透傳
### 步驟3：submit_event_at_created_at 加 catch_unwind 回報
### 步驟4：AppState 新增 message_dedup_cache、completion_callback
### 步驟5：Tauri Event 系統替代 callback

## 驗收標準

- 冪等測試、並發測試、429/網路失敗測試、panic 測試、重啟一致性、LRU 測試

## 回滾機制

git checkout 4 個檔案 + Cargo.toml，cargo build 驗證

## 失敗模式對照表

| # | 場景 | 修正 | 機制 |
|---|------|------|------|
| 1 | Relay 429 | 標記 Failed 重試 | try_reserve -> mark_failed |
| 2 | 網路分區 | 標記 Failed 重試 | 同上 |
| 3 | 並發發送 | 真正原子 | Pending 防重入 |
| 4 | Callback panic | 捕獲不傳播 | catch_unwind |
| 5 | 重啟遺失 | 先查 Relay | has_managed_agent... |
| 6 | LRU 誤刪 | 僅清 Confirmed/Failed | filter != Pending |

版本：v2.1 完整版
作者：@Honey2
審查人：@Sentry
授權人：可愛小寶寶
執行人：@Pollen
