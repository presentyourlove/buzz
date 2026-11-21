//! Orchestrator Module
//!
//! Phase 4: proactive-trigger-orchestrator 完整實作
//! 包含 CRUD REST API、CircuitBreaker 熔斷機制、AppState 整合

pub mod api;
pub mod circuit_breaker;
pub mod routes;
pub mod state;

// Re-export key types
pub use api::{
    CreateTriggerRequest, UpdateTriggerRequest, ListTriggersParams, ListTriggersQuery,
    TriggerResponse, TriggerHealthResponse, HealthResponse, ErrorResponse,
};
pub use circuit_breaker::{CircuitBreakerConfig, TriggerCircuitBreakerManager, TriggerCircuitBreakerState};
pub use crate::health_analyzer::CircuitBreakerStatus;
pub use crate::triggers::types::{Trigger, TriggerType, TriggerStatus, BusEvent};
pub use state::AppState;
pub use routes::create_router;
