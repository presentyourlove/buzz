//! Health Dashboard Module
//! Provides HTTP endpoints for monitoring System Health Guardian status

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::config::Config;
use crate::health_analyzer::{AnalysisResult, CircuitBreakerFile};
use crate::health_diagnoser::{HealthCheckResult, OverallStatus};
use crate::health_guardian::{ConsecutiveFailures, HealthReport, run_health_check_once};
use crate::health_reporter::{build_report, render_json, render_markdown};

/// Shared state for the health dashboard
#[derive(Clone)]
pub struct DashboardState {
    config: Arc<Config>,
    last_report: Arc<RwLock<Option<HealthReport>>>,
    consecutive_failures: Arc<RwLock<ConsecutiveFailures>>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DashboardState {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            last_report: Arc::new(RwLock::new(None)),
            consecutive_failures: Arc::new(RwLock::new(ConsecutiveFailures {
                count: 0,
                last_check: String::new(),
                last_status: OverallStatus::Healthy,
            })),
            server_handle: None,
        }
    }

    pub async fn update_report(&self, report: HealthReport) {
        let mut last_report = self.last_report.write().await;
        *last_report = Some(report);
    }

    pub async fn get_report(&self) -> Option<HealthReport> {
        self.last_report.read().await.clone()
    }

    pub async fn update_consecutive_failures(&self, failures: ConsecutiveFailures) {
        let mut cf = self.consecutive_failures.write().await;
        *cf = failures;
    }

    pub async fn get_consecutive_failures(&self) -> ConsecutiveFailures {
        self.consecutive_failures.read().await.clone()
    }

    pub async fn start_server(&mut self, addr: std::net::SocketAddr) -> Result<(), std::io::Error> {
        let app = create_router(self.clone());
        
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("Health Dashboard listening on {}", addr);
        
        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Health Dashboard server error: {}", e);
            }
        });
        
        self.server_handle = Some(handle);
        Ok(())
    }

    pub async fn stop_server(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
    }
}

fn create_router(state: DashboardState) -> Router {
    Router::new()
        .route("/health", get(health_endpoint))
        .route("/health/json", get(health_json_endpoint))
        .route("/health/markdown", get(health_markdown_endpoint))
        .route("/health/report", get(full_report_endpoint))
        .route("/health/circuit-breakers", get(circuit_breakers_endpoint))
        .route("/health/consecutive-failures", get(consecutive_failures_endpoint))
        .route("/health/config", get(config_endpoint))
        .route("/dashboard", get(dashboard_html))
        .with_state(state)
}

async fn health_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    
    match report {
        Some(r) => {
            let status = match r.overall_status {
                OverallStatus::Healthy => "healthy",
                OverallStatus::Degraded => "degraded",
                OverallStatus::Critical => "critical",
            };
            (StatusCode::OK, format!("{}", status)).into_response()
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, "no health data").into_response(),
    }
}

async fn health_json_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    
    match report {
        Some(r) => Json(serde_json::json!({
            "status": format!("{:?}", r.overall_status).to_lowercase(),
            "timestamp": r.timestamp,
            "checks_summary": {
                "total": r.checks.background_processes.len() + r.checks.ports.len() + r.checks.disk.len() + r.checks.memory.len(),
                "healthy": r.checks.background_processes.iter().filter(|p| p.alive).count() + r.checks.ports.iter().filter(|p| p.open).count(),
            },
            "failures_count": r.failures.len(),
            "circuit_breakers": r.circuit_breakers.len(),
        })).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "no health data"}))).into_response(),
    }
}

async fn health_markdown_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    
    match report {
        Some(r) => {
            let markdown = render_markdown(&r).unwrap_or_else(|_| "Failed to render report".to_string());
            Html(markdown).into_response()
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, Html("No health data available".to_string())).into_response(),
    }
}

async fn full_report_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    
    match report {
        Some(r) => Json(r).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "no health data"}))).into_response(),
    }
}

async fn circuit_breakers_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    
    match report {
        Some(r) => Json(r.circuit_breakers).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "no health data"}))).into_response(),
    }
}

async fn consecutive_failures_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let failures = state.get_consecutive_failures().await;
    Json(failures).into_response()
}

async fn config_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let hg = &state.config.health_guardian;
    
    match hg {
        Some(cfg) => Json(serde_json::json!({
            "enabled": cfg.enabled,
            "channel_id": cfg.channel_id,
            "schedule": cfg.schedule,
            "daily_deep_schedule": cfg.daily_deep_schedule,
            "work_logs_path": cfg.work_logs_path.to_string_lossy().to_string(),
            "circuit_breaker_path": cfg.circuit_breaker_path.to_string_lossy().to_string(),
            "openrouter_port": cfg.openrouter_port,
            "proxy_port": cfg.proxy_port,
            "alert_threshold": {
                "max_consecutive_failures": cfg.alert_threshold.max_consecutive_failures,
                "circuit_breaker_auto_reset": cfg.alert_threshold.circuit_breaker_auto_reset,
                "process_restart_cooldown_secs": cfg.alert_threshold.process_restart_cooldown_secs,
            }
        })).into_response(),
        None => Json(serde_json::json!({"enabled": false})).into_response(),
    }
}

async fn dashboard_html(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    let failures = state.get_consecutive_failures().await;
    
    let (status_class, status_text) = match report.as_ref().map(|r| r.overall_status) {
        Some(OverallStatus::Healthy) => ("healthy", "Healthy"),
        Some(OverallStatus::Degraded) => ("degraded", "Degraded"),
        Some(OverallStatus::Critical) => ("critical", "Critical"),
        None => ("unknown", "Unknown"),
    };
    
    let html = format!(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Buzz Agent Health Dashboard</title>
    <style>
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, s
