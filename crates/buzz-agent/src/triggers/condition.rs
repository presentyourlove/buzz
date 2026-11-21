//! Condition DSL - Declarative trigger conditions with boolean logic
//!
//! Provides an AST-based Condition DSL with JSON serialization,
//! short-circuit boolean evaluation, and pluggable fact providers.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

/// Evaluation errors for Condition DSL
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum EvaluationError {
    #[error("Fact provider not found: {0}")]
    ProviderNotFound(String),
    #[error("Fact not available: {0}")]
    MissingFact(String),
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
    #[error("Operator not supported: {0}")]
    OperatorNotSupported(String),
    #[error("Invalid comparison: {0}")]
    InvalidComparison(String),
    #[error("Evaluation error: {0}")]
    Other(String),
}

pub type ConditionError = EvaluationError;

/// Comparison operators for atomic conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    #[serde(alias = "Eq", alias = "eq", alias = "Equals")]
    Eq,
    #[serde(alias = "Ne", alias = "ne", alias = "NotEquals")]
    Ne,
    #[serde(alias = "Gt", alias = "gt", alias = "GreaterThan")]
    Gt,
    #[serde(alias = "Gte", alias = "gte", alias = "GreaterThanOrEqual")]
    Gte,
    #[serde(alias = "Lt", alias = "lt", alias = "LessThan")]
    Lt,
    #[serde(alias = "Lte", alias = "lte", alias = "LessThanOrEqual")]
    Lte,
    #[serde(alias = "Contains", alias = "contains")]
    Contains,
    #[serde(alias = "StartsWith", alias = "starts_with")]
    StartsWith,
    #[serde(alias = "EndsWith", alias = "ends_with")]
    EndsWith,
    #[serde(alias = "Matches", alias = "matches")]
    Matches,
    #[serde(alias = "In", alias = "in")]
    In,
    #[serde(alias = "NotIn", alias = "not_in")]
    NotIn,
}

pub type Operator = ComparisonOperator;

impl ComparisonOperator {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Gt | Self::Gte | Self::Lt | Self::Lte)
    }

    pub fn is_string(&self) -> bool {
        matches!(
            self,
            Self::Contains | Self::StartsWith | Self::EndsWith | Self::Matches
        )
    }

    pub fn is_collection(&self) -> bool {
        matches!(self, Self::In | Self::NotIn)
    }
}

/// An atomic condition comparing a fact to an expected value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicCondition {
    #[serde(default)]
    pub provider: String,
    pub fact: String,
    pub operator: ComparisonOperator,
    pub value: serde_json::Value,
}

impl AtomicCondition {
    pub fn new(fact: impl Into<String>, operator: ComparisonOperator, value: serde_json::Value) -> Self {
        Self {
            provider: String::new(),
            fact: fact.into(),
            operator,
            value,
        }
    }

    pub fn with_provider(
        provider: impl Into<String>,
        fact: impl Into<String>,
        operator: ComparisonOperator,
        value: serde_json::Value,
    ) -> Self {
        Self {
            provider: provider.into(),
            fact: fact.into(),
            operator,
            value,
        }
    }
}

/// The core AST node for Condition DSL
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    AlwaysTrue,
    AlwaysFalse,
    Atomic(AtomicCondition),
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

#[derive(Serialize, Deserialize)]
struct ConditionDto {
    #[serde(rename = "type")]
    condition_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    provider: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    fact: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operator: Option<ComparisonOperator>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    condition: Option<Box<Condition>>,
}

impl Serialize for Condition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let dto = match self {
            Condition::AlwaysTrue => ConditionDto {
                condition_type: "always_true".to_string(),
                provider: String::new(),
                fact: String::new(),
                operator: None,
                value: serde_json::Value::Null,
                conditions: Vec::new(),
                condition: None,
            },
            Condition::AlwaysFalse => ConditionDto {
                condition_type: "always_false".to_string(),
                provider: String::new(),
                fact: String::new(),
                operator: None,
                value: serde_json::Value::Null,
                conditions: Vec::new(),
                condition: None,
            },
            Condition::Atomic(ac) => ConditionDto {
                condition_type: "atomic".to_string(),
                provider: ac.provider.clone(),
                fact: ac.fact.clone(),
                operator: Some(ac.operator),
                value: ac.value.clone(),
                conditions: Vec::new(),
                condition: None,
            },
            Condition::And(children) => ConditionDto {
                condition_type: "and".to_string(),
                provider: String::new(),
                fact: String::new(),
                operator: None,
                value: serde_json::Value::Null,
                conditions: children.clone(),
                condition: None,
            },
            Condition::Or(children) => ConditionDto {
                condition_type: "or".to_string(),
                provider: String::new(),
                fact: String::new(),
                operator: None,
                value: serde_json::Value::Null,
                conditions: children.clone(),
                condition: None,
            },
            Condition::Not(child) => ConditionDto {
                condition_type: "not".to_string(),
                provider: String::new(),
                fact: String::new(),
                operator: None,
                value: serde_json::Value::Null,
                conditions: Vec::new(),
                condition: Some(child.clone()),
            },
        };
        dto.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dto = ConditionDto::deserialize(deserializer)?;
        match dto.condition_type.as_str() {
            "always_true" => Ok(Condition::AlwaysTrue),
            "always_false" => Ok(Condition::AlwaysFalse),
            "atomic" => {
                let operator = dto.operator.ok_or_else(|| {
                    serde::de::Error::custom("Missing 'operator' field in atomic condition")
                })?;
                Ok(Condition::Atomic(AtomicCondition {
                    provider: dto.provider,
                    fact: dto.fact,
                    operator,
                    value: dto.value,
                }))
            }
            "and" => Ok(Condition::And(dto.conditions)),
            "or" => Ok(Condition::Or(dto.conditions)),
            "not" => {
                let child = dto.condition.ok_or_else(|| {
                    serde::de::Error::custom("Missing 'condition' field in not condition")
                })?;
                Ok(Condition::Not(child))
            }
            unknown => Err(serde::de::Error::custom(format!(
                "Unknown condition type: {unknown}"
            ))),
        }
    }
}

/// Alias for AST node compatibility
pub type ConditionNode = Condition;

impl Condition {
    pub fn always_true() -> Self {
        Condition::AlwaysTrue
    }

    pub fn always_false() -> Self {
        Condition::AlwaysFalse
    }

    pub fn atomic(ac: AtomicCondition) -> Self {
        Condition::Atomic(ac)
    }

    pub fn and(children: Vec<Condition>) -> Self {
        Condition::And(children)
    }

    pub fn or(children: Vec<Condition>) -> Self {
        Condition::Or(children)
    }

    pub fn not(child: Condition) -> Self {
        Condition::Not(Box::new(child))
    }

    /// Recursively collects all atomic conditions contained in this condition
    pub fn collect_atomic(&self) -> Vec<&AtomicCondition> {
        let mut list = Vec::new();
        self.collect_atomic_recursive(&mut list);
        list
    }

    fn collect_atomic_recursive<'a>(&'a self, list: &mut Vec<&'a AtomicCondition>) {
        match self {
            Condition::Atomic(ac) => list.push(ac),
            Condition::And(children) | Condition::Or(children) => {
                for c in children {
                    c.collect_atomic_recursive(list);
                }
            }
            Condition::Not(child) => child.collect_atomic_recursive(list),
            Condition::AlwaysTrue | Condition::AlwaysFalse => {}
        }
    }
}

/// Fact value alias for backwards compatibility
pub type FactValue = serde_json::Value;

/// Pluggable trait for resolving facts by key
pub trait FactProvider: Send + Sync {
    fn name(&self) -> &str;
    fn get_fact(&self, key: &str) -> Result<Option<serde_json::Value>, EvaluationError>;
}

/// Standard Evaluation Context holding fact providers and direct fact values
#[derive(Default)]
pub struct EvaluationContext {
    providers: HashMap<String, Box<dyn FactProvider>>,
    facts: HashMap<String, serde_json::Value>,
}

impl EvaluationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider<P: FactProvider + 'static>(&mut self, provider: P) {
        self.providers.insert(provider.name().to_string(), Box::new(provider));
    }

    pub fn with_provider<P: FactProvider + 'static>(mut self, provider: P) -> Self {
        self.register_provider(provider);
        self
    }

    pub fn set_fact(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.facts.insert(key.into(), value);
    }

    pub fn with_fact(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.set_fact(key, value);
        self
    }

    pub fn resolve_fact(&self, provider_name: &str, fact_name: &str) -> Result<Option<serde_json::Value>, EvaluationError> {
        if provider_name.is_empty() {
            // Check direct facts map first
            if let Some(val) = self.facts.get(fact_name) {
                return Ok(Some(val.clone()));
            }
        }

        // Check provider map
        if let Some(provider) = self.providers.get(provider_name) {
            return provider.get_fact(fact_name);
        }

        // Check prefixed key in direct facts map (e.g. "system.cpu")
        let full_key = if provider_name.is_empty() {
            fact_name.to_string()
        } else {
            format!("{}.{}", provider_name, fact_name)
        };

        if let Some(val) = self.facts.get(&full_key) {
            return Ok(Some(val.clone()));
        }

        if !provider_name.is_empty() && !self.providers.contains_key(provider_name) {
            return Err(EvaluationError::ProviderNotFound(provider_name.to_string()));
        }

        Ok(None)
    }
}

impl FactProvider for EvaluationContext {
    fn name(&self) -> &str {
        "context"
    }

    fn get_fact(&self, key: &str) -> Result<Option<serde_json::Value>, EvaluationError> {
        Ok(self.facts.get(key).cloned())
    }
}

/// Evaluator result enum for three-valued or boolean evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationResult {
    True,
    False,
    Unknown(String),
}

impl EvaluationResult {
    pub fn as_bool(&self) -> bool {
        matches!(self, EvaluationResult::True)
    }
    pub fn is_true(&self) -> bool {
        matches!(self, EvaluationResult::True)
    }
    pub fn is_false(&self) -> bool {
        matches!(self, EvaluationResult::False)
    }
}

/// Condition Evaluator supporting generic FactProvider
pub struct ConditionEvaluator<F: FactProvider> {
    provider: F,
}

impl<F: FactProvider> ConditionEvaluator<F> {
    pub fn new(provider: F) -> Self {
        Self { provider }
    }

    pub fn evaluate(&self, condition: &Condition) -> Result<bool, EvaluationError> {
        evaluate_condition_with_provider(condition, &self.provider)
    }
}

/// Primary evaluation function with short-circuiting
pub fn evaluate_condition(
    condition: &Condition,
    context: &EvaluationContext,
) -> Result<bool, EvaluationError> {
    evaluate_condition_internal(condition, context)
}

fn evaluate_condition_internal(
    condition: &Condition,
    context: &EvaluationContext,
) -> Result<bool, EvaluationError> {
    match condition {
        Condition::AlwaysTrue => Ok(true),
        Condition::AlwaysFalse => Ok(false),
        Condition::Atomic(atomic) => {
            let actual = context
                .resolve_fact(&atomic.provider, &atomic.fact)?
                .ok_or_else(|| {
                    let key = if atomic.provider.is_empty() {
                        atomic.fact.clone()
                    } else {
                        format!("{}.{}", atomic.provider, atomic.fact)
                    };
                    EvaluationError::MissingFact(key)
                })?;
            compare_json_values(&actual, atomic.operator, &atomic.value)
        }
        Condition::And(children) => {
            for child in children {
                // Short-circuit: return false immediately
                if !evaluate_condition_internal(child, context)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Or(children) => {
            for child in children {
                // Short-circuit: return true immediately
                if evaluate_condition_internal(child, context)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Not(child) => {
            let res = evaluate_condition_internal(child, context)?;
            Ok(!res)
        }
    }
}

fn evaluate_condition_with_provider<F: FactProvider>(
    condition: &Condition,
    provider: &F,
) -> Result<bool, EvaluationError> {
    match condition {
        Condition::AlwaysTrue => Ok(true),
        Condition::AlwaysFalse => Ok(false),
        Condition::Atomic(atomic) => {
            let actual = provider
                .get_fact(&atomic.fact)?
                .ok_or_else(|| EvaluationError::MissingFact(atomic.fact.clone()))?;
            compare_json_values(&actual, atomic.operator, &atomic.value)
        }
        Condition::And(children) => {
            for child in children {
                if !evaluate_condition_with_provider(child, provider)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Or(children) => {
            for child in children {
                if evaluate_condition_with_provider(child, provider)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Not(child) => {
            let res = evaluate_condition_with_provider(child, provider)?;
            Ok(!res)
        }
    }
}

/// Compares two JSON values according to the comparison operator
pub fn compare_json_values(
    actual: &serde_json::Value,
    op: ComparisonOperator,
    expected: &serde_json::Value,
) -> Result<bool, EvaluationError> {
    match op {
        ComparisonOperator::Eq => Ok(values_equal(actual, expected)),
        ComparisonOperator::Ne => Ok(!values_equal(actual, expected)),
        ComparisonOperator::Gt => {
            let (a, b) = extract_numbers(actual, expected)?;
            Ok(a > b)
        }
        ComparisonOperator::Gte => {
            let (a, b) = extract_numbers(actual, expected)?;
            Ok(a >= b)
        }
        ComparisonOperator::Lt => {
            let (a, b) = extract_numbers(actual, expected)?;
            Ok(a < b)
        }
        ComparisonOperator::Lte => {
            let (a, b) = extract_numbers(actual, expected)?;
            Ok(a <= b)
        }
        ComparisonOperator::Contains => match (actual, expected) {
            (serde_json::Value::String(s), serde_json::Value::String(sub)) => Ok(s.contains(sub)),
            (serde_json::Value::Array(arr), exp) => Ok(arr.iter().any(|item| values_equal(item, exp))),
            _ => Err(EvaluationError::TypeMismatch {
                expected: "string or array".into(),
                actual: value_type_name(actual).into(),
            }),
        },
        ComparisonOperator::StartsWith => match (actual, expected) {
            (serde_json::Value::String(s), serde_json::Value::String(prefix)) => Ok(s.starts_with(prefix)),
            _ => Err(EvaluationError::TypeMismatch {
                expected: "string".into(),
                actual: value_type_name(actual).into(),
            }),
        },
        ComparisonOperator::EndsWith => match (actual, expected) {
            (serde_json::Value::String(s), serde_json::Value::String(suffix)) => Ok(s.ends_with(suffix)),
            _ => Err(EvaluationError::TypeMismatch {
                expected: "string".into(),
                actual: value_type_name(actual).into(),
            }),
        },
        ComparisonOperator::Matches => match (actual, expected) {
            (serde_json::Value::String(s), serde_json::Value::String(pat)) => {
                if pat.starts_with('^') && pat.ends_with('$') {
                    Ok(s == &pat[1..pat.len() - 1])
                } else if pat.starts_with('^') {
                    Ok(s.starts_with(&pat[1..]))
                } else if pat.ends_with('$') {
                    Ok(s.ends_with(&pat[..pat.len() - 1]))
                } else {
                    Ok(s.contains(pat))
                }
            }
            _ => Err(EvaluationError::TypeMismatch {
                expected: "string".into(),
                actual: value_type_name(actual).into(),
            }),
        },
        ComparisonOperator::In => match expected {
            serde_json::Value::Array(arr) => Ok(arr.iter().any(|item| values_equal(item, actual))),
            _ => Err(EvaluationError::TypeMismatch {
                expected: "array for In operator".into(),
                actual: value_type_name(expected).into(),
            }),
        },
        ComparisonOperator::NotIn => match expected {
            serde_json::Value::Array(arr) => Ok(!arr.iter().any(|item| values_equal(item, actual))),
            _ => Err(EvaluationError::TypeMismatch {
                expected: "array for NotIn operator".into(),
                actual: value_type_name(expected).into(),
            }),
        },
    }
}

fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    if a == b {
        return true;
    }
    // Allow numeric coercion between float and integer (e.g. 10.0 == 10)
    if let (Some(na), Some(nb)) = (a.as_f64(), b.as_f64()) {
        return (na - nb).abs() < f64::EPSILON;
    }
    false
}

fn extract_numbers(
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Result<(f64, f64), EvaluationError> {
    let na = a.as_f64().ok_or_else(|| EvaluationError::TypeMismatch {
        expected: "number".into(),
        actual: value_type_name(a).into(),
    })?;
    let nb = b.as_f64().ok_or_else(|| EvaluationError::TypeMismatch {
        expected: "number".into(),
        actual: value_type_name(b).into(),
    })?;
    Ok((na, nb))
}

fn value_type_name(val: &serde_json::Value) -> &'static str {
    match val {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Simple fact provider for testing and basic use cases
#[derive(Debug, Clone, Default)]
pub struct SimpleProvider;

impl FactProvider for SimpleProvider {
    fn name(&self) -> &str {
        "simple"
    }
    fn get_fact(&self, key: &str) -> Result<Option<serde_json::Value>, EvaluationError> {
        match key {
            "ping" => Ok(Some(json!("pong"))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_condition_json_roundtrip() {
        let cond = Condition::and(vec![
            Condition::always_true(),
            Condition::atomic(AtomicCondition::with_provider(
                "system",
                "cpu.usage",
                ComparisonOperator::Gt,
                json!(85.5),
            )),
            Condition::not(Condition::atomic(AtomicCondition::new(
                "mode",
                ComparisonOperator::Eq,
                json!("idle"),
            ))),
        ]);

        let serialized = serde_json::to_string(&cond).expect("Serialization failed");
        let deserialized: Condition =
            serde_json::from_str(&serialized).expect("Deserialization failed");

        assert_eq!(cond, deserialized);
    }

    #[test]
    fn test_always_true_and_false() {
        let ctx = EvaluationContext::new();
        assert!(evaluate_condition(&Condition::always_true(), &ctx).unwrap());
        assert!(!evaluate_condition(&Condition::always_false(), &ctx).unwrap());
    }

    #[test]
    fn test_atomic_equality_operators() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("status", json!("ready"));
        ctx.set_fact("count", json!(42));

        let cond_eq = Condition::atomic(AtomicCondition::new(
            "status",
            ComparisonOperator::Eq,
            json!("ready"),
        ));
        assert!(evaluate_condition(&cond_eq, &ctx).unwrap());

        let cond_ne = Condition::atomic(AtomicCondition::new(
            "status",
            ComparisonOperator::Ne,
            json!("busy"),
        ));
        assert!(evaluate_condition(&cond_ne, &ctx).unwrap());

        let cond_num_eq = Condition::atomic(AtomicCondition::new(
            "count",
            ComparisonOperator::Eq,
            json!(42.0),
        ));
        assert!(evaluate_condition(&cond_num_eq, &ctx).unwrap());
    }

    #[test]
    fn test_numeric_comparison_operators() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("cpu", json!(75.0));

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("cpu", ComparisonOperator::Gt, json!(70.0))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("cpu", ComparisonOperator::Gte, json!(75.0))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("cpu", ComparisonOperator::Lt, json!(80.0))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("cpu", ComparisonOperator::Lte, json!(75.0))),
            &ctx
        ).unwrap());

        assert!(!evaluate_condition(
            &Condition::atomic(AtomicCondition::new("cpu", ComparisonOperator::Gt, json!(80.0))),
            &ctx
        ).unwrap());
    }

    #[test]
    fn test_string_operators() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("filename", json!("backup_20261118.tar.gz"));

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("filename", ComparisonOperator::Contains, json!("20261118"))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("filename", ComparisonOperator::StartsWith, json!("backup_"))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("filename", ComparisonOperator::EndsWith, json!(".gz"))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("filename", ComparisonOperator::Matches, json!("backup"))),
            &ctx
        ).unwrap());
    }

    #[test]
    fn test_collection_operators() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("role", json!("admin"));
        ctx.set_fact("tags", json!(["urgent", "security", "infra"]));

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("role", ComparisonOperator::In, json!(["guest", "admin"]))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("role", ComparisonOperator::NotIn, json!(["guest", "viewer"]))),
            &ctx
        ).unwrap());

        assert!(evaluate_condition(
            &Condition::atomic(AtomicCondition::new("tags", ComparisonOperator::Contains, json!("security"))),
            &ctx
        ).unwrap());
    }

    #[test]
    fn test_and_short_circuit() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("a", json!(1));

        // First is false, second fact does not exist (missing fact should not error if short-circuited)
        let cond = Condition::and(vec![
            Condition::always_false(),
            Condition::atomic(AtomicCondition::new("non_existent_fact", ComparisonOperator::Eq, json!(1))),
        ]);

        let result = evaluate_condition(&cond, &ctx).expect("Should short-circuit cleanly");
        assert!(!result);
    }

    #[test]
    fn test_or_short_circuit() {
        let ctx = EvaluationContext::new();

        // First is true, second fact does not exist
        let cond = Condition::or(vec![
            Condition::always_true(),
            Condition::atomic(AtomicCondition::new("non_existent_fact", ComparisonOperator::Eq, json!(1))),
        ]);

        let result = evaluate_condition(&cond, &ctx).expect("Should short-circuit cleanly");
        assert!(result);
    }

    #[test]
    fn test_not_negation() {
        let ctx = EvaluationContext::new();
        let not_true = Condition::not(Condition::always_true());
        let not_false = Condition::not(Condition::always_false());

        assert!(!evaluate_condition(&not_true, &ctx).unwrap());
        assert!(evaluate_condition(&not_false, &ctx).unwrap());
    }

    #[test]
    fn test_nested_compound_conditions() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("battery", json!(15));
        ctx.set_fact("plugged_in", json!(false));
        ctx.set_fact("user_active", json!(false));

        // (battery < 20 AND NOT plugged_in) AND NOT user_active
        let low_battery = Condition::atomic(AtomicCondition::new("battery", ComparisonOperator::Lt, json!(20)));
        let not_plugged = Condition::not(Condition::atomic(AtomicCondition::new("plugged_in", ComparisonOperator::Eq, json!(true))));
        let not_active = Condition::not(Condition::atomic(AtomicCondition::new("user_active", ComparisonOperator::Eq, json!(true))));

        let alert_cond = Condition::and(vec![
            Condition::and(vec![low_battery, not_plugged]),
            not_active,
        ]);

        assert!(evaluate_condition(&alert_cond, &ctx).unwrap());
    }

    #[test]
    fn test_missing_fact_error() {
        let ctx = EvaluationContext::new();
        let cond = Condition::atomic(AtomicCondition::new("unknown_metric", ComparisonOperator::Eq, json!(10)));
        let err = evaluate_condition(&cond, &ctx).unwrap_err();
        assert!(matches!(err, EvaluationError::MissingFact(_)));
    }

    #[test]
    fn test_type_mismatch_error() {
        let mut ctx = EvaluationContext::new();
        ctx.set_fact("text", json!("hello"));

        let cond = Condition::atomic(AtomicCondition::new("text", ComparisonOperator::Gt, json!(100)));
        let err = evaluate_condition(&cond, &ctx).unwrap_err();
        assert!(matches!(err, EvaluationError::TypeMismatch { .. }));
    }

    #[test]
    fn test_custom_fact_provider() {
        struct MockSystemProvider;
        impl FactProvider for MockSystemProvider {
            fn name(&self) -> &str {
                "sys"
            }
            fn get_fact(&self, key: &str) -> Result<Option<serde_json::Value>, EvaluationError> {
                match key {
                    "load_avg" => Ok(Some(json!(1.85))),
                    "hostname" => Ok(Some(json!("buzz-node-01"))),
                    _ => Ok(None),
                }
            }
        }

        let ctx = EvaluationContext::new().with_provider(MockSystemProvider);

        let cond = Condition::atomic(AtomicCondition::with_provider(
            "sys",
            "load_avg",
            ComparisonOperator::Gt,
            json!(1.0),
        ));

        assert!(evaluate_condition(&cond, &ctx).unwrap());
    }


    #[test]
    fn test_condition_evaluator_struct() {
        let evaluator = ConditionEvaluator::new(SimpleProvider);
        let cond = Condition::atomic(AtomicCondition::new("ping", ComparisonOperator::Eq, json!("pong")));
        assert!(evaluator.evaluate(&cond).unwrap());
    }

    #[test]
    fn test_collect_atomic() {
        let c1 = AtomicCondition::new("a", ComparisonOperator::Eq, json!(1));
        let c2 = AtomicCondition::new("b", ComparisonOperator::Gt, json!(2));
        let c3 = AtomicCondition::new("c", ComparisonOperator::Lt, json!(3));

        let tree = Condition::and(vec![
            Condition::atomic(c1.clone()),
            Condition::or(vec![
                Condition::atomic(c2.clone()),
                Condition::not(Condition::atomic(c3.clone())),
            ]),
        ]);

        let collected = tree.collect_atomic();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].fact, "a");
        assert_eq!(collected[1].fact, "b");
        assert_eq!(collected[2].fact, "c");
    }
}
