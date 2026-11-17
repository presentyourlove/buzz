// vault_api.rs - Credential vault API for Browser Automation Phase 2
use crate::encrypted_vault::{EncryptedVault, MasterKey, VaultConfig, VaultEntry};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Request to store a credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreCredentialRequest {
    pub key: String,
    pub value: String,
    pub metadata: Option<serde_json::Value>,
}

/// Request to retrieve a credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveCredentialRequest {
    pub key: String,
}

/// Response for credential operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialResponse {
    pub id: Uuid,
    pub key: String,
    pub value: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// List credentials response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCredentialsResponse {
    pub credentials: Vec<CredentialResponse>,
}

/// Vault API for managing credentials
pub struct VaultApi {
    vault: Arc<EncryptedVault>,
}

impl VaultApi {
    /// Create a new vault API
    pub fn new(vault: Arc<EncryptedVault>) -> Self {
        Self { vault }
    }

    /// Initialize vault with passphrase
    pub async fn initialize(passphrase: &str, config: VaultConfig) -> Result<Arc<Self>> {
        let master_key = MasterKey::from_passphrase(passphrase);
        let vault = EncryptedVault::load(master_key, config.vault_path.clone()).await?;
        Ok(Arc::new(Self::new(Arc::new(vault))))
    }

    /// Store a credential
    pub async fn store_credential(&self, request: StoreCredentialRequest) -> Result<CredentialResponse> {
        let metadata = request.metadata
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        
        let id = self.vault.store(&request.key, request.value.as_bytes(), metadata).await?;
        
        let entry = self.vault.list_entries().await
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow!("Failed to retrieve stored credential"))?;
        
        Ok(CredentialResponse {
            id: entry.id,
            key: entry.key,
            value: Some(request.value),
            metadata: Some(serde_json::to_value(entry.metadata)?),
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        })
    }

    /// Retrieve a credential
    pub async fn retrieve_credential(&self, request: RetrieveCredentialRequest) -> Result<CredentialResponse> {
        let value = self.vault.retrieve(&request.key).await?;
        let entries = self.vault.list_entries().await;
        let entry = entries.iter()
            .find(|e| e.key == request.key)
            .ok_or_else(|| anyhow!("Credential not found"))?;
        
        Ok(CredentialResponse {
            id: entry.id,
            key: entry.key.clone(),
            value: Some(String::from_utf8(value)?),
            metadata: Some(serde_json::to_value(entry.metadata.clone())?),
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        })
    }

    /// List all credentials (without values)
    pub async fn list_credentials(&self) -> Result<ListCredentialsResponse> {
        let entries = self.vault.list_entries().await;
        let credentials = entries.into_iter()
            .map(|e| CredentialResponse {
                id: e.id,
                key: e.key,
                value: None, // Don't expose values in list
                metadata: Some(serde_json::to_value(e.metadata).unwrap_or_default()),
                created_at: e.created_at,
                updated_at: e.updated_at,
            })
            .collect();
        
        Ok(ListCredentialsResponse { credentials })
    }

    /// Delete a credential
    pub async fn delete_credential(&self, key: &str) -> Result<()> {
        self.vault.delete(key).await
    }

    /// Update a credential
    pub async fn update_credential(&self, request: StoreCredentialRequest) -> Result<CredentialResponse> {
        let metadata = request.metadata
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        
        self.vault.update(&request.key, request.value.as_bytes(), metadata).await?;
        
        let entries = self.vault.list_entries().await;
        let entry = entries.iter()
            .find(|e| e.key == request.key)
            .ok_or_else(|| anyhow!("Credential not found"))?;
        
        Ok(CredentialResponse {
            id: entry.id,
            key: entry.key.clone(),
            value: Some(request.value),
            metadata: Some(serde_json::to_value(entry.metadata.clone())?),
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        })
    }

    /// Get vault instance for direct access
    pub fn vault(&self) -> Arc<EncryptedVault> {
        self.vault.clone()
    }
}
