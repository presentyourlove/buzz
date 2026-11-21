---
name: "daily-evolution-generators"
description: "每日進化任務卡片生成引擎：4 大自動化任務卡生成器（HealthCard, EvoTask, RiskCard, GateCard）"
---

# Daily Evolution Generators (每日進化任務生成引擎)

## 技能定位
負責在每日進化排程中，自動巡檢系統服務狀態、代碼庫待辦事項、量化投資風險與 Triple Gate 發版閘門，產出標準化 Markdown 與 JSON 卡片供特工閉環決策。

## 包含生成器
1. **HealthCardGenerator**：檢查 Port 8080 (Trigger Orchestrator) 與 Port 4173 (Dashboard) 本機服務健康狀態。
2. **EvoTaskGenerator**：自動掃描專案代碼中的 `TODO:` 與 `FIXME:` 生成待辦任務。
3. **RiskCardGenerator**：計算單日 99% VaR 與年化波動率，提供量化風險評估。
4. **GateCardGenerator**：檢查 Syntax/Semantic/Fingerprint 三閘就緒狀態。

## 調用方式
```python
from generators import HealthCardGenerator, EvoTaskGenerator, RiskCardGenerator, GateCardGenerator

# 產生全套每日巡檢卡片
health = HealthCardGenerator().generate()
tasks = EvoTaskGenerator("c:/私人/ai agent/buzz").generate()
risk = RiskCardGenerator().generate()
gate = GateCardGenerator("c:/私人/ai agent/buzz").generate()
```
