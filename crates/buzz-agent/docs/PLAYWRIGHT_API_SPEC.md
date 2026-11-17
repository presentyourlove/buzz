# Playwright-rs (v0.21.0) 官方原生 API 規格與類型對齊手冊

> **給特工的指引**：本手冊整理自本機已安裝之 `playwright-rs 0.21.0` crate 的真實導出結構。
> **嚴禁**硬套 Node.js 或 Python Playwright 的路徑與型別名稱，在 Rust 中請嚴格遵循以下官方導出路徑。

---

## 1. 核心型別導出路徑對照表 (Crate Root Exports)

多數核心型別直接導出在 `playwright_rs::*` 根路徑，**不要使用不存在的 `playwright_rs::api::`**：

| 功能類別 | 特工常見幻覺 (錯誤 ❌) | 正確的原生型別 (正確 ✅) | 引入路徑 |
| :--- | :--- | :--- | :--- |
| **點擊選項** | `api::ClickOptions` | `ClickOptions` | `playwright_rs::ClickOptions` |
| **滑鼠按鍵** | `api::MouseButton` | `MouseButton` | `playwright_rs::protocol::click::MouseButton` |
| **表單輸入選項** | `api::FillOptions` | `FillOptions` | `playwright_rs::FillOptions` |
| **下拉選單選項** | `api::SelectOption` | `SelectOption` | `playwright_rs::SelectOption` |
| **等待狀態** | `api::WaitForSelectorState` | `WaitForState` | `playwright_rs::WaitForState` |
| **等待選項** | `api::WaitForSelectorOptions`| `WaitForOptions` | `playwright_rs::WaitForOptions` |
| **頁面載入狀態**| `api::WaitUntilState` | `WaitUntil` | `playwright_rs::WaitUntil` |
| **截圖選項** | `api::ScreenshotOptions` | `ScreenshotOptions` | `playwright_rs::ScreenshotOptions` |

---

## 2. 核心操作模式範例 (Idiomatic Usage)

### 2.1 滑鼠點擊 (Click)
```rust
use playwright_rs::ClickOptions;
use playwright_rs::protocol::click::MouseButton;

let button = match button_str {
    "right" => MouseButton::Right,
    "middle" => MouseButton::Middle,
    _ => MouseButton::Left,
};

let click_options = ClickOptions::builder()
    .button(button)
    .click_count(1)
    .build();

page.click(selector, Some(click_options)).await?;
```

### 2.2 表單填充 (Fill)
```rust
use playwright_rs::FillOptions;

let fill_options = FillOptions::builder()
    .force(false)
    .build();

page.fill(selector, value, Some(fill_options)).await?;
```

### 2.3 下拉選單選擇 (Select Option)
```rust
use playwright_rs::SelectOption;

// SelectOption 支援依 value, label 或 index 選擇
let opt = SelectOption::builder()
    .value(val)
    .build();

page.select_option(selector, opt).await?;
```

### 2.4 等待元素 (Wait For State)
```rust
use playwright_rs::{WaitForOptions, WaitForState};

let state = match state_str {
    "attached" => WaitForState::Attached,
    "detached" => WaitForState::Detached,
    "hidden" => WaitForState::Hidden,
    _ => WaitForState::Visible,
};

let wait_opts = WaitForOptions::builder()
    .state(state)
    .timeout(timeout_ms)
    .build();

let locator = page.locator(selector);
locator.wait_for(Some(wait_opts)).await?;
```

---

## 3. Buzz Agent 內部架構語法規則

1. **ToolDef Schema 欄位**：
   ```rust
   // 錯誤 ❌：ToolDef 沒有 parameters 欄位
   ToolDef { name, description, parameters: json!(...) }

   // 正確 ✅：欄位名稱為 input_schema
   ToolDef { name, description, input_schema: json!(...) }
   ```

2. **Option 與 Result 轉換**：
   ```rust
   // 錯誤 ❌：Option 沒有 map_err
   pool.get_context(id).await.map_err(...)?;

   // 正確 ✅：使用 ok_or_else 轉成 Result
   pool.get_context(id).await.ok_or_else(|| AgentError::Other("context not found".to_string()))?;
   ```
