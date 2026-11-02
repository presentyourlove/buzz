# 7 步节点映射表（我方流程步骤 → CCGS 对应机制 → 自动化方式）

| 我方流程步骤 | CCGS 对应机制 | 自动化方式 |
|-------------|--------------|------------|
| 老板授权 | Vertical Authorization | 老板发送 `authorization` 事件启动 |
| Fizz 分派 | Vertical Delegation | Fizz 发送 `dispatch` 事件指派 Honey2 |
| Honey2 规划 | Planning Agent | Honey2 产出计划书并 `@Sentry` |
| Sentry 审核 | Architecture Review | Sentry 执行四大门禁检查 |
| 老板批示 | Gate Decision | 老板发送 `approval` 事件 |
| Pollen 实施 | Implementation Agent | Pollen 开 PR、产出代码 |
| Sentry 复审 | Quality Gate | Sentry 执行代码审核与测试验证 |
| Fizz 结案 | Closure Report | Fizz 发送 `closure` 事件完成 |
