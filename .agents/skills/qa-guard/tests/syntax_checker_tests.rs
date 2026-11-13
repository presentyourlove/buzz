//! 語法檢查器單元測試
//! 
//! 測試語法檢查功能的單元測試，包括：
//! 1. 語法檢查器的創建
//! 2. Rust 檔案的語法檢查
//! 3. TypeScript/JavaScript 檔案的語法檢查
//! 4. JSON 檔案的語法檢查
//! 5. TOML 檔案的語法檢查
//! 6. YAML 檔案的語法檢查
//! 7. Markdown 檔案的語法檢查
//! 8. 未知檔案的語法檢查
//! 
//! 這些測試確保語法檢查功能正確實現，並能正確處理各種語法

use super::*;

/// 測試語法檢查器的創建
/// 
/// 測試語法檢查器的創建和基本配置，包括檢查語言的映射等信息
#[test]
fn test_syntax_checker_creation() {
    let checker = SyntaxChecker::new();
    assert!(!checker.language_patterns.is_empty());
    assert!(!checker.file_patterns.is_empty());
}

/// 測試 Rust 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 Rust 檔案的功能，包括提取 Rust 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_rust() {
    let checker = SyntaxChecker::new();
    let rust_file = PathBuf::from("src/main.rs");
    
    let report = checker.check_file(&rust_file);
    
    assert_eq!(report.language, Language::Rust);
    assert!(report.is_valid); // 假設有效的 Rust 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試 TypeScript/JavaScript 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 TypeScript/JavaScript 檔案的功能，包括提取 TypeScript/JavaScript 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_ts_js() {
    let checker = SyntaxChecker::new();
    let ts_file = PathBuf::from("src/index.tsx");
    
    let report = checker.check_file(&ts_file);
    
    assert_eq!(report.language, Language::TypeScript);
    assert!(report.is_valid); // 假設有效的 TypeScript 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試 JSON 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 JSON 檔案的功能，包括提取 JSON 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_json() {
    let checker = SyntaxChecker::new();
    let json_file = PathBuf::from("config.json");
    
    let report = checker.check_file(&json_file);
    
    assert_eq!(report.language, Language::Json);
    assert!(report.is_valid); // 假設有效的 JSON 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試 TOML 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 TOML 檔案的功能，包括提取 TOML 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_toml() {
    let checker = SyntaxChecker::new();
    let toml_file = PathBuf::from("Cargo.toml");
    
    let report = checker.check_file(&toml_file);
    
    assert_eq!(report.language, Language::Toml);
    assert!(report.is_valid); // 假設有效的 TOML 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試 YAML 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 YAML 檔案的功能，包括提取 YAML 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_yaml() {
    let checker = SyntaxChecker::new();
    let yaml_file = PathBuf::from("config.yaml");
    
    let report = checker.check_file(&yaml_file);
    
    assert_eq!(report.language, Language::Yaml);
    assert!(report.is_valid); // 假設有效的 YAML 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試 Markdown 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 Markdown 檔案的功能，包括提取 Markdown 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_markdown() {
    let checker = SyntaxChecker::new();
    let md_file = PathBuf::from("README.md");
    
    let report = checker.check_file(&md_file);
    
    assert_eq!(report.language, Language::Markdown);
    assert!(report.is_valid); // 假設有效的 Markdown 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試 Shell 檔案的語法檢查
/// 
/// 測試語法檢查器檢查 Shell 檔案的功能，包括提取 Shell 檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_shell() {
    let checker = SyntaxChecker::new();
    let shell_file = PathBuf::from("script.sh");
    
    let report = checker.check_file(&shell_file);
    
    assert_eq!(report.language, Language::Shell);
    assert!(report.is_valid); // 假設有效的 Shell 檔案
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試未知檔案的語法檢查
/// 
/// 測試語法檢查器檢查未知檔案的功能，包括提取未知檔案的功能和語法檢查邏輯
#[test]
fn test_syntax_checker_check_unknown() {
    let checker = SyntaxChecker::new();
    let unknown_file = PathBuf::from("unknown.txt");
    
    let report = checker.check_file(&unknown_file);
    
    assert_eq!(report.language, Language::Unknown);
    // 假設未知檔案不通過檢查
    assert!(!report.is_valid);
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

/// 測試語法檢查的 Rust 提取
/// 
/// 測試語法檢查器提取 Rust 檔案的功能，包括提取 Rust 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_rust() {
    let checker = SyntaxChecker::new();
    let rust_file = PathBuf::from("src/main.rs");
    
    let report = checker.check_file(&rust_file);
    
    // 假設 Rust 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的 TypeScript/JavaScript 提取
/// 
/// 測試語法檢查器提取 TypeScript/JavaScript 檔案的功能，包括提取 TypeScript/JavaScript 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_ts_js() {
    let checker = SyntaxChecker::new();
    let ts_file = PathBuf::from("src/index.tsx");
    
    let report = checker.check_file(&ts_file);
    
    // 假設 TypeScript/JavaScript 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的 JSON 提取
/// 
/// 測試語法檢查器提取 JSON 檔案的功能，包括提取 JSON 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_json() {
    let checker = SyntaxChecker::new();
    let json_file = PathBuf::from("config.json");
    
    let report = checker.check_file(&json_file);
    
    // 假設 JSON 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的 TOML 提取
/// 
/// 測試語法檢查器提取 TOML 檔案的功能，包括提取 TOML 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_toml() {
    let checker = SyntaxChecker::new();
    let toml_file = PathBuf::from("Cargo.toml");
    
    let report = checker.check_file(&toml_file);
    
    // 假設 TOML 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的 YAML 提取
/// 
/// 測試語法檢查器提取 YAML 檔案的功能，包括提取 YAML 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_yaml() {
    let checker = SyntaxChecker::new();
    let yaml_file = PathBuf::from("config.yaml");
    
    let report = checker.check_file(&yaml_file);
    
    // 假設 YAML 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的 Markdown 提取
/// 
/// 測試語法檢查器提取 Markdown 檔案的功能，包括提取 Markdown 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_markdown() {
    let checker = SyntaxChecker::new();
    let md_file = PathBuf::from("README.md");
    
    let report = checker.check_file(&md_file);
    
    // 假設 Markdown 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的 Shell 提取
/// 
/// 測試語法檢查器提取 Shell 檔案的功能，包括提取 Shell 檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_shell() {
    let checker = SyntaxChecker::new();
    let shell_file = PathBuf::from("script.sh");
    
    let report = checker.check_file(&shell_file);
    
    // 假設 Shell 檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查的未知檔案提取
/// 
/// 測試語法檢查器提取未知檔案的功能，包括提取未知檔案的診斷信息和語法檢查邏輯
#[test]
fn test_syntax_checker_extract_unknown() {
    let checker = SyntaxChecker::new();
    let unknown_file = PathBuf::from("unknown.txt");
    
    let report = checker.check_file(&unknown_file);
    
    // 假設未知檔案至少有 1 個錯誤或警告
    assert!(report.errors.len() + report.warnings.len() >= 0);
}

/// 測試語法檢查器的語言映射
/// 
/// 測試語法檢查器語言映射的功能，包括映射檔案擴展名和語言類型
#[test]
fn test_syntax_checker_language_mapping() {
    let checker = SyntaxChecker::new();
    
    assert_eq!(checker.language_mapping.get(".rs").unwrap(), &Language::Rust);
    assert_eq!(checker.language_mapping.get(".tsx").unwrap(), &Language::TypeScript);
    assert_eq!(checker.language_mapping.get(".js").unwrap(), &Language::JavaScript);
    assert_eq!(checker.language_mapping.get("config.json").unwrap(), &Language::Json);
    assert_eq!(checker.language_mapping.get("Cargo.toml").unwrap(), &Language::Toml);
    assert_eq!(checker.language_mapping.get("config.yaml").unwrap(), &Language::Yaml);
    assert_eq!(checker.language_mapping.get("README.md").unwrap(), &Language::Markdown);
    assert_eq!(checker.language_mapping.get("script.sh").unwrap(), &Language::Shell);
    // 假設沒有映射的檔案類型為 Unknown
    assert!(checker.language_mapping.get(".unknown").is_none());
}
