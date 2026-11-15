---
title: "Buzz Context Digestion Gateway - Phase 2 原生 ActionSink 擴充設計"
tags: [architecture, workflow, rust, action-sink, phase-2]
status: active
created: 2026-10-25
---

# Buzz Context Digestion Gateway - Phase 2 原生 ActionSink 擴充設計

## 目標
將 Phase 1 的外部 HTTP Webhook 摘要機制升級為 Rust 核心原生支援的 `DigestContext` 動作，提升執行效能並確保交易一致性。

## 1. 核心資料結構擴充 (`crates/buzz-workflow/src/schema.rs`)

```rust
pub enum ActionDef {
    SendMessage { ... },
    CallWebhook { ... },
    /// 內建上下文消化與摘要動作
    DigestContext {
        /// 要消化的訊息數量上限
        limit: usize,
        /// 回溯時間範圍 (秒)
        since_secs: Option<u64>,
        /// 摘要指令模板
        prompt_template: String,
        /// 輸出目標
        #[serde(default)]
        output_to: DigestOutputTarget,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestOutputTarget {
    Channel,
    Variable,
    Both,
}
```

## 2. ActionSink Trait 擴充 (`crates/buzz-workflow/src/action_sink.rs`)

```rust
pub trait ActionSink: Send + Sync {
    fn send_message(
        &self,
        community_id: CommunityId,
        channel_id: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<MessageId, ActionSinkError>> + Send + '_>>;

    fn call_webhook(
        &self,
        req: WebhookRequest,
    ) -> Pin<Box<dyn Future<Output = Result<WebhookResponse, ActionSinkError>> + Send + '_>>;

    /// 執行原生上下文消化
    fn digest_context(
        &self,
        community_id: CommunityId,
        channel_id: &str,
        params: DigestParams,
    ) -> Pin<Box<dyn Future<Output = Result<DigestResult, ActionSinkError>> + Send + '_>>;
}
```

## 3. Workflow YAML 簡化範例

```yaml
name: "Context Digestion Gateway Phase 2"
description: "使用原生 DigestContext 動作進行高效能上下文消化與摘要"
trigger:
  on: message_posted
  filter: "str_len(trigger_text) > 10"
steps:
  - id: native_digest
    action: digest_context
    limit: 50
    since_secs: 86400
    prompt_template: "摘要以下對話，保留關鍵決策與行動項：\n{{history}}"
    output_to: both
    timeout_secs: 15
```
