//! 排程器 - Cron + Interval + 抖動支援
//!
//! 使用 cron 表達式解析與 tokio 定時器實作

use crate::triggers::types::{NextFireTime, Trigger, TriggerType};
use anyhow::Result;
use chrono::{DateTime, Duration, TimeZone, Timelike, Utc};
use cron::Schedule;
use rand::rng;
use rand::rngs::ThreadRng;
use rand::Rng;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use tokio::time::{interval, Interval};
use uuid::Uuid;

/// 排程器配置
#[derive(Debug, Clone)]
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

    /// 移除觸發器
    pub fn remove_trigger(&self, trigger_id: Uuid) -> bool {
        let mut heap = self.heap.lock().unwrap();
        let len_before = heap.len();
        let new_heap: BinaryHeap<ScheduledItem> = heap
            .drain()
            .filter(|item| item.trigger_id != trigger_id)
            .collect();
        *heap = new_heap;
        heap.len() < len_before
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
            // Cron 表達式
            let schedule: Schedule = cron_expr.parse()?;
            schedule
                .after(&now)
                .next()
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

    /// 獲取待處理觸發器數量
    pub fn pending_count(&self) -> usize {
        self.heap.lock().unwrap().len()
    }

    /// 是否正在運行
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

/// 解析 cron 表達式並獲取未來 N 次觸發時間（用於測試/預覽）
pub fn preview_cron(cron_expr: &str, count: usize) -> Result<Vec<DateTime<Utc>>> {
    let schedule: Schedule = cron_expr.parse()?;
    let now = Utc::now();
    Ok(schedule.after(&now).take(count).collect())
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
