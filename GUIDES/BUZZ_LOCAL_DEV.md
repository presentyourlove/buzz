---
title: "Buzz Local Development & Incremental Modification Standard"
tags: [development, standard, git, workflow]
status: active
created: 2026-10-25
---

# Buzz 本地開發與增量修改標準指引手冊

本手冊旨在規範團隊成員（Honey、Pollen 及未來所有代理與協同開發者）在進行 Buzz 專案原始碼修改與演進時的標準作業程序（SOP），以避免虛假回報、盲目覆寫或建立不必要的空白 Mock 專案。

---

## 1. 本地真實專案路徑規範
- **絕對路徑對齊**：所有針對 Buzz 原始碼的操作與修改，必須精準對應至真實的本機開發專案路徑：
  `c:\私人\ai agent\buzz`
- **嚴禁建立空白 Mock 庫**：絕不能在 `.buzz/REPOS` 或其他相對路徑下建立同名或空白的假專案來替代真實程式碼庫，確保所有的 Patch 與實作皆直接反映在真實生產/開發專案中。

---

## 2. 既有大型模組的「增量修改原則」
- **先讀取、後修改**：在修改任何既有程式碼（如 `crates/buzz-workflow/src/schema.rs` 等千行大檔）之前，必須先使用 `read_file` 仔細閱讀相關模組的上下文結構與定義。
- **嚴禁整檔覆寫**：禁止將整個檔案重新輸出或直接覆蓋。必須採用精準的區域性增量修改（Incremental Edit / Atomic Replace），僅在適當的 `enum`、`struct` 或 `trait` 中加入所需的變體、欄位或方法。

---

## 3. Git Patch 產出與存放到 OUTBOX 的標準 SOP
- **確認變更狀態**：在目標專案目錄下執行 `git status` 與 `git diff`，確認所有修改皆為精準且無冗餘的增量變更。
- **標準輸出路徑**：將 Git diff 導出並儲存至專案統一的 OUTBOX 目錄：
  `C:\Users\user\.buzz\OUTBOX/PHASE2_NATIVE_DIGEST.patch`（或依任務命名之對應 patch 檔案）。
- **驗證 Patch 存在性**：產出後必須使用 `read_file` 確認 patch 檔案確實存在且內容正確。

---

## 4. 報告進度與代碼區塊輸出規範
- **杜絕空白括號與虛假宣告**：所有進度回報必須具體、真實，絕不允許出現未填寫變數的空白括號（如 `()`) 或口頭宣告上線卻無實體檔案的狀況。
- **代碼變更摘要規範**：向主管（@presentyourlove）回報成果時，必須以清晰的 Markdown 代碼區塊（code block）列出修改的行數、模組位置與 diff 摘要，確保資訊透明且可供交叉審查。
- **強制標註與回覆規範**：頻道回報時必須使用 `--reply-to` 保持 Thread 討論串結構，並正確標註 `@presentyourlove`。

---

## 5. 完工標準 DoD 鐵律（嚴禁半成品）
- **嚴禁 Mock 假資料（No Mock/Stub）**：實作必須對接真實後端 API、資料庫（如 pgvector、SQLite）或事件流。嚴禁使用 `const mock...` 假資料或空函式敷衍宣告完工。
- **強制閉環掛載（Must be Wired Up）**：嚴禁建立無人引用的孤兒檔案（Orphaned File）。任何新增的 Hook、Trait 或模組，必須至少被 1 個正式主流程組件 `import` 並實際調用。
- **端到端 Patch（End-to-End Patch）**：導出的 Git Patch 必須同時涵蓋「新模組本體」與「調用點組件的修改」，兩者缺一不可。未達標者一律視為未完工。
