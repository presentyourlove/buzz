---
name: "comm-quality-guard"
description: "通訊品質與格式安全衛士：過濾訊息中的角括號與特殊標點，根治前端渲染食字與標籤吞噬"
---

# Comm Quality Guard (通訊品質衛士)

## 技能定位
針對特工向 Buzz 頻道發送訊息時，因使用未跳脫的角括號 `<variable>`、`<path>` 或特定行首標點（如 `...`、`---`），導致 Buzz 前端 Markdown/HTML 解析器誤吞字（食字）的問題，提供自動化清理防護。

## 調用方式
在 Python 腳本或特工過濾管線中引用：

```python
from validator import sanitize_message

safe_content = sanitize_message(raw_content)
```

## 核心防護特性
1. **角括號轉全形**：將 `<` 與 `>` 轉為 `＜` 與 `＞`，避免被視為未閉合的 HTML 標籤。
2. **行首省略號保護**：在行首或換行後的 `...` 插入零寬空格（ZWSP），防止省略號語法造成渲染崩潰。
3. **刪除線與水平線保護**：防止誤觸 Markdown 刪除線與分界線語法。
