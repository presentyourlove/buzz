//! Integration tests for TriggerRuntime background service
use buzz_agent::triggers::{
    condition::{AtomicCondition, ComparisonOperator, Condition, SimpleProvider},
    event_bus::{EventBus, EventBusConfig},
    runtime::{TriggerRuntime, TriggerRuntimeConfig},
    scheduler::{Scheduler, SchedulerConfig},
    trigger_registry::TriggerRegistry,
    types::{Action, DeduplicationConfig, Trigger, TriggerType, EventSource, TriggerFiredEvent},
};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::test]
async fn test_trigger_runtime_end_to_end_fire() {
    let config = TriggerRuntimeConfig {
        check_interval_secs: 0,
        scheduler_config: SchedulerConfig::default(),
        event_bus_config: EventBusConfig::default(),
        ..Default::default()
    };

    let mut runtime = TriggerRuntime::with_config(config);
    
    let trigger = Trigger {
        id: Uuid::new_v4(),
        name: "test_trigger".to_string(),
        description: "Test trigger for integration test".to_string(),
        enabled: true,
        trigger_type: TriggerType::Schedule {
            cron: Some("* * * * * *".to_string()),
            interval_secs: None,
            timezone: "UTC".to_string(),
            jitter_secs: 0,
        },
        conditions: vec![Condition::atomic(AtomicCondition::new(
            "always_true",
            ComparisonOperator::Eq,
            json!(true),
        ))],
        actions: vec![Action::EmitEvent {
            event_type: "test_fired".to_string(),
            payload: json!({}),
        }],
        deduplication: DeduplicationConfig::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_fired_at: None,
        fire_count: 0,
        next_fire: None,
    };

    runtime.register_trigger(trigger.clone()).await.unwrap();
    runtime.start().await.unwrap();
    assert!(runtime.is_running().await);
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let health = runtime.health_check().await;
    assert!(health.running);
    assert_eq!(health.trigger_count, 1);
    
    runtime.stop().await.unwrap();
    assert!(!runtime.is_running().await);
}

#[tokio::test]
async fn test_trigger_runtime_multiple_triggers() {
    let config = TriggerRuntimeConfig {
        check_interval_secs: 0,
        scheduler_config: SchedulerConfig::default(),
        event_bus_config: EventBusConfig::default(),
        ..Default::default()
    };

    let mut runtime = TriggerRuntime::with_config(config);
    
    for i in 0..3 {
        let trigger = Trigger {
            id: Uuid::new_v4(),
            name: format!("test_trigger_{}", i),
            description: format!("Test trigger {}", i),
            enabled: true,
            trigger_type: TriggerType::Schedule {
                cron: Some("* * * * * *".to_string()),
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
            conditions: vec![Condition::atomic(AtomicCondition::new(
                "always_true",
                ComparisonOperator::Eq,
                json!(true),
            ))],
            actions: vec![Action::EmitEvent {
                event_type: format!("test_fired_{}", i),
                payload: json!({"index": i}),
            }],
            deduplication: DeduplicationConfig::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_fired_at: None,
            fire_count: 0,
            next_fire: None,
        };
        runtime.register_trigger(trigger).await.unwrap();
    }
    
    runtime.start().await.unwrap();
    assert!(runtime.is_running().await);
    
    let health = runtime.health_check().await;
    assert_eq!(health.trigger_count, 3);
    
    runtime.stop().await.unwrap();
    assert!(!runtime.is_running().await);
}

#[tokio::test]
async fn test_trigger_runtime_unregister_trigger() {
    let config = TriggerRuntimeConfig::default();
    let mut runtime = TriggerRuntime::with_config(config);
    
    let trigger = Trigger {
        id: Uuid::new_v4(),
        name: "test_unregister".to_string(),
        description: "Test unregister".to_string(),
        enabled: true,
        trigger_type: TriggerType::Schedule {
            cron: Some("* * * * * *".to_string()),
            interval_secs: None,
            timezone: "UTC".to_string(),
            jitter_secs: 0,
        },
        conditions: vec![Condition::atomic(AtomicCondition::new(
            "always_true",
            ComparisonOperator::Eq,
            json!(true),
        ))],
        actions: vec![Action::EmitEvent {
            event_type: "test_fired".to_string(),
            payload: json!({}),
        }],
        deduplication: DeduplicationConfig::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_fired_at: None,
        fire_count: 0,
        next_fire: None,
    };
    
    runtime.register_trigger(trigger.clone()).await.unwrap();
    let health_before = runtime.health_check().await;
    assert_eq!(health_before.trigger_count, 1);
    
    runtime.unregister_trigger(trigger.id).await.unwrap();
    let health_after = runtime.health_check().await;
    assert_eq!(health_after.trigger_count, 0);
}

#[tokio::test]
async fn test_trigger_runtime_start_stop_idempotent() {
    let config = TriggerRuntimeConfig::default();
    let mut runtime = TriggerRuntime::with_config(config);
    
    runtime.start().await.unwrap();
    runtime.start().await.unwrap();
    assert!(runtime.is_running().await);
    
    runtime.stop().await.unwrap();
    runtime.stop().await.unwrap();
    assert!(!runtime.is_running().await);
}

#[tokio::test]
async fn test_trigger_runtime_config_check_interval() {
    let config = TriggerRuntimeConfig {
        check_interval_secs: 1,
        scheduler_config: SchedulerConfig::default(),
        event_bus_config: EventBusConfig::default(),
        ..Default::default()
    };
    
    let runtime = TriggerRuntime::with_config(config);
    assert_eq!(runtime.config.check_interval_secs, 1);
}
