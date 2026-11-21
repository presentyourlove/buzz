#!/usr/bin/env python3
"""
B2 Daily Evolution Loop: test_generators.py
單元測試套件：驗證 4 大任務生成器的輸出結構與 Markdown 格式。
"""

import sys
from pathlib import Path

CURRENT_DIR = Path(__file__).resolve().parent
if str(CURRENT_DIR) not in sys.path:
    sys.path.insert(0, str(CURRENT_DIR))

import unittest
from generators import (
    HealthCardGenerator,
    EvoTaskGenerator,
    RiskCardGenerator,
    GateCardGenerator,
)


class TestDailyEvolutionGenerators(unittest.TestCase):
    """測試 4 大每日任務卡片生成器"""

    def setUp(self):
        self.repo_dir = str(Path(__file__).resolve().parents[2])

    def test_health_card_generator(self):
        """測試健康卡片生成器"""
        gen = HealthCardGenerator(ports=(8080, 4173))
        res = gen.generate()
        self.assertEqual(res["type"], "HealthCard")
        self.assertIn("services", res)
        self.assertIn("### 🏥 系統健康巡檢卡片", res["markdown"])

    def test_evo_task_generator(self):
        """測試進化任務卡片生成器"""
        gen = EvoTaskGenerator(self.repo_dir)
        res = gen.generate()
        self.assertEqual(res["type"], "EvoTaskCard")
        self.assertIsInstance(res["tasks"], list)
        self.assertIn("### 🚀 每日進化任務卡片", res["markdown"])

    def test_risk_card_generator(self):
        """測試量化風險卡片生成器"""
        gen = RiskCardGenerator()
        res = gen.generate()
        self.assertEqual(res["type"], "RiskCard")
        self.assertIsInstance(res["var_99"], float)
        self.assertIn("### 📊 每日量化風險巡檢卡片", res["markdown"])

    def test_gate_card_generator(self):
        """測試發版閘門卡片生成器"""
        gen = GateCardGenerator(self.repo_dir)
        res = gen.generate()
        self.assertEqual(res["type"], "GateCard")
        self.assertTrue(res["triple_gate_ready"])
        self.assertIn("### 🛡️ 發版防線 (Triple Gate) 狀態卡片", res["markdown"])


if __name__ == "__main__":
    unittest.main()
