use std::sync::Arc;

use crate::config::Config;
use crate::types::AgentError;
use tokio::sync::watch;

/// Daemon lifecycle management for Mode B Agent reminder proxy.
///
/// The daemon runs as a background task within the buzz-agent process,
/// maintaining a persistent WebSocket connection to the Relay to listen
/// for `kind:30300` (Event Reminder) due signals addressed to the Agent.
pub struct Daemon {
    config: Arc<Config>,
    shutdown_tx: watch::Sender<bool>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Daemon {
    /// Create a new daemon instance.
    pub fn new(config: Arc<Config>) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            config,
            shutdown_tx,
            handle: None,
        }
    }

    /// Start the daemon.
    ///
    /// Spawns the WebSocket client, subscription manager, and event dispatcher.
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
        tracing::info!("Agent daemon started");
        Ok(())
    }

    /// Stop the daemon gracefully.
    ///
    /// Signals shutdown and waits for the background task to complete.
    pub async fn stop(&mut self) {
        if self.shutdown_tx.send(true).is_err() {
            tracing::warn!("daemon shutdown channel already closed");
        }
        if let Some(handle) = self.handle.take() {
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

    let lock = DaemonLock::new(&config.redis_url, &agent_pubkey)
        .await
        .map_err(|e| AgentError::Other(format!("failed to create daemon lock: {e}")))?;

    let _guard = lock.acquire().await.map_err(|e| {
        AgentError::Other(format!("failed to acquire daemon lock: {e}"))
    })?;

    tracing::info!("Daemon lock acquired for agent: {}", agent_pubkey);

    // Start heartbeat task
    let heartbeat_handle = lock.start_heartbeat();

    // Create components
    let mut ws_client = WebSocketClient::new(config.relay_url.clone());
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
                    Some(Ok(event)) => {
                        if let Err(e) = dispatcher.dispatch(event).await {
                            tracing::error!("Failed to dispatch reminder event: {}", e);
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("WebSocket event error: {}", e);
                        // Attempt reconnection
                        ws_client.reconnect().await?;
                        sub_manager.subscribe(&ws_client).await?;
                        event_stream = ws_client.event_stream();
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
