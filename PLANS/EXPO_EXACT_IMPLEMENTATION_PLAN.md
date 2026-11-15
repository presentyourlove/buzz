# EXPO_EXACT_IMPLEMENTATION_PLAN.md
tags: [expo-exact, implementation-plan, approved]
status: draft
created: 2026-11-12
reviewed_at: 2026-11-12T09:05:00+00:00
version: "1.0"
verifier: "Honey2"

# 目標
- 實作 expo/exact 的 Agent-Centric Data Flow，使 Agent 的數據變更轉換為有類型、可觀察的 Call。
- 提供 Agent 執行環境的隔離機制，參考 Lee-xeo/Claude-Cage 的輕量化虛擬機技術，確保安全與隱私。
- 減少開發者在 Agent 開發時的認知負荷，提升自動化測試與 UI 驅動的穩定性。

# 範圍與基底路徑
- **代碼庫根目錄**：`c:\私人\ai agent\buzz`
- **核心處理層**：在 `crates/buzz-agent` 與 `crates/buzz-core` 中加入 Agent-Centric Data Flow 的處理層。
- **介面**：定義新的 Call 類型，確保所有 Agent 行為都透過 Typed Call 發出。
- **隔離**：實作輕量級環境隔離（可選），確保 Agent 無法直接讀取宿主機機敏檔案或私鑰。
- **整合**：與現有 Buzz 訊息系統兼容，保持向後相容。

# 里程碑
1. 需求分析與設計 (1 天)
   - 閱讀 expo/exact 相關文件與代碼。
   - 設計 Call 類型與 Data Flow 流程。
   - 設計隔離機制（若需要）。
2. 實作 - Data Flow 層 (2-3 天)
   - 在 `crates/buzz-agent` 與 `crates/buzz-core` 中實作數據轉換層。
   - 定義新的訊息類型或 Call 結構。
   - 更新 Agent 相關代碼以使用新流程。
3. 實作 - 隔離機制 (2 天)
   - 參考 Claude-Cage 的隔離技術，實作環境隔離。
   - 確保 Agent 無法直接存取敏感路徑。
4. 測試與驗證 (2 天)
   - 編寫單元測試與集成測試。
   - 驗證 Agent 行為的可觀察性與類型安全。
5. 文件與審核 (1 天)
   - 更新相關文檔。
   - 提交 Sentry 進行架構完備性與品質審核。

# 實作步驟
## 1. 需求分析與設計
- 閱讀 `expo/exact` 文檔，特別是資料流轉換概念。
- 閱讀 `Lee-xeo/Claude-Cage` 相關技術文檔（若有公開 repo）。
- 在本機代碼庫 `c:\私人\ai agent\buzz` 中定位 Agent 相關代碼：
   - `crates/buzz-agent/src/agent.rs`
   - `crates/buzz-core/src/channel.rs`
   - `crates/buzz-core/src/event.rs`
- 設計新的 `Call` 結構，包含：
  - `id: String`
  - `type: String` (如 "data_change", "command_execute")
  - `payload: serde_json::Value`
  - `timestamp: i64`
  - `sender_pubkey: String`
- 設計 Agent 端的數據流轉換：任何 Agent 發起的數據變更必須被封包為 `Call` 並發送。

## 2. 實作 Data Flow 層
- 在 `crates/buzz-agent/src/agent.rs` 中加入 `handle_data_change` 函數，將原始數據變更轉換為 `Call`。
- 實作 `TypedCall` enum，使用 Rust 的 enum 確保型態安全。
- 更新 `crates/buzz-core/src/channel.rs`，使其能夠識別並處理 `Call` 類型的訊息。
- 確保所有現有 Agent 代碼能夠透過新 API 發出 `Call`，或者提供遷移路徑。

## 3. 實作 隔離機制（可選）
- 參考 Claude-Cage 的容器化技術，實作 Rust 的 `namespace` 或 `chroot` 環境。
- 在 Agent 啟動時，創建獨立的環境，限制檔案系統存取權限。
- 在 Agent 的 `send_message` 或 `execute_command` 中，檢查並阻止對宿主機敏感路徑的存取。

## 4. 測試與驗證
- 編寫單元測試：`tests/agent_typed_call.rs`，測試 `Call` 的序列化、 deserialization。
- 編寫集成測試：使用 `buzz messages send` 發送包含 `Call` 的訊息，驗證後端正確處理。
- 測試隔離機制：確保 Agent 無法讀取 `/etc/passwd` 或 `~/.ssh` 等敏感路徑。

## 5. 文件與審核
- 更新 `README.md` 或 `AGENTS.md`，說明新功能。
- 撰寫 `EXPO_EXACT_IMPLEMENTATION_PLAN.md` 並存檔於 `PLANS` 目錄。
- 在討論串回報：「計畫書已就緒，請 @Sentry 進行架構完備性與品質審核。」

## 6. 失敗模式推演與 MVP 範圍
### 6.1 失敗模式推演
- **系統只讀模式**：當儲存層為唯讀時，Agent 應優雅處理錯誤並回報而非崩潰。若無法寫入，應回報 `Call` 類型為 `data_change_failed` 並包含錯誤訊息。
- **Agent 異常重啟**：若 Agent 於處理中間斷重啟，必須確保數據流轉狀態一致，避免遺失或重複處理。實作請使用冪等性作業（Idempotent Operation）與重試機制。
- **權限不足**：當 Agent 缺少對特定資源的存取權時，應回報特定錯誤碼而非拋出未處理的例外。

### 6.2 MVP 範圍
- **核心功能**：僅實作 `data_change` → `Call` 轉換與發送，確保型態安全與可觀察性。
- **後續擴展**：隔離機制（容器化/namespace）與進階錯誤處理將列為二期（Mode B）目標，優先驗證核心流程是否符合接受標準。
- **驗收門檻**：MVP 版本必須通過 Sentry 初審，通過後方可啟動二期功能開發。

### 6.3 真實 crates 型別出處
- **`crates/buzz-agent/src/agent.rs`**: 新增 `handle_data_change` 函數，簽名為 `fn handle_data_change(raw_payload: serde_json::Value) -> Result<TypedCall, AgentError>`。
- **`crates/buzz-core/src/channel.rs`**: 新增 `process_call` 函數，負責識別 `Call` 類型訊息並進行型態驗證，其型態定義位於 `crates/buzz-core/src/types.rs`。
- **`TypedCall` enum**: 定義於 `crates/buzz-agent/src/typed_call.rs`，包含變體 `DataChange { id, payload, timestamp, sender_pubkey }` 與 `CommandExecute { ... }`，確保 Rust 型體安全，避免 `serde_json::Value` 隱式轉換風險。

### 6.4 環境變數處理
- **API Key**：所有敏感金鑰（如第三方服務金鑰）必須透過 `.env` 檔案載入，並在編譯時期透過 `std::env::var()` 讀取，絕對不可硬編碼於源碼。
- **環境變數驗證**：啟動時檢查必要環境變數是否存在，若缺少關鍵變數則於啟動階段回報錯誤並優雅退出，而非運行時崩潰。
- **.env 範本**：提供 `.env.example` 供開發者參考，其中包含所有可選環境變數與預設值說明，確保開發與生產環境的一致性。
- [ ] 代碼編譯通過 `cargo build --release -p buzz-core`。
- [ ] 所有新增測試全部通過，覆蓋率 ≥ 80%。
- [ ] Agent 行為的 Call 可觀察，且不再直接修改宿主機代碼。
- [ ] 隔離機制（若實作）阻止 Agent 存取宿主機機敏資源。
- [ ] Sentry 審核通過，標記為 `approved`。

# 回滾機制
- 若出現重大缺陷，執行 `git -C "C:/私人/ai agent/buzz" reset --hard HEAD~1` 回滾到上一版本。
- 停用新增的 `handle_data_change` 函數，恢復原有的數據處理流程。

# 交付物
- `buzz-core` 代碼修改（含新型 `Call` 結構與處理邏輯）。
- 測試套件（單元 + 集成）。
- 更新文檔（README、AGENTS、計畫書）。
- 提交 Sentry 審核。
