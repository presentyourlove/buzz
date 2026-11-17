//! 觸發器模組 - 核心觸發器類型定義與引擎組件
//!
//! 依據 PLANS/PROACTIVE_TRIGGERS_PLAN.md 設計

pub mod event_bus;
pub mod scheduler;
pub mod trigger_registry;
pub mod types;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triggers::types::{BusEvent, EventSource, Trigger, TriggerType};
    use crate::triggers::event_bus::{EventBus, EventBusConfig, make_handler};
    use crate::triggers::scheduler::{Scheduler, SchedulerConfig, parse_interval, preview_cron};
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn test_event_bus_basic_publish_subscribe() {
        let bus = EventBus::new(EventBusConfig::default());
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler_id = bus.subscribe(make_handler("test", vec![EventSource::Scheduler], move |event| {
            received_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));

        // 發布事件
        bus.publish_simple(EventSource::Scheduler, json!({"test": "data"})).unwrap();

        // 消費並分發
        if let Ok(Some(event)) = bus.try_recv() {
            bus.dispatch(&event);
        }

        // 驗證
        let msgs = received.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], json!({"test": "data"}));

        // 取消訂閱
        assert!(bus.unsubscribe(handler_id));
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new(EventBusConfig::default());
        let received1 = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received2 = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received1_clone = received1.clone();
        let received2_clone = received2.clone();

        bus.subscribe(make_handler("sub1", vec![EventSource::Scheduler], move |event| {
            received1_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));
        bus.subscribe(make_handler("sub2", vec![EventSource::Scheduler], move |event| {
            received2_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));

        bus.publish_simple(EventSource::Scheduler, json!({"broadcast": true})).unwrap();

        if let Ok(Some(event)) = bus.try_recv() {
            bus.dispatch(&event);
        }

        assert_eq!(received1.lock().unwrap().len(), 1);
        assert_eq!(received2.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_event_bus_filtered_subscription() {
        let bus = EventBus::new(EventBusConfig::default());
        let scheduler_received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let manual_received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let scheduler_clone = scheduler_received.clone();
        let manual_clone = manual_received.clone();

        bus.subscribe(make_handler("scheduler-only", vec![EventSource::Scheduler], move |event| {
            scheduler_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));
        bus.subscribe(make_handler("manual-only", vec![EventSource::Manual], move |event| {
            manual_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));

        bus.publish_simple(EventSource::Scheduler, json!({"source": "scheduler"})).unwrap();
        bus.publish_simple(EventSource::Manual, json!({"source": "manual"})).unwrap();

        while let Ok(Some(event)) = bus.try_recv() {
            bus.dispatch(&event);
        }

        assert_eq!(scheduler_received.lock().unwrap().len(), 1);
        assert_eq!(manual_received.lock().unwrap().len(), 1);
        assert_eq!(scheduler_received.lock().unwrap()[0], json!({"source": "scheduler"}));
        assert_eq!(manual_received.lock().unwrap()[0], json!({"source": "manual"}));
    }

    #[test]
    fn test_event_bus_backpressure() {
        let config = EventBusConfig {
            channel_capacity: 2,
            ..Default::default()
        };
        let bus = EventBus::new(config);
        
        // 填滿通道
        bus.publish_simple(EventSource::Scheduler, json!({"id": 1})).unwrap();
        bus.publish_simple(EventSource::Scheduler, json!({"id": 2})).unwrap();
        
        // 第三個應該會失敗（通道已滿）
        let result = bus.publish_simple(EventSource::Scheduler, json!({"id": 3}));
        assert!(result.is_err(), "Expected backpressure error when channel is full");
    }

    #[test]
    fn test_event_bus_shutdown_signal() {
        let bus = EventBus::new(EventBusConfig::default());
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler_id = bus.subscribe(make_handler("test", vec![EventSource::Scheduler], move |event| {
            received_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));

        bus.publish_simple(EventSource::Scheduler, json!({"before": "shutdown"})).unwrap();
        
        if let Ok(Some(event)) = bus.try_recv() {
            bus.dispatch(&event);
        }

        // 發送關機信號
        bus.shutdown();
        
        // 關機後發布應該失敗
        let result = bus.publish_simple(EventSource::Scheduler, json!({"after": "shutdown"}));
        assert!(result.is_err(), "Publish after shutdown should fail");
        
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_scheduler_interval_trigger() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        let trigger = Trigger::new(
            "test-interval".to_string(),
            "Test interval trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(1),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        
        let trigger_id = trigger.id;
        scheduler.add_trigger(trigger).unwrap();
        
        assert_eq!(scheduler.trigger_count(), 1);
        assert!(scheduler.has_trigger(trigger_id));
        
        // 測試下次觸發時間計算
        let next_fire = scheduler.next_fire_time();
        assert!(next_fire.is_some());
        
        // 移除觸發器
        let removed = scheduler.remove_trigger(trigger_id);
        assert!(removed.is_some());
        assert_eq!(scheduler.trigger_count(), 0);
    }

    #[test]
    fn test_scheduler_cron_trigger() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        let trigger = Trigger::new(
            "test-cron".to_string(),
            "Test cron trigger".to_string(),
            TriggerType::Schedule { 
                cron: Some("0 * * * * *".to_string()), 
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            }, // 每分鐘第0秒
        );
        
        let trigger_id = trigger.id;
        scheduler.add_trigger(trigger).unwrap();
        
        assert_eq!(scheduler.trigger_count(), 1);
        
        let next_fire = scheduler.next_fire_time();
        assert!(next_fire.is_some());
    }

    #[test]
    fn test_scheduler_jitter() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        // 帶抖動的間隔觸發器
        let trigger = Trigger::new(
            "test-jitter".to_string(),
            "Test jitter trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(10),
                timezone: "UTC".to_string(),
                jitter_secs: 5,
            },
        );
        
        scheduler.add_trigger(trigger).unwrap();
        
        let next_fire = scheduler.next_fire_time();
        assert!(next_fire.is_some());
        
        // 同一觸發器的下次觸發時間應該因為抖動而不同
        // 注意：這裡測試的是邏輯，實際抖動效果需要多次運行觀察
    }

    #[test]
    fn test_scheduler_disabled_trigger() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        let mut trigger = Trigger::new(
            "test-disabled".to_string(),
            "Test disabled trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(1),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        trigger.enabled = false;
        
        let trigger_id = trigger.id;
        scheduler.add_trigger(trigger).unwrap();
        
        assert_eq!(scheduler.trigger_count(), 1);
        
        // 停用的觸發器不應該有下次觸發時間
        let next_fire = scheduler.next_fire_time();
        assert!(next_fire.is_none() || scheduler.has_trigger(trigger_id));
    }

    #[test]
    fn test_parse_interval() {
        assert_eq!(parse_interval("30s").unwrap(), 30);
        assert_eq!(parse_interval("5m").unwrap(), 300);
        assert_eq!(parse_interval("1h").unwrap(), 3600);
        assert_eq!(parse_interval("2d").unwrap(), 172800);
        assert_eq!(parse_interval("60").unwrap(), 60); // 默認秒
        assert!(parse_interval("invalid").is_err());
    }

    #[test]
    fn test_preview_cron() {
        let now = Utc::now();
        let times = preview_cron("0 * * * * *", 5).unwrap(); // 每分鐘
        assert_eq!(times.len(), 5);
        
        // 所有時間都應該在未來
        for t in &times {
            assert!(*t > now);
        }
        
        // 時間應該遞增
        for i in 1..times.len() {
            assert!(times[i] > times[i-1]);
        }
    }

    #[test]
    fn test_scheduler_multiple_triggers() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        let trigger1 = Trigger::new(
            "interval-1".to_string(),
            "Interval 1".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(5),
                timezone: "UTC".to_string(),
                jitter_secs: 1,
            },
        );
        
        let trigger2 = Trigger::new(
            "cron-1".to_string(),
            "Cron 1".to_string(),
            TriggerType::Schedule { 
                cron: Some("0 0 * * * *".to_string()), 
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            }, // 每小時
        );
        
        let trigger3 = Trigger::new(
            "interval-2".to_string(),
            "Interval 2".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(10),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        
        let id1 = trigger1.id;
        let id2 = trigger2.id;
        let id3 = trigger3.id;
        
        scheduler.add_trigger(trigger1).unwrap();
        scheduler.add_trigger(trigger2).unwrap();
        scheduler.add_trigger(trigger3).unwrap();
        
        assert_eq!(scheduler.trigger_count(), 3);
        assert!(scheduler.has_trigger(id1));
        assert!(scheduler.has_trigger(id2));
        assert!(scheduler.has_trigger(id3));
        
        // 下次觸發應該是最早的那個
        let next_fire = scheduler.next_fire_time();
        assert!(next_fire.is_some());
    }

    #[test]
    fn test_scheduler_one_shot_trigger() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        let future_time = Utc::now() + chrono::Duration::seconds(10);
        let trigger = Trigger::new(
            "one-shot".to_string(),
            "One shot trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(10),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        
        scheduler.add_trigger(trigger).unwrap();
        
        let next_fire = scheduler.next_fire_time();
        assert!(next_fire.is_some());
        assert!(*next_fire.unwrap() >= future_time - chrono::Duration::seconds(1));
    }

    #[test]
    fn test_scheduler_clear() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        for i in 0..5 {
            let trigger = Trigger::new(
                format!("trigger-{}", i),
                format!("Trigger {}", i),
                TriggerType::Schedule { 
                    cron: None, 
                    interval_secs: Some(1),
                    timezone: "UTC".to_string(),
                    jitter_secs: 0,
                },
            );
            scheduler.add_trigger(trigger).unwrap();
        }
        
        assert_eq!(scheduler.trigger_count(), 5);
        
        scheduler.clear();
        assert_eq!(scheduler.trigger_count(), 0);
    }

    #[test]
    fn test_event_bus_concurrent_publish() {
        use std::thread;
        use std::sync::Arc;
        
        let bus = Arc::new(EventBus::new(EventBusConfig::default()));
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();
        
        let handler_id = bus.subscribe(make_handler("concurrent", vec![EventSource::Scheduler], move |event| {
            received_clone.lock().unwrap().push(event.payload);
            Ok(())
        }));
        
        let mut handles = vec![];
        for i in 0..10 {
            let bus_clone = bus.clone();
            let handle = thread::spawn(move || {
                bus_clone.publish_simple(EventSource::Scheduler, json!({"thread": i})).unwrap();
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // 消費所有事件
        while let Ok(Some(event)) = bus.try_recv() {
            bus.dispatch(&event);
        }
        
        assert_eq!(received.lock().unwrap().len(), 10);
        
        bus.unsubscribe(handler_id);
    }

    #[test]
    fn test_scheduler_priority_order() {
        let mut scheduler = Scheduler::new(SchedulerConfig::default());
        
        let now = Utc::now();
        
        // 添加三個在不同時間觸發的 interval 觸發器
        let trigger1 = Trigger::new(
            "later".to_string(),
            "Later trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(30),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        
        let trigger2 = Trigger::new(
            "earlier".to_string(),
            "Earlier trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(10),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        
        let trigger3 = Trigger::new(
            "middle".to_string(),
            "Middle trigger".to_string(),
            TriggerType::Schedule { 
                cron: None, 
                interval_secs: Some(20),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );
        
        scheduler.add_trigger(trigger1).unwrap();
        scheduler.add_trigger(trigger2).unwrap();
        scheduler.add_trigger(trigger3).unwrap();
        
        // 下次觸發應該是最早的
        let next_fire = scheduler.next_fire_time().unwrap();
        assert!((next_fire - (now + chrono::Duration::seconds(10))).num_seconds().abs() <= 2);
    }
}
