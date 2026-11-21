//! Integration tests for Condition DSL in buzz-agent
//!
//! Validates AST structure, JSON serialization, comparison operators,
//! short-circuit boolean evaluation, and pluggable fact providers.

use buzz_agent::triggers::condition::{
    AtomicCondition, ComparisonOperator, Condition, ConditionEvaluator, EvaluationContext,
    EvaluationError, FactProvider, evaluate_condition,
};
use serde_json::json;

#[test]
fn test_condition_json_roundtrip() {
    let cond = Condition::and(vec![
        Condition::atomic(AtomicCondition::new(
            "system.cpu_percent",
            ComparisonOperator::Gt,
            json!(85.0),
        )),
        Condition::or(vec![
            Condition::atomic(AtomicCondition::new(
                "process.status",
                ComparisonOperator::Eq,
                json!("unhealthy"),
            )),
            Condition::not(Condition::atomic(AtomicCondition::new(
                "system.free_disk_gb",
                ComparisonOperator::Gte,
                json!(10.0),
            ))),
        ]),
    ]);

    let serialized = serde_json::to_string_pretty(&cond).expect("Serialization failed");
    let deserialized: Condition =
        serde_json::from_str(&serialized).expect("Deserialization failed");

    assert_eq!(cond, deserialized);
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
    struct SimpleProvider;
    impl FactProvider for SimpleProvider {
        fn name(&self) -> &str {
            "simple"
        }
        fn get_fact(&self, key: &str) -> Result<Option<serde_json::Value>, EvaluationError> {
            if key == "ping" {
                Ok(Some(json!("pong")))
            } else {
                Ok(None)
            }
        }
    }

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
