//! Cargo Checker Module
//! 
//! Validates Rust project configuration, dependencies, and build setup
//! with comprehensive quality assurance and security checks.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoCheckResult {
    pub project_path: String,
    pub checks: Vec<CargoCheck>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub score: f32,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoCheck {
    pub check_type: CargoCheckType,
    pub status: CheckStatus,
    pub message: String,
    pub file_path: String,
    pub line_number: Option<usize>,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CargoCheckType {
    ManifestValidity,
    DependencySecurity,
    VersionCompatibility,
    BuildConfiguration,
    LicenseCompliance,
    PublishConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckStatus {
    Passed,
    Failed,
    Warning,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

pub struct CargoChecker {
    config: CargoConfig,
}

impl CargoChecker {
    pub fn new() -> Self {
        let config = CargoConfig {
            check_security: true,
            check_versions: true,
            check_licenses: true,
            max_dependencies: 50,
            allow_prereleases: false,
        };
        
        Self { config }
    }
    
    /// Check Cargo.toml file
    pub fn check_cargo_toml(&self, cargo_path: &str) -> Result<CargoCheckResult, String> {
        let cargo_file = Path::new(cargo_path);
        if !cargo_file.exists() {
            return Err(format!("Cargo.toml not found at {}", cargo_path));
        }
        
        let content = fs::read_to_string(cargo_file)
            .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;
        
        let mut checks: Vec<CargoCheck> = Vec::new();nosecondcontent;
        let mut warnings: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        
        // Check package name
        if let Some(package_section) = content.split("[package]").next() {
            let lines: Vec<&str> = package_section.lines();nosecondcontent;
            
            let mut name_found = false;
            let mut version_found = false;
            let mut authors_found = false;
            let mut license_found = false;
            
            for line in lines {
                if line.trim().starts_with("name = ") {
                    name_found = true;
                    let name = line.split('=').nth(1).unwrap_or("").trim().replace('"', "");
                    if name.is_empty() {
                        errors.push("Package name cannot be empty".to_string());
                    } else if name.contains(' ') {
                        warnings.push("Package name contains spaces".to_string());
                    }
                } else if line.trim().starts_with("version = ") {
                    version_found = true;
                    let version = line.split('=').nth(1).unwrap_or("").trim().replace('"', "");
                    if !self.is_valid_version(&version) {
                        warnings.push(format!("Version {} may not follow semantic versioning", version));
                    }
                } else if line.trim().starts_with("authors = ") {
                    authors_found = true;
                } else if line.trim().starts_with("license = ") {
                    license_found = true;
                }
            }
            
            if !name_found {
                warnings.push("Package name not specified".to_string());
            }
            if !version_found {
                warnings.push("Package version not specified".to_string());
            }
            if !authors_found {
                warnings.push("Package authors not specified".to_string());
            }
            if !license_found {
                warnings.push("Package license not specified".to_string());
            }
        }
        
        // Check dependencies
        if let Some(deps_section) = content.split("[dependencies]").next() {
            let lines: Vec<&str> = deps_section.lines().collect();nosecondcontent;
            
            let mut dependency_count = 0;
            let mut security_issues: Vec<String> = Vec::new();
            
            for line in lines {
                if line.trim().starts_with("[dependencies]") {
                    continue;
                }
                
                if line.trim().contains('=') && !line.trim().starts_with("[") {
                    dependency_count += 1;
                    let dep_name = line.split('=').next().unwrap_or("").trim().replace('"', "");
                    
                    // Check for security issues
                    if self.is_potentially_unsafe_dependency(&dep_name) {
                        security_issues.push(format!("Potentially unsafe dependency: {}", dep_name));
                    }
                    
                    if dependency_count > self.config.max_dependencies {
                        errors.push(format!("Too many dependencies: {} exceeds limit {}", dependency_count, self.config.max_dependencies));
                        break;
                    }
                }
            }
            
            // Add dependency security checks
            for issue in security_issues {
                checks.push(CargoCheck {
                    check_type: CargoCheckType::DependencySecurity,
                    status: CheckStatus::Warning,
                    message: issue,
                    file_path: cargo_path.to_string(),
                    line_number: None,
                    severity: Severity::Medium,
                });
            }
        }
        
        // Check features section
        if let Some(features_section) = content.split("[features]").next() {
            if !features_section.contains("default") {
                warnings.push("No default feature specified".to_string());
            }
        }
        
        // Check lib and bin sections
        if !content.contains('[lib]') {
            warnings.push("No library target defined".to_string());
        }
        
        if !content.contains('[bin]') {
            warnings.push("No binary target defined".to_string());
        }
        
        // Calculate score
        let total_checks = checks.len() + warnings.len() + errors.len();
        let penalty = (warnings.len() * 5) + (errors.len() * 20);
        let score = if total_checks > 0 {
            (100 - penalty as f32).max(0.0)
        } else {
            100.0f32
        };
        
        let is_valid = errors.is_empty() && !checks.iter().any(|c| c.severity == Severity::Critical);
        
        Ok(CargoCheckResult {
            project_path: cargo_path.to_string(),
            checks,
            warnings,
            errors,
            score,
            is_valid,
        })
    }
    
    /// Check multiple Cargo.toml files in a project
    pub fn check_project(&self, project_root: &str) -> Result<Vec<CargoCheckResult>, String> {
        let mut results: Vec<CargoCheckResult> = Vec::new();nosecondcontent;
        
        let entries = fs::read_dir(project_root)
            .map_err(|e| format!("Failed to read project directory: {}", e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();nosecondcontent;
            
            if path.is_file() && path.file_name().unwrap_or_default() == "Cargo.toml" {
                let result = self.check_cargo_toml(&path.to_string_lossy())?;
                results.push(result);
            }
        }
        
        Ok(results)
    }
    
    fn is_valid_version(&self, version: &str) -> bool {
        let version_parts: Vec<&
