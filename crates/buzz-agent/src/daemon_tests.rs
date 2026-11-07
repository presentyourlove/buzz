#[cfg(test)]
mod daemon_tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;

    /// Test that DaemonLock can be created with valid Redis URL
    #[tokio::test]
    #[ignore = "requires Redis"]
    async fn daemon_lock_acquire_release() {
        let lock = crate::daemon_lock::DaemonLock::new("redis://localhost:6379", "test_pubkey").await.unwrap();
        let mut lock = lock;
        lock.acquire().await.unwrap();
        lock.release().await.unwrap();
    }

    /// Test that DaemonLock prevents double acquire
    #[tokio::test]
    #[ignore = "requires Redis"]
    async fn daemon_lock_double_acquire_fails() {
        let lock1 = crate::daemon_lock::DaemonLock::new("redis://localhost:6379", "test_pubkey2").await.unwrap();
        let mut lock1 = lock1;
        lock1.acquire().await.unwrap();

        let lock2 = crate::daemon_lock::DaemonLock::new("redis://localhost:6379", "test_pubkey2").await.unwrap();
        let mut lock2 = lock2;
        assert!(lock2.acquire().await.is_err());

        lock1.release().await.unwrap();
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;

    /// Test parsing reminder content from kind:30300 event
    #[test]
    fn parse_reminder_content() {
        let keys = Keys::generate();
        let target_pubkey = keys.public_key().to_hex();
        
        let reminder_content = json!({
            "target": {
                "dm_recipient_pubkey": target_pubkey,
                "channelId": "chan1",
                "preview": "test message",
                "authorPubkey": "author1"
            },
            "status": "pending",
            "note": "喝水"
        });

        let ciphertext = nostr::nips::nip44::encrypt(
            keys.secret_key(),
            keys.public_key(),
            &reminder_content.to_string(),
            nostr::nips::nip44::Version::V2,
        ).unwrap();

        let event = EventBuilder::new(Kind::Custom(30300), ciphertext)
            .tag(Tag::parse(["d", "test123"]).unwrap())
            .tag(Tag::parse(["not_before", "1234567890"]).unwrap())
            .sign_with_keys(&keys)
            .unwrap();

        // Verify we can decrypt
        let plaintext = nostr::nips::nip44::decrypt(
            keys.secret_key(),
            keys.public_key(),
            &event.content,
            nostr::nips::nip44::Version::V2,
        ).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&plaintext).unwrap();
        assert_eq!(parsed["status"], "pending");
        assert_eq!(parsed["note"], "喝水");
        assert_eq!(parsed["target"]["dm_recipient_pubkey"], target_pubkey);
    }
}

#[cfg(test)]
mod ws_client_tests {
    use super::*;

    /// Test WebSocketClient creation
    #[test]
    fn ws_client_new() {
        let client = crate::daemon_ws::WebSocketClient::new("wss://relay.example.com".to_string());
        assert_eq!(client.relay_url, "wss://relay.example.com");
    }
}

#[cfg(test)]
mod subscription_manager_tests {
    use super::*;
    use nostr::Keys;

    /// Test SubscriptionManager creation and filter building
    #[test]
    fn subscription_manager_new() {
        let keys = Keys::generate();
        let manager = crate::daemon_sub::SubscriptionManager::new(keys.public_key().to_hex());
        assert!(manager.current_sub_id.is_none());
    }
}
