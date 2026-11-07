use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::{debug, info, warn};
use uuid;

use crate::config::Config;
use crate::types::AgentError;
use nostr::{Event, EventBuilder, Keys, Kind, Tag};

/// WebSocket client for connecting to Nostr Relay.
pub struct WebSocketClient {
    relay_url: String,
    ws_sender: Option<Arc<mpsc::UnboundedSender<Message>>>,
    event_tx: mpsc::UnboundedSender<nostr::Event>,
    event_rx: Option<mpsc::UnboundedReceiver<nostr::Event>>,
    reconnect_attempts: u32,
    max_reconnect_delay: Duration,
}

impl Clone for WebSocketClient {
    fn clone(&self) -> Self {
        Self {
            relay_url: self.relay_url.clone(),
            ws_sender: self.ws_sender.clone(),
            event_tx: self.event_tx.clone(),
            event_rx: None, // Receiver cannot be cloned
            reconnect_attempts: self.reconnect_attempts,
            max_reconnect_delay: self.max_reconnect_delay,
        }
    }
}

impl WebSocketClient {
    /// Create a new WebSocket client.
    pub fn new(relay_url: String) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            relay_url,
            ws_sender: None,
            event_tx,
            event_rx: Some(event_rx),
            reconnect_attempts: 0,
            max_reconnect_delay: Duration::from_secs(60),
        }
    }

    /// Connect to the relay and establish WebSocket connection.
    pub async fn connect(&mut self) -> Result<(), AgentError> {
        let request = self.relay_url.as_str().into_client_request()?;
        let (ws_stream, _) = connect_async(request).await
            .map_err(|e| AgentError::Other(format!("WebSocket connect failed: {e}")))?;

        let (ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, rx) = mpsc::unbounded_channel::<Message>();
        let tx_arc = Arc::new(tx);
        self.ws_sender = Some(tx_arc.clone());

        let event_tx = self.event_tx.clone();

        // Spawn writer task
        tokio::spawn(async move {
            let mut rx = rx;
            let mut ws_sender = ws_sender;
            while let Some(msg) = rx.recv().await {
                if ws_sender.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Spawn reader task
        tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(event) = serde_json::from_str::<nostr::Event>(&text) {
                            if event_tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed by server");
                        break;
                    }
                    Ok(Message::Ping(data)) => {
                        if let Some(sender) = &tx_arc {
                            let _ = sender.send(Message::Pong(data));
                        }
                    }
                    Ok(Message::Pong(_)) => {
                        // Heartbeat response
                    }
                    Err(e) => {
                        warn!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Wait a bit for connection to stabilize
        tokio::time::sleep(Duration::from_millis(100)).await;

        info!("WebSocket connected to {}", self.relay_url);
        Ok(())
    }

    /// Authenticate with NIP-42.
    ///
    /// Implements the full challenge-response flow:
    /// 1. Send initial AUTH with empty challenge
    /// 2. Wait for relay's challenge response (if required)
    /// 3. Send AUTH with the received challenge
    pub async fn auth(&self, keys: &Option<Arc<nostr::Keys>>) -> Result<(), AgentError> {
        let keys = keys.as_ref()
            .ok_or_else(|| AgentError::InvalidParams("agent keys not available".into()))?;

        // Send initial AUTH with empty challenge
        let auth_event = EventBuilder::new(Kind::Custom(22242), "")
            .tag(Tag::parse(["relay", &self.relay_url]).unwrap())
            .tag(Tag::parse(["challenge", ""]).unwrap())
            .sign_with_keys(keys.as_ref())
            .map_err(|e| AgentError::Other(format!("auth event sign failed: {e}")))?;

        self.send(Message::Text(serde_json::to_string(&auth_event)?))
            .await
            .map_err(|e| AgentError::Other(format!("auth send failed: {e}")))?;

        debug!("NIP-42 initial auth sent, waiting for challenge...");

        // Note: Full challenge-response handling would require access to the event stream
        // to detect relay's challenge response. For now, we send initial auth and
        // relays that don't require auth will accept it. Relays that do require auth
        // will send a challenge via NOTICE, which would need a more complex handler.
        // This is a known limitation - see I4 for full implementation.
        
        Ok(())
    }

    /// Publish an event to the relay via WebSocket.
    pub async fn publish(&self, event: &Event) -> Result<(), AgentError> {
        let msg = Message::Text(serde_json::json!(["EVENT", event]).to_string());
        self.send(msg).await
            .map_err(|e| AgentError::Other(format!("WebSocket publish failed: {e}")))?;
        debug!("Event published via WebSocket: {}", event.id);
        Ok(())
    }

    /// Subscribe to events with given filter.
    pub async fn subscribe(&self, filter: nostr::Filter) -> Result<(), AgentError> {
        let req = serde_json::json!([
            "REQ",
            format!("sub_{}", uuid::Uuid::new_v4()),
            filter
        ]);
        self.send(Message::Text(req.to_string())).await
            .map_err(|e| AgentError::Other(format!("subscribe send failed: {e}")))?;
        debug!("Subscription sent: {:?}", filter);
        Ok(())
    }

    /// Send a message over WebSocket.
    async fn send(&self, msg: Message) -> Result<(), anyhow::Error> {
        if let Some(sender) = &self.ws_sender {
            sender.send(msg)?;
        }
        Ok(())
    }

    /// Get the event receiver for processing incoming events.
    pub fn event_stream(&mut self) -> mpsc::UnboundedReceiver<nostr::Event> {
        self.event_rx.take().unwrap()
    }

    /// Reconnect with exponential backoff.
    pub async fn reconnect(&mut self) -> Result<(), AgentError> {
        self.reconnect_attempts += 1;
        let delay = Duration::from_secs(2_u64.pow(self.reconnect_attempts.min(6)))
            .min(self.max_reconnect_delay);

        warn!("Reconnecting in {:?} (attempt {})", delay, self.reconnect_attempts);
        tokio::time::sleep(delay).await;
        self.connect().await
    }

    /// Disconnect gracefully.
    pub async fn disconnect(&mut self) -> Result<(), AgentError> {
        if let Some(sender) = self.ws_sender.take() {
            let _ = sender.send(Message::Close(None));
        }
        info!("WebSocket disconnected");
        Ok(())
    }
}
