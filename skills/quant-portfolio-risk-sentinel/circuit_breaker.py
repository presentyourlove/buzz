#!/usr/bin/env python3
"""
S3 Quant Portfolio Risk Sentinel: circuit_breaker.py
量化投資組合風險熔斷狀態機：Normal -> Warning -> CircuitOpen -> Cooldown -> Normal
"""

import time
from enum import Enum
from typing import Dict, Any, Optional


class CircuitState(str, Enum):
    NORMAL = "Normal"
    WARNING = "Warning"
    CIRCUIT_OPEN = "CircuitOpen"
    COOLDOWN = "Cooldown"


class RiskCircuitBreaker:
    """
    量化風險熔斷控制器
    """

    def __init__(
        self,
        var_threshold: float = 0.05,       # 預設單日 99% VaR 警戒線：5%
        drawdown_warning: float = 0.10,    # 回撤預警線：10%
        drawdown_trip: float = 0.15,       # 熔斷硬指標：15%
        cooldown_seconds: int = 86400      # 冷卻期預設 24 小時
    ):
        self.var_threshold = var_threshold
        self.drawdown_warning = drawdown_warning
        self.drawdown_trip = drawdown_trip
        self.cooldown_seconds = cooldown_seconds

        self.current_state = CircuitState.NORMAL
        self.consecutive_safe_count = 0
        self.consecutive_warning_count = 0
        self.last_state_change_time = time.time()
        self.trip_reason = ""

    def evaluate(self, current_var: float, current_drawdown: float) -> Dict[str, Any]:
        """
        輸入當前 VaR 與最大回撤，推演狀態機轉移並回傳決策
        """
        now = time.time()
        prev_state = self.current_state

        # 1. 如果當前處於熔斷打開狀態 (CircuitOpen)，檢查是否可進入 Cooldown
        if self.current_state == CircuitState.CIRCUIT_OPEN:
            # 必須透過明確信號轉入 Cooldown
            pass

        # 2. 如果處於冷卻期 (Cooldown)
        elif self.current_state == CircuitState.COOLDOWN:
            elapsed = now - self.last_state_change_time
            if current_var > self.var_threshold * 1.5 or current_drawdown > self.drawdown_trip:
                # 冷卻期內再次嚴重惡化，重回 CircuitOpen
                self._transition_to(CircuitState.CIRCUIT_OPEN, "冷卻期間風險指標再次劇烈超標")
            elif elapsed >= self.cooldown_seconds:
                # 冷卻期滿且無超標，轉為正常
                self._transition_to(CircuitState.NORMAL, "24小時冷卻期滿且指標已恢復安全水平")

        # 3. 正常或預警狀態下的常規判定
        else:
            is_critical = (current_var >= self.var_threshold * 1.5) or (current_drawdown >= self.drawdown_trip)
            is_warning = (current_var >= self.var_threshold) or (current_drawdown >= self.drawdown_warning)

            if is_critical:
                self._transition_to(
                    CircuitState.CIRCUIT_OPEN,
                    f"指標劇烈爆表 (VaR: {current_var:.2%}, Drawdown: {current_drawdown:.2%})"
                )
            elif is_warning:
                self.consecutive_warning_count += 1
                self.consecutive_safe_count = 0
                if self.consecutive_warning_count >= 3 and self.current_state != CircuitState.CIRCUIT_OPEN:
                    self._transition_to(CircuitState.CIRCUIT_OPEN, "連續 3 次觸發 Warning 累計升級熔斷")
                elif self.current_state == CircuitState.NORMAL:
                    self._transition_to(CircuitState.WARNING, f"指標超出預警線 (VaR: {current_var:.2%})")
            else:
                # 安全狀態
                self.consecutive_safe_count += 1
                self.consecutive_warning_count = 0
                if self.current_state == CircuitState.WARNING and self.consecutive_safe_count >= 3:
                    self._transition_to(CircuitState.NORMAL, "連續 3 次檢查維持安全水平，預警解除")

        return {
            "state": self.current_state.value,
            "previous_state": prev_state.value,
            "is_tripped": self.current_state == CircuitState.CIRCUIT_OPEN,
            "trip_reason": self.trip_reason if self.current_state == CircuitState.CIRCUIT_OPEN else "",
            "var": current_var,
            "drawdown": current_drawdown,
            "timestamp": now,
        }

    def trigger_cooldown_manual(self, reason: str = "手動啟動風險冷卻流程"):
        """手動介入：將 CircuitOpen 推進至 Cooldown 狀態"""
        if self.current_state == CircuitState.CIRCUIT_OPEN:
            self._transition_to(CircuitState.COOLDOWN, reason)

    def reset_to_normal(self, reason: str = "管理員手動復位"):
        """手動重置回 Normal"""
        self._transition_to(CircuitState.NORMAL, reason)

    def _transition_to(self, new_state: CircuitState, reason: str):
        self.current_state = new_state
        self.last_state_change_time = time.time()
        self.trip_reason = reason
        if new_state == CircuitState.NORMAL:
            self.consecutive_safe_count = 0
            self.consecutive_warning_count = 0
