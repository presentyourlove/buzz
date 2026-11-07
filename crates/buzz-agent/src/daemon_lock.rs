use std::time::Duration;

use anyhow::Result;
use deadpool_redis::{Config as RedisConfig, Pool, Runtime};
use redis::AsyncCommands;
use tracing::{debug, info, warn};

use crate::types::AgentError;

/// Distributed lock for ensuring single Agent Daemon instance per pubkey.
///
/// Uses Redis SET NX with TTL for distributed locking across multiple pods.
/// Includes heartbeat task to extend lock periodically.
pub struct DaemonLock {
    pool: Pool,
    lock_key: String,
    ttl: Duration,
    acquired: bool,
}

impl DaemonLock {
    /// Create a new daemon lock.
    pub async fn new(redis_url: &str, agent_pubkey: &str) -> Result<Self, AgentError> {
        let mut cfg = RedisConfig::default();
        cfg.url = Some(redis_url.to_string());
        
        let pool = cfg.create_pool(Some(Runtime::Tokio1))
            .map_err(|e| AgentError::Other(format!("failed to create Redis pool: {e}")))?;

        // Test connection
        let mut conn = pool.get().await
            .map_err(|e| AgentError::Other(format!("failed to get Redis connection: {e}")))?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await
            .map_err(|e| AgentError::Other(format!("Redis PING failed: {e}")))?;

        Ok(Self {
            pool,
            lock_key: format!("buzz_agent_daemon:{}", agent_pubkey),
            ttl: Duration::from_secs(10),
            acquired: false,
        })
    }

    /// Acquire the lock.
    pub async fn acquire(&mut self) -> Result<(), AgentError> {
        let mut conn = self.pool.get().await
            .map_err(|e| AgentError::Other(format!("failed to get Redis connection: {e}")))?;

        // Use SET NX EX for atomic acquire with TTL
        let result: Option<String> = redis::cmd("SET")
            .arg(&self.lock_key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(self.ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::Other(format!("Redis SET failed: {e}")))?;

        if result.is_some() {
            self.acquired = true;
            info!("Daemon lock acquired: {}", self.lock_key);
            Ok(())
        } else {
            warn!("Daemon lock already held by another instance: {}", self.lock_key);
            Err(AgentError::Other("daemon lock already held".into()))
        }
    }

    /// Release the lock.
    pub async fn release(&mut self) -> Result<(), AgentError> {
        if !self.acquired {
            return Ok(());
        }

        let mut conn = self.pool.get().await
            .map_err(|e| AgentError::Other(format!("failed to get Redis connection: {e}")))?;

        // Only delete if we own it (value is "1")
        let script = redis::Script::new(r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
        "#);

        let _: i32 = script.key(&self.lock_key).arg("1").invoke_async(&mut conn).await
            .map_err(|e| AgentError::Other(format!("Redis lock release failed: {e}")))?;

        self.acquired = false;
        info!("Daemon lock released: {}", self.lock_key);
        Ok(())
    }

    /// Extend the lock TTL (heartbeat).
    pub async fn extend(&mut self) -> Result<(), AgentError> {
        if !self.acquired {
            return Err(AgentError::Other("lock not acquired".into()));
        }

        let mut conn = self.pool.get().await
            .map_err(|e| AgentError::Other(format!("failed to get Redis connection: {e}")))?;

        let result: bool = redis::cmd("EXPIRE")
            .arg(&self.lock_key)
            .arg(self.ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| AgentError::Other(format!("Redis EXPIRE failed: {e}")))?;

        if !result {
            self.acquired = false;
            warn!("Lock expired during extend, lost ownership: {}", self.lock_key);
            return Err(AgentError::Other("lock expired, lost ownership".into()));
        }

        debug!("Daemon lock extended: {}", self.lock_key);
        Ok(())
    }

    /// Start background heartbeat task to extend lock periodically.
    /// Returns a JoinHandle that should be aborted on shutdown.
    pub fn start_heartbeat(&self) -> tokio::task::JoinHandle<()> {
        let pool = self.pool.clone();
        let lock_key = self.lock_key.clone();
        let ttl = self.ttl;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(ttl / 3); // Extend at 1/3 TTL
            loop {
                interval.tick().await;
                let mut conn = match pool.get().await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Heartbeat failed to get Redis connection: {}", e);
                        continue;
                    }
                };

                let script = redis::Script::new(r#"
                    if redis.call("GET", KEYS[1]) == ARGV[1] then
                        return redis.call("EXPIRE", KEYS[1], ARGV[2])
                    else
                        return 0
                    end
                "#);

                let result: Result<i32, _> = script
                    .key(&lock_key)
                    .arg("1")
                    .arg(ttl.as_secs())
                    .invoke_async(&mut conn)
                    .await;

                if result.is_err() || result.unwrap() == 0 {
                    warn!("Heartbeat failed to extend lock, may have lost ownership: {}", lock_key);
                    break;
                }
                debug!("Daemon lock heartbeat: {}", lock_key);
            }
        })
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        if self.acquired {
            // Best-effort release on drop
            let pool = self.pool.clone();
            let lock_key = self.lock_key.clone();
            tokio::spawn(async move {
                if let Ok(mut conn) = pool.get().await {
                    let script = redis::Script::new(r#"
                        if redis.call("GET", KEYS[1]) == ARGV[1] then
                            return redis.call("DEL", KEYS[1])
                        else
                            return 0
                        end
                    "#);
                    let _ = script.key(&lock_key).arg("1").invoke_async(&mut conn).await;
                }
            });
        }
    }
}
