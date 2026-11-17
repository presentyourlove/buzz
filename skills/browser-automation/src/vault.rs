//! Encrypted Vault Integration
//! 
//! Integrates with Antigravity's Encrypted Vault for secure credential management.
//! Provides secure storage and retrieval of credentials for browser automation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

/// Credential type for browser automation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    /// Username/password
    Password,
    /// API key
    ApiKey,
    /// Bearer token
    BearerToken,
    /// OAuth token
    OAuthToken,
    /// SSH key
    SshKey,
    /// Certificate
    Certificate,
    /// Cookie
    Cookie,
    /// Custom type
    Custom,
}

/// Antigravity credential (for compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// Unique credential ID
    pub id: Uuid,
    /// Credential type
    pub credential_type: CredentialType,
    /// Domain this credential is for
    pub domain: String,
    /// Username/identifier
    pub username: String,
    /// Encrypted secret (password, token, etc.)
    pub secret: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Last used timestamp
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    /// Usage count
    pub usage_count: u64,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Credential reference for browser automation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserCredential {
    /// Unique credential ID
    pub id: Uuid,
    /// Credential type
    pub credential_type: CredentialType,
    /// Domain this credential is for
    pub domain: String,
    /// Username/identifier
    pub username: String,
    /// Encrypted secret (password, token, etc.)
    pub secret: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Last used timestamp
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    /// Usage count
    pub usage_count: u64,
}

impl From<Credential> for BrowserCredential {
    fn from(cred: Credential) -> Self {
        Self {
            id: cred.id,
            credential_type: cred.credential_type,
            domain: cred.domain,
            username: cred.username,
            secret: cred.secret,
            metadata: cred.metadata,
            last_used: cred.last_used,
            usage_count: cred.usage_count,
        }
    }
}

/// Vault client for browser automation
pub struct VaultClient {
    /// Antigravity vault storage
    storage: Arc<dyn VaultStorage>,
    /// In-memory cache for performance
    cache: Arc<RwLock<HashMap<Uuid, BrowserCredential>>>,
}

/// Vault storage trait (to be implemented by antigravity)
#[async_trait::async_trait]
pub trait VaultStorage: Send + Sync {
    async fn list_credentials(&self) -> Result<Vec<Credential>>;
    async fn get_credential(&self, id: Uuid) -> Result<Option<Credential>>;
    async fn store_credential(&self, cred: Credential) -> Result<()>;
    async fn update_credential(&self, cred: Credential) -> Result<()>;
    async fn delete_credential(&self, id: Uuid) -> Result<()>;
}

impl VaultClient {
    /// Create a new vault client
    pub async fn new(storage: Arc<dyn VaultStorage>) -> Result<Self> {
        let client = Self {
            storage,
            cache: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Warm up cache
        client.refresh_cache().await?;
        
        Ok(client)
    }
    
    /// Refresh the credential cache from storage
    pub async fn refresh_cache(&self) -> Result<()> {
        let credentials = self.storage.list_credentials().await
            .context("Failed to list credentials from vault")?;
        
        let mut cache = self.cache.write().await;
        cache.clear();
        
        for cred in credentials {
            cache.insert(cred.id, cred.into());
        }
        
        info!("Vault cache refreshed with {} credentials", cache.len());
        Ok(())
    }
    
    /// Get a credential by ID
    pub async fn get_credential(&self, credential_id: Uuid) -> Result<Option<BrowserCredential>> {
        // Try cache first
        {
            let cache = self.cache.read().await;
            if let Some(cred) = cache.get(&credential_id) {
                return Ok(Some(cred.clone()));
            }
        }
        
        // Fallback to storage
        let cred = self.storage.get_credential(credential_id).await
            .context("Failed to get credential from vault")?;
        
        Ok(cred.map(|c| c.into()))
    }
    
    /// Get credentials for a specific domain
    pub async fn get_credentials_for_domain(&self, domain: &str) -> Result<Vec<BrowserCredential>> {
        let cache = self.cache.read().await;
        
        let results: Vec<BrowserCredential> = cache
            .values()
            .filter(|c| c.domain == domain || domain.ends_with(&format!(".{}", c.domain)))
            .cloned()
            .collect();
        
        Ok(results)
    }
    
    /// Get credentials by type
    pub async fn get_credentials_by_type(&self, cred_type: CredentialType) -> Result<Vec<BrowserCredential>> {
        let cache = self.cache.read().await;
        
        let results: Vec<BrowserCredential> = cache
            .values()
            .filter(|c| c.credential_type == cred_type)
            .cloned()
            .collect();
        
        Ok(results)
    }
    
    /// Store a new credential
    pub async fn store_credential(&self, credential: BrowserCredential) -> Result<()> {
        let antigravity_cred = Credential {
            id: credential.id,
            credential_type: credential.credential_type,
            domain: credential.domain.clone(),
            username: credential.username.clone(),
            secret: credential.secret.clone(),
            metadata: credential.metadata.clone(),
            last_used: credential.last_used,
            usage_count: credential.usage_count,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        self.storage.store_credential(antigravity_cred).await
            .context("Failed to store credential in vault")?;
        
        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(credential.id, credential);
        
        info!("Stored new credential in vault");
        Ok(())
    }
    
    /// Update an existing credential
    pub async fn update_credential(&self, credential: BrowserCredential) -> Result<()> {
        let antigravity_cred = Credential {
            id: credential.id,
            credential_type: credential.credential_type,
            domain: credential.domain.clone(),
            username: credential.username.clone(),
            secret: credential.secret.clone(),
            metadata: credential.metadata.clone(),
            last_used: Some(chrono::Utc::now()),
            usage_count: credential.usage_count + 1,
            created_at: chrono::Utc::now(), // Will be overwritten by storage
            updated_at: chrono::Utc::now(),
        };
        
        self.storage.update_credential(antigravity_cred).await
            .context("Failed to update credential in vault")?;
        
        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(credential.id, credential);
        
        Ok(())
    }
    
    /// Delete a credential
    pub async fn delete_credential(&self, credential_id: Uuid) -> Result<()> {
        self.storage.delete_credential(credential_id).await
            .context("Failed to delete credential from vault")?;
        
        let mut cache = self.cache.write().await;
        cache.remove(&credential_id);
        
        info!("Deleted credential from vault");
        Ok(())
    }
    
    /// Mark credential as used (updates last_used and usage_count)
    pub async fn mark_used(&self, credential_id: Uuid) -> Result<()> {
        if let Some(mut cred) = self.get_credential(credential_id).await? {
            cred.last_used = Some(chrono::Utc::now());
            cred.usage_count += 1;
            self.update_credential(cred).await?;
        }
        Ok(())
    }
    
    /// List all credentials (for admin/debug)
    pub async fn list_all(&self) -> Result<Vec<BrowserCredential>> {
        let cache = self.cache.read().await;
        Ok(cache.values().cloned().collect())
    }
    
    /// Get credential count
    pub async fn count(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
    
    /// Find credentials matching a pattern (domain, username, etc.)
    pub async fn find_credentials(&self, pattern: &str) -> Result<Vec<BrowserCredential>> {
        let cache = self.cache.read().await;
        let pattern_lower = pattern.to_lowercase();
        
        let results: Vec<BrowserCredential> = cache
            .values()
            .filter(|c| {
                c.domain.to_lowercase().contains(&pattern_lower)
                    || c.username.to_lowercase().contains(&pattern_lower)
            })
            .cloned()
            .collect();
        
        Ok(results)
    }
}

/// Credential selector for automatic credential selection
pub struct CredentialSelector {
    vault: Arc<VaultClient>,
}

impl CredentialSelector {
    pub fn new(vault: Arc<VaultClient>) -> Self {
        Self { vault }
    }
    
    /// Select the best credential for a domain and action
    pub async fn select_for_domain(
        &self,
        domain: &str,
        action: &str,
    ) -> Result<Option<BrowserCredential>> {
        let all = self.vault.list_all().await?;
        
        // Filter by domain
        let mut domain_matches: Vec<_> = all
            .into_iter()
            .filter(|c| c.domain == domain || domain.ends_with(&format!(".{}", c.domain)))
            .collect();
        
        if domain_matches.is_empty() {
            return Ok(None);
        }
        
        // Prefer credentials that have the required action in metadata
        domain_matches.sort_by(|a, b| {
            let a_has_action = a.metadata.get("actions")
                .map(|s| s.contains(action))
                .unwrap_or(false);
            let b_has_action = b.metadata.get("actions")
                .map(|s| s.contains(action))
                .unwrap_or(false);
            b_has_action.cmp(&a_has_action)
        });
        
        Ok(domain_matches.into_iter().next())
    }
}
