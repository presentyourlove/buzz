---
name: "collab-protocol"
description: "将 7 步公文闭环编码为状态机的协作协议自动化技能。支持 minimal（仅 Fizz+Honey2）与 standard（含 Sentry 审核）两阶严格度。"
version: "1.0.0"
author: "Honey2"
tags: [collaboration, protocol, state-machine, automation]
---

# 协作协议自动化技能

## 用途
将人工执行的「7 步公文闭环」固化为可重复、可审计、可自动化的技能调用。

## 支持严格度
- **minimal**：仅 Fizz（分派）+ Honey2（规划）执行，跳过 Sentry 审核与 Pollen 实施
- **standard**：完整 7 步（Fizz → Honey2 → Sentry → 老板 → Pollen → Sentry → Fizz）

## 参数
```json
{
  "task_id": "string",           // 任务标识
  "strictness": "minimal|standard", // 严格度，默认 standard
  "initiator": "pubkey_hex",     // 发起人（老板或代理）
  "payload": {}                  // 任务具体内容
}
```

## 状态机节点
| 步骤 | 角色 | 动作 | 输出 |
|------|------|------|------|
| 1 | 老板 | 下达授权指令 | `authorization` event |
| 2 | Fizz | 接收分派、指派 Honey2 | `dispatch` event |
| 3 | Honey/Honey2 | 产出计划书 | `plan.md` + `@Sentry` |
| 4 | Sentry | 四大门禁审核 | `review_result` (PASS/REJECT) |
| 5 | 老板 | 门禁批示（同意/驳回） | `approval` event |
| 6 | Pollen | 实施代码/产出物 | `implementation` + PR |
| 7 | Sentry | 复审验收 | `acceptance` event |
| 8 | Fizz | 结案回报告 | `closure` event |

## 技术实现
1. 状态机定义保存在 `protocol.yaml` 中
2. 通过 `load_skill` API 载入
3. Agent 根据当前状态与严格度，判断下一步骤与负责角色
4. Agent 在频道中发送对应事件（dispatch、review、approval 等）
5. 通过 Nostr 事件记录状态转移，形成可审计轨迹
