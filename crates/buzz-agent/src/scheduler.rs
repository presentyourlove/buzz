//! Proactive Scheduler — Cron + Interval Trigger Engine
//!
//! Provides a lightweight in-process scheduler for proactive autonomous triggers.
//! Integrates with `CronSchedule` (cron.rs) and `EventBus` (event_bus.rs) to
//! evaluate due triggers on a 1-second tick and publish events with configurable jitter.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Utc};
use rand::Rng;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::cron::CronSchedule;
use crate::event_bus::{Event, EventBus, BoxedEvent};
use crate::types::AgentError;

/// Unique identifier for a registered trigger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TriggerId(pub u64);

impl TriggerId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TriggerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trigger schedule type: Cron expression or fixed Interval
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerSchedule {
    /// Cron expression (5 or 6 fields, standard cron syntax)
    Cron { expression: String },
    /// Fixed interval with optional jitter
    Interval { 
        seconds: u64,
        #[serde(default = "default_jitter_seconds")]
        jitter_seconds: u64,
    },
}

fn default_jitter_seconds() -> u64 {
    0
}

impl TriggerSchedule {
    /// Compute the next fire time after the given time
    pub fn next_after(&self, after: SystemTime) -> Result<SystemTime, AgentError> {
        match self {
            TriggerSchedule::Cron { expression } => {
                let mut cron = CronSchedule::new(expression.clone());
                cron.next_after(after)
            }
            TriggerSchedule::Interval { seconds, jitter_seconds } => {
                let after_dt: DateTime<Utc> = after.into();
                let base_next = after_dt + chrono::Duration::seconds(*seconds as i64);
                
                if *jitter_seconds > 0 {
                    let jitter = rand::random_range(0..=*jitter_seconds);
                    let jittered = base_next + chrono::Duration::seconds(jitter as i64);
                    Ok(jittered.into())
                } else {
                    Ok(base_next.into())
                }
            }
        }
    }
}

/// A registered trigger with its schedule and event factory
pub struct Trigger<E: Event + serde::Serialize + Clone + 'static> {
    pub id: TriggerId,
    pub name: String,
    pub schedule: TriggerSchedule,
    pub enabled: bool,
    pub event_factory: Arc<dyn Fn() -> E + Send + Sync>,
    pub next_fire: SystemTime,
    pub last_fire: Option<SystemTime>,
    pub fire_count: u64,
}

impl<E: Event + serde::Serialize + Clone + 'static> std::fmt::Debug for Trigger<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trigger")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("schedule", &self.schedule)
            .field("enabled", &self.enabled)
            .field("next_fire", &self.next_fire)
            .field("last_fire", &self.last_fire)
            .field("fire_count", &self.fire_count)
            .finish_non_exhaustive()
    }
}

/// Scheduler configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchedulerConfig {
    /// Tick interval for checking due triggers (default: 1 second)
    #[serde(default = "default_tick_interval")]
    pub tick_interval_seconds: u64,
    /// Default jitter for interval triggers (0-30 seconds, default: 0)
    #[serde(default = "default_jitter_max")]
    pub default_jitter_max_seconds: u64,
    /// Maximum number of triggers (default: 1000)
    #[serde(default = "default_max_triggers")]
    pub max_triggers: usize,
}

fn default_tick_interval() -> u64 { 1 }
fn default_jitter_max() -> u64 { 0 }
fn default_max_triggers() -> usize { 1000 }

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick_interval_seconds: 1,
            default_jitter_max_seconds: 0,
            max_triggers: 1000,
        }
    }
}

/// Commands for the scheduler control channel
enum SchedulerCmd {
    AddTrigger {
        name: String,
        schedule: TriggerSchedule,
        event_factory: Box<dyn Fn() -> BoxedEvent + Send + Sync>,
        response: mpsc::Sender<Result<TriggerId, AgentError>>,
    },
    RemoveTrigger {
        id: TriggerId,
        response: mpsc::Sender<Result<(), AgentError>>,
    },
    EnableTrigger {
        id: TriggerId,
        response: mpsc::Sender<Result<(), AgentError>>,
    },
    DisableTrigger {
        id: TriggerId,
        response: mpsc::Sender<Result<(), AgentError>>,
    },
    ListTriggers {
        response: mpsc::Sender<Vec<TriggerInfo>>,
    },
    Shutdown,
}

/// Serializable trigger info for listing/inspection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriggerInfo {
    pub id: TriggerId,
    pub name: String,
    pub schedule: TriggerSchedule,
    pub enabled: bool,
    pub next_fire: Option<SystemTime>,
    pub last_fire: Option<SystemTime>,
    pub fire_count: u64,
}

/// Type-erased trigger trait for storing heterogeneous triggers
trait ErasedTrigger: Send + Sync {
    fn id(&self) -> TriggerId;
    fn name(&self) -> &str;
    fn schedule(&self) -> &TriggerSchedule;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn next_fire(&self) -> SystemTime;
    fn set_next_fire(&mut self, time: SystemTime);
    fn last_fire(&self) -> Option<SystemTime>;
    fn fire_count(&self) -> u64;

    fn fire(&mut self, event_bus: Arc<EventBus>) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send>>;

    fn to_info(&self) -> TriggerInfo;
}

impl<E: Event + serde::Serialize + Clone + 'static> ErasedTrigger for Trigger<E> {
    fn id(&self) -> TriggerId { self.id }
    fn name(&self) -> &str { &self.name }
    fn schedule(&self) -> &TriggerSchedule { &self.schedule }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    fn next_fire(&self) -> SystemTime { self.next_fire }
    fn set_next_fire(&mut self, time: SystemTime) { self.next_fire = time; }
    fn last_fire(&self) -> Option<SystemTime> { self.last_fire }
    fn fire_count(&self) -> u64 { self.fire_count }
    
    #[allow(unused_assignments, unused_variables)]
    fn fire(&mut self, event_bus: Arc<EventBus>) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send>> {
        let event_factory = self.event_factory.clone();
        let schedule = self.schedule.clone();
        let mut last_fire = self.last_fire;
        let mut fire_count = self.fire_count;
        let mut next_fire = self.next_fire;
        
        Box::pin(async move {
            let event = event_factory();
            let event_type = event.event_type();
            let bus = event_bus.clone();
            
            tokio::spawn(async move {
                if let Err(e) = bus.publish(event).await {
                    warn!(%event_type, %e, "Failed to publish scheduled event");
                }
            });
            
            last_fire = Some(SystemTime::now());
            fire_count += 1;
            next_fire = schedule.next_after(SystemTime::now())?;
            
            Ok(())
        })
    }
    
    fn to_info(&self) -> TriggerInfo {
        TriggerInfo {
            id: self.id,
            name: self.name.clone(),
            schedule: self.schedule.clone(),
            enabled: self.enabled,
            next_fire: Some(self.next_fire),
            last_fire: self.last_fire,
            fire_count: self.fire_count,
        }
    }
}

/// Type-erased trigger implementation that stores a BoxedEvent factory
struct ErasedTriggerImpl {
    id: TriggerId,
    name: String,
    schedule: TriggerSchedule,
    enabled: bool,
    event_factory: Arc<dyn Fn() -> BoxedEvent + Send + Sync>,
    next_fire: SystemTime,
    last_fire: Option<SystemTime>,
    fire_count: u64,
}

impl ErasedTrigger for ErasedTriggerImpl {
    fn id(&self) -> TriggerId { self.id }
    fn name(&self) -> &str { &self.name }
    fn schedule(&self) -> &TriggerSchedule { &self.schedule }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    fn next_fire(&self) -> SystemTime { self.next_fire }
    fn set_next_fire(&mut self, time: SystemTime) { self.next_fire = time; }
    fn last_fire(&self) -> Option<SystemTime> { self.last_fire }
    fn fire_count(&self) -> u64 { self.fire_count }

    fn fire(&mut self, bus: Arc<EventBus>) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send>> {
        if !self.enabled {
            return Box::pin(async { Ok(()) });
        }
        let event_factory = self.event_factory.clone();
        let schedule = self.schedule.clone();
        let enabled = self.enabled;
        
        Box::pin(async move {
            if !enabled {
                return Ok(());
            }
            let event = event_factory();
            let event_type = event.event_type();
            let bus = bus.clone();
            
            tokio::spawn(async move {
                if let Err(e) = bus.publish_boxed(event).await {
                    warn!(%event_type, %e, "Failed to publish scheduled event");
                }
            });
            
            Ok(())
        })
    }

    fn to_info(&self) -> TriggerInfo {
        TriggerInfo {
            id: self.id,
            name: self.name.clone(),
            schedule: self.schedule.clone(),
            enabled: self.enabled,
            next_fire: Some(self.next_fire),
            last_fire: self.last_fire,
            fire_count: self.fire_count,
        }
    }
}

/// Scheduler — manages cron and interval triggers with 1-second tick loop
pub struct Scheduler {
    config: SchedulerConfig,
    event_bus: Arc<EventBus>,
    triggers: Arc<RwLock<HashMap<TriggerId, Box<dyn ErasedTrigger>>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    tick_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Scheduler {
    /// Create a new scheduler with the given config and event bus
    pub fn new(config: SchedulerConfig, event_bus: Arc<EventBus>) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        Self {
            config,
            event_bus,
            triggers: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            tick_handle: tokio::sync::Mutex::new(None),
        }
    }

    /// Start the scheduler's 1-second tick loop
    pub async fn start(&self) -> Result<(), AgentError> {
        let triggers = self.triggers.clone();
        let event_bus = self.event_bus.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut shutdown_rx = shutdown_rx;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let jitter = std::time::Duration::from_secs(config.default_jitter_max_seconds);
                        Self::tick(&triggers, &event_bus, jitter).await;
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
            info!("Scheduler tick loop stopped");
        });

        *self.tick_handle.lock().await = Some(handle);
        Ok(())
    }

    /// Gracefully shutdown the scheduler
    pub async fn shutdown(&self) -> Result<(), AgentError> {
        let _ = self.shutdown_tx.send(true);
        if let Some(handle) = self.tick_handle.lock().await.take() {
            let _ = handle.await;
        }
        // Shutdown event bus
        self.event_bus.shutdown().await;
        Ok(())
    }

    /// Internal tick: check all triggers and fire those that are due
    async fn tick(
        triggers: &Arc<RwLock<HashMap<TriggerId, Box<dyn ErasedTrigger>>>>,
        event_bus: &Arc<EventBus>,
        jitter_range: std::time::Duration,
    ) {
        let now = SystemTime::now();
        let mut to_fire: Vec<TriggerId> = Vec::new();

        // Collect triggers that are due (read lock)
        {
            let triggers_read = triggers.read().await;
            for (id, trigger) in triggers_read.iter() {
                if trigger.is_enabled() && trigger.next_fire() <= now {
                    to_fire.push(*id);
                }
            }
        }

        // Fire due triggers (write lock, one at a time to avoid deadlock)
        for id in to_fire {
            let mut triggers_write = triggers.write().await;
            if let Some(trigger) = triggers_write.get_mut(&id) {
                let event_bus_clone = Arc::clone(event_bus);
                if let Err(e) = trigger.fire(event_bus_clone).await {
                    warn!(trigger_id = %id, %e, "Trigger fire failed");
                }
            }
        }
    }

    /// Add a cron trigger
    pub async fn add_cron_trigger(
        &self,
        name: String,
        cron_expr: String,
        event_factory: Arc<dyn Fn() -> BoxedEvent + Send + Sync>,
    ) -> Result<TriggerId, AgentError> {
        let schedule = TriggerSchedule::Cron { expression: cron_expr.clone() };
        // Validate the cron expression by creating a CronSchedule
        let mut cron = CronSchedule::new(cron_expr.clone());
        cron.get_schedule()?; // This validates the expression
        let next_fire = schedule.next_after(SystemTime::now())?;
        let id = TriggerId::new();

        let trigger = ErasedTriggerImpl {
            id,
            name,
            schedule,
            enabled: true,
            event_factory,
            next_fire,
            last_fire: None,
            fire_count: 0,
        };

        self.triggers.write().await.insert(id, Box::new(trigger));
        Ok(id)
    }

    /// Add an interval trigger
    pub async fn add_interval_trigger(
        &self,
        name: String,
        interval: std::time::Duration,
        event_factory: Arc<dyn Fn() -> BoxedEvent + Send + Sync>,
    ) -> Result<TriggerId, AgentError> {
        let schedule = TriggerSchedule::Interval { 
            seconds: interval.as_secs(), 
            jitter_seconds: self.config.default_jitter_max_seconds 
        };
        let next_fire = schedule.next_after(SystemTime::now())?;
        let id = TriggerId::new();

        let trigger = ErasedTriggerImpl {
            id,
            name,
            schedule,
            enabled: true,
            event_factory,
            next_fire,
            last_fire: None,
            fire_count: 0,
        };

        self.triggers.write().await.insert(id, Box::new(trigger));
        Ok(id)
    }

    /// Remove a trigger by ID
    pub async fn remove_trigger(&self, id: TriggerId) -> Result<(), AgentError> {
        self.triggers.write().await.remove(&id);
        Ok(())
    }

    /// Enable a trigger
    pub async fn enable_trigger(&self, id: TriggerId) -> Result<(), AgentError> {
        if let Some(trigger) = self.triggers.write().await.get_mut(&id) {
            trigger.set_enabled(true);
        }
        Ok(())
    }

    /// Disable a trigger
    pub async fn disable_trigger(&self, id: TriggerId) -> Result<(), AgentError> {
        if let Some(trigger) = self.triggers.write().await.get_mut(&id) {
            trigger.set_enabled(false);
        }
        Ok(())
    }

    /// List all triggers
    pub async fn list_triggers(&self) -> Vec<TriggerInfo> {
        self.triggers
            .read()
            .await
            .values()
            .map(|t| t.to_info())
            .collect()
    }

    /// Get a specific trigger by ID
    pub async fn get_trigger(&self, id: TriggerId) -> Option<TriggerInfo> {
        self.triggers.read().await.get(&id).map(|t| t.to_info())
    }

    /// Manually fire a trigger now (for testing/manual execution)
    pub async fn trigger_now(&self, id: TriggerId) -> Result<(), AgentError> {
        let mut triggers_write = self.triggers.write().await;
        if let Some(trigger) = triggers_write.get_mut(&id) {
            let event_bus_clone = Arc::clone(&self.event_bus);
            trigger.fire(event_bus_clone).await?;
        }
        Ok(())
    }
}
