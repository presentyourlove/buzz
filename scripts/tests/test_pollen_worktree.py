#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Pollen Git Worktree 工具測試套件
驗證：命名防注入、生命週期管理、編譯環境變數隔離、合流與清理機制。
"""

import os
import sys
import unittest
import tempfile
import subprocess
from pathlib import Path

# 將 scripts 加入 sys.path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

import pollen_worktree


class TestPollenWorktree(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.test_task_id = "test-wt-sandbox-01"

    def tearDown(self):
        # 測試後確保清理
        try:
            pollen_worktree.cleanup_worktree(self.test_task_id, force=True)
        except Exception:
            pass

    def test_01_validate_task_id(self):
        """測試 task_id 命名安全驗證"""
        valid_ids = ["task1", "task-123", "task_abc", "FEATURE-99"]
        for tid in valid_ids:
            try:
                pollen_worktree.validate_task_id(tid)
            except ValueError:
                self.fail(f"合法 ID 不應拋出異常: {tid}")

        invalid_ids = ["../task", "task/sub", "task;rm", "task name", "task*"]
        for tid in invalid_ids:
            with self.assertRaises(ValueError):
                pollen_worktree.validate_task_id(tid)

    def test_02_full_lifecycle(self):
        """測試 Worktree 建立、列出、運行指令、合流與清理完整生命週期"""
        # 1. 建立
        res_create = pollen_worktree.create_worktree(self.test_task_id)
        self.assertEqual(res_create["status"], "success")
        wt_path = Path(res_create["path"])
        self.assertTrue(wt_path.exists())
        self.assertTrue((wt_path / ".target").exists())

        # 2. 列出清單
        wts = pollen_worktree.list_worktrees()
        found = any(wt.get("task_id") == self.test_task_id for wt in wts)
        self.assertTrue(found, "清單中應包含新建立的沙盒")

        # 3. 在沙盒中執行命令，驗證 CARGO_TARGET_DIR 正確隔離
        test_py = wt_path / "_check_env.py"
        test_py.write_text(
            "import os, sys\n"
            "target = os.environ.get('CARGO_TARGET_DIR', '')\n"
            "sys.exit(0 if '.target' in target else 1)\n",
            encoding="utf-8"
        )

        ret = pollen_worktree.run_in_worktree(self.test_task_id, [sys.executable, "_check_env.py"])
        self.assertEqual(ret, 0, "沙盒執行環境應帶有獨立 CARGO_TARGET_DIR")
        if test_py.exists():
            test_py.unlink()

        # 4. 在沙盒內進行輕量修改並 commit
        dummy_file = wt_path / "_wt_dummy.txt"
        dummy_file.write_text("worktree sandbox test\n", encoding="utf-8")
        
        add_res = subprocess.run(["git", "add", "_wt_dummy.txt"], cwd=wt_path, capture_output=True, text=True)
        self.assertEqual(add_res.returncode, 0)
        
        ci_res = subprocess.run(["git", "commit", "-m", "test(sandbox): worktree dummy commit"], cwd=wt_path, capture_output=True, text=True)
        self.assertEqual(ci_res.returncode, 0)

        # 5. 合流回主工作區
        res_merge = pollen_worktree.merge_worktree(self.test_task_id, auto_cleanup=False)
        self.assertEqual(res_merge["status"], "success")

        # 驗證主工作區是否已包含此檔案
        merged_file = pollen_worktree.REPO_ROOT / "_wt_dummy.txt"
        self.assertTrue(merged_file.exists())

        # 6. 清理 Worktree
        res_clean = pollen_worktree.cleanup_worktree(self.test_task_id, force=False)
        self.assertEqual(res_clean["status"], "success")
        self.assertFalse(wt_path.exists())

        # 清除主幹上的測試檔案並做一個 revert 或 clean
        if merged_file.exists():
            merged_file.unlink()
            subprocess.run(["git", "add", "_wt_dummy.txt"], cwd=pollen_worktree.REPO_ROOT, capture_output=True)
            subprocess.run(["git", "commit", "-m", "chore(test): remove worktree dummy test file"], cwd=pollen_worktree.REPO_ROOT, capture_output=True)


if __name__ == "__main__":
    unittest.main()
