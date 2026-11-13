//! Patch 解析器模組
//! 
//! 負責解析 Git Patch 格式（git diff --unified=3），構建結構化的 Patch 數據結構
//! 
//! 主要功能包括：
//! 1. 解析 Patch 文件內容，提取檔案信息和修改內容
//! 2. 構建 PatchFile 和 Hunk 數據結構
//! 3. 支援新檔案、已刪除檔案和已重命名檔案
//! 4. 提供便利函數來獲取修改文件和 Rust 文件列表
//! 
//! # 使用範例
//! 
//! ```rust
//! use qa_guard::patch_parser::{ParsedPatch, PatchParser};
//! 
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let patch_content = "diff --git a/src/main.rs b/src/main.rs\nindex 1234567..89abcde 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,5 +1,5 @@\n-fn main()\n+async fn main() {\n     println!(\"Hello\");\n }\n";
//!     
//!     let parsed_patch = PatchParser::parse(patch_content)?;
//!     println!("解析成功，共 {} 個檔案", parsed_patch.files.len());
//!     for file in &parsed_patch.files {
//!         println!("  - {}", file.old_path.display());
//!     }
//!     
//!     let rust_files = parsed_patch.get_rust_files();
//!     println!("找到 {} 個 Rust 檔案", rust_files.len());
//!     
//!     Ok(())
//! }
//! ```

use std::path::{Path, PathBuf};
use std::collections::HashSet;

/// 完整的 Patch 文件結構
/// 
/// 包含所有修改的文件信息、統計數據和原始 Patch 內容
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedPatch {
    /// 修正的檔案列表
    pub files: Vec<PatchFile>,
    /// 是否為二進制檔案的修正
    pub is_binary: bool,
    /// 新增的總行數
    pub total_additions: usize,
    /// 刪除的總行數
    pub total_deletions: usize,
    /// 原始 Patch 內容（用於調試和重新處理）
    pub original_content: String,
}

impl ParsedPatch {
    /// 創建新的 ParsedPatch
    /// 
    /// # 參數
    /// 
    /// * `files` - 修正的檔案列表
    /// * `is_binary` - 是否為二進制修正
    /// * `total_additions` - 新增的總行數
    /// * `total_deletions` - 刪除的總行數
    /// * `original_content` - 原始 Patch 內容
    /// 
    /// # 返回值
    /// 
    /// 返回新的 ParsedPatch 實例
    pub fn new(
        files: Vec<PatchFile>,
        is_binary: bool,
        total_additions: usize,
        total_deletions: usize,
        original_content: String,
    ) -> Self {
        Self {
            files,
            is_binary,
            total_additions,
            total_deletions,
            original_content,
        }
    }
    
    /// 根據 Patch 內容創建 ParsedPatch
    /// 
    /// # 參數
    /// 
    /// * `patch_content` - Git Patch 格式的內容字符串
    /// 
    /// # 返回值
    /// 
    /// 返回包含解析結果的 ParsedPatch，並可能返回 ParseError
    /// 
    /// # 錯誤
    /// 
    /// 如果 Patch 格式無效或解析失敗，將返回相應的錯誤
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::patch_parser::ParsedPatch;
    /// 
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let patch_content = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-fn main()\n+async fn main() {\n     println!(\"Hello\");\n }\n";
    ///     
    ///     let parsed_patch = ParsedPatch::parse(patch_content)?;
    ///     println!("成功解析了 {} 個檔案", parsed_patch.files.len());
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub fn parse(patch_content: &str) -> Result<Self, ParseError> {
        PatchParser::parse(patch_content)
    }
    
    /// 獲取所有被修改的檔案
    /// 
    /// # 返回值
    /// 
    /// 返回一個包含所有被修改的 PatchFile 的切片
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::patch_parser::ParsedPatch;
    /// 
    /// fn main() {
    ///     let parsed_patch = ParsedPatch::parse("diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-fn main()\n+async fn main() {\n     println!(\"Hello\");\n }\n").unwrap();
    ///     
    ///     for file in parsed_patch.get_modified_files() {
    ///         println!("檔案: {:?}", file.old_path);
    ///     }
    /// }
    /// ```
    pub fn get_modified_files(&self) -> Vec<&PatchFile> {
        self.files.iter().filter(|file| file.is_new || file.is_deleted || file.is_renamed || !file.hunks.is_empty()).collect()
    }
    
    /// 獲取所有 Rust 檔案
    /// 
    /// # 返回值
    /// 
    /// 返回一個包含所有 Rust 檔案的切片
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::patch_parser::ParsedPatch;
    /// 
    /// fn main() {
    ///     let parsed_patch = ParsedPatch::parse("diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-fn main()\n+async fn main() {\n     println!(\"Hello\");\n }\n").unwrap();
    ///     
    ///     let rust_files = parsed_patch.get_rust_files();
    ///     println!("找到 {} 個 Rust 檔案", rust_files.len());
    /// }
    /// ```
    pub fn get_rust_files(&self) -> Vec<&PatchFile> {
        self.files.iter().filter(|file| {
            let path_str = file.old_path.to_string_lossy();
            path_str.ends_with(".rs") || path_str.ends_with(".toml") || path_str.ends_with("Cargo.toml")
        }).collect()
    }
}

/// Git Patch 解析器
/// 
/// 負責解析 Git Patch 格式的字串，構建結構化的 ParsedPatch 數據結構
pub struct PatchParser;

impl PatchParser {
    /// 解析 Git Patch 格式的字串
    /// 
    /// # 參數
    /// 
    /// * `patch_content` - Git Patch 格式的內容字符串
    /// 
    /// # 返回值
    /// 
    /// 返回解析結果，如果解析失敗，則返回 ParseError
    /// 
    /// # Git Patch 格式解析邏輯
    /// 
    /// 1. **檢查基本結構** - 驗證是否為有效的 Git Patch 格式
    /// 2. **解析文件頭部** - 提取檔案名稱、路徑等信息
    /// 3. **解析 hunks** - 構建 Hunk 結構，包含上下文行、新增行和刪除行
    /// 4. **構建 PatchFile** - 組合檔案信息和 hunks
    /// 5. **構建 ParsedPatch** - 組合所有檔案，計算統計信息
    /// 
    /// # 錯誤類型
    /// 
    /// - `ParseError::InvalidHeader` - 無效的 Patch 頭部
    /// - `ParseError::InvalidHunk` - 無效的 Hunk 格式
    /// - `ParseError::InvalidContent` - 無效的 Patch 內容
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// use qa_guard::patch_parser::PatchParser;
    /// 
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let patch_content = "diff --git a/src/main.rs b/src/main.rs\nindex 1234567..89abcde 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,5 +1,5 @@\n-fn main()\n+async fn main() {\n     println!(\"Hello\");\n }\n";
    ///     
    ///     let parsed_patch = PatchParser::parse(patch_content)?;
    ///     println!("解析成功，共 {} 個檔案", parsed_patch.files.len());
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub fn parse(patch_content: &str) -> Result<ParsedPatch, ParseError> {
        let mut files = Vec::new();
        let mut is_binary = false;
        let mut total_additions = 0;
        let mut total_deletions = 0;
        
        // 按行處理 Patch 內容
        let mut lines = patch_content.lines();
        let mut current_file: Option<(PatchFile, Vec<String>)> = None;
        let mut current_hunk_lines: Vec<String> = Vec::new();
        
        while let Some(line) = lines.next() {
            // 處理文件頭部
            if line.starts_with("diff --git ") {
                // 保存上一個檔案
                if let Some((file, hunk_lines)) = current_file.take() {
                    if !hunk_lines.is_empty() {
                        // 創建 Hunk
                        if let Some(hunk) = PatchParser::parse_hunk(&hunk_lines) {
                            let mut patch_file = file;
                            patch_file.hunks.push(hunk);
                            files.push(patch_file);
                        }
                    }
                }
                
                // 解析新檔案
                let (old_path, new_path, is_new, is_deleted, is_renamed) = PatchParser::parse_file_header(line)?;
                current_file = Some((
                    PatchFile::new(old_path, new_path, is_new, is_deleted, is_renamed),
                    Vec::new(),
                ));
                current_hunk_lines.clear();
            } else if line.starts_with("+++ ") {
                // 跳過新的檔案頭部
                continue;
            } else if line.starts_with("@@ ") {
                current_hunk_lines.push(line.to_string());
            } else if line.starts_with('+') && !line.starts_with("+++") {
                total_additions += 1;
                current_hunk_lines.push(line.to_string());
            } else if line.starts_with('-') && !line.starts_with("---") {
                total_deletions += 1;
                current_hunk_lines.push(line.to_string());
            } else if line.starts_with("Binary files") {
                is_binary = true;
            } else {
                current_hunk_lines.push(line.to_string());
            }
        }

        if let Some((mut file, _)) = current_file.take() {
            if let Some(hunk) = PatchParser::parse_hunk(&current_hunk_lines) {
                file.hunks.push(hunk);
            }
            files.push(file);
        }

        Ok(ParsedPatch::new(
            files,
            is_binary,
            total_additions,
            total_deletions,
            patch_content.to_string(),
        ))
    }
}
