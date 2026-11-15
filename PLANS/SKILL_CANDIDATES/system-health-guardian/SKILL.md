---
name: system-health-guardian
description: >
  自動化執行 CLI 狀態診斷、環境變數檢驗、熔斷日誌排查與常駐守護重啟標準作業流程。
version: 1
---

# Buzz 系統健康監控與熔斷自癒技能

> **提煉來源**：自系統維運歷史與健康檢查機制中提煉 (觸發詞頻: 7)  
> **狀態**：候選中 (Pending Approval)  
> **規格標準**：Buzz Native Skill v1.0

---

## 🎯 觸發場景 (Trigger Conditions)
- 日常進化流程啟動前之例行前置體檢
- 背景守護行程異常退出或通訊失敗時
- 熔斷器跳脫 (Circuit Breaker Tripped) 時之故障排查

---

## 📋 輸入與前置需求 (Prerequisites)
- 本機具備 Python 3 與 Node.js 執行環境
- 具備讀取 WORK_LOGS/daily_evolution_failure.log 權限
- 可調用 buzz-background-services.vbs 進行常駐守護管理

---

## 🛠️ 標準作業程序 (Standard Operating Procedure)
1. 執行 health_check.py 檢查 Buzz CLI 與核心變數
2. 檢查 WORK_LOGS/ 下之失敗日誌與錯誤堆疊
3. 檢視 3 大背景常駐行程 (OpenRouter, Sync, Proxy) 運作狀態
4. 若發現熔斷日誌，排查根本原因並於確認修復後進行重置
5. 彙整健全性報告呈報至專屬頻道

---

## 🔒 安全與授權約束 (Security & Authorization Constraints)
- 遇不可逆之系統崩潰必須立即停止 (STOP) 並通知老闆
- 禁止私自清除未排查之關鍵錯誤日誌
- 嚴格遵循 Buzz 代理人通訊紀律與老闆審批關卡 (Boss Approval Gatekeeper)
- 禁止私自執行未授權的高風險系統命令
