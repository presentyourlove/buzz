//! 排程器 - Cron + Interval + 抖動支援
//!
//! 使用 cron 表達式解析與 tokio 定時器實作

use crate::triggers::types::{NextFireTime, Trigger, TriggerType};
use anyhow::Result;
use buzz_cron::{BuzzSchedule, CronScheduleConfig, TimezoneConfig};
use chrono::{DateTime, Duration, TimeZone, Timelike, Utc};
use rand::rng;
use rand::rngs::ThreadRng;
use rand::Rng;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use tokio::time::{interval, Interval};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

/// 排程器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// 檢查間隔（秒）
    pub check_interval_secs: u64,
    /// 最大並發觸發數
    pub max_concurrent: usize,
    /// 是否啟用抖動
    pub enable_jitter: bool,
    /// 預設抖動範圍（秒）
    pub default_jitter_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 30,
            max_concurrent: 10,
            enable_jitter: true,
            default_jitter_secs: 60,
        }
    }
}

/// 排程器項目（用於優先隊列）
#[derive(Debug, Clone)]
struct ScheduledItem {
    next_fire: DateTime<Utc>,
    trigger_id: Uuid,
    trigger: Trigger,
}

impl PartialEq for ScheduledItem {
    fn eq(&self, other: &Self) -> bool {
        self.next_fire == other.next_fire
    }
}

impl Eq for ScheduledItem {}

impl PartialOrd for ScheduledItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // 反轉順序：最早觸發的在頂部（min-heap 行為）
        other.next_fire.partial_cmp(&self.next_fire)
    }
}

impl Ord for ScheduledItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.next_fire.cmp(&self.next_fire)
    }
}

/// 排程器
#[derive(Clone)]
pub struct Scheduler {
    config: SchedulerConfig,
    heap: Arc<Mutex<BinaryHeap<ScheduledItem>>>,
    running: Arc<Mutex<bool>>,
    check_interval: Arc<Mutex<Option<Interval>>>,
}

impl Scheduler {
    /// 建立新的排程器
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            heap: Arc::new(Mutex::new(BinaryHeap::new())),
            running: Arc::new(Mutex::new(false)),
            check_interval: Arc::new(Mutex::new(None)),
        }
    }

    /// 添加觸發器到排程器
    pub fn add_trigger(&self, trigger: Trigger) -> Result<()> {
        if !trigger.enabled {
            return Ok(());
        }

        if let TriggerType::Schedule {
            cron,
            interval_secs,
            timezone: _,
            jitter_secs,
        } = &trigger.trigger_type
        {
            let next_fire = self.compute_next_fire(cron, *interval_secs, *jitter_secs)?;
            let item = ScheduledItem {
                next_fire,
                trigger_id: trigger.id,
                trigger,
            };
            let mut heap = self.heap.lock().unwrap();
            heap.push(item);
        }
        Ok(())
    }

    /// 移除觸發器並返回被移除的觸發器
    pub fn remove_trigger(&self, trigger_id: Uuid) -> Option<Trigger> {
        let mut heap = self.heap.lock().unwrap();
        let len_before = heap.len();
        let mut removed_trigger = None;
        let new_heap: BinaryHeap<ScheduledItem> = heap
            .drain()
            .filter(|item| {
                if item.trigger_id == trigger_id {
                    removed_trigger = Some(item.trigger.clone());
                    false
                } else {
                    true
                }
            })
            .collect();
        *heap = new_heap;
        if heap.len() < len_before {
            removed_trigger
        } else {
            None
        }
    }

    /// 更新觸發器（移除舊的，添加新的）
    pub fn update_trigger(&self, trigger: Trigger) -> Result<()> {
        self.remove_trigger(trigger.id);
        self.add_trigger(trigger)
    }

    /// 計算下一次觸發時間
    fn compute_next_fire(
        &self,
        cron: &Option<String>,
        interval_secs: Option<u64>,
        jitter_secs: u64,
    ) -> Result<DateTime<Utc>> {
        let now = Utc::now();
        let mut next = if let Some(cron_expr) = cron {
            // Cron 表達式 (透過 buzz_cron 支援 5/6 欄位與時區)
            let schedule = BuzzSchedule::from_config(CronScheduleConfig {
                expression: cron_expr.clone(),
                timezone: Some(TimezoneConfig::utc()),
                with_seconds: false,
            })?;
            schedule
                .next_after_utc(now)
                .ok_or_else(|| anyhow::anyhow!("No next cron time"))?
        } else if let Some(secs) = interval_secs {
            // 固定間隔
            now + Duration::seconds(secs as i64)
        } else {
            return Err(anyhow::anyhow!(
                "Schedule trigger must have either cron or interval_secs"
            ));
        };

        // 應用抖動
        if self.config.enable_jitter && jitter_secs > 0 {
            let jitter = rand::random::<u64>() % (jitter_secs as u64 + 1);
            next = next + Duration::seconds(jitter as i64);
        }

        Ok(next)
    }

    /// 計算觸發器下一次觸發時間並重新入隊
    pub fn reschedule(&self, trigger: &Trigger) -> Result<DateTime<Utc>> {
        if let TriggerType::Schedule {
            cron,
            interval_secs,
            timezone: _,
            jitter_secs,
        } = &trigger.trigger_type
        {
            let next_fire = self.compute_next_fire(cron, *interval_secs, *jitter_secs)?;
            let item = ScheduledItem {
                next_fire,
                trigger_id: trigger.id,
                trigger: trigger.clone(),
            };
            let mut heap = self.heap.lock().unwrap();
            heap.push(item);
            Ok(next_fire)
        } else {
            Err(anyhow::anyhow!("Not a schedule trigger"))
        }
    }

    /// 獲取觸發器總數
    pub fn trigger_count(&self) -> usize {
        self.heap.lock().unwrap().len()
    }

    /// 獲取下一次觸發時間（最早的觸發器）
    pub fn next_fire_time(&self) -> Option<DateTime<Utc>> {
        self.heap.lock().unwrap().peek().map(|item| item.next_fire)
    }

    /// 檢查觸發器是否存在
    pub fn has_trigger(&self, id: Uuid) -> bool {
        let heap = self.heap.lock().unwrap();
        heap.iter().any(|item| item.trigger_id == id)
    }

    /// 獲取指定觸發器的下次觸發時間
    pub fn next_fire_time_for(&self, id: Uuid) -> Option<DateTime<Utc>> {
        let heap = self.heap.lock().unwrap();
        heap.iter()
            .find(|item| item.trigger_id == id)
            .map(|item| item.next_fire)
    }

    /// 獲取下一個待觸發的項目（不移除）
    pub fn peek_next(&self) -> Option<NextFireTime> {
        let heap = self.heap.lock().unwrap();
        heap.peek().map(|item| NextFireTime {
            trigger_id: item.trigger_id,
            next_fire: item.next_fire,
        })
    }

    /// 彈出所有已到期的觸發器
    pub fn pop_due(&self) -> Vec<Trigger> {
        let now = Utc::now();
        let mut heap = self.heap.lock().unwrap();
        let mut due = Vec::new();

        while let Some(item) = heap.peek() {
            if item.next_fire <= now {
                let item = heap.pop().unwrap();
                due.push(item.trigger);
            } else {
                break;
            }
        }

        due
    }

    /// 獲取所有排程項目（用於持久化/調試）
    pub fn get_all_scheduled(&self) -> Vec<NextFireTime> {
        let heap = self.heap.lock().unwrap();
        heap.iter()
            .map(|item| NextFireTime {
                trigger_id: item.trigger_id,
                next_fire: item.next_fire,
            })
            .collect()
    }

    /// 檢查排程器是否正在運行
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// 啟動背景檢查循環
    pub fn start(&self) {
        let mut running = self.running.lock().unwrap();
        if *running {
            return;
        }
        *running = true;

        let check_interval_secs = self.config.check_interval_secs;
        let heap = self.heap.clone();
        let running = self.running.clone();

        let interval = tokio::time::interval(StdDuration::from_secs(check_interval_secs));
        *self.check_interval.lock().unwrap() = Some(interval);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(StdDuration::from_secs(check_interval_secs));
            while *running.lock().unwrap() {
                interval.tick().await;
                // 清理已過期但未被消費的項目（防止堆積）
                let now = Utc::now();
                let heap_guard = heap.lock().unwrap();
                while let Some(item) = heap_guard.peek() {
                    // 如果項目超過 1 小時還沒被處理，可能是系統異常，記錄警告
                    if item.next_fire < now - Duration::hours(1) {
                        tracing::warn!(
                            "Trigger {} overdue by >1h, next_fire: {}",
                            item.trigger_id,
                            item.next_fire
                        );
                        break; // 保留項目，不自動丟棄
                    }
                    break;
                }
            }
        });
    }

    /// 停止背景檢查循環
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
        *self.check_interval.lock().unwrap() = None;
    }
}

/// 解析 cron 表達式並獲取未來 N 次觸發時間（用於測試/預覽）
pub fn preview_cron(cron_expr: &str, count: usize) -> Result<Vec<DateTime<Utc>>> {
    let schedule = BuzzSchedule::from_config(CronScheduleConfig {
        expression: cron_expr.to_string(),
        timezone: Some(TimezoneConfig::utc()),
        with_seconds: false,
    })?;
    let now = Utc::now();
    Ok(schedule.upcoming_after_utc(now).take(count).collect())
}

/// 解析人類可讀的間隔字串（如 "5m", "1h", "30s"）
pub fn parse_interval(interval_str: &str) -> Result<u64> {
    let interval_str = interval_str.trim().to_lowercase();
    if interval_str.ends_with('s') {
        let val: u64 = interval_str[..interval_str.len() - 1].trim().parse()?;
        Ok(val)
    } else if interval_str.ends_with('m') {
        let val: u64 = interval_str[..interval_str.len() - 1].trim().parse()?;
        Ok(val * 60)
    } else if interval_str.ends_with('h') {
        let val: u64 = interval_str[..interval_str.len() - 1].trim().parse()?;
        Ok(val * 3600)
    } else if interval_str.ends_with('d') {
        let val: u64 = interval_str[..interval_str.len() - 1].trim().parse()?;
        Ok(val * 86400)
    } else {
        let val: u64 = interval_str.parse()?;
        Ok(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_scheduler_parse_interval() {
        assert_eq!(parse_interval("30s").unwrap(), 30);
        assert_eq!(parse_interval("5m").unwrap(), 300);
        assert_eq!(parse_interval("1h").unwrap(), 3600);
        assert_eq!(parse_interval("1d").unwrap(), 86400);
        assert_eq!(parse_interval("60").unwrap(), 60);
    }

    #[test]
    fn test_scheduler_compute_next_fire_cron() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        let trigger = Trigger::new(
            "test".to_string(),
            "test trigger".to_string(),
            TriggerType::Schedule {
                cron: Some("0 * * * * *".to_string()), // Every minute
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );

        let next_fire = scheduler.compute_next_fire(&Some("0 * * * * *".to_string()), None, 0).unwrap();
        let now = Utc::now();
        // Next fire should be within the next minute
        assert!(next_fire > now);
        assert!(next_fire <= now + Duration::minutes(1));
    }

    #[test]
    fn test_scheduler_compute_next_fire_interval() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        let trigger = Trigger::new(
            "test".to_string(),
            "test trigger".to_string(),
            TriggerType::Schedule {
                cron: None,
                interval_secs: Some(60),
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );

        let next_fire = scheduler.compute_next_fire(&None, Some(60), 0).unwrap();
        let now = Utc::now();
        assert!(next_fire > now);
        assert!(next_fire <= now + Duration::seconds(61)); // 60s + small buffer
    }

    #[test]
    fn test_scheduler_compute_next_fire_with_jitter() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: true,
            default_jitter_secs: 30,
        });

        let next_fire = scheduler.compute_next_fire(&None, Some(60), 10).unwrap();
        let now = Utc::now();
        // With 10s jitter, next fire should be between 60-70 seconds from the internal now
        // Allow 2 seconds tolerance for test execution time difference
        assert!(next_fire >= now + Duration::seconds(58));
        assert!(next_fire <= now + Duration::seconds(72));
    }

    #[test]
    fn test_scheduler_jitter_variance() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: true,
            default_jitter_secs: 30,
        });

        let mut samples = Vec::new();
        for _ in 0..100 {
            let next = scheduler.compute_next_fire(&None, Some(60), 30).unwrap();
            let offset = (next - Utc::now()).num_seconds();
            samples.push(offset);
        }

        // Check variance - with 30s jitter, we should see spread
        let min = *samples.iter().min().unwrap();
        let max = *samples.iter().max().unwrap();
        assert!(max - min > 5, "Jitter should produce variance, got min={}, max={}", min, max);
    }

    #[test]
    fn test_scheduler_reschedule() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        let mut trigger = Trigger::new(
            "test".to_string(),
            "test trigger".to_string(),
            TriggerType::Schedule {
                cron: Some("0 * * * * *".to_string()), // Every minute
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );

        let next1 = scheduler.reschedule(&trigger).unwrap();
        assert!(next1 > Utc::now());

        // Trigger should be in heap
        assert_eq!(scheduler.trigger_count(), 1);

        // Reschedule again - should add another entry to heap
        let next2 = scheduler.reschedule(&trigger).unwrap();
        assert!(next2 > Utc::now());
        
        // Should have 2 entries now
        assert_eq!(scheduler.trigger_count(), 2);
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        assert!(!scheduler.is_running());

        scheduler.start();
        assert!(scheduler.is_running());

        scheduler.stop();
        assert!(!scheduler.is_running());
    }

    #[test]
    fn test_scheduler_next_fire_time() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        assert!(scheduler.next_fire_time().is_none());

        let trigger = Trigger::new(
            "test".to_string(),
            "test trigger".to_string(),
            TriggerType::Schedule {
                cron: Some("0 * * * * *".to_string()), // Every minute
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );

        scheduler.reschedule(&trigger).unwrap();
        assert!(scheduler.next_fire_time().is_some());
    }

    #[test]
    fn test_scheduler_peek_next() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        assert!(scheduler.peek_next().is_none());

        let trigger = Trigger::new(
            "test".to_string(),
            "test trigger".to_string(),
            TriggerType::Schedule {
                cron: Some("0 * * * * *".to_string()), // Every minute
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );

        scheduler.reschedule(&trigger).unwrap();
        let peeked = scheduler.peek_next().unwrap();
        assert_eq!(peeked.trigger_id, trigger.id);
    }

    #[test]
    fn test_scheduler_pop_due_triggers() {
        let scheduler = Scheduler::new(SchedulerConfig {
            check_interval_secs: 1,
            max_concurrent: 10,
            enable_jitter: false,
            default_jitter_secs: 0,
        });

        let trigger = Trigger::new(
            "test".to_string(),
            "test trigger".to_string(),
            TriggerType::Schedule {
                cron: Some("0 * * * * *".to_string()), // Every minute
                interval_secs: None,
                timezone: "UTC".to_string(),
                jitter_secs: 0,
            },
        );

        scheduler.reschedule(&trigger).unwrap();

        // Trigger is in future, shouldn't be due yet
        let due = scheduler.pop_due();
        assert!(due.is_empty());
    }

    #[test]
    fn test_scheduler_preview_cron() {
        let times = preview_cron("0 * * * * *", 5).unwrap();
        assert_eq!(times.len(), 5);
        // All should be at second 0 (every minute at :00 seconds)
        for t in &times {
            assert_eq!(t.second(), 0);
        }
    }
}
