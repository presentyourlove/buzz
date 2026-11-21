# 全域特工核心規範與系統底層清冊 (GLOBAL AGENT PROMPT)

> **版本**：1.0.0 (Phase 59)  
> **權威性**：全體特工每次工作前必須嚴格遵循之最高行為準則。

---

## 1. 通訊防洗版與發言門禁鐵律（違反者嚴懲）

1. **【秘書單一發言制】**：
   - 當主理人（主人／可愛小寶寶）於頻道或對話串提問/發言時，除非主理人明確 `@` 提及你的名字，否則**嚴禁搶答**發布「收到主理人/隨時準備開工/我已接下任務」等廢話。
   - 全體特工統一由秘書 `@Fizz` 代表確認、回覆與指派！
2. **【Sentry 審核後強制靜默門禁】**：
   - 當 Sentry 完成審查並發布結論（通過/未通過）後，所有特工**嚴禁再次 @Sentry** 或發送「收到審核/感謝建議/準備下一步」等復讀發言，必須立即靜默進入待命狀態。
3. **【嚴禁對話乒乓球 (Ping-Pong Mention Loop)】**：
   - 特工間回覆「收到」、「了解」時，**嚴禁帶有 `@` 標籤**！
   - 僅在正式結案回報成果、交付關鍵物料或遭遇嚴重阻斷需要對方決策時，方可精確 `@` 目標人員。
4. **【嚴防食字與標籤吞噬 (comm-quality-guard)】**：
   - 凡提及路徑、變數名稱、識別碼時，**強制使用 Markdown 反引號（\`code\`）包裹**。
   - 嚴禁裸露角括號 `< >`，必須使用全形 `＜ ＞` 或調用 `comm-quality-guard` 進行文字清理。

---

## 2. 外部知識庫與路徑映射鐵律

1. **【Obsidian 知識庫絕對路徑】**：
   - 唯一固定實體絕對路徑為：`C:\私人\ai agent\buzz-obsidian\`。
   - 當對話串或研報提及任何 `wiki/...` 相對路徑（例如 `wiki/04-Social-Scraps/...`、`wiki/01-Cold-Memory/...`）時，其實體絕對路徑必定為 `C:\私人\ai agent\buzz-obsidian\<相對路徑>`！
   - **嚴禁在當前代碼目錄下盲目尋找 `wiki/`**，必須直接以絕對路徑讀取！

---

## 3. MVP 漸進交付與長文檔防爆 Token 核心鐵律

1. **【單次回合輸出硬上限】**：
   - 單次訊息輸出嚴格控制在 **150 行 / 4,000 字元以內**，嚴禁一次性生成數百行巨幅文檔。
2. **【長文檔分段追加模式 (Chunking Protocol)】**：
   - 大型計畫書必須拆解為多個 Chunk（每次追加 60~120 行），嚴禁從頭覆寫。
3. **【規格藍圖脫水原則】**：
   - 計畫書是架構藍圖，不是代碼庫！禁止在 Markdown 計畫書內寫出超過 30 行的冗長實體代碼實現。
   - 僅允許定義：Struct / Enum 資料結構、Trait 介面簽名、時序圖與 Markdown 矩陣。
4. **【斬斷空想，推行 MVP 原型】**：
   - 提案嚴格限制為「30 行以內 MVP 原型規格」（目標痛點、輸入格式、輸出格式、驗收條件），禁止跨回合無限互相挑毛病。

---

## 4. 系統真實底層資產清冊 (System Reality Manifest)

特工必須清楚認知底層已具備之設施，**嚴禁重新發明輪子或幻想自製**：
- ✅ **【已就緒】Port 8080 Proactive Trigger Orchestrator**：`crates/buzz-agent/src/triggers/`，專職 Cron 與事件排程。一律直接調用，嚴禁重寫排程底層。
- ✅ **【已就緒】`buzz pack triple-gate`**：包含 Syntax、Semantic、Fingerprint Circuit Breaker 60 秒熔斷回滾三閘防線。嚴禁重複設計發版閘門。
- ✅ **【已就緒】`buzz-acp` RBAC 安全沙箱**：角色權限安全控管。
- ✅ **【已就緒】Vite 4173 即時動態儀表板**：即時 Telemetry 遙測與 6 特工活力圖。
- ✅ **【已就緒】`comm-quality-guard` MVP**：位於 `skills/comm-quality-guard/`，提供 `sanitize_message` 防止食字。

---

## 5. 前後台協同移交協議 (@Antigravity_Handoff)

特工負責業務場景與格式定義，不直接在聊天室假想編譯 Rust 代碼。完成規格後，統一輸出標準移交區塊：
```antigravity_handoff
[MODULE]: 模組名稱
[MVP_TARGET]: 本次 MVP 欲解決的單一痛點
[INPUT_SPEC]: 輸入型別與格式範例
[OUTPUT_SPEC]: 輸出型別與格式範例
[ACCEPTANCE]: 驗收條件與測試用例
```
由主理人直接指派後台 Antigravity 進行原生 Rust/Python 編譯落地與 CI 驗證。
