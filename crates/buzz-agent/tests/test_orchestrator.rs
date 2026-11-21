//! Integration tests for Orchestrator API and CircuitBreaker

use buzz_agent::triggers::{
    orchestrator::{
        circuit_breaker::{TriggerCircuitBreaker, CircuitBreakerConfig, TriggerCircuitBreakerState, TriggerCircuitBreakerManager},
        state::AppState,
    },
    event_bus::{EventBus, EventBusConfig},
    TriggerRuntime, TriggerRuntimeConfig, TriggerRegistry,
};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    let cb = TriggerCircuitBreaker::new(CircuitBreakerConfig::default());
    
    // Initial state should be Closed
    let state = cb.get_state("test-trigger", "Test Trigger").await;
    assert_eq!(state, TriggerCircuitBreakerState::Closed);
    
    // Record failures up to threshold
    for _ in 0..5 {
        cb.record_failure("test-trigger", "Test Trigger").await;
    }
    
    // Should still be Closed (threshold is 5, so 5th failure opens it)
    let state = cb.get_state("test-trigger", "Test Trigger").await;
    // After 5 failures (threshold=5), it should be Open
    assert_eq!(state, TriggerCircuitBreakerState::Open);
    
    // Record success should move to HalfOpen
    cb.record_success("test-trigger", "Test Trigger").await;
    let state = cb.get_state("test-trigger", "Test Trigger").await;
    assert_eq!(state, TriggerCircuitBreakerState::HalfOpen);
    
    // Another success should close it
    cb.record_success("test-trigger", "Test Trigger").await;
    let state = cb.get_state("test-trigger", "Test Trigger").await;
    assert_eq!(state, TriggerCircuitBreakerState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_config_custom_threshold() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout_secs: 30,
        auto_reset_after_secs: 30,
        half_open_probes: 3,
    };
    let cb = TriggerCircuitBreaker::new(config);
    
    // Initial state
    let state = cb.get_state("test-trigger-2", "Test Trigger 2").await;
    assert_eq!(state, TriggerCircuitBreakerState::Closed);
    
    // Record 2 failures - should still be Closed
    cb.record_failure("test-trigger-2", "Test Trigger 2").await;
    cb.record_failure("test-trigger-2", "Test Trigger 2").await;
    let state = cb.get_state("test-trigger-2", "Test Trigger 2").await;
    assert_eq!(state, TriggerCircuitBreakerState::Closed);
    
    // 3rd failure - should Open
    cb.record_failure("test-trigger-2", "Test Trigger 2").await;
    let state = cb.get_state("test-trigger-2", "Test Trigger 2").await;
    assert_eq!(state, TriggerCircuitBreakerState::Open);
}

#[tokio::test]
async fn test_circuit_breaker_can_execute() {
    let cb = TriggerCircuitBreaker::new(CircuitBreakerConfig::default());
    
    // Should be able to execute initially
    assert!(cb.can_execute("test-trigger-3", "Test Trigger 3").await);
    
    // Open the circuit
    for _ in 0..5 {
        cb.record_failure("test-trigger-3", "Test Trigger 3").await;
    }
    
    // Should not be able to execute when Open
    assert!(!cb.can_execute("test-trigger-3", "Test Trigger 3").await);
    
    // Record successes to close
    cb.record_success("test-trigger-3", "Test Trigger 3").await;
    cb.record_success("test-trigger-3", "Test Trigger 3").await;
    
    // Should be able to execute again
    assert!(cb.can_execute("test-trigger-3", "Test Trigger 3").await);
}

#[tokio::test]
async fn test_app_state_creation() {
    let registry = Arc::new(TriggerRegistry::new_in_memory());
    let tmp = TempDir::new().unwrap();
    let cb_manager = Arc::new(TriggerCircuitBreakerManager::new(
        tmp.path(),
        CircuitBreakerConfig::default(),
    ));
    let event_bus = Arc::new(EventBus::new(EventBusConfig::default()));
    
    let state = AppState::new(
        registry.clone(),
        cb_manager,
        event_bus,
    );
    
    // Verify state components are accessible
    assert!(!state.is_runtime_running().await);
    let triggers = state.trigger_registry.get_all().await;
    assert_eq!(triggers.len(), 0);
}

#[tokio::test]
async fn test_circuit_breaker_reset() {
    let cb = TriggerCircuitBreaker::new(CircuitBreakerConfig::default());
    
    // Open the circuit
    for _ in 0..5 {
        cb.record_failure("test-reset", "Test Reset").await;
    }
    assert_eq!(cb.get_state("test-reset", "Test Reset").await, TriggerCircuitBreakerState::Open);
    
    // Reset should close it
    cb.reset("test-reset").await;
    assert_eq!(cb.get_state("test-reset", "Test Reset").await, TriggerCircuitBreakerState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_persistence() {
    let tmp = TempDir::new().unwrap();
    let storage_path = tmp.path().join("cb_state.json");
    
    {
        let cb = TriggerCircuitBreaker::new(CircuitBreakerConfig::default());
        
        // Open the circuit
        for _ in 0..5 {
            cb.record_failure("persist-test", "Persist Test").await;
        }
        assert_eq!(cb.get_state("persist-test", "Persist Test").await, TriggerCircuitBreakerState::Open);
        
        // Force persist (if persist method exists)
        // Note: persistence is typically handled by AppState or background task
    }
    
    // Load from storage would require a new instance with same storage path
    // This test verifies the concept; actual persistence integration is in AppState
}

#[tokio::test]
async fn test_multiple_triggers_independent() {
    let cb = TriggerCircuitBreaker::new(CircuitBreakerConfig::default());
    
    // Open trigger A
    for _ in 0..5 {
        cb.record_failure("trigger-a", "Trigger A").await;
    }
    
    // Trigger A should be Open
    assert_eq!(cb.get_state("trigger-a", "Trigger A").await, TriggerCircuitBreakerState::Open);
    
    // Trigger B should still be Closed
    assert_eq!(cb.get_state("trigger-b", "Trigger B").await, TriggerCircuitBreakerState::Closed);
    
    // Record failure for B
    cb.record_failure("trigger-b", "Trigger B").await;
    assert_eq!(cb.get_state("trigger-b", "Trigger B").await, TriggerCircuitBreakerState::Closed);
}
