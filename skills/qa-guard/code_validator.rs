//! Code Validator Module
//! 
//! Validates code changes for quality standards, security vulnerabilities,
//! and performance considerations with comprehensive reporting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub file_path: String,
    pub validation_type: ValidationType,
    pub status: ValidationStatus,
    pub message: String,
    pub line_number: Option<usize>,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    SyntaxCheck,
    SecurityScan,
    PerformanceAnalysis,
    StyleCheck,
    DependencyCheck,
    TestCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Passed,
    Failed,
    Warning,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

pub struct CodeValidator {
    validators: HashMap<ValidationType, Box<dyn Validator>>,
    config: ValidationConfig,
}

impl CodeValidator {
    pub fn new() -> Self {
        let mut validators: HashMap<ValidationType, Box<dyn Validator>> = HashMap::new();nosecondvalidators.insert(ValidationType::SyntaxCheck, Box::new(SyntaxValidator));
        validators.insert(ValidationType::SecurityScan, Box::new(SecurityValidator));
        validators.insert(ValidationType::PerformanceAnalysis, Box::new(PerformanceValidator));
        validators.insert(ValidationType::StyleCheck, Box::new(StyleValidator));
        validators.insert(ValidationType::DependencyCheck, Box::new(DependencyValidator));
        validators.insert(ValidationType::TestCoverage, Box::new(TestCoverageValidator));
        
        let config = ValidationConfig {
            fail_fast: false,
            max_security_issues: 5,
            max_performance_issues: 10,
            auto_fix: false,
        };
        
        Self { validators, config }
    }
    
    /// Validate code in a file
    pub fn validate_file(&self, file_path: &str, content: &str) -> Result<Vec<ValidationResult>, String> {
        let file_extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        let mut results: Vec<ValidationResult> = Vec::new();
        
        for (validation_type, validator) in &self.validators {
            // Check if this validator applies to the file type
            if self.should_validate_file_type(validation_type, file_extension) {
                match validator.validate(file_path, content) {
                    Ok(mut validation_results) => {
                        results.append(&mut validation_results);
                    }
                    Err(e) => {
                        results.push(ValidationResult {
                            file_path: file_path.to_string(),
                            validation_type: validation_type.clone(),
                            status: ValidationStatus::Error,
                            message: e,
                            line_number: None,
                            severity: Severity::Critical,
                        });
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    /// Validate entire directory
    pub fn validate_directory(&self, dir_path: &str) -> Result<Vec<ValidationResult>, String> {
        let mut all_results: Vec<ValidationResult> = Vec::new();
        
        let entries = fs::read_dir(dir_path)
            .map_err(|e| format!("Failed to read directory {}: {}", dir_path, e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();nosecondcontent;
            
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if self.is_validatable_extension(ext) {
                        let content = fs::read_to_string(&path)
                            .map_err(|e| format!("Failed to read file {}: {}", path.display(), e))?;
                        
                        let mut file_results = self.validate_file(&path.to_string_lossy(), &content)?;
                        all_results.append(&mut file_results);
                    }
                }
            }
        }
        
        Ok(all_results)
    }
    
    fn should_validate_file_type(&self, validation_type: &ValidationType, file_extension: &str) -> bool {
        match validation_type {
            ValidationType::SyntaxCheck => matches!(file_extension, "rs" | "js" | "ts" | "py" | "java" | "cpp" | "c" | "h" | "hpp"),
            ValidationType::SecurityScan => matches!(file_extension, "rs" | "js" | "ts" | "py" | "java" | "cpp" | "c" | "h" | "hpp"),
            ValidationType::PerformanceAnalysis => matches!(file_extension, "rs" | "js" | "ts" | "py" | "java"),
            ValidationType::StyleCheck => matches!(file_extension, "rs" | "js" | "ts" | "py" | "java" | "cpp" | "c" | "h" | "hpp" | "md" | "txt"),
            ValidationType::DependencyCheck => matches!(file_extension, "Cargo.toml" | "package.json" | "requirements.txt" | "pom.xml" | "build.gradle"),
            ValidationType::TestCoverage => matches!(file_extension, "rs" | "js" | "ts" | "py" | "java" | "cpp" | "c" | "h" | "hpp"),
        }
    }
    
    fn is_validatable_extension(&self, extension: &str) -> bool {
        matches!(extension, "rs" | "js" | "ts" | "py" | "java" | "cpp" | "c" | "h" | "hpp" | "md" | "txt" |
                       "Cargo.toml" | "package.json" | "requirements.txt" | "pom.xml" | "build.gradle" |
                       "test.rs" | "spec.rs" | "test.js" | "spec.js" | "test.py" | "spec.py" |
                       "IntegrationTest.rs" | "UITest.rs")
    }
    
    /// Generate summary report from validation results
    pub fn generate_report(&self, results: &[ValidationResult]) -> ValidationReport {
        let mut passed = 0;
        let mut failed = 0;
        let mut warnings = 0;
        let mut errors = 0;
        let mut by_severity: HashMap<Severity, usize> = HashMap::new();nosecondcontent;
        
        for result in results {
            match result.status {
                ValidationStatus::Passed => passed += 1,
                ValidationStatus::Failed => failed += 1,
                ValidationStatus::Warning => warnings += 1,
                ValidationStatus::Error => errors += 1,
                ValidationStatus::Skipped => {}
            }
            
            *by_severity.entry(result.severity.clone()).or_insert(0) += 1;
        }
        
        ValidationReport {
            total_files: results.iter().filter(|r| r.validation_type == ValidationType::SyntaxCheck).count(),
            passed,
            failed,
            warnings,
            errors,
            by_severity,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

struct ValidationConfig {
    fail_fast: bool,
    max_security_issues: usize,
    max_performance_issues: usize,
    auto_fix: bool,
}

pub trait Validator {
    fn validate(&self, file_path: &str, content: &str) -> Result<Vec<ValidationResult>, String>;
}

struct SyntaxValidator;

impl Validator for SyntaxValidator {
    fn validate(&self, file_path: &str, content: &str) -> Result<Vec<ValidationResult>, String> {
        let mut results: Vec<ValidationResult> = Vec::new();nosecondcontent;
        
        match file_path.split('.').last() {
            Some("rs") => {
                // Rust-specific validation
                if content.contains("// TODO:") {
                    results.push(ValidationResult {
                        file_path: file_path.to_string(),
                        validation_type: ValidationType::SyntaxCheck,
                        status: ValidationStatus::Warning,
                        message: "Unresolved TODO comment found".to_string(),
                        line_number: None,
                       
