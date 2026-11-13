use std::sync::Arc;

use anyhow::Result;
use nostr::{Event, EventBuilder, Keys, Kind, Tag};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::daemon_ws::WebSocketClient;
use crate::types::AgentError;

/// Event dispatcher for handling Due Signal (kind:30300) events.
///
/// Parses incoming reminder events, decrypts the NIP-44 content,
/// and sends a DM (kind:4/NIP-44 encrypted) to the target user.
pub struct EventDispatcher {
    config: Arc<Config>,
    ws_client: WebSocketClient,
}

impl EventDispatcher {
    /// Create a new event dispatcher.
    pub fn new(config: Arc<Config>, ws_client: WebSocketClient) -> Self {
        Self { config, ws_client }
    }

    /// Dispatch a received event.
    ///
    /// Validates the event is a kind:30300 reminder due signal,
    /// decrypts the NIP-44 content, and sends a DM to the target user.
    pub async fn dispatch(&self, event: Event) -> Result<(), AgentError> {
        // Verify event kind
        if event.kind != Kind::Custom(30300) {
            debug!("Ignoring non-reminder event: kind={}", event.kind);
            return Ok(());
        }

        // Verify it's for us (should be guaranteed by subscription filter)
        let my_pubkey = self.config.agent_keys.as_ref()
            .ok_or_else(|| AgentError::InvalidParams("agent keys not available".into()))?
            .public_key();
        
        if event.pubkey != my_pubkey {
            warn!("Received reminder for different pubkey, ignoring");
            return Ok(());
        }

        // Extract d-tag
        let d_tag = event.tags.iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("d"))
            .and_then(|t| t.as_slice().get(1))
            .and_then(|v| Some(v.as_str()))
            .ok_or_else(|| AgentError::Other("missing d-tag".into()))?;

        // Decrypt content (NIP-44 v2 self-decryption)
        let keys = self.config.agent_keys.as_ref()
            .ok_or_else(|| AgentError::InvalidParams("agent keys not available".into()))?;
        
        let plaintext = nostr::nips::nip44::decrypt(
            keys.secret_key(),
            &keys.public_key(),
            &event.content,
        ).map_err(|e| AgentError::Other(format!("NIP-44 decrypt failed: {e}")))?;

        // Parse reminder content
        let content: serde_json::Value = serde_json::from_str(&plaintext)
            .map_err(|e| AgentError::Other(format!("failed to parse reminder content: {e}")))?;

        let status = content.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
        let note = content.get("note").and_then(|v| v.as_str()).unwrap_or("");
        let target = content.get("target");

        // Only process pending reminders
        if status != "pending" {
            info!("Ignoring reminder with status: {}", status);
            return Ok(());
        }

        // Extract target user pubkey from target (dm_recipient_pubkey or authorPubkey)
        let target_pubkey = target
            .and_then(|t| t.get("dm_recipient_pubkey"))
            .and_then(|v| v.as_str())
            .or_else(|| target.and_then(|t| t.get("authorPubkey")).and_then(|v| v.as_str()))
            .ok_or_else(|| AgentError::Other("missing target recipient pubkey".into()))?;

        // Send DM reminder (kind:4 NIP-44 encrypted)
        self.send_dm_reminder(target_pubkey, note, d_tag).await?;

        info!("DM reminder sent for d-tag: {} to user: {}", d_tag, target_pubkey);
        Ok(())
    }

    /// Send a Direct Message (kind:4 / NIP-44 encrypted) reminder to the target user.
    async fn send_dm_reminder(
        &self,
        target_pubkey: &str,
        note: &str,
        d_tag: &str,
    ) -> Result<(), AgentError> {
        let keys = self.config.agent_keys.as_ref()
            .ok_or_else(|| AgentError::InvalidParams("agent keys not available".into()))?;

        let target_pk = nostr::PublicKey::from_hex(target_pubkey)
            .map_err(|e| AgentError::Other(format!("invalid target pubkey: {e}")))?;

        // Build DM content
        let dm_content = format!("⏰ 提醒：{}\n\n(d-tag: {})", note, d_tag);

        // Encrypt content to target using NIP-44 v2
        let encrypted_content = nostr::nips::nip44::encrypt(
            keys.secret_key(),
            &target_pk,
            &dm_content,
            nostr::nips::nip44::Version::V2,
        ).map_err(|e| AgentError::Other(format!("NIP-44 encrypt failed: {e}")))?;

        // Build DM event (kind:4)
        let dm_event = EventBuilder::new(Kind::Custom(4), encrypted_content)
            .tag(Tag::parse(["p", target_pubkey]).unwrap())
            .tag(Tag::parse(["d", d_tag]).unwrap())
            .sign_with_keys(keys.as_ref())
            .map_err(|e| AgentError::Other(format!("DM event sign failed: {e}")))?;

        // Publish via WebSocket (using the persistent connection)
        self.ws_client.publish(&dm_event).await?;
        
        Ok(())
    }
}
