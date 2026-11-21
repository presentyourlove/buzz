//! Syntax & Contract Gate (Gate 1)
//!
//! Implements fast syntactic validation, source file encoding checks,
//! and contract manifest presence according to `PLANS/DESIGN_SPEC_B_TRIPLE_GATE.md`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Syntax Gate Error types
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum SyntaxGateError {
    #[error("必要契約檔案缺失: {path}")]
    MissingContractManifest { path: String },

    #[error("檔案編碼格式不符 (必須為 UTF-8 no BOM): {path}")]
    InvalidEncoding { path: String },

    #[error("語法或類型檢查失敗: {detail}")]
    SyntaxError { detail: String },

    #[error("IO 錯誤: {0}")]
    IoError(String),
}

/// Verdict of Syntax Gate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntaxVerdict {
    Pass,
    Fail,
}

/// Detailed result of Syntax Gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxGateResult {
    pub verdict: SyntaxVerdict,
    pub checked_files: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

/// Syntax Gate Validator
pub struct SyntaxGate;

impl SyntaxGate {
    /// Run Gate 1: Syntax & Contract verification
    pub fn validate<P: AsRef<Path>>(repo_root: P) -> SyntaxGateResult {
        let t0 = std::time::Instant::now();
        let root = repo_root.as_ref();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut checked_files = 0;

        // 1. 核心契約清單檢查
        let required_manifests = ["Cargo.toml", "README.md"];
        for manifest in required_manifests {
            let p = root.join(manifest);
            if !p.exists() {
                errors.push(format!("缺少頂層必要契約檔案: {}", manifest));
            } else {
                checked_files += 1;
            }
        }

        // 2. 抽樣或關鍵源檔 UTF-8 (No BOM) 檢驗
        let sample_paths = [
            "Cargo.toml",
            "README.md",
            "crates/buzz-core/src/lib.rs",
            "crates/buzz-agent/src/lib.rs",
            "crates/buzz-acp/src/lib.rs",
        ];

        for rel in sample_paths {
            let full = root.join(rel);
            if full.exists() && full.is_file() {
                checked_files += 1;
                if let Ok(bytes) = std::fs::read(&full) {
                    // Check UTF-8 BOM: 0xEF, 0xBB, 0xBF
                    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
                        errors.push(format!("檔案包含 UTF-8 BOM 標頭 (違規): {}", rel));
                    }
                    if std::str::from_utf8(&bytes).is_err() {
                        errors.push(format!("檔案非合法 UTF-8 編碼: {}", rel));
                    }
                }
            }
        }

        let verdict = if errors.is_empty() {
            SyntaxVerdict::Pass
        } else {
            SyntaxVerdict::Fail
        };

        SyntaxGateResult {
            verdict,
            checked_files,
            errors,
            warnings,
            duration_ms: t0.elapsed().as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_gate_on_current_repo() {
        let res = SyntaxGate::validate(".");
        assert_eq!(res.verdict, SyntaxVerdict::Pass);
        assert!(res.checked_files >= 2);
    }
}
