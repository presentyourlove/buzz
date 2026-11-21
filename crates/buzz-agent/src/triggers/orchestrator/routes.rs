//! Orchestrator Axum Router
//!
//! 建立完整的 REST API 路由，包含 CORS、追蹤、優雅關機

use crate::triggers::orchestrator::{api, state::AppState};
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::Level;

/// 建立 Orchestrator API Router
pub fn create_router(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Trigger CRUD
        .route("/api/triggers", post(api::create_trigger))
        .route("/api/triggers", get(api::list_triggers))
        .route("/api/triggers/{id}", get(api::get_trigger))
        .route("/api/triggers/{id}", put(api::update_trigger))
        .route("/api/triggers/{id}", delete(api::delete_trigger))
        // Manual trigger
        .route("/api/triggers/{id}/trigger", post(api::manual_trigger))
        // Health endpoints
        .route("/api/triggers/{id}/health", get(api::get_trigger_health))
        .route("/health", get(api::health_check))
        // Realtime Agent & Task APIs (Dual-Track A + B)
        .route("/api/v1/agents", get(api::list_agents))
        .route("/api/v1/agents/{id}/status", patch(api::update_agent_status))
        .route("/api/v1/tasks", get(api::list_tasks))
        .route("/ws", get(api::ws_dashboard_handler))
        // State & middleware
        .with_state(app_state)
        .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<axum::body::Body>| {
            tracing::span!(
                Level::INFO,
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                version = ?request.version(),
            )
        }))
        .layer(cors)
}
