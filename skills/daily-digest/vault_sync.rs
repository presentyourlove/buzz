//! Vault Synchronization Module
//! 
//! Handles synchronization of generated daily digests with Obsidian vault
//! including conflict resolution and backup mechanisms.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct VaultSync {
    pub vault_root: String,
    pub daily_notes_dir: String,
    pub backup_dir: String,
    pub sync_config: SyncConfig,
}

#[derive(Debug)]
pub struct SyncConfig {
    pub overwrite_existing: bool,
    pub create_backups: bool,
    pub conflict_resolution: ConflictResolution,
}

#[derive(Debug)]
pub enum ConflictResolution {
    KeepLocal,
    KeepRemote,
    ManualReview,
    AutoMerge,
}

impl VaultSync {
    pub fn new(vault_root: &str) -> Self {
        Self {
            vault_root: vault_root.to_string(),
            daily_notes_dir: "02-Conversation-Digest/Daily".to_string(),
            backup_dir: "backup".to_string(),
            sync_config: SyncConfig {
                overwrite_existing: true,
                create_backups: true,
                conflict_resolution: ConflictResolution::AutoMerge,
            },
        }
    }
    
    /// Sync daily digest to Obsidian vault
    pub fn sync_digest(&self, date: &str, markdown_content: &str) -> Result<(), Box<dyn std::error::Error>> {
        let target_dir = Path::new(&self.vault_root)
            .join(&self.daily_notes_dir)
            .join(date.replace("/", "-"));
        
        fs::create_dir_all(&target_dir)?;
        
        let filename = format!("{}.md", date.replace("/", "-").replace(" ", "_"));
        let filepath = target_dir.join(filename);
        
        // Create backup if configured
        if self.sync_config.create_backups {
            self.create_backup(&filepath)?;
        }
        
        // Check if file exists and handle conflicts
        if filepath.exists() && !self.sync_config.overwrite_existing {
            match self.sync_config.conflict_resolution {
                ConflictResolution::KeepLocal => return Ok(()),
                ConflictResolution::KeepRemote => {
                    fs::write(&filepath, markdown_content)?;
                }
                ConflictResolution::ManualReview => {
                    // In manual review mode, we'd typically prompt user
                    // For automated processing, we'll keep both versions
                    let backup_name = format!("{}_backup.md", date.replace("/", "-").replace(" ", "_"));
                    let backup_path = target_dir.join(backup_name);
                    fs::write(backup_path, markdown_content)?;
                }
                ConflictResolution::AutoMerge => {
                    self.merge_with_existing(&filepath, markdown_content)?;
                }
            }
        } else {
            fs::write(&filepath, markdown_content)?;
        }
        
        Ok(())
    }
    
    /// Create backup of existing file
    fn create_backup(&self, filepath: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if filepath.exists() {
            let backup_filename = format!(
                "{}_backup_{}.md",
                filepath.file_stem().unwrap_or_else(|| "file".to_string()).to_string_lossy(),
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            let backup_path = filepath.with_file_name(backup_filename);
            fs::copy(filepath, backup_path)?;
        }
        Ok(())
    }
    
    /// Merge new content with existing file
    fn merge_with_existing(&self, filepath: &Path, new_content: &str) -> Result<(), Box<dyn std::error::Error>> {
        let existing_content = fs::read_to_string(filepath)?;
        let merged = self.merge_digests(existing_content, new_content);
        fs::write(filepath, merged)?;
        Ok(())
    }
    
    /// Merge two digest contents
    fn merge_digests(&self, existing: String, new: String) -> String {
        // Simple merge strategy: add new decisions to existing ones
        let mut lines: Vec<String> = existing.lines().collect();
        let mut new_lines: Vec<String> = new.lines().collect();nosecondlines.push_str("\n\n## Synced Updates\n");
        // Add non-header lines from new content
        for line in new_lines {
            if !line.starts_with('#') && !line.starts_with("## Synced Updates") && !line.starts_with("Date:") 
               && !line.starts_with("Channel:") {
                lines.push(line);
            }
        }
        lines.join("\n")
    }
    
    /// Validate vault connection and accessibility
    pub fn validate_vault_connection(&self) -> Result<(), Box<dyn std::error::Error>> {
        let vault_path = Path::new(&self.vault_root);
        if !vault_path.exists() {
            return Err(format!("Vault path does not exist: {}", self.vault_root).into());
        }
        if !vault_path.is_dir() {
            return Err(format!("Vault path is not a directory: {}", self.vault_root).into());
        }
        
        let daily_notes_path = vault_path.join(&self.daily_notes_dir);
        if !daily_notes_path.exists() {
            fs::create_dir_all(&daily_notes_path)?;
        }
        
        Ok(())
    }
    
    /// Get sync status for monitoring
    pub fn get_sync_status(&self) -> SyncStatus {
        let vault_path = Path::new(&self.vault_root);
        let is_vault_accessible = vault_path.exists() && vault_path.is_dir();
        let is_daily_notes_accessible = vault_path.join(&self.daily_notes_dir).exists();
        
        SyncStatus {
            vault_root: self.vault_root.clone(),
            daily_notes_dir: self.daily_notes_dir.clone(),
            is_vault_accessible,
            is_daily_notes_accessible,
            last_sync: None, // Would typically store last sync timestamp
        }
    }
}

#[derive(Debug)]
pub struct SyncStatus {
    pub vault_root: String,
    pub daily_notes_dir: String,
    pub is_vault_accessible: bool,
    pub is_daily_notes_accessible: bool,
    pub last_sync: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    
    #[test]
    fn test_vault_sync_new_file() {
        let temp_dir = tempdir().unwrap();
        let vault_root = temp_dir.path().to_string_lossy().to_string();
        let sync = VaultSync::new(&vault_root);
        
        let result = sync.validate_vault_connection();
        assert!(result.is_ok());
        
        let status = sync.get_sync_status();
        assert!(status.is_vault_accessible);
        assert!(!status.is_daily_notes_accessible); // Directory doesn't exist yet
        
        let date = "2026/11/02";
        let content = "# Test Digest\n\nThis is a test.";
        let sync_result = sync.sync_digest(date, content);
        assert!(sync_result.is_ok());
        
        let expected_file = format!("{}/{}/{}.md", vault_root, "02-Conversation-Digest/Daily", "2026-11-02");
        assert!(Path::new(&expected_file).exists());
        
        let file_content = fs::read_to_string(expected_file).unwrap();
        assert_eq!(file_content, content);
    }
    
    #[test]
    fn test_backup_creation() {
        let temp_dir = tempdir().unwrap();
        let vault_root = temp_dir.path().to_string_lossy().to_string();
        let sync = VaultSync::new(&vault_root);
        
        // Create initial file
        let date = "2026/11/02";
        let content1 = "# Original Digest";
        sync.sync_digest(date, content1).unwrap();
        
        // Create backup-enabled sync
        let sync_with_backup = VaultSync {
            vault_root: vault_root.clone(),
            daily_notes_dir: "02-Conversation-Digest/Daily".to_string(),
            backup_dir: "backup".to_string(),
            sync_config: SyncConfig {
                overwrite_existing: true,
                create_backups: true,
                conflict_resolution: ConflictResolution::AutoMerge,
            },
        };
        
        let content2 = "# Updated Digest";
        sync_with_backup.sync_digest(date, content2).unwrap();
        
        // Check that backup was created
        let backup_pattern = format!("{}/backu
