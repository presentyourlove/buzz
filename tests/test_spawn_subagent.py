#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
test_spawn_subagent.py - 動態 Sub-agent 派發工具單元測試
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path("c:/私人/ai agent/buzz").resolve()
SPAWN_SCRIPT = REPO_ROOT / "scripts" / "tools" / "spawn_subagent.py"


class TestSubagentDispatcher(unittest.TestCase):
    """測試 Sub-agent 派發器之核心功能與隔離機制"""

    def setUp(self):
        self.temp_dir = tempfile.mkdtemp(prefix="test_subagent_")
        self.script_path = str(SPAWN_SCRIPT)

    def tearDown(self):
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_dry_run_execution(self):
        """測試乾跑模式（驗證任務結構、路徑生成與狀態返回）"""
        cmd = [
            sys.executable,
            self.script_path,
            "--parent", "Pollen",
            "--task-name", "unit_test_task",
            "--isolated-dir", self.temp_dir,
            "--dry-run"
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        self.assertEqual(res.returncode, 0, f"Command failed: {res.stderr}")

        # 解析 stdout JSON
        data = json.loads(res.stdout)
        self.assertEqual(data["parent_name"], "Pollen")
        self.assertEqual(data["task_name"], "unit_test_task")
        self.assertEqual(data["status"], "dry_run")
        self.assertTrue(os.path.isdir(data["sandbox_path"]))

        # 驗證沙盒內 .gitignore 存在
        gitignore = Path(data["sandbox_path"]) / ".gitignore"
        self.assertTrue(gitignore.exists(), ".gitignore should exist in sandbox")

    def test_output_file_generation(self):
        """測試結果寫入指定輸出檔案"""
        out_file = Path(self.temp_dir) / "custom_result.json"
        cmd = [
            sys.executable,
            self.script_path,
            "--parent", "Honey2",
            "--task-name", "research_subtask",
            "--isolated-dir", self.temp_dir,
            "--output-file", str(out_file),
            "--dry-run"
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        self.assertEqual(res.returncode, 0)
        self.assertTrue(out_file.exists(), "Output file must be written")

        data = json.loads(out_file.read_text(encoding="utf-8"))
        self.assertEqual(data["parent_name"], "Honey2")
        self.assertEqual(data["status"], "dry_run")

    def test_prompt_file_reading(self):
        """測試從檔案讀取 Prompt"""
        prompt_file = Path(self.temp_dir) / "test_prompt.txt"
        prompt_content = "請幫我分析資料庫連線池設定。"
        prompt_file.write_text(prompt_content, encoding="utf-8")

        cmd = [
            sys.executable,
            self.script_path,
            "--parent", "Sentry",
            "--task-name", "audit_subtask",
            "--isolated-dir", self.temp_dir,
            "--prompt-file", str(prompt_file),
            "--dry-run"
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        self.assertEqual(res.returncode, 0)
        data = json.loads(res.stdout)
        self.assertEqual(data["parent_name"], "Sentry")

    def test_missing_prompt_failure_without_dry_run(self):
        """測試在非乾跑模式下缺少 Prompt 應返回非零錯誤碼"""
        cmd = [
            sys.executable,
            self.script_path,
            "--parent", "Wax",
            "--task-name", "missing_prompt_test"
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        self.assertNotEqual(res.returncode, 0, "Should fail when prompt is empty without --dry-run")


if __name__ == "__main__":
    unittest.main()

