---
name: example-skill-name
description: "簡明扼要說明此技能之職責、預期產出與適用情境（繁體中文）"
argument-hint: "[<required_param>] [--option <value>]"
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
tags: [template, standard, ect-aligned]
---

# 技能名稱 (Skill Name)

> **技能標識**：`example-skill-name`  
> **維護特工**：[[Honey2]] (企劃) / [[Pollen]] (實作) / [[Sentry]] (審核)  
> **版本**：v1.0.0  

---

## 🎯 一、 啟動時機 (When to Activate)

明確列出本技能應被觸發的具體條件與業務情境：
- [ ] 用戶提出特定領域請求（如「分析某資產走勢」或「執行壓力測試」）
- [ ] 上游特工指派子任務時
- [ ] 系統生命週期事件觸發時

> ⚠️ **非適用情境 (Out of Scope)**：何種情況下**嚴禁**使用此技能（避免特工濫用）。

---

## 📥 二、 輸入介面與參數校驗 (Input Schema & Validation)

| 參數名稱 | 類型 | 必填 | 預設值 | 描述與約束規則 |
|----------|------|------|--------|----------------|
| `target_path` | string | 是 | 無 | 目標檔案或目錄路徑，必須為絕對路徑或安全相對路徑 |
| `mode` | enum | 否 | `standard` | 執行模式：`lean` (精簡) / `standard` (標準) / `full` (完整) |
| `max_limit` | integer | 否 | `100` | 處理上限，防止記憶體或 Token 溢出 |

---

## 🔄 三、 標準作業流程 (Step-by-Step SOP)

### 步驟 0：前置安全檢查 (Pre-flight Validation)
1. 檢驗入參有效性與存在性。
2. 檢查目標路徑是否觸發目錄穿越防禦（嚴禁包含 `..`）。
3. 確認特工擁有足夠的操作權限。

### 步驟 1：核心邏輯執行 (Core Execution Phase)
1. 依據輸入參數調用指定工具或腳本。
2. **大檔處理規範**：若涉及讀取大文件（>40KB），必須使用 `offset` 與 `limit` 進行分頁串流讀取。
3. 嚴禁在未備份的情況下覆寫歷史關鍵檔案。

### 步驟 2：後置語法與完整性驗證 (Integrity Check)
1. 檢查寫入的產出物是否成功落盤。
2. 進行結構化語法校驗（如 JSON / YAML 驗證）。
3. 產生執行摘要日誌。

---

## 🚨 四、 防呆紅旗指標 (Red Flags & Anti-Patterns)

凡遇到以下任何情況，**必須立即中止 (STOP)** 並回報錯誤，嚴禁自行猜測或強行繼續：

- 🚩 **紅旗 1（大檔整讀）**：試圖一次性讀取未分頁的大檔案（>40KB），引發上下文雪球效應。
- 🚩 **紅旗 2（危險命令）**：包含 `rm -rf`, `format`, `del /s /q`, `git reset --hard`, `git push --force`。
- 🚩 **紅旗 3（金鑰洩漏）**：在程式碼、提示詞或產出中直接硬編碼 API Key、Token 或密碼。
- 🚩 **紅旗 4（文字假完工）**：在尚未確認檔案實際寫入硬碟的情況下，即向用戶宣稱「已完成」。
- 🚩 **紅旗 5（角括號濫用）**：在 Markdown 回應中裸露使用角括號 `< >` 造成解析錯誤（一律改用反引號 `` ` `` 或圓括號）。

---

## 🛡️ 五、 驗收門禁標準 (Quality Gate / Definition of Done)

在完成任務交付前，必須逐項核對以下 DoD 門禁：

- [ ] **1. 代碼與產出物真實落盤**：已透過工具實體確認目標檔案存在且內容完整。
- [ ] **2. 語法檢查通過**：無未閉合標籤、無 JSON 解析錯誤、無編譯語法警告。
- [ ] **3. 格式遵循繁體中文**：所有產出文件均使用繁體中文 (Traditional Chinese)。
- [ ] **4. 雙向門禁無違規**：未被 `PreToolUse` 或 `PostToolUse` 攔截警告。
- [ ] **5. 記憶歸檔就緒**：已備妥關鍵決策與摘要，供 Wax 同步至 Obsidian 第二大腦。

