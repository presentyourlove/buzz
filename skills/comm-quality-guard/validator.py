#!/usr/bin/env python3
"""
comm-quality-guard MVP: sanitize_message
核心目標：消除角括號 `< >` 未跳脫與特定標點導致的食字問題。
前端 React 解析器會誤判 `<...>` 為自訂 HTML 標籤並隱藏內容。
"""

import re

# 零寬空格 - 使用 chr 避免 regex 替換字串中的 \u 轉義問題
ZWSP = chr(0x200b)


def sanitize_message(text: str) -> str:
    """
    清理訊息文字，防止前端渲染時被誤判為 HTML 標籤而吞噬內容。

    處理規則：
    1. 角括號 `<` `>` 轉為全形 `＜` `＞`（避免被視為 HTML 標籤）
    2. 連續特殊標點組合（如 `...`、`---`、`~~~`）在特定語境下可能觸發 Markdown 解析異常，保護處理

    Args:
        text: 原始輸入文字

    Returns:
        經過清理後的安全文字
    """
    if not text:
        return text

    # 1. 角括號轉全形 - 核心修復：防止 <filepath>、<URL>、<var> 被吞噬
    text = text.replace("<", "＜").replace(">", "＞")

    # 2. 保護連續點號 `...` 在行首或特定語境（避免被解析為省略號語法導致渲染異常）
    # 僅在行首或換行後的 `...` 前加零寬空格保護
    text = re.sub(r"(^|\n)\.{3,}", r"\1" + ZWSP + "...", text)

    # 3. 保護連續波浪號 `~~` 和 `~~~`（防誤觸刪除線/代碼塊語法）
    text = re.sub(r"(^|\n)~~+", r"\1" + ZWSP + "~~", text)

    # 4. 保護連續減號 `---`（防誤觸水平線語法）
    text = re.sub(r"(^|\n)---+", r"\1" + ZWSP + "---", text)

    return text


if __name__ == "__main__":
    # 簡單自測
    test_cases = [
        r"檔案路徑：<C:\Users\test> 被吞噬",
        "網址：<https://example.com> 也會被隱藏",
        "變數名 <variable_name> 消失",
        "正常文字不應改變",
        "... 開頭的省略號",
        "--- 水平線標記",
        "~~刪除線~~ 標記",
        "混合測試：<path> 和 ... 和 ---",
    ]

    for tc in test_cases:
        result = sanitize_message(tc)
        print(f"原始: {tc}")
        print(f"清理: {result}")
        print("---")
