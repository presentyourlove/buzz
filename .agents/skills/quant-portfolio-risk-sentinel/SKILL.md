---
name: "quant-portfolio-risk-sentinel"
description: "量化投資組合風險哨兵：VaR 99%、CVaR 與最大回撤計算引擎，具備 4 態自適應熔斷狀態機"
---

# Quant Portfolio Risk Sentinel (量化投資組合風險哨兵)

## 技能定位
針對量化交易策略、多資產組合與持倉資產，進行即時市場風險度量與極端黑天鵝防護。具備高精度風險矩陣計算（VaR/CVaR）與四態自動熔斷保護。

## 調用方式

```python
from risk_calculator import calculate_var_parametric, calculate_cvar, calculate_max_drawdown
from circuit_breaker import RiskCircuitBreaker

# 1. 計算風險指標
var_99 = calculate_var_parametric(daily_returns, confidence=0.99)
cvar = calculate_cvar(daily_returns, confidence=0.99)
mdd = calculate_max_drawdown(price_series)

# 2. 評估熔斷狀態
cb = RiskCircuitBreaker(var_threshold=0.05, drawdown_trip=0.15)
decision = cb.evaluate(current_var=var_99, current_drawdown=mdd)

if decision["is_tripped"]:
    print(f"⚠️ 觸發量化熔斷：{decision['trip_reason']}")
```

## 核心特性
- **VaR 99% 精度**：支援參數法與歷史模擬法，精確度量單日最大潛在損失。
- **Expected Shortfall (CVaR)**：捕捉極端尾部巨災風險。
- **自適應 4 態熔斷機**：`Normal` ➔ `Warning` ➔ `CircuitOpen` ➔ `Cooldown`。
