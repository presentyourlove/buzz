// encrypted_vault.rs - Encrypted credential vault for Browser Automation Phase 2
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::Aes256Gcm;
use anyhow::{anyhow, Result};
use blake3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Vault entry containing encrypted credential data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub id: Uuid,
    pub key: String,              // Credential identifier (e.g., "github_token", "aws_access_key")
    pub value: Vec<u8>,           // Encrypted value
    pub nonce: Vec<u8>,           // Nonce used for encryption
    pub metadata: HashMap<String, String>, // Additional metadata (domain, scope, etc.)
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Master key derived from user passphrase
#[derive(Debug, Clone)]
pub struct MasterKey {
    key: [u8; 32],
}

impl MasterKey {
    /// Derive master key from passphrase using Blake3
    pub fn from_passphrase(passphrase: &str) -> Self {
        let hash = blake3::hash(passphrase.as_bytes());
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash.as_bytes()[..32]);
        Self { key }
    }

    /// Get the raw key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

/// Encrypted Vault for storing credentials securely
#[derive(Clone)]
pub struct EncryptedVault {
    master_key: Arc<MasterKey>,
    entries: Arc<RwLock<HashMap<Uuid, VaultEntry>>>,
    vault_path: std::path::PathBuf,
}

impl EncryptedVault {
    /// Create a new encrypted vault
    pub fn new(master_key: MasterKey, vault_path: std::path::PathBuf) -> Self {
        Self {
            master_key: Arc::new(master_key),
            entries: Arc::new(RwLock::new(HashMap::new())),
            vault_path,
        }
    }

    /// Load vault from disk
    pub async fn load(master_key: MasterKey, vault_path: std::path::PathBuf) -> Result<Self> {
        let vault = Self::new(master_key, vault_path.clone());
        
        if vault_path.exists() {
            let data = tokio::fs::read(&vault_path).await?;
            let entries: HashMap<Uuid, VaultEntry> = serde_json::from_slice(&data)?;
            *vault.entries.write().await = entries;
        }
        
        Ok(vault)
    }

    /// Save vault to disk
    pub async fn save(&self) -> Result<()> {
        let entries = self.entries.read().await.clone();
        let data = serde_json::to_vec(&entries)?;
        tokio::fs::write(&self.vault_path, data).await?;
        Ok(())
    }

    /// Encrypt a value using AES-256-GCM
    fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let cipher = Aes256Gcm::new(self.master_key.as_bytes().into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;
        Ok((ciphertext, nonce.to_vec()))
    }

    /// Decrypt a value using AES-256-GCM
    fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new(self.master_key.as_bytes().into());
        let plaintext = cipher.decrypt(nonce.into(), ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;
        Ok(plaintext)
    }

    /// Store a credential in the vault
    pub async fn store(&self, key: &str, value: &[u8], metadata: HashMap<String, String>) -> Result<Uuid> {
        let (encrypted_value, nonce) = self.encrypt(value)?;
        
        let now = chrono::Utc::now();
        let entry = VaultEntry {
            id: Uuid::new_v4(),
            key: key.to_string(),
            value: encrypted_value,
            nonce,
            metadata,
            created_at: now,
            updated_at: now,
        };
        
        let id = entry.id;
        self.entries.write().await.insert(id, entry);
        self.save().await?;
        
        Ok(id)
    }

    /// Retrieve a credential from the vault
    pub async fn retrieve(&self, key: &str) -> Result<Vec<u8>> {
        let entries = self.entries.read().await;
        let entry = entries.values()
            .find(|e| e.key == key)
            .ok_or_else(|| anyhow!("Credential not found: {}", key))?;
        
        self.decrypt(&entry.value, &entry.nonce)
    }

    /// Retrieve a credential by ID
    pub async fn retrieve_by_id(&self, id: Uuid) -> Result<Vec<u8>> {
        let entries = self.entries.read().await;
        let entry = entries.get(&id)
            .ok_or_else(|| anyhow!("Credential not found: {}", id))?;
        
        self.decrypt(&entry.value, &entry.nonce)
    }

    /// List all credential keys
    pub async fn list_keys(&self) -> Vec<String> {
        self.entries.read().await.values().map(|e| e.key.clone()).collect()
    }

    /// List all entries with metadata (without decrypting values)
    pub async fn list_entries(&self) -> Vec<VaultEntry> {
        self.entries.read().await.values().cloned().collect()
    }

    /// Delete a credential
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut entries = self.entries.write().await;
        let id = entries.values()
            .find(|e| e.key == key)
            .map(|e| e.id)
            .ok_or_else(|| anyhow!("Credential not found: {}", key))?;
        
        entries.remove(&id);
        self.save().await?;
        Ok(())
    }

    /// Update a credential
    pub async fn update(&self, key: &str, value: &[u8], metadata: HashMap<String, String>) -> Result<()> {
        let (encrypted_value, nonce) = self.encrypt(value)?;
        
        let mut entries = self.entries.write().await;
        let entry = entries.values_mut()
            .find(|e| e.key == key)
            .ok_or_else(|| anyhow!("Credential not found: {}", key))?;
        
        entry.value = encrypted_value;
        entry.nonce = nonce;
        entry.metadata = metadata;
        entry.updated_at = chrono::Utc::now();
        
        self.save().await?;
        Ok(())
    }

    /// Get metadata for a credential
    pub async fn get_metadata(&self, key: &str) -> Result<HashMap<String, String>> {
        let entries = self.entries.read().await;
        let entry = entries.values()
            .find(|e| e.key == key)
            .ok_or_else(|| anyhow!("Credential not found: {}", key))?;
        
        Ok(entry.metadata.clone())
    }
}

/// Vault configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub vault_path: std::path::PathBuf,
    pub auto_save: bool,
}

impl Default for VaultConfig {
    fn default() -> Self {
        let vault_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("buzz")
            .join("vault.json");
        
        Self {
            vault_path,
            auto_save: true,
        }
    }
}
