//! Health Dashboard Module
//! Provides HTTP endpoints for monitoring System Health Guardian status

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
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
use crate::health_analyzer::{AnalysisResult, CircuitBreakerStatus};
use crate::health_diagnoser::{HealthCheckResult, OverallStatus, ProcessStatus};
use crate::health_guardian::{ConsecutiveFailures, run_health_check_once};
use crate::health_reporter::{build_report, render_json, render_markdown, HealthReport};

/// Shared state for the health dashboard
#[derive(Clone)]
pub struct DashboardState {
    config: Arc<Config>,
    last_report: Arc<RwLock<Option<HealthReport>>>,
    consecutive_failures: Arc<RwLock<ConsecutiveFailures>>,
    server_handle: Option<std::sync::Arc<tokio::task::JoinHandle<()>>>,
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
        
        self.server_handle = Some(std::sync::Arc::new(handle));
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
                "total": r.checks.background_processes.len(),
                "healthy": r.checks.background_processes.iter().filter(|p| p.alive).count(),
            },
            "failures_count": r.failures.len(),
            "circuit_breakers": r.circuit_breakers.len(),
        })).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "no health data"}))).into_response(),
    }
}

async fn health_markdown_endpoint(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );

    match report {
        Some(r) => {
            let markdown = render_markdown(&r);
            (StatusCode::OK, headers, markdown).into_response()
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            headers,
            "No health data available".to_string(),
        )
            .into_response(),
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
    
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    
    match hg {
        Some(cfg) => (headers, Json(serde_json::json!({
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
        }))).into_response(),
        None => (headers, Json(serde_json::json!({"enabled": false}))).into_response(),
    }
}

async fn dashboard_html(State(state): State<DashboardState>) -> impl IntoResponse {
    let report = state.get_report().await;
    let failures = state.get_consecutive_failures().await;
    
    let (status_class, status_text) = match report.as_ref().map(|r| r.overall_status.clone()) {
        Some(OverallStatus::Healthy) => ("healthy", "Healthy"),
        Some(OverallStatus::Degraded) => ("degraded", "Degraded"),
        Some(OverallStatus::Critical) => ("critical", "Critical"),
        None => ("unknown", "Unknown"),
    };
    
    let processes_html = generate_processes_html(report.as_ref());
    let circuit_breakers_html = generate_circuit_breakers_html(report.as_ref());
    let timestamp = escape_html(report.as_ref().map(|r| r.timestamp.as_str()).unwrap_or("N/A"));
    let failure_count = report.as_ref().map(|r| r.failures.len()).unwrap_or(0);
    let consecutive_count = failures.count;
    
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Buzz Agent Health Dashboard</title>
    <style>
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f5f5f5; padding: 20px; }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        h1 {{ color: #333; margin-bottom: 10px; }}
        .status-badge {{ display: inline-block; padding: 6px 12px; border-radius: 4px; font-weight: bold; text-transform: uppercase; }}
        .status-healthy {{ background: #d4edda; color: #155724; }}
        .status-degraded {{ background: #fff3cd; color: #856404; }}
        .status-critical {{ background: #f8d7da; color: #721c24; }}
        .status-unknown {{ background: #e2e3e5; color: #383d41; }}
        .card {{ background: white; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); margin-bottom: 20px; overflow: hidden; }}
        .card-header {{ padding: 15px 20px; border-bottom: 1px solid #eee; font-weight: 600; }}
        .card-body {{ padding: 20px; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #eee; }}
        th {{ background: #f8f9fa; font-weight: 600; }}
        tr:hover {{ background: #f8f9fa; }}
        .process-alive {{ color: #28a745; }}
        .process-dead {{ color: #dc3545; }}
        .port-open {{ color: #28a745; }}
        .port-closed {{ color: #dc3545; }}
        .meta {{ color: #6c757d; font-size: 0.9em; margin-top: 10px; }}
        .auto-refresh {{ margin-top: 20px; padding: 15px; background: #e7f3ff; border-radius: 4px; text-align: center; color: #0056b3; }}
    </style>
    <script>
        setTimeout(() => window.location.reload(), 30000);
    </script>
</head>
<body>
    <div class="container">
        <h1>Buzz Agent Health Dashboard</h1>
        <div class="meta">Last updated: {timestamp} | Consecutive failures: {consecutive_count} | Total failures: {failure_count}</div>
        <span class="status-badge status-{status_class}">{status_text}</span>
        
        <div class="card">
            <div class="card-header">Background Processes</div>
            <div class="card-body">{processes_html}</div>
        </div>
        
        <div class="card">
            <div class="card-header">Circuit Breakers</div>
            <div class="card-body">{circuit_breakers_html}</div>
        </div>
        
        <div class="auto-refresh">Auto-refreshing every 30 seconds...</div>
    </div>
</body>
</html>"#
    );
    
    Html(html).into_response()
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn generate_processes_html(report: Option<&HealthReport>) -> String {
    let Some(report) = report else { return "No data".to_string(); };
    if report.checks.background_processes.is_empty() {
        return "No background processes configured".to_string();
    }
    let mut html = String::from("<table><thead><tr><th>Process</th><th>Status</th><th>PID</th><th>Port</th><th>Listening</th><th>Details</th></tr></thead><tbody>");
    for p in &report.checks.background_processes {
        let status_class = if p.alive { "process-alive" } else { "process-dead" };
        let status_text = if p.alive { "Running" } else { "Stopped" };
        let port = p.expected_port.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
        let listening = if p.port_listening { "Yes" } else { "No" };
        html.push_str(&format!(
            "<tr><td>{}</td><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&p.name), status_class, status_text, p.pid.unwrap_or(0), port, listening, escape_html(&p.details)
        ));
    }
    html.push_str("</tbody></table>");
    html
}

fn generate_circuit_breakers_html(report: Option<&HealthReport>) -> String {
    let Some(report) = report else { return "No data".to_string(); };
    if report.circuit_breakers.is_empty() {
        return "No circuit breakers triggered".to_string();
    }
    let mut html = String::from("<table><thead><tr><th>Tripped</th><th>Failure Count</th><th>Last Failure</th><th>Reason</th></tr></thead><tbody>");
    for cb in &report.circuit_breakers {
        let tripped_text = if cb.tripped { "Yes" } else { "No" };
        let last_failure = escape_html(cb.last_failure.as_deref().unwrap_or("-"));
        let reason = escape_html(cb.reason.as_deref().unwrap_or("-"));
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            tripped_text, cb.failure_count, last_failure, reason
        ));
    }
    html.push_str("</tbody></table>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html_prevents_xss() {
        let malicious = "<script>alert('xss')</script>&\"";
        assert_eq!(
            escape_html(malicious),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;&amp;&quot;"
        );
    }
}
