//! 語法檢查器模組
//!
//! 負責對變更檔案（特別是 Rust 原始碼、TOML、JSON）進行基礎靜態語法與括號成對檢查

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// 程式語言分類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Toml,
    Json,
    Python,
    Markdown,
    Unknown,
}

impl Language {
    /// 從副檔名推測程式語言
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
            "rs" => Self::Rust,
            "toml" => Self::Toml,
            "json" => Self::Json,
            "py" => Self::Python,
            "md" => Self::Markdown,
            _ => Self::Unknown,
        }
    }
}

/// 語法錯誤項目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxError {
    pub line: usize,
    pub message: String,
}

/// 語法警告項目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxWarning {
    pub line: usize,
    pub message: String,
}

/// 單一檔案語法檢查報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxReport {
    pub file_path: PathBuf,
    pub language: Language,
    pub is_valid: bool,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<SyntaxWarning>,
}

/// 語法檢查器
#[derive(Debug, Clone, Default)]
pub struct SyntaxChecker;

impl SyntaxChecker {
    /// 建立新的語法檢查器
    pub fn new() -> Self {
        Self
    }

    /// 檢查指定檔案的語法完整性
    pub fn check_file(&self, file_path: &Path) -> SyntaxReport {
        let language = Language::from_path(file_path);
        let Ok(content) = fs::read_to_string(file_path) else {
            return SyntaxReport {
                file_path: file_path.to_path_buf(),
                language,
                is_valid: true,
                errors: Vec::new(),
                warnings: Vec::new(),
            };
        };

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        match language {
            Language::Rust => {
                Self::check_balanced_delimiters(&content, &mut errors);
            }
            Language::Json => {
                if let Err(err) = serde_json::from_str::<serde_json::Value>(&content) {
                    errors.push(SyntaxError {
                        line: err.line(),
                        message: format!("JSON 解析失敗: {err}"),
                    });
                }
            }
            _ => {}
        }

        if content.contains("\r\n") {
            warnings.push(SyntaxWarning {
                line: 1,
                message: "偵測到 CRLF 換行符，建議統一使用 LF".to_string(),
            });
        }

        SyntaxReport {
            file_path: file_path.to_path_buf(),
            language,
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// 簡易檢查 Rust 大括號是否平衡（忽略字串與行註解）
    fn check_balanced_delimiters(content: &str, errors: &mut Vec<SyntaxError>) {
        let mut brace_depth: i32 = 0;
        for (idx, raw_line) in content.lines().enumerate() {
            let line_num = idx + 1;
            let trimmed = raw_line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            let mut in_str = false;
            let mut escape = false;
            for ch in raw_line.chars() {
                if escape {
                    escape = false;
                    continue;
                }
                if ch == '\\' && in_str {
                    escape = true;
                    continue;
                }
                if ch == '"' {
                    in_str = !in_str;
                    continue;
                }
                if in_str {
                    continue;
                }
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth < 0 {
                            errors.push(SyntaxError {
                                line: line_num,
                                message: "發現多餘的右大括號 `}`".to_string(),
                            });
                            brace_depth = 0;
                        }
                    }
                    _ => {}
                }
            }
        }
        if brace_depth > 0 {
            errors.push(SyntaxError {
                line: content.lines().count().max(1),
                message: format!("尚有 {brace_depth} 個未閉合的左大括號 `{{`"),
            });
        }
    }
}
