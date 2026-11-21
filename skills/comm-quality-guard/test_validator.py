#!/usr/bin/env python3
"""
comm-quality-guard MVP: test_validator.py
測試 sanitize_message 函式的核心功能
"""

import sys
from pathlib import Path

# 確保當前目錄在 sys.path 中，相容從任意目錄執行 unittest
CURRENT_DIR = Path(__file__).resolve().parent
if str(CURRENT_DIR) not in sys.path:
    sys.path.insert(0, str(CURRENT_DIR))

import unittest
from validator import sanitize_message

# 零寬空格常數
ZWSP = "\u200b"


class TestSanitizeMessage(unittest.TestCase):
    """測試 sanitize_message 的各項防禦功能"""

    def test_angle_brackets_converted_to_fullwidth(self):
        """核心測試：角括號轉全形，防止被前端吞噬"""
        self.assertEqual(
            sanitize_message(r"檔案路徑：<C:\Users\test>"),
            r"檔案路徑：＜C:\Users\test＞"
        )
        self.assertEqual(
            sanitize_message("網址：<https://example.com>"),
            "網址：＜https://example.com＞"
        )
        self.assertEqual(
            sanitize_message("變數 <var_name> 測試"),
            "變數 ＜var_name＞ 測試"
        )

    def test_normal_text_unchanged(self):
        """正常文字不應被改變"""
        self.assertEqual(
            sanitize_message("正常中文文字"),
            "正常中文文字"
        )
        self.assertEqual(
            sanitize_message("Normal English text"),
            "Normal English text"
        )
        self.assertEqual(
            sanitize_message("包含數字 123 和符號 !@#$%^&*()"),
            "包含數字 123 和符號 !@#$%^&*()"
        )

    def test_ellipsis_at_line_start_protected(self):
        """行首的省略號 `...` 加零寬空格保護"""
        self.assertEqual(
            sanitize_message("... 開頭的省略號"),
            ZWSP + "... 開頭的省略號"
        )
        self.assertEqual(
            sanitize_message("第一行\n... 第二行省略號"),
            "第一行\n" + ZWSP + "... 第二行省略號"
        )

    def test_horizontal_rule_protected(self):
        """行首的 `---` 水平線標記加保護"""
        self.assertEqual(
            sanitize_message("--- 水平線"),
            ZWSP + "--- 水平線"
        )
        self.assertEqual(
            sanitize_message("內容\n--- 分隔線"),
            "內容\n" + ZWSP + "--- 分隔線"
        )

    def test_strikethrough_protected(self):
        """行首的 `~~` 刪除線標記加保護"""
        self.assertEqual(
            sanitize_message("~~刪除線~~"),
            ZWSP + "~~刪除線~~"
        )
        self.assertEqual(
            sanitize_message("文字\n~~另一行刪除線"),
            "文字\n" + ZWSP + "~~另一行刪除線"
        )

    def test_mixed_content(self):
        """混合內容測試：角括號會被轉換，行中間的特殊標點不受保護（僅行首保護）"""
        input_text = "混合測試：<path> 和 ... 和 ---"
        # 角括號轉全形，但行中間的 ... 和 --- 不加保護
        expected = "混合測試：＜path＞ 和 ... 和 ---"
        self.assertEqual(sanitize_message(input_text), expected)

    def test_empty_string(self):
        """空字串處理"""
        self.assertEqual(sanitize_message(""), "")
        self.assertEqual(sanitize_message(None), None)

    def test_multiple_angle_brackets(self):
        """多個角括號都應轉換"""
        self.assertEqual(
            sanitize_message("<a> <b> <c>"),
            "＜a＞ ＜b＞ ＜c＞"
        )

    def test_nested_brackets_not_affected(self):
        """已轉換的全形括號不應再次轉換（冪等性）"""
        text = "已轉換：＜test＞"
        self.assertEqual(sanitize_message(text), text)

    def test_code_block_markers_not_affected_in_middle(self):
        """行中間的特殊標記不應被保護（僅行首）"""
        self.assertEqual(
            sanitize_message("文字 ... 繼續"),
            "文字 ... 繼續"
        )
        self.assertEqual(
            sanitize_message("文字 --- 繼續"),
            "文字 --- 繼續"
        )
        self.assertEqual(
            sanitize_message("文字 ~~ 繼續"),
            "文字 ~~ 繼續"
        )


if __name__ == "__main__":
    unittest.main()
