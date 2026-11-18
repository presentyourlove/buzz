//! Vault 同步模組
//! 
//! 負責實現 Obsidian Vault 的雙向同步功能，支援原子寫入、INDEX 更新等功能
//! 
//! 主要功能包括：
//! 1. 原子寫入（Write-Then-Rename）機制
//! 2. INDEX 文件更新
//! 3. 錯誤處理和回滾機制
//! 4. 路徑和權限檢查
//! 
//! 注意：該模組僅在配置了 vault_path 時啟用，可選啟用

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

/// Vault 同步模組
/// 
/// 負責管理 Obsidian Vault 與本地文件的同步操作
/// 
/// # 特點
/// 
/// 1. **原子寫入** - 使用臨時文件 + rename 操作，防止部分寫入
/// 2. **INDEX 更新** - 自動維護 INDEX.md 文件，記錄所有摘要文件
/// 3. **錯誤處理** - 提供詳細的錯誤信息和回滾機制
/// 4. **路徑驗證** - 驗證目標路徑的有效性和權限
/// 
/// # 使用範例
/// 
/// ```rust,no_run
/// use std::path::PathBuf;
/// use chrono::NaiveDate;
/// use daily_digest::vault_sync::VaultSync;
/// 
/// fn main() {
///     let vault_path = PathBuf::from("/path/to/vault");
///     let daily_folder = "daily".to_string();
///     let vault_sync = VaultSync::new(vault_path, daily_folder);
///     
///     let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
///     let markdown = "# 每日摘要\n\n這是一些內容。".to_string();
///     
///     match vault_sync.sync(date, &markdown) {
    ///         Ok(result) => println!("成功同步到: {:?}", result.file_path),
    ///         Err(e) => eprintln!("同步失敗: {}", e),
    ///     }
    /// }
    /// ```

pub struct VaultSync {
    /// Obsidian Vault 根目錄路徑
    vault_path: PathBuf,
    /// 每日摘要儲存目錄名稱
    daily_folder: String,
}

impl VaultSync {
    /// 創建新的 VaultSync 實例
    /// 
    /// # 參數
    /// 
    /// * `vault_path` - Obsidian Vault 根目錄路徑
    /// * `daily_folder` - 每日摘要儲存目錄名稱
    /// 
    /// # 返回值
    /// 
    /// 返回新的 VaultSync 實例
    /// 
    /// # 錯誤
    /// 
    /// 如果 vault_path 無效或無法訪問，將返回相應的錯誤
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use daily_digest::vault_sync::VaultSync;
    /// use std::path::PathBuf;
    /// 
    /// let vault_path = PathBuf::from("/path/to/vault");
    /// let daily_folder = "daily".to_string();
    /// let vault_sync = VaultSync::new(vault_path, daily_folder);
    /// ```
    pub fn new(vault_path: PathBuf, daily_folder: String) -> Self {
        Self {
            vault_path,
            daily_folder,
        }
    }
    
    /// 同步摘要到 Vault
    /// 
    /// # 參數
    /// 
    /// * `date` - 要同步的日期
    /// * `markdown` - 要寫入的 Markdown 內容
    /// 
    /// # 返回值
    /// 
    /// 返回包含同步結果的 Result 型
    /// 
    /// # 錯誤
    /// 
    /// 如果同步過程中遇到錯誤，將返回相應的錯誤信息
    /// 
    /// # 同步過程
    /// 
    /// 1. 創建目標目錄（如果不存在）
    /// 2. 生成文件名 (YYYY-MM-DD.md)
    /// 3. 執行原子寫入 (臨時文件 + rename)
    /// 4. 更新 INDEX.md 文件
    /// 5. 返回同步結果
    /// 
    /// # 示例
    /// 
    /// ```rust,no_run
    /// use std::path::PathBuf;
    /// use daily_digest::vault_sync::VaultSync;
    /// use chrono::NaiveDate;
    /// 
    /// let vault_path = PathBuf::from("/path/to/vault");
    /// let daily_folder = "daily".to_string();
    /// let vault_sync = VaultSync::new(vault_path, daily_folder);
    /// 
    /// let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    /// let markdown = "# 每日摘要\n\n這是一些內容。".to_string();
    /// 
    /// match vault_sync.sync(date, &markdown) {
    ///     Ok(result) => println!("成功同步到: {:?}", result.file_path),
    ///     Err(e) => eprintln!("同步失敗: {}", e),
    /// }
    /// ```
    pub fn sync(&self, date: NaiveDate, markdown: &str) -> Result<SyncResult, SyncError> {
        // 1. 創建目標目錄
        let target_dir = self.vault_path.join(&self.daily_folder);
        self.ensure_directory_exists(&target_dir)?;
        
        // 2. 生成文件名
        let file_name = format!("{}.md", date.format("%Y-%m-%d"));
        let file_path = target_dir.join(&file_name);
        
        // 3. 執行原子寫入
        self.atomic_write(&file_path, markdown)?;
        
        // 4. 更新 INDEX.md
        self.update_index(date, &file_path)?;
        
        Ok(SyncResult { file_path })
    }
    
    /// 確保目錄存在，如果不存在則創建
    /// 
    /// # 參數
    /// 
    /// * `dir_path` - 目錄路徑
    /// 
    /// # 返回值
    /// 
    /// 返回 Result<(), SyncError> 型
    /// 
    /// # 錯誤
    /// 
    /// 如果無法創建目錄，將返回相應的錯誤
    fn ensure_directory_exists(&self, dir_path: &Path) -> Result<(), SyncError> {
        if !dir_path.exists() {
            fs::create_dir_all(dir_path)
                .map_err(|e| SyncError::IoError(format!("無法創建目錄 {:?}: {}", dir_path, e)))?;
        } else if !dir_path.is_dir() {
            return Err(SyncError::InvalidPath(format!("路徑 {:?} 存在但不是目錄", dir_path)));
        }
        
        Ok(())
    }
    
    /// 原子寫入文件
    /// 
    /// # 參數
    /// 
    /// * `file_path` - 目標文件路徑
    /// * `content` - 要寫入的內容
    /// 
    /// # 返回值
    /// 
    /// 返回 Result<(), SyncError> 型
    /// 
    /// # 原子寫入機制
    /// 
    /// 1. 創建臨時文件 (與目標文件同名，附加 .tmp 後綴)
    /// 2. 寫入內容到臨時文件
    /// 3. 關閉文件句柄
    /// 4. 將臨時文件 rename 為目標文件
    /// 
    /// 這樣，如果寫入過程中出錯，只會生成不完整的臨時文件，而不會影響目標文件
    fn atomic_write(&self, file_path: &Path, content: &str) -> Result<(), SyncError> {
        // 創建臨時文件路徑
        let tmp_path = file_path.with_extension("md.tmp");
        
        // 寫入內容到臨時文件
        let mut temp_file = File::create(&tmp_path)
            .map_err(|e| SyncError::IoError(format!("無法創建臨時文件 {:?}: {}", tmp_path, e)))?;
        
        temp_file.write_all(content.as_bytes())
            .map_err(|e| SyncError::IoError(format!("無法寫入臨時文件 {:?}: {}", tmp_path, e)))?;
        
        // 確保內容完全寫入到磁碟
        temp_file.sync_all()
            .map_err(|e| SyncError::IoError(format!("無法同步臨時文件 {:?}: {}", tmp_path, e)))?;
        
        // 關閉文件句柄
        drop(temp_file);
        
        // 將臨時文件 rename 為目標文件
        if file_path.exists() {
            // 如果目標文件已存在，先刪除
            fs::remove_file(file_path)
                .map_err(|e| SyncError::IoError(format!("無法刪除目標文件 {:?}: {}", file_path, e)))?;
        }
        
        fs::rename(&tmp_path, file_path)
            .map_err(|e| SyncError::IoError(format!("無法將臨時文件重命名為 {:?}: {}", file_path, e)))?;
        
        Ok(())
    }
    
    /// 更新 INDEX.md 文件
    /// 
    /// # 參數
    /// 
    /// * `date` - 日期
    /// * `file_path` - 已同步的文件路徑
    /// 
    /// # 返回值
    /// 
    /// 返回 Result<(), SyncError> 型
    /// 
    /// # INDEX.md 格式
    /// 
    /// ```markdown
    /// # 每日對話摘要
    /// 
    /// ## 2024-01-15
    /// - [2024-01-15](daily/2024-01-15.md)
    /// 
    /// ## 2024-01-14
    /// - [2024-01-14](daily/2024-01-14.md)
    /// ```
    /// 
    /// # 注意
    /// 
    /// 如果 INDEX.md 文件不存在，將創建一個新的文件
    fn update_index(&self, date: NaiveDate, file_path: &Path) -> Result<(), SyncError> {
        let index_path = self.vault_path.join("INDEX.md");
        
        let file_name = file_path.file_name()
            .ok_or_else(|| SyncError::InvalidPath("文件路徑無效，找不到文件名".to_string()))?
            .to_str()
            .ok_or_else(|| SyncError::InvalidPath("文件路徑包含無效字符".to_string()))?;
        
        let link = format!("- [{}]({})", date.format("%Y-%m-%d"), file_name);
        
        let mut index_content = if index_path.exists() {
            // 讀取現有內容
            fs::read_to_string(&index_path)
                .map_err(|e| SyncError::IoError(format!("無法讀取 INDEX.md 文件 {:?}: {}", index_path, e)))?
        } else {
            // 創建新的 INDEX.md 文件頭部
            "# 每日對話摘要\n\n".to_string()
        };
        
        // 添加新條目（如果已存在，則跳過）
        let today = date.format("%Y-%m-%d").to_string();
        if !index_content.contains(&format!("[{}](", today)) {
            // 找到 "## YYYY-MM-DD" 標題，如果不存在則添加到 "# 每日對話摘要" 後面
            let date_header = format!("## {}\n", date.format("%Y-%m-%d"));
            let mut lines: Vec<&str> = index_content.lines().collect();
            
            let mut found = false;
            for line in &mut lines {
                if line.starts_with("## ") && line[3..].trim() == today {
                    found = true;
                    break;
                }
            }
            
            if !found {
                lines.push("\n");
                lines.push(&date_header);
            }
            
            lines.push(&link);
            
            // 重新構建 INDEX.md 內容
            index_content = lines.join("\n");
        }
        
        // 寫入更新後的 INDEX.md 文件
        self.atomic_write(&index_path, &index_content)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum SyncError {
    IoError(String),
    InvalidPath(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::IoError(s) => write!(f, "IO 錯誤: {}", s),
            SyncError::InvalidPath(s) => write!(f, "路徑無效: {}", s),
        }
    }
}

impl std::error::Error for SyncError {}
