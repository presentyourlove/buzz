//! Local Event Bus — Async Pub/Sub with Backpressure
//! 
//! Provides a lightweight in-process event bus for agent-internal communication.
//! Supports typed events, multiple subscribers per event type, bounded channels
//! for backpressure, and graceful shutdown.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::timeout;
use tracing::{info, warn};

use crate::types::AgentError;

/// Maximum number of events buffered per subscriber before backpressure applies
const DEFAULT_SUBSCRIBER_BUFFER: usize = 100;

/// Maximum time to wait for a slow subscriber before considering it lagging
const SUBSCRIBER_TIMEOUT: Duration = Duration::from_secs(5);

/// Trait for event types that can be published on the bus
pub trait Event: Send + Sync + Clone + 'static {
    /// Unique event type identifier for routing
    fn event_type(&self) -> &'static str;
}

/// Generic boxed event for dynamic dispatch
#[derive(Clone)]
pub struct BoxedEvent {
    pub event_type: &'static str,
    pub payload: Value,
}

impl BoxedEvent {
    /// Get the event type identifier
    pub fn event_type(&self) -> &'static str {
        self.event_type
    }
}

impl std::fmt::Debug for BoxedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedEvent")
            .field("event_type", &self.event_type)
            .field("payload", &self.payload)
            .finish()
    }
}

/// Subscription handle for unsubscribing
#[derive(Debug, Clone)]
pub struct Subscription {
    pub event_type: &'static str,
    pub subscriber_id: usize,
}

/// Event subscriber trait - implemented by types that want to receive events
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Called when an event is received
    async fn on_event(&self, event: BoxedEvent) -> Result<(), AgentError>;
    
    /// Optional: called when subscriber is registered
    async fn on_register(&self) -> Result<(), AgentError> {
        Ok(())
    }
    
    /// Optional: called when subscriber is unregistered
    async fn on_unregister(&self) -> Result<(), AgentError> {
        Ok(())
    }
}

/// Internal subscriber wrapper with bounded channel
struct SubscriberEntry {
    tx: mpsc::Sender<BoxedEvent>,
    subscriber: Arc<dyn EventSubscriber>,
    event_type: &'static str,
    subscriber_id: usize,
}

impl std::fmt::Debug for SubscriberEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriberEntry")
            .field("event_type", &self.event_type)
            .field("subscriber_id", &self.subscriber_id)
            .finish()
    }
}

/// Local Event Bus for in-process pub/sub
#[derive(Debug)]
pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<&'static str, Vec<SubscriberEntry>>>>,
    next_subscriber_id: Arc<std::sync::atomic::AtomicUsize>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl EventBus {
    /// Create a new EventBus
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            next_subscriber_id: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            shutdown_tx: None,
        }
    }

    /// Initialize shutdown signal (call once before starting subscribers)
    pub fn init_shutdown(&mut self) -> broadcast::Receiver<()> {
        let (tx, rx) = broadcast::channel(1);
        self.shutdown_tx = Some(tx);
        rx
    }

    /// Get shutdown receiver for subscribers to listen on
    pub fn shutdown_rx(&self) -> Option<broadcast::Receiver<()>> {
        self.shutdown_tx.as_ref().map(|tx| tx.subscribe())
    }

    /// Subscribe to events of a specific type
    /// 
    /// Returns a Subscription handle that can be used to unsubscribe
    pub async fn subscribe<S: EventSubscriber + 'static>(
        &self,
        event_type: &'static str,
        subscriber: S,
    ) -> Result<Subscription, AgentError> {
        let subscriber_id = self.next_subscriber_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let subscriber = Arc::new(subscriber);
        
        // Create bounded channel for this subscriber
        let (tx, mut rx) = mpsc::channel::<BoxedEvent>(DEFAULT_SUBSCRIBER_BUFFER);
        
        let entry = SubscriberEntry {
            tx: tx.clone(),
            subscriber: subscriber.clone(),
            event_type,
            subscriber_id,
        };

        // Register subscriber
        {
            let mut subs = self.subscribers.write().await;
            subs.entry(event_type).or_default().push(entry);
        }

        // Call on_register
        subscriber.on_register().await?;

        // Spawn event processing task for this subscriber
        let subscriber_clone = subscriber.clone();
        let event_type_clone = event_type;
        let shutdown_rx = self.shutdown_rx();
        
        tokio::spawn(async move {
            let mut shutdown = shutdown_rx;
            loop {
                tokio::select! {
                    biased;
                    _ = async {
                        if let Some(ref mut rx) = shutdown {
                            rx.recv().await.ok();
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {
                        // Shutdown signal received
                        break;
                    }
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                // Apply timeout for backpressure handling
                                let result = timeout(SUBSCRIBER_TIMEOUT, subscriber_clone.on_event(event)).await;
                                if let Err(_) = result {
                                    tracing::warn!(
                                        "Subscriber for '{}' timed out processing event",
                                        event_type_clone
                                    );
                                } else if let Err(e) = result.unwrap() {
                                    tracing::error!(
                                        "Subscriber for '{}' error: {}",
                                        event_type_clone, e
                                    );
                                }
                            }
                            None => {
                                // Channel closed, subscriber done
                                break;
                            }
                        }
                    }
                }
            }
            // Call on_unregister on cleanup
            let _ = subscriber_clone.on_unregister().await;
        });

        Ok(Subscription {
            event_type,
            subscriber_id,
        })
    }

    /// Unsubscribe using a Subscription handle
    pub async fn unsubscribe(&self, subscription: &Subscription) -> Result<(), AgentError> {
        let mut subs = self.subscribers.write().await;
        if let Some(vec) = subs.get_mut(subscription.event_type) {
            vec.retain(|e| e.subscriber_id != subscription.subscriber_id);
            if vec.is_empty() {
                subs.remove(subscription.event_type);
            }
        }
        Ok(())
    }

    /// Publish an event to all subscribers of its type
    /// 
    /// Returns the number of subscribers that received the event
    /// Returns error if no subscribers for this event type (use `publish_optional` for non-error)
    pub async fn publish<E: Event + Serialize>(&self, event: E) -> Result<usize, AgentError> {
        let boxed = BoxedEvent {
            event_type: event.event_type(),
            payload: serde_json::to_value(event)?,
        };
        self.publish_boxed(boxed).await
    }

    /// Publish a boxed event (for dynamic event types)
    pub async fn publish_boxed(&self, event: BoxedEvent) -> Result<usize, AgentError> {
        let subs = self.subscribers.read().await;
        let subscribers = subs.get(event.event_type);
        
        match subscribers {
            Some(vec) if !vec.is_empty() => {
                let mut delivered = 0;
                for entry in vec {
                    // Try to send with immediate backpressure check
                    match entry.tx.try_send(event.clone()) {
                        Ok(_) => delivered += 1,
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            warn!(event_type = %event.event_type(), subscriber_id = %entry.subscriber_id, "Subscriber channel full, dropping event");
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            warn!(event_type = %event.event_type(), subscriber_id = %entry.subscriber_id, "Subscriber channel closed");
                        }
                    }
                }
                Ok(delivered)
            }
            _ => Err(AgentError::EventBusNoSubscribers { event_type: event.event_type().to_string() }),
        }
    }

    /// Publish an event, returning Ok(0) if no subscribers (non-error)
    pub async fn publish_optional<E: Event + Serialize>(&self, event: E) -> Result<usize, AgentError> {
        let boxed = BoxedEvent {
            event_type: event.event_type(),
            payload: serde_json::to_value(event)?,
        };
        match self.publish_boxed(boxed).await {
            Ok(n) => Ok(n),
            Err(AgentError::EventBusNoSubscribers { .. }) => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Graceful shutdown: close all subscriber channels and wait for handlers
    pub async fn shutdown(&self) {
        info!("EventBus shutting down...");
        let subs = self.subscribers.read().await;
        for (_, vec) in subs.iter() {
            for entry in vec {
                // Close the sender side; receiver will get None on next recv
                let _ = entry.tx.closed().await;
            }
        }
        // Give subscribers a moment to process final messages
        tokio::time::sleep(Duration::from_millis(100)).await;
        info!("EventBus shutdown complete");
    }
}
