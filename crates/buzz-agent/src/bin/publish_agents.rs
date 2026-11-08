use std::fs;
use futures_util::{SinkExt, StreamExt};
use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Buzz Relay Agent Synchronizer ===");

    // 1. Owner Keys
    let nsec = "nsec1ngdqf8w8hg2mq424znxnlc7qtpch9lwxf7pl0dtkn72ga0d9nyfqsjg7hq";
    let keys = Keys::parse(nsec)?;
    let owner_pubkey = keys.public_key().to_hex();
    println!("Owner Pubkey: {}", owner_pubkey);

    // 2. Read managed-agents.json
    let app_data = std::env::var("APPDATA")?;
    let path = format!("{}\\xyz.block.buzz.app\\agents\\managed-agents.json", app_data);
    let content = fs::read_to_string(&path)?;
    let agents: Vec<Value> = serde_json::from_str(&content)?;

    // 3. Connect to Relay WebSocket
    let relay_ws = "wss://presentyourlove.communities.buzz.xyz";
    println!("Connecting to relay WebSocket: {}", relay_ws);
    let (ws_stream, _) = connect_async(relay_ws).await?;
    let (mut write, mut read) = ws_stream.split();
    println!("WebSocket connected successfully! Waiting for initial messages...");

    // 4. Authenticate via NIP-42 if challenged
    let mut authenticated = false;

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                println!("Relay initial message: {}", text);
                let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if let Some(arr) = parsed.as_array() {
                    if arr.get(0).and_then(|v| v.as_str()) == Some("AUTH") {
                        if let Some(challenge) = arr.get(1).and_then(|v| v.as_str()) {
                            println!("Received AUTH challenge: {}", challenge);
                            let auth_event = EventBuilder::new(Kind::Custom(22242), "")
                                .tag(Tag::parse(["relay", relay_ws])?)
                                .tag(Tag::parse(["challenge", challenge])?)
                                .sign_with_keys(&keys)?;
                            let auth_msg = format!("[\"AUTH\",{}]", auth_event.as_json());
                            println!("Sending signed NIP-42 AUTH response...");
                            write.send(Message::Text(auth_msg.into())).await?;
                        }
                    } else if arr.get(0).and_then(|v| v.as_str()) == Some("OK") {
                        let ok_status = arr.get(2).and_then(|v| v.as_bool()).unwrap_or(false);
                        let reason = arr.get(3).and_then(|v| v.as_str()).unwrap_or("");
                        println!("AUTH OK: {} ({})", ok_status, reason);
                        if ok_status {
                            authenticated = true;
                            break;
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }

    if !authenticated {
        println!("Note: Continuing to event publication...");
    }

    let mut published = 0;

    for a in &agents {
        let pubkey = match a.get("pubkey").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("Agent");
        let runtime = "buzz/buzz-agent";

        let mut content_map = serde_json::Map::new();
        content_map.insert("name".into(), Value::String(name.into()));
        if let Some(m) = a.get("model") { content_map.insert("model".into(), m.clone()); }
        if let Some(p) = a.get("provider") { content_map.insert("provider".into(), p.clone()); }
        content_map.insert("parallelism".into(), serde_json::json!(10));
        content_map.insert("respond_to".into(), serde_json::json!("anyone"));
        content_map.insert("respond_to_allowlist".into(), serde_json::json!([]));
        let content_str = Value::Object(content_map).to_string();

        let tags = vec![
            Tag::parse(["d", pubkey])?,
            Tag::parse(["runtime", runtime])?,
        ];

        let event = EventBuilder::new(Kind::Custom(30177), content_str)
            .tags(tags)
            .sign_with_keys(&keys)?;

        let event_json = event.as_json();
        let msg = format!("[\"EVENT\",{}]", event_json);

        println!("Sending kind:30177 for {} (pubkey: {})...", name, &pubkey[..8]);
        write.send(Message::Text(msg.into())).await?;

        // Wait for OK response
        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    println!("  Relay reply: {}", text);
                    if text.starts_with("[\"OK\"") || text.starts_with("[\"NOTICE\"") {
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("  Read error: {}", e);
                    break;
                }
            }
        }
        published += 1;
    }

    println!("\nSuccessfully published {} agents with runtime tags to Relay!", published);
    Ok(())
}
