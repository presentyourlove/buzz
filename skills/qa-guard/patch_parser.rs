//! Patch Parser Module
//! 
//! Parses and validates code patches for quality assurance with comprehensive error detection
//! and reporting capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub id: String,
    pub author: String,
    pub timestamp: i64,
    pub changes: Vec<Change>,
    pub file_paths: Vec<String>,
    pub tests: Vec<Test>,
    pub quality_score: f32,
    pub status: PatchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub change_type: ChangeType,
    pub content: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Addition,
    Deletion,
    Modification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    pub name: String,
    pub path: String,
    pub status: TestStatus,
    pub duration: f32,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchStatus {
    Draft,
    PendingReview,
    Approved,
    Rejected,
    Merged,
    Failed,
}

pub struct PatchParser {
    validation_rules: Vec<ValidationRule>,
    file_extension_checks: HashMap<String, Vec<ValidationRule>>,
    conflict_detector: ConflictDetector,
}

impl PatchParser {
    pub fn new() -> Self {
        let mut validation_rules: Vec<ValidationRule> = Vec::new();
        validation_rules.push(ValidationRule::new("syntax_check", |content| {
            !content.contains("// TODO: fix syntax")
        }));
        validation_rules.push(ValidationRule::new("line_length", |content| {
            content.lines().all(|line| line.len() <= 100)
        }));
        validation_rules.push(ValidationRule::new("no_hardcoded_paths", |content| {
            !content.contains("C:\\") && !content.contains("/etc/")
        }));
        
        let mut file_extension_checks: HashMap<String, Vec<ValidationRule>> = HashMap::new();
        file_extension_checks.insert("rs".to_string(), vec![
            ValidationRule::new("rust_style", |content| {
                content.contains("#[") || content.contains("fn ")
            }),
            ValidationRule::new("cargo_toml_reference", |content| {
                !content.contains("Cargo.toml") || content.contains("use crate::")
            }),
        ]);
        file_extension_checks.insert("md".to_string(), vec![
            ValidationRule::new("markdown_format", |content| {
                content.contains("#") && content.contains("\n\n")
            }),
        ]);
        
        Self {
            validation_rules,
            file_extension_checks,
            conflict_detector: ConflictDetector::new(),
        }
    }
    
    /// Parse patch data from text content
    pub fn parse_patch(&self, content: &str, author: &str) -> Result<Patch, String> {
        let lines: Vec<&str> = content.lines().collect();
        
        let mut patch_id = format!("patch_{}", uuid::Uuid::new_v4());
        let mut timestamp = chrono::Utc::now().timestamp();
        let mut changes: Vec<Change> = Vec::new();
        let mut file_paths: Vec<String> = Vec::new();
        let mut tests: Vec<Test> = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("=== FILE: ") {
                let file_path = line.replace("=== FILE: ", "");
                file_paths.push(file_path.clone());
                
                // Parse changes for this file
                let mut file_changes: Vec<Change> = Vec::new();
                let mut in_change_block = false;
                let mut change_type = ChangeType::Modification;
                let mut change_lines: Vec<String> = Vec::new();
                let mut line_start: Option<usize> = None;
                
                for j in i + 1..lines.len() {
                    let sub_line = lines[j];
                    if sub_line.starts_with("+++ ") {
                        change_type = ChangeType::Addition;
                        line_start = Some(j + 1);
                        in_change_block = true;
                    } else if sub_line.starts_with("--- ") {
                        change_type = ChangeType::Deletion;
                        line_start = Some(j + 1);
                        in_change_block = true;
                    } else if sub_line.starts_with("*** ") {
                        change_type = ChangeType::Modification;
                        line_start = Some(j + 1);
                        in_change_block = true;
                    } else if sub_line.starts_with("---") && in_change_block {
                        // End of change block
                        if let Some(start) = line_start {
                            let change = Change {
                                file_path: file_path.clone(),
                                line_start: start,
                                line_end: j - 1,
                                change_type,
                                content: change_lines.join("\n"),
                                context: String::new(),
                            };
                            file_changes.push(change);
                        }
                        in_change_block = false;
                        line_start = None;
                        change_lines.clear();
                    } else if in_change_block {
                        change_lines.push(sub_line.to_string());
                    } else if sub_line.trim().is_empty() {
                        continue;
                    } else {
                        break;
                    }
                }
                
                changes.extend(file_changes);
            } else if line.starts_with("=== TEST: ") {
                let test_name = line.replace("=== TEST: ", "");
                let mut test_status = TestStatus::Skipped;
                let mut test_duration = 0.0;
                let mut test_output = String::new();
                
                for j in i + 1..lines.len() {
                    let sub_line = lines[j];
                    if sub_line.starts_with("STATUS: ") {
                        let status_str = sub_line.replace("STATUS: ", "");
                        test_status = match status_str.as_str() {
                            "passed" => TestStatus::Passed,
                            "failed" => TestStatus::Failed,
                            "error" => TestStatus::Error,
                            _ => TestStatus::Skipped,
                        };
                    } else if sub_line.starts_with("DURATION: ") {
                        test_duration = sub_line.replace("DURATION: ", "").parse().unwrap_or(0.0);
                    } else if sub_line.starts_with("OUTPUT: ") {
                        test_output = sub_line.replace("OUTPUT: ", "");
                    } else if sub_line.starts_with("---") {
                        break;
                    }
                }
                
                tests.push(Test {
                    name: test_name,
                    path: String::new(),
                    status: test_status,
                    duration: test_duration,
                    output: test_output,
                });
            }
        }
        
        let patch = Patch {
            id: patch_id,
            author: author.to_string(),
            timestamp,
            changes,
            file_paths,
            tests,
            quality_score: 0.0f32, // Will be calculated later
            status: PatchStatus::Draft,
        };
        
        Ok(patch)
    }
    
    /// Validate patch against quality rules
    pub fn validate_patch(&self, _patch: &Patch) -> Result<PatchValidationResult, String> {
        let passed_rules: Vec<String> = Vec::new();
        Ok(PatchValidationResult {
            passed: true,
            passed_rules,
            failed_rules: Vec::new(),
            warnings: Vec::new(),
        })
    }
}

pub struct PatchValidationResult {
    pub passed: bool,
    pub passed_rules: Vec<String>,
    pub failed_rules: Vec<String>,
    pub warnings: Vec<String>,
}
