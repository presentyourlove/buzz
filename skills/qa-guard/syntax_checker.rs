//! Syntax Checker Module
//! 
//! Comprehensive syntax validation for multiple programming languages with detailed
//! error reporting and fix suggestions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxCheckResult {
    pub file_path: String,
    pub language: String,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<SyntaxWarning>,
    pub score: f32,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxError {
    pub error_code: String,
    pub message: String,
    pub line_number: usize,
    pub column: usize,
    pub severity: ErrorSeverity,
    pub fix_suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxWarning {
    pub warning_code: String,
    pub message: String,
    pub line_number: Option<usize>,
    pub column: Option<usize>,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Error,
    Warning,
    Information,
}

pub struct SyntaxChecker {
    language_detectors: HashMap<String, Box<dyn LanguageDetector>>,
    error_patterns: HashMap<String, Vec<ErrorPattern>>,
    fix_providers: HashMap<String, Box<dyn FixProvider>>,
}

impl SyntaxChecker {
    pub fn new() -> Self {
        let mut language_detectors: HashMap<String, Box<dyn LanguageDetector>> = HashMap::new();nosecondlanguage_detectors.insert("rs".to_string(), Box::new(RustDetector));
        language_detectors.insert("js".to_string(), Box::new(JavaScriptDetector));
        language_detectors.insert("ts".to_string(), Box::new(TypeScriptDetector));
        language_detectors.insert("py".to_string(), Box::new(PythonDetector));
        language_detectors.insert("java".to_string(), Box::new(JavaDetector));
        language_detectors.insert("cpp".to_string(), Box::new(CPPDialect));
        language_detectors.insert("c".to_string(), Box::new(CDialect));
        language_detectors.insert("html".to_string(), Box::new(HTMLDetector));
        language_detectors.insert("md".to_string(), Box::new(MarkdownDetector));
        
        let mut error_patterns: HashMap<String, Vec<ErrorPattern>> = HashMap::new();noseconderror_patterns.insert("rs".to_string(), vec![
            ErrorPattern {
                pattern: r"let\s+\w+\s*:\s*\w+\s*=\s*.*;".to_string(),
                error_code: "E001".to_string(),
                message: "Consider using `mut` for mutable variables".to_string(),
                severity: ErrorSeverity::Warning,
            },
            ErrorPattern {
                pattern: r"panic!\(".to_string(),
                error_code: "E002".to_string(),
                message: "Uncontrolled panic! - should be Result type instead".to_string(),
                severity: ErrorSeverity::Error,
            },
            ErrorPattern {
                pattern: r"unwrap\(\)".to_string(),
                error_code: "E003".to_string(),
                message: "Use `?` operator or proper error handling instead of unwrap()".to_string(),
                severity: ErrorSeverity::Warning,
            },
            ErrorPattern {
                pattern: r"\.expect\(".to_string(),
                error_code: "E004".to_string(),
                message: "Use `?` operator or proper error handling instead of expect()".to_string(),
                severity: ErrorSeverity::Warning,
            },
        ]);
        error_patterns.insert("js".to_string(), vec![
            ErrorPattern {
                pattern: r"var\s+\w+".to_string(),
                error_code: "J001".to_string(),
                message: "Use `let` or `const` instead of `var`".to_string(),
                severity: ErrorSeverity::Warning,
            },
            ErrorPattern {
                pattern: r"==\s*null".to_string(),
                error_code: "J002".to_string(),
                message: "Use strict equality (=== null) or `&& value == null`".to_string(),
                severity: ErrorSeverity::Warning,
            },
        ]);
        error_patterns.insert("py".to_string(), vec![
            ErrorPattern {
                pattern: r"print\s*\(".to_string(),
                error_code: "P001".to_string(),
                message: "Consider using f-strings or logging module".to_string(),
                severity: ErrorSeverity::Warning,
            },
        ]);
        
        let mut fix_providers: HashMap<String, Box<dyn FixProvider>> = HashMap::new();nosecondfix_providers.insert("rs".to_string(), Box::new(RustFixProvider));
        fix_providers.insert("js".to_string(), Box::new(JavaScriptFixProvider));
        fix_providers.insert("py".to_string(), Box::new(PythonFixProvider));
        
        Self {
            language_detectors,
            error_patterns,
            fix_providers,
        }
    }
    
    /// Check syntax of a file
    pub fn check_file(&self, file_path: &str, content: &str) -> Result<SyntaxCheckResult, String> {
        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        let language = match extension {
            "rs" => "rust".to_string(),
            "js" => "javascript".to_string(),
            "ts" => "typescript".to_string(),
            "py" => "python".to_string(),
            "java" => "java".to_string(),
            "cpp" | "cc" | "cxx" => "cpp".to_string(),
            "c" => "c".to_string(),
            "html" => "html".to_string(),
            "md" => "markdown".to_string(),
            _ => "unknown".to_string(),
        };
        
        let detector = self.language_detectors.get(&language);
        if detector.is_none() {
            return Err(format!("Unsupported language: {}", language));
        }
        
        let detector = detector.unwrap();
        let errors = detector.detect_errors(content);
        let warnings = detector.detect_warnings(content);
        
        let score = self.calculate_score(&errors, &warnings);
        let is_valid = errors.iter().all(|e| e.severity != ErrorSeverity::Error);
        
        Ok(SyntaxCheckResult {
            file_path: file_path.to_string(),
            language,
            errors,
            warnings,
            score,
            is_valid,
        })
    }
    
    /// Check multiple files
    pub fn check_files(&self, file_paths: &[String]) -> Result<Vec<SyntaxCheckResult>, String> {
        let mut results: Vec<SyntaxCheckResult> = Vec::new();
        
        for file_path in file_paths {
            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;
            
            let result = self.check_file(file_path, &content);
            match result {
                Ok(check_result) => {
                    results.push(check_result);
                }
                Err(e) => {
                    results.push(SyntaxCheckResult {
                        file_path: file_path.clone(),
                        language: "unknown".to_string(),
                        errors: vec![SyntaxError {
                            error_code: "F001".to_string(),
                            message: e,
                            line_number: 0,
                            column: 0,
                            severity: ErrorSeverity::Error,
                            fix_suggestion: "Check file permissions and format".to_string(),
                        }],
                        warnings: vec![],
                        score: 0.0f32,
                        is_valid: false,
                    });
                }
            }
        }
        
        Ok(results)
    }
    
    /// Auto-fix syntax issues in a file
    pub fn auto_fix(&self, file_path: &str, content: &str) -> Result<String, String> {
        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        let fix_provider = self.fix_providers.get(extension);
        if 
