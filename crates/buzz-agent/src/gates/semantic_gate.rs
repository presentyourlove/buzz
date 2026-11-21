//! Semantic & Contract Gate Implementation
//!
//! Defined according to `PLANS/DESIGN_SPEC_B_TRIPLE_GATE.md`.
//!
//! Semantic Gate (Gate 1b) 負責語意與契約檢查：
//! 1. 契約測試驗證 - 生產代碼路徑必須有對應契約測試
//! 2. OpenAPI Schema 驗證 - 請求/響應符合定義
//! 3. 語義契約驗證 - 業務語義正確性
//! 4. 介面穩定性檢查 - API/事件幂等性、向後相容性
//! 5. 錯誤處理契約 - 捕獲失敗必留重試記錄或傳播

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Semantic Gate 驗證錯誤類型
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum SemanticGateError {
    #[error("契約測試缺失: {path} - {detail}")]
    MissingContractTest { path: String, detail: String },

    #[error("OpenAPI Schema 驗證失敗: {path} - {detail}")]
    SchemaValidationFailed { path: String, detail: String },

    #[error("語義契約違反: {path} - {detail}")]
    SemanticContractViolation { path: String, detail: String },

    #[error("介面不穩定: {path} - {detail}")]
    UnstableInterface { path: String, detail: String },

    #[error("錯誤處理契約違反: {path} - {detail}")]
    ErrorHandlingViolation { path: String, detail: String },

    #[error("驗證器內部錯誤: {0}")]
    InternalError(String),

    #[error("IO 錯誤: {0}")]
    IoError(String),
}

/// 驗證結果等級
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// 阻斷性錯誤 - 必須修復
    Error,
    /// 警告 - 建議修復但不阻斷
    Warning,
    /// 資訊 - 僅供參考
    Info,
}

/// 單項驗證結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub evidence_refs: Vec<String>,
}

/// 完整驗證結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub issues: Vec<ValidationIssue>,
    pub checked_paths: Vec<String>,
    pub evidence_refs: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            checked_paths: Vec::new(),
            evidence_refs: Vec::new(),
        }
    }

    pub fn add_issue(&mut self, issue: ValidationIssue) {
        if issue.severity == ValidationSeverity::Error {
            self.passed = false;
        }
        self.issues.push(issue);
    }

    pub fn add_checked_path(&mut self, path: String) {
        self.checked_paths.push(path);
    }

    pub fn merge(&mut self, other: ValidationResult) {
        if !other.passed {
            self.passed = false;
        }
        self.issues.extend(other.issues);
        self.checked_paths.extend(other.checked_paths);
        self.evidence_refs.extend(other.evidence_refs);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// 契約測試規格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractSpec {
    pub test_name: String,
    pub production_seam: String,      // 生產代碼路徑
    pub test_path: String,            // 測試文件路徑
    pub validates: Vec<String>,       // 驗證的功能點
    pub is_falsifiable: bool,         // 可被破壞驗證
}

/// OpenAPI Schema 定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSchema {
    pub schema_name: String,
    pub schema_json: serde_json::Value,
    pub version: String,
    pub endpoints: Vec<String>,
}

/// 語義契約規則
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticRule {
    pub rule_id: String,
    pub description: String,
    pub production_path: String,
    pub expected_behavior: String,
    pub check_fn: String, // 函數名稱，用於動態查找
}

/// 介面穩定性檢查項
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityCheck {
    pub interface_name: String,
    pub interface_type: InterfaceType,
    pub current_version: String,
    pub breaking_changes: Vec<String>,
    pub deprecated_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterfaceType {
    RestApi,
    NostrEvent,
    WebSocket,
    CliCommand,
    InternalTrait,
}

/// 核心驗證器 Trait
pub trait SemanticValidator: Send + Sync {
    fn validator_name(&self) -> &str;
    fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError>;
}

/// 驗證上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationContext {
    pub project_root: String,
    pub changed_files: Vec<String>,
    pub git_diff: Option<String>,
    pub contract_specs: Vec<ContractSpec>,
    pub openapi_schemas: Vec<OpenApiSchema>,
    pub semantic_rules: Vec<SemanticRule>,
    pub stability_checks: Vec<StabilityCheck>,
    pub gate1_evidence: Vec<String>, // Gate 1 證據引用
}

impl ValidationContext {
    pub fn new(project_root: String) -> Self {
        Self {
            project_root,
            changed_files: Vec::new(),
            git_diff: None,
            contract_specs: Vec::new(),
            openapi_schemas: Vec::new(),
            semantic_rules: Vec::new(),
            stability_checks: Vec::new(),
            gate1_evidence: Vec::new(),
        }
    }
}

/// 契約測試驗證器
pub struct ContractTestValidator;

impl SemanticValidator for ContractTestValidator {
    fn validator_name(&self) -> &str {
        "ContractTestValidator"
    }

    fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError> {
        let mut result = ValidationResult::new();

        // 檢查每個生產代碼路徑是否有對應契約測試
        for spec in &context.contract_specs {
            result.add_checked_path(spec.production_seam.clone());

            // 驗證測試文件存在
            let test_path = std::path::Path::new(&context.project_root).join(&spec.test_path);
            if !test_path.exists() {
                result.add_issue(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    code: "G1-CT-001".to_string(),
                    path: spec.production_seam.clone(),
                    message: format!("契約測試文件不存在: {}", spec.test_path),
                    suggestion: Some(format!("請在 {} 建立契約測試", spec.test_path)),
                    evidence_refs: vec![],
                });
                continue;
            }

            // 驗證測試可被破壞
            if !spec.is_falsifiable {
                result.add_issue(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    code: "G1-CT-002".to_string(),
                    path: spec.test_path.clone(),
                    message: "契約測試不可被破壞驗證".to_string(),
                    suggestion: Some("確保測試綁定生產 seam，而非 mock handler".to_string()),
                    evidence_refs: vec![],
                });
            }

            // 驗證測試覆蓋的功能點
            for validated in &spec.validates {
                result.evidence_refs.push(format!("{}:{}", spec.test_name, validated));
            }
        }

        Ok(result)
    }
}

/// OpenAPI Schema 驗證器
pub struct SchemaValidator;

impl SemanticValidator for SchemaValidator {
    fn validator_name(&self) -> &str {
        "SchemaValidator"
    }

    fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError> {
        let mut result = ValidationResult::new();

        for schema in &context.openapi_schemas {
            result.add_checked_path(schema.schema_name.clone());

            // 基礎 Schema 結構驗證
            if schema.schema_json.is_null() {
                result.add_issue(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    code: "G1-SV-001".to_string(),
                    path: schema.schema_name.clone(),
                    message: "Schema 內容為空".to_string(),
                    suggestion: Some("提供有效的 OpenAPI 3.x Schema 物件".to_string()),
                    evidence_refs: vec![],
                });
                continue;
            }

            // 驗證必要欄位
            if !schema.schema_json.get("type").is_some() && !schema.schema_json.get("properties").is_some() {
                result.add_issue(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    code: "G1-SV-002".to_string(),
                    path: schema.schema_name.clone(),
                    message: "Schema 缺少 type 或 properties 定義".to_string(),
                    suggestion: Some("補全 OpenAPI Schema 定義".to_string()),
                    evidence_refs: vec![],
                });
            }

            // 記錄端點關聯
            for endpoint in &schema.endpoints {
                result.evidence_refs.push(format!("{}:{}", schema.schema_name, endpoint));
            }
        }

        Ok(result)
    }
}

/// 語義契約驗證器
pub struct SemanticContractValidator;

impl SemanticValidator for SemanticContractValidator {
    fn validator_name(&self) -> &str {
        "SemanticContractValidator"
    }

    fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError> {
        let mut result = ValidationResult::new();

        for rule in &context.semantic_rules {
            result.add_checked_path(rule.production_path.clone());

            // 這裡應調用具體的檢查函數
            // 在實際實作中，可透過函數註冊表或動態載入執行
            result.evidence_refs.push(format!("semantic_rule:{}", rule.rule_id));

            // 預設通過，具體規則需註冊具體檢查邏輯
        }

        Ok(result)
    }
}

/// 介面穩定性驗證器
pub struct StabilityValidator;

impl SemanticValidator for StabilityValidator {
    fn validator_name(&self) -> &str {
        "StabilityValidator"
    }

    fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError> {
        let mut result = ValidationResult::new();

        for check in &context.stability_checks {
            result.add_checked_path(check.interface_name.clone());

            if !check.breaking_changes.is_empty() {
                result.add_issue(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    code: "G1-ST-001".to_string(),
                    path: check.interface_name.clone(),
                    message: format!("檢測到破壞性變更: {:?}", check.breaking_changes),
                    suggestion: Some("確保向後相容，或提供遷移路徑".to_string()),
                    evidence_refs: vec![],
                });
            }

            if !check.deprecated_items.is_empty() {
                result.add_issue(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    code: "G1-ST-002".to_string(),
                    path: check.interface_name.clone(),
                    message: format!("發現棄用項目: {:?}", check.deprecated_items),
                    suggestion: Some("標記棄用並提供替代方案".to_string()),
                    evidence_refs: vec![],
                });
            }
        }

        Ok(result)
    }
}

/// 錯誤處理契約驗證器
pub struct ErrorHandlingValidator;

impl SemanticValidator for ErrorHandlingValidator {
    fn validator_name(&self) -> &str {
        "ErrorHandlingValidator"
    }

    fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError> {
        let mut result = ValidationResult::new();

        // 檢查變更文件中的錯誤處理模式
        for file in &context.changed_files {
            if file.ends_with(".rs") {
                result.add_checked_path(file.clone());

                // 這裡可整合靜態分析工具
                // 暫時記錄需人工審查
                result.evidence_refs.push(format!("error_handling_review:{}", file));
            }
        }

        Ok(result)
    }
}

/// Semantic Gate 主協調器
pub struct SemanticGate {
    validators: Vec<Box<dyn SemanticValidator>>,
}

impl SemanticGate {
    pub fn new() -> Self {
        Self {
            validators: vec![
                Box::new(ContractTestValidator),
                Box::new(SchemaValidator),
                Box::new(SemanticContractValidator),
                Box::new(StabilityValidator),
                Box::new(ErrorHandlingValidator),
            ],
        }
    }

    pub fn with_validators(mut self, validators: Vec<Box<dyn SemanticValidator>>) -> Self {
        self.validators = validators;
        self
    }

    pub fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, SemanticGateError> {
        let mut aggregated = ValidationResult::new();

        for validator in &self.validators {
            match validator.validate(context) {
                Ok(result) => {
                    aggregated.merge(result);
                }
                Err(e) => {
                    aggregated.add_issue(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        code: "G1-INT-001".to_string(),
                        path: validator.validator_name().to_string(),
                        message: format!("驗證器執行失敗: {}", e),
                        suggestion: Some("檢查驗證器實作與輸入上下文".to_string()),
                        evidence_refs: vec![],
                    });
                }
            }
        }

        Ok(aggregated)
    }
}

impl Default for SemanticGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_merge() {
        let mut r1 = ValidationResult::new();
        r1.add_issue(ValidationIssue {
            severity: ValidationSeverity::Error,
            code: "TEST-001".to_string(),
            path: "test.rs".to_string(),
            message: "test error".to_string(),
            suggestion: None,
            evidence_refs: vec![],
        });

        let mut r2 = ValidationResult::new();
        r2.add_issue(ValidationIssue {
            severity: ValidationSeverity::Warning,
            code: "TEST-002".to_string(),
            path: "test2.rs".to_string(),
            message: "test warning".to_string(),
            suggestion: None,
            evidence_refs: vec![],
        });
        r2.passed = true;

        r1.merge(r2);
        assert!(!r1.passed);
        assert_eq!(r1.issues.len(), 2);
    }

    #[test]
    fn test_validation_context_new() {
        let ctx = ValidationContext::new("/tmp/test".to_string());
        assert_eq!(ctx.project_root, "/tmp/test");
        assert!(ctx.changed_files.is_empty());
    }

    #[test]
    fn test_semantic_gate_creation() {
        let gate = SemanticGate::new();
        assert_eq!(gate.validators.len(), 5);
    }

    #[test]
    fn test_contract_test_validator() {
        let validator = ContractTestValidator;
        let ctx = ValidationContext::new("/tmp".to_string());
        let result = validator.validate(&ctx).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_schema_validator() {
        let validator = SchemaValidator;
        let mut ctx = ValidationContext::new("/tmp".to_string());
        ctx.openapi_schemas.push(OpenApiSchema {
            schema_name: "test".to_string(),
            schema_json: serde_json::json!({"type": "object"}),
            version: "1.0".to_string(),
            endpoints: vec!["/api/test".to_string()],
        });
        let result = validator.validate(&ctx).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_stability_validator_no_breaking_changes() {
        let validator = StabilityValidator;
        let mut ctx = ValidationContext::new("/tmp".to_string());
        ctx.stability_checks.push(StabilityCheck {
            interface_name: "test_api".to_string(),
            interface_type: InterfaceType::RestApi,
            current_version: "1.0".to_string(),
            breaking_changes: vec![],
            deprecated_items: vec![],
        });
        let result = validator.validate(&ctx).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_stability_validator_with_breaking_changes() {
        let validator = StabilityValidator;
        let mut ctx = ValidationContext::new("/tmp".to_string());
        ctx.stability_checks.push(StabilityCheck {
            interface_name: "test_api".to_string(),
            interface_type: InterfaceType::RestApi,
            current_version: "2.0".to_string(),
            breaking_changes: vec!["removed_field".to_string()],
            deprecated_items: vec![],
        });
        let result = validator.validate(&ctx).unwrap();
        assert!(!result.passed);
        assert_eq!(result.issues[0].code, "G1-ST-001");
    }

    #[test]
    fn test_semantic_gate_error_serialization() {
        let err = SemanticGateError::MissingContractTest {
            path: "test.rs".to_string(),
            detail: "missing test".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("MissingContractTest"));
        assert!(json.contains("test.rs"));
    }
}
