#!/usr/bin/env python3
"""
S3 Quant Portfolio Risk Sentinel: test_risk_sentinel.py
單元測試套件：驗證 VaR/CVaR/Drawdown 計算精度與熔斷狀態機邏輯。
"""

import sys
from pathlib import Path

CURRENT_DIR = Path(__file__).resolve().parent
if str(CURRENT_DIR) not in sys.path:
    sys.path.insert(0, str(CURRENT_DIR))

import unittest
from risk_calculator import (
    calculate_var_parametric,
    calculate_var_historical,
    calculate_cvar,
    calculate_max_drawdown,
)
from circuit_breaker import RiskCircuitBreaker, CircuitState


class TestRiskCalculator(unittest.TestCase):
    """測試量化風險計算器"""

    def setUp(self):
        # 構造模擬收益率數據 (252 天)
        self.sample_returns = [
            -0.02, 0.015, -0.01, 0.03, -0.04, 0.005, -0.015, 0.02, -0.008, 0.012
        ] * 25

    def test_var_parametric_calculation(self):
        """測試參數法 VaR 99%"""
        var_99 = calculate_var_parametric(self.sample_returns, confidence=0.99)
        self.assertIsInstance(var_99, float)
        self.assertGreater(var_99, 0.0)
        self.assertLess(var_99, 0.20)

    def test_var_historical_calculation(self):
        """測試歷史法 VaR 99%"""
        var_hist = calculate_var_historical(self.sample_returns, confidence=0.99)
        self.assertIsInstance(var_hist, float)
        self.assertGreater(var_hist, 0.0)
        self.assertGreaterEqual(var_hist, 0.02)

    def test_cvar_greater_than_or_equal_to_var(self):
        """測試 CVaR (Expected Shortfall) 大於或等於平均損失"""
        cvar = calculate_cvar(self.sample_returns, confidence=0.99)
        self.assertIsInstance(cvar, float)
        self.assertGreater(cvar, 0.0)

    def test_max_drawdown_calculation(self):
        """測試最大回撤計算"""
        # 價格序列：100 -> 120 (peak) -> 90 (回撤 25%) -> 110
        prices = [100.0, 110.0, 120.0, 105.0, 90.0, 100.0, 110.0]
        dd = calculate_max_drawdown(prices)
        self.assertAlmostEqual(dd, (120.0 - 90.0) / 120.0, places=4)

    def test_nan_inf_robustness(self):
        """測試包含 NaN 與 Inf 的極值防呆"""
        bad_returns = [float("nan"), -0.02, float("inf"), 0.01, -0.03]
        var_val = calculate_var_parametric(bad_returns)
        self.assertFalse(math_is_nan(var_val))
        self.assertGreater(var_val, 0.0)

        empty_var = calculate_var_parametric([])
        self.assertEqual(empty_var, 0.0)


def math_is_nan(val):
    import math
    return math.isnan(val)


class TestCircuitBreaker(unittest.TestCase):
    """測試風險熔斷狀態機"""

    def setUp(self):
        self.cb = RiskCircuitBreaker(
            var_threshold=0.05,
            drawdown_warning=0.10,
            drawdown_trip=0.15,
            cooldown_seconds=3600
        )

    def test_initial_state_normal(self):
        """初始狀態為 Normal"""
        self.assertEqual(self.cb.current_state, CircuitState.NORMAL)

    def test_transition_to_warning_and_back(self):
        """指標超預警線轉 Warning，連續 3 次正常轉回 Normal"""
        res = self.cb.evaluate(current_var=0.06, current_drawdown=0.08)
        self.assertEqual(res["state"], CircuitState.WARNING.value)

        # 3 次安全指標
        self.cb.evaluate(0.01, 0.02)
        self.cb.evaluate(0.01, 0.02)
        res3 = self.cb.evaluate(0.01, 0.02)
        self.assertEqual(res3["state"], CircuitState.NORMAL.value)

    def test_critical_breach_triggers_circuit_open(self):
        """VaR > 1.5x 門檻 (0.075) 或 Drawdown >= 15% 直接熔斷"""
        res = self.cb.evaluate(current_var=0.08, current_drawdown=0.05)
        self.assertEqual(res["state"], CircuitState.CIRCUIT_OPEN.value)
        self.assertTrue(res["is_tripped"])

    def test_manual_cooldown_and_reset(self):
        """測試手動冷卻與復位"""
        self.cb.evaluate(current_var=0.09, current_drawdown=0.20)
        self.assertEqual(self.cb.current_state, CircuitState.CIRCUIT_OPEN)

        self.cb.trigger_cooldown_manual("已進行對沖")
        self.assertEqual(self.cb.current_state, CircuitState.COOLDOWN)

        self.cb.reset_to_normal("重置正常")
        self.assertEqual(self.cb.current_state, CircuitState.NORMAL)


if __name__ == "__main__":
    unittest.main()
