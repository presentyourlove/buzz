use std::sync::Arc;

use anyhow::Result;
use nostr::{Filter, Keys, Kind, PublicKey};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, info};
use uuid;

use crate::types::AgentError;
use crate::daemon_ws::WebSocketClient;

/// Subscription manager for Nostr Relay event subscriptions.
///
/// Manages dynamic REQ filters for kind:30300 events addressed to the agent.
pub struct SubscriptionManager {
    agent_pubkey: PublicKey,
    pub(crate) current_sub_id: Option<String>,
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new(agent_pubkey: String) -> Self {
        let pubkey = PublicKey::from_hex(&agent_pubkey)
            .expect("invalid agent pubkey");
        Self {
            agent_pubkey: pubkey,
            current_sub_id: None,
        }
    }

    /// Subscribe to kind:30300 events for this agent.
    pub async fn subscribe(&mut self, ws_client: &WebSocketClient) -> Result<(), AgentError> {
        // Close existing subscription if any
        if let Some(sub_id) = &self.current_sub_id {
            self.unsubscribe(ws_client, sub_id).await?;
        }

        // Build filter for kind:30300 authored by this agent
        let filter = Filter::new()
            .kind(Kind::Custom(30300))
            .author(self.agent_pubkey);

        let sub_id = format!("sub_{}", uuid::Uuid::new_v4());
        self.current_sub_id = Some(sub_id.clone());

        let req = serde_json::json!([
            "REQ",
            sub_id,
            filter
        ]);

        ws_client.send(Message::Text(req.to_string().into())).await
            .map_err(|e| AgentError::Other(format!("subscribe send failed: {e}")))?;

        info!("Subscribed to kind:30300 for agent: {}", self.agent_pubkey);
        debug!("Filter: {:?}", filter);
        Ok(())
    }

    /// Unsubscribe from current subscription.
    pub async fn unsubscribe(&self, ws_client: &WebSocketClient, sub_id: &str) -> Result<(), AgentError> {
        let req = serde_json::json!([
            "CLOSE",
            sub_id
        ]);
        ws_client.send(Message::Text(req.to_string().into())).await
            .map_err(|e| AgentError::Other(format!("unsubscribe send failed: {e}")))?;
        info!("Unsubscribed from {}", sub_id);
        Ok(())
    }

    /// Resubscribe (close old, create new).
    pub async fn resubscribe(&mut self, ws_client: &WebSocketClient) -> Result<(), AgentError> {
        self.subscribe(ws_client).await
    }
}
