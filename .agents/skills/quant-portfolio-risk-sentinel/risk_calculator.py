#!/usr/bin/env python3
"""
S3 Quant Portfolio Risk Sentinel: risk_calculator.py
量化投資組合風險計算引擎：Parametric VaR, Historical VaR, CVaR (Expected Shortfall), Max Drawdown.
"""

import math
from typing import List, Union


def calculate_var_parametric(returns: List[float], confidence: float = 0.99) -> float:
    """
    計算參數法 VaR (風險價值，常態分佈假設)。
    回傳值為正數，代表在給定置信度下預期的最大單日損失百分比。
    """
    if not returns or len(returns) < 2:
        return 0.0

    # 清除無效值
    clean_returns = [r for r in returns if not (math.isnan(r) or math.isinf(r))]
    if len(clean_returns) < 2:
        return 0.0

    mean = sum(clean_returns) / len(clean_returns)
    variance = sum((r - mean) ** 2 for r in clean_returns) / (len(clean_returns) - 1)
    std_dev = math.sqrt(variance)

    # 查表或逆正態近似 (常用分位數 Z 值)
    # 0.95 -> 1.6449, 0.99 -> 2.3263, 0.999 -> 3.0902
    if confidence >= 0.999:
        z = 3.0902
    elif confidence >= 0.99:
        z = 2.3263
    elif confidence >= 0.95:
        z = 1.6449
    else:
        # 一般近似
        z = 1.2816 if confidence >= 0.90 else 1.0

    # VaR = -(mean - z * std_dev) = z * std_dev - mean
    var_value = z * std_dev - mean
    return max(0.0, float(var_value))


def calculate_var_historical(returns: List[float], confidence: float = 0.99) -> float:
    """
    計算歷史模擬法 VaR。
    """
    if not returns:
        return 0.0

    clean_returns = sorted([r for r in returns if not (math.isnan(r) or math.isinf(r))])
    if not clean_returns:
        return 0.0

    # 尋找分位數索引 (例如 99% 置信度代表取最差 1% 的收益率)
    cutoff_percent = 1.0 - confidence
    idx = int(len(clean_returns) * cutoff_percent)
    idx = max(0, min(idx, len(clean_returns) - 1))

    worst_loss = -clean_returns[idx]
    return max(0.0, float(worst_loss))


def calculate_cvar(returns: List[float], confidence: float = 0.99) -> float:
    """
    計算條件風險價值 CVaR (Expected Shortfall，即超過 VaR 門檻之後的平均超額損失)。
    """
    if not returns:
        return 0.0

    clean_returns = sorted([r for r in returns if not (math.isnan(r) or math.isinf(r))])
    if not clean_returns:
        return 0.0

    cutoff_percent = 1.0 - confidence
    cutoff_idx = int(len(clean_returns) * cutoff_percent)
    cutoff_idx = max(1, min(cutoff_idx, len(clean_returns)))

    tail_losses = clean_returns[:cutoff_idx]
    avg_tail_return = sum(tail_losses) / len(tail_losses)
    return max(0.0, float(-avg_tail_return))


def calculate_max_drawdown(prices: List[float]) -> float:
    """
    計算價格序列的歷史最大回撤 (Max Drawdown)。
    回傳值為 0.0 ~ 1.0 之間的浮點數（例如 0.15 代表 15% 回撤）。
    """
    if not prices or len(prices) < 2:
        return 0.0

    clean_prices = [p for p in prices if not (math.isnan(p) or math.isinf(p)) and p > 0]
    if len(clean_prices) < 2:
        return 0.0

    peak = clean_prices[0]
    max_dd = 0.0

    for p in clean_prices:
        if p > peak:
            peak = p
        else:
            dd = (peak - p) / peak
            if dd > max_dd:
                max_dd = dd

    return float(max_dd)
