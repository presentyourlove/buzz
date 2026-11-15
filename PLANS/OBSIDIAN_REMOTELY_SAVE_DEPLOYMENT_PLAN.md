---
title: "Obsidian Remotely Save Plugin 自動部署實施計畫書"
tags: [obsidian, plugin, deployment, automation]
status: active
created: 2026-10-25
---

# Obsidian Remotely Save Plugin 自動部署實施計畫書

## 1. 專案概述

**目標**：將 Obsidian 開源外掛「Remotely Save」自動部署至 `c:\私人\ai agent\buzz-obsidian\.obsidian\plugins\remotely-save\`

**外掛資訊**：
- **外掛 ID**：`remotely-save`
- **外掛名稱**：Remotely Save
- **最新版本**：0.5.25 (2024-10-20 發布)
- **作者**：fyears / remotely-save 組織
- **原始碼倉庫**：https://github.com/remotely-save/remotely-save
- **釋出頁面**：https://github.com/remotely-save/remotely-save/releases

## 2. 外掛安裝結構分析

### 2.1 標準 Obsidian 外掛目錄結構

```
<vault-root>/
└── .obsidian/
    └── plugins/
        └── remotely-save/          ← 外掛目錄 (與 manifest.json 的 id 一致)
            ├── main.js             ← 編譯後的主要程式碼 (~4.2 MB)
            ├── manifest.json       ← 外掛元資料 (374 bytes)
            └── styles.css          ← 樣式表 (3 KB)
```

### 2.2 必要檔案說明

| 檔案 | 來源 | 用途 |
|------|------|------|
| `main.js` | GitHub Release assets | 外掛核心邏輯，TypeScript 編譯產物 |
| `manifest.json` | GitHub Release assets / 原始碼 | 外掛識別資訊 (id, name, version, minAppVersion 等) |
| `styles.css` | GitHub Release assets | 外掛 UI 樣式 |

### 2.3 Manifest.json 關鍵欄位

```json
{
  "id": "remotely-save",
  "name": "Remotely Save",
  "version": "0.5.25",
  "minAppVersion": "0.13.21",
  "description": "Yet another unofficial plugin allowing users to synchronize notes between local device and the cloud service.",
  "author": "fyears",
  "authorUrl": "https://github.com/fyears",
  "isDesktopOnly": false,
  "fundingUrl": "https://remotelysave.com"
}
```

**關鍵點**：`id` 必須與目錄名稱 `remotely-save` 完全一致，否則 Obsidian 無法識別外掛。

## 3. 部署策略規劃

### 3.1 部署方式比較

| 方式 | 優點 | 缺點 | 推薦度 |
|------|------|------|--------|
| **GitHub Release 下載 (推薦)** | 官方預編譯、版本固定、無需建置環境 | 需網路連線 | ⭐⭐⭐⭐⭐ |
| 從源碼建置 | 可自訂版本、可修改程式碼 | 需 Node.js/pnpm、建置時間長 | ⭐⭐ |
| BRAT 外掛管理 | 自動更新、支援開發版 | 需額外安裝 BRAT、非標準部署 | ⭐⭐⭐ |

### 3.2 採用策略：GitHub Release 直接下載

**理由**：
1. 官方提供預編譯的 `main.js`、`manifest.json`、`styles.css`
2. 版本明確可控，支援指定版本或最新版
3. 無建置依賴，適合自動化腳本
4. 檔案完整性可透過 SHA256 驗證

### 3.3 部署流程設計

```
開始
  ↓
檢查目標目錄是否存在，不存在則建立
  ↓
獲取最新版本號 (或使用指定版本)
  ↓
並行下載三個檔案到暫存目錄
  ↓
驗證檔案完整性 (大小、SHA256 可選)
  ↓
原子性替換：移動暫存檔案到目標目錄
  ↓
驗證部署結果 (檔案存在、manifest.json 可解析、id 匹配)
  ↓
清理暫存檔案
  ↓
完成
```

## 4. 實施計畫

### 4.1 階段一：環境準備與目錄建立

**任務**：
- [ ] 建立目標目錄 `c:\私人\ai agent\buzz-obsidian\.obsidian\plugins\remotely-save\`
- [ ] 建立暫存下載目錄 (建議：`%TEMP%\remotely-save-deploy-<timestamp>\`)
- [ ] 驗證寫入權限

**預估時間**：5 分鐘

### 4.2 階段二：版本獲取邏輯

**任務**：
- [ ] 實作 GitHub API 呼叫獲取最新 Release 資訊
  - API: `GET https://api.github.com/repos/remotely-save/remotely-save/releases/latest`
- [ ] 解析 `tag_name` (如 `0.5.25`)
- [ ] 支援可選參數：指定版本號

**預估時間**：15 分鐘

### 4.3 階段三：檔案下載與驗證

**任務**：
- [ ] 並行下載三個資產檔案：
  - `main.js` → `https://github.com/remotely-save/remotely-save/releases/download/<tag>/main.js`
  - `manifest.json` → `https://github.com/remotely-save/remotely-save/releases/download/<tag>/manifest.json`
  - `styles.css` → `https://github.com/remotely-save/remotely-save/releases/download/<tag>/styles.css`
- [ ] 實作下載重試機制 (指數退避，最多 3 次)
- [ ] 驗證檔案大小與 Release 資訊一致
- [ ] (可選) 計算 SHA256 並與已知值比對

**預估時間**：30 分鐘

### 4.4 階段四：原子性部署

**任務**：
- [ ] 使用暫存目錄下載，驗證無誤後再移動
- [ ] Windows 上使用 `Move-Item -Force` 實現原子替換
- [ ] 保留舊版本備份 (可選，建議保留 1 代)
- [ ] 處理檔案鎖定情況 (Obsidian 運行中可能鎖定檔案)

**預估時間**：20 分鐘

### 4.5 階段五：部署後驗證

**任務**：
- [ ] 確認三個檔案皆存在於目標目錄
- [ ] 解析 `manifest.json` 驗證 JSON 格式正確
- [ ] 驗證 `manifest.json` 中的 `id` 等於 `remotely-save`
- [ ] 驗證 `version` 欄位與預期版本一致
- [ ] 輸出部署摘要日誌

**預估時間**：10 分鐘

### 4.6 階段六：自動化腳本封裝

**任務**：
- [ ] 撰寫 PowerShell 腳本 `deploy-remotely-save.ps1`
- [ ] 支援參數：
  - `-Version` (可選，預設 latest)
  - `-TargetPath` (可選，預設 `c:\私人\ai agent\buzz-obsidian\.obsidian\plugins\remotely-save`)
  - `-Force` (跳過確認提示)
  - `-Backup` (備份舊版本)
- [ ] 加入詳細日誌輸出 (Write-Host, Write-Verbose)
- [ ] 返回適當的 Exit Code (0=成功, 非0=失敗)

**預估時間**：30 分鐘

## 5. 風險評估與緩解措施

| 風險 | 機率 | 影響 | 緩解措施 |
|------|------|------|----------|
| GitHub API 速率限制 | 中 | 高 | 使用 `If-None-Match` / `ETag` 快取；提供離線模式 |
| 網路不稳定導致下載失敗 | 中 | 中 | 重試機制 + 超時設定 (30s) |
| Obsidian 運行中檔案被鎖定 | 高 | 高 | 檢測鎖定 → 提示關閉 Obsidian → 強制覆蓋選項 |
| 目標目錄權限不足 | 低 | 高 | 腳本啟動時檢查寫入權限，失敗早期退出 |
| Manifest.json 版本不匹配 | 低 | 中 | 部署後強制驗證 version 欄位 |
| GitHub Release 結構變更 | 低 | 高 | 版本檢查邏輯解耦；支援手動指定下載 URL |

## 6. 檔案路徑與配置參考

### 6.1 關鍵路徑

```powershell
# 目標部署目錄
$TargetDir = "c:\私人\ai agent\buzz-obsidian\.obsidian\plugins\remotely-save"

# GitHub Release 下載 URL 模板
$BaseUrl = "https://github.com/remotely-save/remotely-save/releases/download"
$Files = @("main.js", "manifest.json", "styles.css")
```

### 6.2 部署腳本參數設計

```powershell
param(
    [string]$Version = "latest",           # 版本標籤，如 "0.5.25" 或 "latest"
    [string]$TargetPath = "c:\私人\ai agent\buzz-obsidian\.obsidian\plugins\remotely-save",
    [switch]$Force,                        # 跳過確認提示
    [switch]$Backup,                       # 備份現有版本
    [switch]$WhatIf                        # 模擬執行不實際寫入
)
```

## 7. 驗收標準

部署視為成功需滿足所有條件：

- [ ] 目標目錄存在且包含三個檔案：`main.js`、`manifest.json`、`styles.css`
- [ ] `manifest.json` 為有效 JSON，且 `id` = `"remotely-save"`
- [ ] `manifest.json` 的 `version` 與請求版本一致
- [ ] `main.js` 檔案大小 > 1 MB (確保非空檔/錯誤頁面)
- [ ] 無錯誤日誌輸出，Exit Code = 0

## 8. 後續擴充建議

1. **版本鎖定機制**：記錄已部署版本到 `.deployed-version` 檔案，支援增量更新檢查
2. **定時自動更新**：配合 Windows Task Scheduler 定期檢查新版本
3. **多 Vault 支援**：參數化 Vault 路徑，支援批次部署到多個 Vault
4. **離線安裝包**：預先下載打包為 ZIP，支援完全離線環境部署
5. **健康檢查**：部署後自動啟動 Obsidian (headless) 驗證外掛可正常載入

## 9. 實施時程總覽

| 階段 | 工作項目 | 預估工時 | 累計 |
|------|----------|----------|------|
| 1 | 環境準備與目錄建立 | 0.1h | 0.1h |
| 2 | 版本獲取邏輯 | 0.25h | 0.35h |
| 3 | 檔案下載與驗證 | 0.5h | 0.85h |
| 4 | 原子性部署 | 0.35h | 1.2h |
| 5 | 部署後驗證 | 0.15h | 1.35h |
| 6 | 自動化腳本封裝 | 0.5h | 1.85h |
| **總計** | | **~2 小時** | |

---

**文件版本**：1.0  
**建立者**：Honey  
**狀態**：待審核執行
