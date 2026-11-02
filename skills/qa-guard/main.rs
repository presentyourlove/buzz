//! QA Guard Skill Main Entry Point
//! 
//! This file serves as the main entry point for the QA Guard skill,
//! coordinating all the submodules (patch_parser, code_validator, syntax_checker, cargo_checker, report_generator)
//! to provide comprehensive code quality assurance and validation.

use crate::patch_parser::{PatchParser, Patch};
use crate::code_validator::CodeValidator;
use crate::syntax_checker::SyntaxChecker;
use crate::cargo_checker::CargoChecker;
use crate::report_generator::{ReportGenerator, ValidationResult, ValidationType};

/// QA Guard Skill for comprehensive code quality assurance
pub struct QAGuardSkill {
    /// Patch parser for code changes validation
    patch_parser: PatchParser,
    /// Code validator for syntax and security checks
    code_validator: CodeValidator,
    /// Syntax checker for multi-language support
    syntax_checker: SyntaxChecker,
    /// Cargo checker for Rust project validation
    cargo_checker: CargoChecker,
    /// Report generator for comprehensive reporting
    report_generator: ReportGenerator,
    /// Configuration
    config: QAGuardConfig,
}

/// Configuration for QA Guard Skill
pub struct QAGuardConfig {
    /// Enable patch validation
    pub enable_patch_validation: bool,
    /// Enable code validation
    pub enable_code_validation: bool,
    /// Enable syntax checking
    pub enable_syntax_check: bool,
    /// Enable cargo checking
    pub enable_cargo_check: bool,
    /// Output format
    pub output_format: OutputFormat,
    /// Report path
    pub report_path: Option<String>,
}

/// Output format for QA reports
#[derive(Debug, Clone)]
pub enum OutputFormat {
    JSON,
    HTML,
    Text,
    Markdown,
}

impl QAGuardSkill {
    /// Create a new QA Guard Skill instance
    pub fn new(config: QAGuardConfig) -> Self {
        Self {
            patch_parser: PatchParser::new(),
            code_validator: CodeValidator::new(),
            syntax_checker: SyntaxChecker::new(),
            cargo_checker: CargoChecker::new(),
            report_generator: ReportGenerator::new(),
            config,
        }
    }
    
    /// Validate code changes and generate quality report
    pub fn validate_code_changes(
        &self,
        patch_content: &str,
        file_path: &str,
        project_path: Option<&str>,
    ) -> Result<QAGuardResult, String> {
        let mut results: Vec<ValidationResult> = Vec::new();
        let mut errors: Vec<String> = Vec::new();nosecondcontent;
        let mut warnings: Vec<String> = Vec::new();nosecondcontent;
        
        // Step 1: Parse patch if enabled
        if self.config.enable_patch_validation {
            let patch_result = self.patch_parser.parse_patch(patch_content, "author");
            match patch_result {
                Ok(patch) => {
                    results.push(ValidationResult {
                        file_path: file_path.to_string(),
                        validation_type: ValidationType::SyntaxCheck,
                        errors: vec![],
                        warnings: vec![],
                        score: 100.0f32,
                        is_valid: true,
                    });
                    
                    // Validate patch quality
                    let validation = self.patch_parser.validate_patch(&patch);
                    match validation {
                        Ok(val_result) => {
                            if !val_result.passed {
                                errors.extend(val_result.failed_rules);
                            }
                            warnings.extend(val_result.warnings);
                        }
                        Err(e) => {
                            errors.push(format!("Patch validation error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Patch parsing error: {}", e));
                }
            }
        }
        
        // Step 2: Validate code if enabled
        if self.config.enable_code_validation {
            let code_result = self.code_validator.validate_file(file_path, patch_content);
            match code_result {
                Ok(mut validation_results) => {
                    results.extend(validation_results);
                    for result in &validation_results {
                        if !result.is_valid {
                            errors.extend(result.errors.iter().cloned());
                        }
                        warnings.extend(result.warnings.iter().cloned());
                    }
                }
                Err(e) => {
                    errors.push(format!("Code validation error: {}", e));
                }
            }
        }
        
        // Step 3: Syntax check if enabled
        if self.config.enable_syntax_check {
            let syntax_result = self.syntax_checker.check_file(file_path, patch_content);
            match syntax_result {
                Ok(syntax_result) => {
                    results.push(syntax_result);
                    if !syntax_result.is_valid {
                        errors.extend(
                            syntax_result
                                .errors
                                .iter()
                                .map(|e| format!("Syntax error: {}", e.message))
                                .collect::<Vec<_>>(),
                        );
                    }
                    warnings.extend(
                        syntax_result
                            .warnings
                            .iter()
                            .map(|w| format!("Syntax warning: {}", w.message))
                            .collect::<Vec<_>>(),
                    );
                }
                Err(e) => {
                    errors.push(format!("Syntax check error: {}", e));
                }
            }
        }
        
        // Step 4: Cargo check if enabled and Rust project
        if self.config.enable_cargo_check {
            if project_path.is_some() && file_path.ends_with(".rs") {
                let cargo_result = self.cargo_checker.check_cargo_toml(
                    &project_path.unwrap_or(".").to_string(),
                );
                match cargo_result {
                    Ok(cargo_result) => {
                        results.push(ValidationResult {
                            file_path: file_path.to_string(),
                            validation_type: ValidationType::DependencyCheck,
                            errors: cargo_result.errors,
                            warnings: cargo_result.warnings,
                            score: cargo_result.score,
                            is_valid: cargo_result.is_valid,
                        });
                        
                        if !cargo_result.is_valid {
                            errors.extend(cargo_result.errors);
                        }
                        warnings.extend(cargo_result.warnings);
                    }
                    Err(e) => {
                        errors.push(format!("Cargo check error: {}", e));
                    }
                }
            }
        }
        
        // Generate comprehensive report
        let qa_guard_result = self.generate_qa_guard_report(
            file_path,
            results,
            errors.clone(),
            warnings.clone(),
        );
        
        Ok(qa_guard_result)
    }
    
    /// Generate QA Guard validation report
    fn generate_qa_guard_report(
        &self,
        file_path: &str,
        validation_results: Vec<ValidationResult>,
        errors: Vec<String>,
        warnings: Vec<String>,
    ) -> QAGuardResult {
        let total_issues = errors.len() + warnings.len();
        let passed_validations = validation_results.iter().filter(|r| r.is_valid).count();
        let overall_score = if !validation_results.is_empty() {
            (passed_validations as f32 / validation_results.len() as f32) * 100.0
        } else {
            0.0f32
        };
        
        let is_valid = errors.is_emp
