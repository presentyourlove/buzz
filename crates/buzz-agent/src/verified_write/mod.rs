//! OpenAdapt 可信寫入與讀回驗證模組 (VerifiedWrite)
//! 
//! 特性：
//! 1. 寫入前預檢 (Pre-Check)：路徑安全、大小上限 (40KB)、常見語言語法格式快速校驗。
//! 2. 寫入後讀回驗證 (Read-Back Verification)：將檔案重讀回記憶體並進行 SHA-256 雜湊雙向比對，阻斷幻覺壞保存與截斷。
//! 3. 原子寫入 (Atomic Swap)：以暫存檔 (.tmp) 寫入後原子替換，避免中途寫入不全損壞源碼。

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod ecrr;
pub use ecrr::{ExploreCompileRunRepair, EcrrState, EcrrReceipt};

/// 寫入錯誤列舉
#[derive(Debug, Clone, PartialEq)]
pub enum WriteError {
    PathSecurityViolation(String),
    FileSizeExceeded { actual: usize, max: usize },
    SyntaxPrecheckFailed(String),
    IoError(String),
    ReadBackHashMismatch { expected: String, actual: String },
    ReadBackSizeMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::PathSecurityViolation(msg) => write!(f, "路徑安全校驗失敗 (Path Traversal / Prohibited): {msg}"),
            WriteError::FileSizeExceeded { actual, max } => write!(f, "檔案大小超出防爆上限 (最大 {max} 位元組，實際 {actual} 位元組)"),
            WriteError::SyntaxPrecheckFailed(msg) => write!(f, "語法結構預檢失敗: {msg}"),
            WriteError::IoError(msg) => write!(f, "磁碟 I/O 寫入失敗: {msg}"),
            WriteError::ReadBackHashMismatch { expected, actual } => write!(f, "讀回驗證失敗 (Read-Back Mismatch): 預期雜湊 {expected}，實際讀回 {actual}"),
            WriteError::ReadBackSizeMismatch { expected, actual } => write!(f, "讀回大小不符: 預期 {expected} 位元組，實際讀回 {actual} 位元組"),
        }
    }
}

impl std::error::Error for WriteError {}

/// 寫入成功驗證回執
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WriteReceipt {
    pub target_path: PathBuf,
    pub bytes_written: usize,
    pub sha256_hash: String,
    pub verified_at: u64,
}

/// 可信寫入核心 Trait
pub trait VerifiedWrite {
    /// 寫入前安全與語法預檢
    fn pre_check(&self, path: &Path, content: &str) -> Result<(), WriteError>;

    /// 執行原子寫入並進行磁碟讀回驗證
    fn write_and_verify(&self, path: &Path, content: &str) -> Result<WriteReceipt, WriteError>;
}

/// 預設可信寫入器實作
#[derive(Debug, Clone)]
pub struct DefaultVerifiedWriter {
    pub max_file_size: usize,
}

impl Default for DefaultVerifiedWriter {
    fn default() -> Self {
        Self {
            max_file_size: 40 * 1024, // 預設 40KB
        }
    }
}

impl DefaultVerifiedWriter {
    pub fn new(max_file_size: usize) -> Self {
        Self { max_file_size }
    }

    /// 計算字串之 SHA-256 雜湊值
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        result.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl VerifiedWrite for DefaultVerifiedWriter {
    fn pre_check(&self, path: &Path, content: &str) -> Result<(), WriteError> {
        // 1. 路徑安全檢查
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(WriteError::PathSecurityViolation(
                "禁止包含相對路徑跳脫符號 (..)".into(),
            ));
        }

        // 2. 檔案大小檢查
        let bytes = content.as_bytes();
        if bytes.len() > self.max_file_size {
            return Err(WriteError::FileSizeExceeded {
                actual: bytes.len(),
                max: self.max_file_size,
            });
        }

        // 3. 語法結構輕量預檢
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "json" => {
                    if let Err(e) = serde_json::from_str::<serde_json::Value>(content) {
                        return Err(WriteError::SyntaxPrecheckFailed(format!(
                            "JSON 語法解析失敗: {e}"
                        )));
                    }
                }
                "rs" => {
                    // 檢查括號平衡度 (快速語意守護)
                    let mut brace_count = 0isize;
                    for c in content.chars() {
                        match c {
                            '{' => brace_count += 1,
                            '}' => brace_count -= 1,
                            _ => {}
                        }
                    }
                    if brace_count != 0 {
                        return Err(WriteError::SyntaxPrecheckFailed(format!(
                            "Rust 大括號不匹配 (平衡差值: {brace_count})"
                        )));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn write_and_verify(&self, path: &Path, content: &str) -> Result<WriteReceipt, WriteError> {
        // 先執行預檢
        self.pre_check(path, content)?;

        let bytes = content.as_bytes();
        let expected_hash = Self::compute_sha256(bytes);
        let expected_size = bytes.len();

        // 建立父目錄
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| WriteError::IoError(e.to_string()))?;
        }

        // 原子寫入：寫入 .tmp 暫存檔
        let tmp_path = path.with_extension(format!(
            "tmp_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));

        fs::write(&tmp_path, bytes).map_err(|e| WriteError::IoError(e.to_string()))?;

        // 讀回驗證 (Read-Back Verification)
        let read_back_bytes = fs::read(&tmp_path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            WriteError::IoError(format!("讀回暫存檔案失敗: {e}"))
        })?;

        if read_back_bytes.len() != expected_size {
            let _ = fs::remove_file(&tmp_path);
            return Err(WriteError::ReadBackSizeMismatch {
                expected: expected_size,
                actual: read_back_bytes.len(),
            });
        }

        let actual_hash = Self::compute_sha256(&read_back_bytes);
        if actual_hash != expected_hash {
            let _ = fs::remove_file(&tmp_path);
            return Err(WriteError::ReadBackHashMismatch {
                expected: expected_hash,
                actual: actual_hash,
            });
        }

        // 讀回驗證 100% 一致，原子重命名為目標檔案
        fs::rename(&tmp_path, path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            WriteError::IoError(format!("原子覆蓋重命名失敗: {e}"))
        })?;

        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(WriteReceipt {
            target_path: path.to_path_buf(),
            bytes_written: expected_size,
            sha256_hash: expected_hash,
            verified_at: now_sec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_verified_write_success() {
        let writer = DefaultVerifiedWriter::default();
        let tmp_dir = env::temp_dir().join("buzz_verified_write_test");
        let file_path = tmp_dir.join("test_write.json");

        let content = r#"{"name": "buzz", "active": true}"#;
        let receipt = writer.write_and_verify(&file_path, content).unwrap();

        assert_eq!(receipt.bytes_written, content.len());
        assert_eq!(receipt.sha256_hash, DefaultVerifiedWriter::compute_sha256(content.as_bytes()));

        // 讀取檔案確認內容一致
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, content);

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_precheck_path_traversal() {
        let writer = DefaultVerifiedWriter::default();
        let path = Path::new("../dangerous/file.txt");
        let err = writer.pre_check(path, "content").unwrap_err();
        assert!(matches!(err, WriteError::PathSecurityViolation(_)));
    }

    #[test]
    fn test_precheck_file_size() {
        let writer = DefaultVerifiedWriter::new(10);
        let path = Path::new("file.txt");
        let err = writer.pre_check(path, "12345678901").unwrap_err();
        assert!(matches!(err, WriteError::FileSizeExceeded { .. }));
    }

    #[test]
    fn test_precheck_syntax_json() {
        let writer = DefaultVerifiedWriter::default();
        let path = Path::new("config.json");
        let err = writer.pre_check(path, "{invalid_json}").unwrap_err();
        assert!(matches!(err, WriteError::SyntaxPrecheckFailed(_)));
    }

    #[test]
    fn test_precheck_syntax_rust_braces() {
        let writer = DefaultVerifiedWriter::default();
        let path = Path::new("code.rs");
        let err = writer.pre_check(path, "fn test() { let a = 1;").unwrap_err();
        assert!(matches!(err, WriteError::SyntaxPrecheckFailed(_)));
    }
}
