//! Patch 解析器單元測試
//! 
//! 測試 Patch 解析功能的單元測試，包括：
//! 1. 簡單 Patch 的解析
//! 2. 新檔案 Patch 的解析
//! 3. 已刪除檔案 Patch 的解析
//! 4. 修改檔案的檢索
//! 5. Rust 檔案的檢索
//! 6. Patch 文件的顯示名稱
//! 7. 編碼錯誤的顯示
//! 
//! 這些測試確保 Patch 解析功能正確實現，並能正確處理各種 Patch 格式

use super::*;

/// 測試簡單 Patch 的解析
/// 
/// 測試包含一個簡單修改的 Patch 的解析，包括檔案名稱、行號等信息
#[test]
fn test_parse_simple_patch() {
    let patch_content = "diff --git a/src/main.rs b/src/main.rs\nindex 1234567..89abcde 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,5 +1,5 @@\n-fn main() {\n+async fn main() {\n     println!(\"Hello\");\n }\n";
    
    let parsed_patch = ParsedPatch::parse(patch_content).unwrap();
    
    assert_eq!(parsed_patch.files.len(), 1);
    assert!(parsed_patch.files[0].is_rust_file());
    assert_eq!(parsed_patch.files[0].hunks.len(), 1);
    assert_eq!(parsed_patch.files[0].hunks[0].old_start, 1);
    assert_eq!(parsed_patch.files[0].hunks[0].old_lines, 5);
    assert_eq!(parsed_patch.files[0].hunks[0].new_start, 1);
    assert_eq!(parsed_patch.files[0].hunks[0].new_lines, 5);
    assert_eq!(parsed_patch.total_additions, 1);
    assert_eq!(parsed_patch.total_deletions, 1);
    assert_eq!(parsed_patch.is_binary, false);
}

/// 測試新檔案 Patch 的解析
/// 
/// 測試包含一個新檔案的 Patch 的解析，包括是否為新檔案、文件大小等信息
#[test]
fn test_parse_new_file() {
    let patch_content = "diff --git a/src/new_file.rs b/src/new_file.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/new_file.rs\n@@ -0,0 +1,3 @@\n+fn new_function() {\n+    println!(\"New\");\n+}\n";
    
    let parsed_patch = ParsedPatch::parse(patch_content).unwrap();
    
    assert_eq!(parsed_patch.files.len(), 1);
    assert!(parsed_patch.files[0].is_new);
    assert!(parsed_patch.files[0].is_rust_file());
    assert_eq!(parsed_patch.files[0].hunks.len(), 1);
    assert_eq!(parsed_patch.files[0].hunks[0].total_additions(), 3);
    assert_eq!(parsed_patch.files[0].hunks[0].total_deletions(), 0);
    assert_eq!(parsed_patch.total_additions, 3);
    assert_eq!(parsed_patch.total_deletions, 0);
}

/// 測試已刪除檔案 Patch 的解析
/// 
/// 測試包含一個已刪除檔案的 Patch 的解析，包括是否為已刪除檔案、行號等信息
#[test]
fn test_parse_deleted_file() {
    let patch_content = "diff --git a/src/old_file.rs b/src/old_file.rs\ndeleted file mode 100644\n--- a/src/old_file.rs\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-fn old_function() {\n-    println!(\"Old\");\n-}\n";
    
    let parsed_patch = ParsedPatch::parse(patch_content).unwrap();
    
    assert_eq!(parsed_patch.files.len(), 1);
    assert!(parsed_patch.files[0].is_deleted);
    assert!(parsed_patch.files[0].is_rust_file());
    assert_eq!(parsed_patch.files[0].hunks.len(), 1);
    assert_eq!(parsed_patch.files[0].hunks[0].total_additions(), 0);
    assert_eq!(parsed_patch.files[0].hunks[0].total_deletions(), 3);
    assert_eq!(parsed_patch.total_additions, 0);
    assert_eq!(parsed_patch.total_deletions, 3);
}

/// 測試獲取修改檔案
/// 
/// 測試獲取 Patch 中所有修改檔案的功能，包括新檔案、已刪除檔案和修改檔案
#[test]
fn test_get_modified_files() {
    let patch_content = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-fn main() {\n+async fn main() {\n     println!(\"Hello\");\n }\n";
    
    let parsed_patch = ParsedPatch::parse(patch_content).unwrap();
    let modified_files = parsed_patch.get_modified_files();
    
    assert_eq!(modified_files.len(), 1);
    assert_eq!(modified_files[0].old_path, PathBuf::from("src/main.rs"));
    assert_eq!(modified_files[0].new_path, PathBuf::from("src/main.rs"));
    assert!(!modified_files[0].is_new);
    assert!(!modified_files[0].is_deleted);
}

/// 測試獲取 Rust 檔案
/// 
/// 測試獲取 Patch 中所有 Rust 檔案的功能，包括 .rs、.toml 和 Cargo.toml 等檔案
#[test]
fn test_get_rust_files() {
    let patch_content = "diff --git a/src/main.rs b/src/main.rs\ndiff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1,1 +1,1 @@\n-README\n+README Updated\n";
    
    let parsed_patch = ParsedPatch::parse(patch_content).unwrap();
    let rust_files = parsed_patch.get_rust_files();
    
    assert_eq!(rust_files.len(), 1);
    assert_eq!(rust_files[0].old_path, PathBuf::from("src/main.rs"));
}

/// 測試 Patch 文件的顯示名稱
/// 
/// 測試獲取 Patch 文件的顯示名稱的功能，包括新檔案、已刪除檔案和重命名檔案的顯示名稱
#[test]
fn test_patch_file_display_name() {
    let file = PatchFile::new(
        PathBuf::from("src/main.rs"),
        PathBuf::from("src/main.rs"),
        false,
        false,
        false,
    );
    assert_eq!(file.display_name(), "src/main.rs");
    
    let new_file = PatchFile::new(
        PathBuf::from("/dev/null"),
        PathBuf::from("src/new.rs"),
        true,
        false,
        false,
    );
    assert_eq!(new_file.display_name(), "新檔案");
    
    let deleted_file = PatchFile::new(
        PathBuf::from("src/old.rs"),
        PathBuf::from("/dev/null"),
        false,
        true,
        false,
    );
    assert_eq!(deleted_file.display_name(), "已刪除");
}

/// 測試 Patch 文件是否為 Rust 檔案
/// 
/// 測試判斷 Patch 文件是否為 Rust 檔案的功能，包括 .rs、.toml 和 Cargo.toml 等檔案
#[test]
fn test_patch_file_is_rust_file() {
    let rust_file = PatchFile::new(
        PathBuf::from("src/main.rs"),
        PathBuf::from("src/main.rs"),
        false,
        false,
        false,
    );
    assert!(rust_file.is_rust_file());
    
    let non_rust_file = PatchFile::new(
        PathBuf::from("README.md"),
        PathBuf::from("README.md"),
        false,
        false,
        false,
    );
    assert!(!non_rust_file.is_rust_file());
    
    let cargo_toml_file = PatchFile::new(
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.toml"),
        false,
        false,
        false,
    );
    assert!(cargo_toml_file.is_rust_file());
}

/// 測試 Hunk 的顯示
/// 
/// 測試獲取 Hunk 的顯示字符串的功能，包括上下文、行號和修改內容
#[test]
fn test_hunk_display() {
    let hunk = Hunk::new(1, 3, 1, 3, vec![
        HunkLine::Context("fn main() {".to_string()),
        HunkLine::Deletion("    old_line".to_string()),
        HunkLine::Addition("    new_line".to_string()),
    ]);
    
    let display = hunk.display();
    assert!(display.contains("@@ -1,3 +1,3 @@"));
    assert!(display.contains("fn main() {"));
    assert!(display.contains("old_line"));
    assert!(display.contains("new_line"));
}

/// 測試 Hunk 的統計信息
/// 
/// 測試獲取 Hunk 的統計信息（新增行、刪除行、上下文行）的機能
#[test]
fn test_hunk_totals() {
    let hunk = Hunk::new(1, 5, 1, 5, vec![
        HunkLine::Context("line 1".to_string()),
        HunkLine::Addition("new line".to_string()),
        HunkLine::Deletion("old line".to_string()),
        HunkLine::Context("line 2".to_string()),
    ]);
    
    assert_eq!(hunk.total_additions(), 1);
    assert_eq!(hunk.total_deletions(), 1);
    assert_eq!(hunk.total_context(), 2);
}

/// 測試 ParseError 的顯示
/// 
/// 測試 ParseError 的錯誤顯示功能，包括無效的 Patch 頭部、無效的 Hunk 頭部等錯誤
#[test]
fn test_parse_error_display() {
    let error = ParseError::InvalidHeader("test error".to_string());
    assert!(error.to_string().contains("無效的 Patch 頭部"));
    assert!(error.to_string().contains("test error"));
}
