use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::health_guardian::phase_b_run_daemon;
use crate::types::{AgentError, AgentKeys, AlwaysOnConfig};
use async_trait::async_trait;
use tokio::sync::{watch, mpsc, Semaphore};
use tokio::time::interval;

/// Background service trait for long-running services in the daemon
#[async_trait]
pub trait BackgroundService: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    type Health: Send + Sync + 'static;

    async fn start(&mut self) -> Result<(), Self::Error>;
    async fn stop(&mut self) -> Result<(), Self::Error>;
    fn health_check(&self) -> Self::Health;
    fn name(&self) -> &'static str;
}

/// Daemon lifecycle states for Always-On agent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    /// Daemon is starting up
    Starting,
    /// Daemon is actively processing events
    Active,
    /// Daemon is sleeping (idle timeout reached)
    Sleeping,
    /// Daemon is waking up from sleep
    Waking,
    /// Daemon is stopping gracefully
    Stopping,
    /// Daemon has failed
    Failed,
}

impl DaemonState {
    /// Check if daemon is in a running state (can process events)
    pub fn is_running(&self) -> bool {
        matches!(self, DaemonState::Active | DaemonState::Waking)
    }
    
    /// Check if daemon is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, DaemonState::Stopping | DaemonState::Failed)
    }
}

/// Background listener configuration
#[derive(Debug, Clone)]
pub struct BackgroundListenerConfig {
    /// Concurrency semaphore for limiting parallel tasks
    pub task_semaphore: Arc<Semaphore>,
    /// Memory limit in bytes
    pub memory_limit_bytes: u64,
    /// Current memory usage tracker
    pub memory_usage: Arc<std::sync::atomic::AtomicU64>,
    /// Idle timeout duration
    pub idle_timeout: Duration,
    /// Channels to monitor
    pub monitored_channels: Vec<String>,
}

/// Background listener for Always-On agent
/// Handles event subscription, concurrency control, memory tracking, and sleep/wake logic
pub struct BackgroundListener {
    config: BackgroundListenerConfig,
    state: Arc<std::sync::RwLock<DaemonState>>,
    last_activity: Arc<std::sync::RwLock<std::time::Instant>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    event_tx: mpsc::UnboundedSender<nostr::Event>,
}

impl BackgroundListener {
    /// Create a new background listener from AlwaysOnConfig
    pub fn new(always_on_config: &AlwaysOnConfig) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        
        let task_semaphore = Arc::new(Semaphore::new(always_on_config.max_concurrent_tasks as usize));
        let memory_limit_bytes = (always_on_config.max_memory_mb as u64) * 1024 * 1024;
        let memory_usage = Arc::new(std::sync::atomic::AtomicU64::new(0));
        
        Self {
            config: BackgroundListenerConfig {
                task_semaphore,
                memory_limit_bytes,
                memory_usage,
                idle_timeout: Duration::from_secs(always_on_config.idle_timeout_secs),
                monitored_channels: always_on_config.monitored_channels.clone(),
            },
            state: Arc::new(std::sync::RwLock::new(DaemonState::Starting)),
            last_activity: Arc::new(std::sync::RwLock::new(std::time::Instant::now())),
            shutdown_tx,
            shutdown_rx,
            event_tx,
        }
    }
    
    /// Get current daemon state
    pub fn state(&self) -> DaemonState {
        *self.state.read().unwrap()
    }
    
    /// Transition to a new state
    pub fn transition_state(&self, new_state: DaemonState) {
        let mut state = self.state.write().unwrap();
        tracing::info!("Daemon state transition: {:?} -> {:?}", *state, new_state);
        *state = new_state;
    }
    
    /// Record activity (resets idle timer)
    pub fn record_activity(&self) {
        *self.last_activity.write().unwrap() = std::time::Instant::now();
        // If sleeping, transition to waking
        if self.state() == DaemonState::Sleeping {
            self.transition_state(DaemonState::Waking);
        }
    }
    
    /// Check if idle timeout has been reached
    pub fn is_idle(&self) -> bool {
        let last_activity = *self.last_activity.read().unwrap();
        last_activity.elapsed() >= self.config.idle_timeout
    }
    
    /// Try to acquire a task permit (for concurrency control)
    pub async fn acquire_task_permit(&self) -> Result<tokio::sync::SemaphorePermit<'_>, AgentError> {
        self.config.task_semaphore.acquire().await
            .map_err(|_| AgentError::Other("Failed to acquire task permit - semaphore closed".to_string()))
    }
    
    /// Track memory allocation
    pub fn allocate_memory(&self, bytes: u64) -> Result<(), AgentError> {
        let current = self.config.memory_usage.load(std::sync::atomic::Ordering::Relaxed);
        let new_total = current.saturating_add(bytes);
        if new_total > self.config.memory_limit_bytes {
            return Err(AgentError::Other(format!(
                "Memory limit exceeded: {} bytes allocated, limit {} bytes",
                new_total, self.config.memory_limit_bytes
            )));
        }
        self.config.memory_usage.store(new_total, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
    
    /// Release memory allocation
    pub fn release_memory(&self, bytes: u64) {
        let current = self.config.memory_usage.load(std::sync::atomic::Ordering::Relaxed);
        self.config.memory_usage.store(current.saturating_sub(bytes), std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Get current memory usage
    pub fn memory_usage(&self) -> u64 {
        self.config.memory_usage.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    /// Start the background listener loop (sleep/wake monitoring)
    pub async fn run_sleep_wake_loop(&mut self) {
        let mut check_interval = interval(Duration::from_secs(10)); // Check every 10 seconds
        
        loop {
            tokio::select! {
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        tracing::info!("Background listener received shutdown signal");
                        self.transition_state(DaemonState::Stopping);
                        break;
                    }
                }
                _ = check_interval.tick() => {
                    // Check if we should transition to sleep
                    if self.state() == DaemonState::Active && self.is_idle() {
                        tracing::info!("Idle timeout reached, transitioning to Sleeping");
                        self.transition_state(DaemonState::Sleeping);
                        // Unsubscribe from relays to save resources
                        // TODO: Implement relay unsubscription
                    }
                    
                    // Check if we should wake up (handled by record_activity)
                    if self.state() == DaemonState::Waking {
                        tracing::info!("Waking up from sleep, transitioning to Active");
                        self.transition_state(DaemonState::Active);
                        // Re-subscribe to relays
                        // TODO: Implement relay resubscription
                    }
                }
            }
        }
    }
    
    /// Send shutdown signal
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
    
    /// Get event sender for publishing events
    pub fn event_sender(&self) -> mpsc::UnboundedSender<nostr::Event> {
        self.event_tx.clone()
    }
}

/// Daemon lifecycle management for Mode B Agent reminder proxy.
///
/// The daemon runs as a background task within the buzz-agent process,
/// maintaining a persistent WebSocket connection to the Relay to listen
/// for `kind:30300` (Event Reminder) due signals addressed to the Agent.
/// Also runs the System Health Guardian (Phase B) on a cron schedule.
pub struct Daemon {
    config: Arc<Config>,
    shutdown_tx: watch::Sender<bool>,
    handle: Option<tokio::task::JoinHandle<()>>,
    health_guardian_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Daemon {
    /// Create a new daemon instance.
    pub fn new(config: Arc<Config>) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            config,
            shutdown_tx,
            handle: None,
            health_guardian_handle: None,
        }
    }

    /// Start the daemon.
    ///
    /// Spawns the WebSocket client, subscription manager, event dispatcher,
    /// and the System Health Guardian (Phase B).
    /// Returns an error if the daemon is already running or redis_url is not configured.
    pub async fn start(&mut self) -> Result<(), AgentError> {
        if self.handle.is_some() {
            return Err(AgentError::InvalidParams("daemon already running".into()));
        }
        
        if self.config.redis_url.trim().is_empty() {
            return Err(AgentError::InvalidParams("BUZZ_AGENT_REDIS_URL must be configured for Mode B daemon".into()));
        }

        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = run_daemon(config, shutdown_rx).await {
                tracing::error!("daemon exited with error: {}", e);
            }
        });

        self.handle = Some(handle);
        
        // Start Health Guardian Phase B if configured
        if self.config.health_guardian.is_some() {
            let hg_shutdown_rx = self.shutdown_tx.subscribe();
            let hg_config = self.config.clone();
            let agent_keys = self.config.agent_keys.clone()
                .ok_or_else(|| AgentError::InvalidParams("agent keys required for health guardian".into()))?;
            
            let hg_handle = tokio::spawn(async move {
                if let Err(e) = phase_b_run_daemon(hg_config, agent_keys, hg_shutdown_rx).await {
                    tracing::error!("Health Guardian exited with error: {}", e);
                }
            });
            
            self.health_guardian_handle = Some(hg_handle);
            tracing::info!("System Health Guardian (Phase B) started");
        }

        tracing::info!("Agent daemon started");
        Ok(())
    }

    /// Stop the daemon gracefully.
    ///
    /// Signals shutdown and waits for the background tasks to complete.
    pub async fn stop(&mut self) {
        if self.shutdown_tx.send(true).is_err() {
            tracing::warn!("daemon shutdown channel already closed");
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.health_guardian_handle.take() {
            let _ = handle.await;
        }
        tracing::info!("Agent daemon stopped");
    }

    /// Restart the daemon (stop then start).
    pub async fn restart(&mut self) -> Result<(), AgentError> {
        self.stop().await;
        self.start().await
    }

    /// Check if the daemon is currently running.
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }
}

/// Main daemon run loop.
async fn run_daemon(
    config: Arc<Config>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<(), AgentError> {
    use crate::daemon_ws::WebSocketClient;
    use crate::daemon_sub::SubscriptionManager;
    use crate::daemon_dispatch::EventDispatcher;
    use crate::daemon_lock::DaemonLock;

    // Acquire distributed lock to ensure single instance per agent pubkey
    let agent_pubkey = config
        .agent_keys
        .as_ref()
        .map(|k| k.public_key().to_hex())
        .ok_or_else(|| AgentError::InvalidParams("agent keys not configured".into()))?;

    let mut lock = DaemonLock::new(&config.redis_url, &agent_pubkey)
        .await
        .map_err(|e| AgentError::Other(format!("failed to create daemon lock: {e}")))?;

    let _guard = lock.acquire().await.map_err(|e| {
        AgentError::Other(format!("failed to acquire daemon lock: {e}"))
    })?;

    tracing::info!("Daemon lock acquired for agent: {}", agent_pubkey);

    // Start heartbeat task
    let heartbeat_handle = lock.start_heartbeat();

    // Create components
    let mut ws_client = WebSocketClient::new(config.redis_url.clone());
    let mut sub_manager = SubscriptionManager::new(agent_pubkey.clone());

    // Connect and authenticate
    ws_client.connect().await?;
    ws_client.auth(&config.agent_keys).await?;

    // Subscribe to kind:30300 events for this agent
    sub_manager.subscribe(&ws_client).await?;

    // Create dispatcher with a clone of the WebSocketClient for publishing
    let dispatcher = EventDispatcher::new(config.clone(), ws_client.clone());

    // Event processing loop
    let mut event_stream = ws_client.event_stream();

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!("Daemon received shutdown signal");
                    break;
                }
            }
            event = event_stream.recv() => {
                match event {
                    Some(event) => {
                        if let Err(e) = dispatcher.dispatch(event).await {
                            tracing::error!("Failed to dispatch reminder event: {}", e);
                        }
                    }
                    None => {
                        tracing::warn!("WebSocket stream ended");
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    heartbeat_handle.abort();
    ws_client.disconnect().await?;
    tracing::info!("Daemon shutdown complete");
    Ok(())
}
