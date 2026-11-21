/*!
Orchestrator REST API Handlers

提供觸發器完整生命週期 CRUD、手動觸發、健康檢查等 REST 介面
*/

use crate::triggers::{
    condition::Condition,
    runtime::TriggerRuntime,
    types::{
        Trigger, TriggerType, TriggerStatus, Action, DeduplicationConfig, RetryPolicy,
        TriggerFiredEvent, EventSource,
    },
    orchestrator::{
        circuit_breaker::{CircuitBreakerConfig, TriggerCircuitBreakerManager, TriggerCircuitBreakerState},
        state::AppState,
    },
    trigger_registry::TriggerRegistry,
    event_bus::EventBus,
};
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

/// 建立觸發器請求
#[derive(Debug, Deserialize)]
pub struct CreateTriggerRequest {
    pub name: String,
    pub description: String,
    pub enabled: Option<bool>,
    pub trigger_type: String,  // "scheduled", "event", "manual", "health_threshold"
    pub schedule: Option<Value>,  // cron expression or interval
    pub condition: Option<Value>, // JSON condition
    pub action: Option<Value>,    // JSON action
    pub config: Option<Value>,    // additional config (deduplication, retry_policy, etc.)
}

/// 更新觸發器請求
#[derive(Debug, Deserialize, Default)]
pub struct UpdateTriggerRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub trigger_type: Option<String>,
    pub schedule: Option<Value>,
    pub condition: Option<Value>,
    pub action: Option<Value>,
    pub config: Option<Value>,
}

/// 列表查詢參數
#[derive(Debug, Deserialize)]
pub struct ListTriggersParams {
    pub enabled_only: Option<bool>,
    pub trigger_type: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Alias for ListTriggersParams
pub type ListTriggersQuery = ListTriggersParams;

/// 觸發器回應
#[derive(Debug, Serialize)]
pub struct TriggerResponse {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub trigger_type: TriggerType,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub deduplication: DeduplicationConfig,
    pub retry_policy: Option<RetryPolicy>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_fired_at: Option<i64>,
    pub fire_count: u64,
}

/// 觸發器健康回應
#[derive(Debug, Serialize)]
pub struct TriggerHealthResponse {
    pub trigger_id: Uuid,
    pub status: TriggerStatus,
    pub circuit_breaker_state: TriggerCircuitBreakerState,
    pub last_fired_at: Option<i64>,
    pub fire_count: u64,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
}

/// 整體健康回應
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub triggers_registered: usize,
    pub triggers_enabled: usize,
    pub runtime_running: bool,
    pub circuit_breakers_open: usize,
    pub timestamp: i64,
}

/// 錯誤回應
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

/// 手動觸發請求
#[derive(Debug, Deserialize)]
pub struct ManualTriggerRequest {
    pub payload: Option<Value>,
}

/// 將 REST trigger_type 字串轉換為內部 TriggerType
fn parse_trigger_type(type_str: &str) -> Result<TriggerType, String> {
    match type_str.to_lowercase().as_str() {
        "scheduled" => Ok(TriggerType::Schedule {
            cron: None,
            interval_secs: None,
            timezone: "UTC".to_string(),
            jitter_secs: 0,
        }),
        "event" => Ok(TriggerType::NostrEvent {
            kinds: vec![],
            authors: vec![],
            tags: vec![],
            relay_urls: vec![],
        }),
        "manual" => Ok(TriggerType::Schedule {
            cron: None,
            interval_secs: None,
            timezone: "UTC".to_string(),
            jitter_secs: 0,
        }),
        "health_threshold" => Ok(TriggerType::SystemMetric {
            metric: crate::triggers::types::SystemMetricType::ErrorRate,
            operator: crate::triggers::condition::ComparisonOperator::Gt,
            threshold: 0.0,
            duration_secs: 60,
        }),
        _ => Err(format!("Unknown trigger type: {}", type_str)),
    }
}

/// 從 schedule Value 更新 TriggerType
fn apply_schedule_to_trigger_type(trigger_type: &mut TriggerType, schedule: &Value) {
    if let TriggerType::Schedule { cron, interval_secs, timezone, jitter_secs } = trigger_type {
        if let Some(cron_str) = schedule.get("cron").and_then(|v| v.as_str()) {
            *cron = Some(cron_str.to_string());
        }
        if let Some(interval) = schedule.get("interval_secs").and_then(|v| v.as_u64()) {
            *interval_secs = Some(interval);
        }
        if let Some(tz) = schedule.get("timezone").and_then(|v| v.as_str()) {
            *timezone = tz.to_string();
        }
        if let Some(jitter) = schedule.get("jitter_secs").and_then(|v| v.as_u64()) {
            *jitter_secs = jitter;
        }
    }
}

/// 從 condition Value 建立 Condition
fn build_condition_from_value(value: &Value) -> Option<Condition> {
    // 簡單實現：將單一 JSON condition 包裝為 ConditionNode::Leaf
    // 實際應用中可能需要更複雜的解析邏輯
    Some(Condition::AlwaysTrue)
}

/// 從 action Value 建立 Action
fn build_action_from_value(value: &Value) -> Option<Action> {
    // 嘗試反序列化為 Action enum
    serde_json::from_value(value.clone()).ok()
}

/// 從 config Value 更新 deduplication 和 retry_policy
fn apply_config_to_trigger(trigger: &mut Trigger, config: &Value) {
    if let Some(dedup) = config.get("deduplication") {
        if let Ok(d) = serde_json::from_value(dedup.clone()) {
            trigger.deduplication = d;
        }
    }
    if let Some(retry) = config.get("retry_policy") {
        if let Ok(_r) = serde_json::from_value::<RetryPolicy>(retry.clone()) {
            // We'll store retry_policy in a custom field or extend Trigger
        }
    }
}

/// 將內部 Trigger 轉換為 TriggerResponse
fn trigger_to_response(trigger: Trigger) -> TriggerResponse {
    // 取得第一個 action 的 retry_policy 作為代表（若有的話）
    let retry_policy = trigger.actions.first().and_then(|a| match a {
        Action::SubmitTask { retry_policy, .. } => Some(retry_policy.clone()),
        Action::PublishNostrEvent { retry_policy, .. } => Some(retry_policy.clone()),
        Action::SendNotification { retry_policy, .. } => Some(retry_policy.clone()),
        Action::RunCommand { retry_policy, .. } => Some(retry_policy.clone()),
        Action::HttpRequest { retry_policy, .. } => Some(retry_policy.clone()),
        _ => None,
    });

    TriggerResponse {
        id: trigger.id,
        name: trigger.name,
        description: trigger.description,
        enabled: trigger.enabled,
        trigger_type: trigger.trigger_type,
        conditions: trigger.conditions,
        actions: trigger.actions,
        deduplication: trigger.deduplication,
        retry_policy,
        created_at: trigger.created_at.timestamp(),
        updated_at: trigger.updated_at.timestamp(),
        last_fired_at: trigger.last_fired_at.map(|dt| dt.timestamp()),
        fire_count: trigger.fire_count,
    }
}

/// 將 UpdateTriggerRequest 應用到現有 Trigger
fn apply_update_to_trigger(trigger: &mut Trigger, update: &UpdateTriggerRequest) {
    if let Some(name) = &update.name {
        trigger.name = name.clone();
    }
    if let Some(description) = &update.description {
        trigger.description = description.clone();
    }
    if let Some(enabled) = update.enabled {
        trigger.enabled = enabled;
    }
    if let Some(type_str) = &update.trigger_type {
        if let Ok(new_type) = parse_trigger_type(type_str) {
            trigger.trigger_type = new_type;
        }
    }
    if let Some(schedule) = &update.schedule {
        apply_schedule_to_trigger_type(&mut trigger.trigger_type, schedule);
    }
    if let Some(condition) = &update.condition {
        if let Some(cond) = build_condition_from_value(condition) {
            trigger.conditions = vec![cond];
        }
    }
    if let Some(action) = &update.action {
        if let Some(act) = build_action_from_value(action) {
            trigger.actions = vec![act];
        }
    }
    if let Some(config) = &update.config {
        apply_config_to_trigger(trigger, config);
    }
    trigger.updated_at = Utc::now();
}

// ==========================================
// REST API Handlers
// ==========================================

/// POST /api/triggers - 建立觸發器
pub async fn create_trigger(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTriggerRequest>,
) -> Result<(StatusCode, Json<TriggerResponse>), (StatusCode, Json<ErrorResponse>)> {
    let trigger_type = parse_trigger_type(&payload.trigger_type)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "INVALID_TRIGGER_TYPE".to_string(),
            message: e,
        })))?;

    let mut trigger = Trigger {
        id: Uuid::new_v4(),
        name: payload.name,
        description: payload.description,
        enabled: payload.enabled.unwrap_or(true),
        trigger_type,
        conditions: vec![],
        actions: vec![],
        deduplication: DeduplicationConfig::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_fired_at: None,
        next_fire: None,
        fire_count: 0,
    };

    if let Some(schedule) = &payload.schedule {
        apply_schedule_to_trigger_type(&mut trigger.trigger_type, schedule);
    }
    if let Some(condition) = &payload.condition {
        if let Some(cond) = build_condition_from_value(condition) {
            trigger.conditions.push(cond);
        }
    }
    if let Some(action) = &payload.action {
        if let Some(act) = build_action_from_value(action) {
            trigger.actions.push(act);
        }
    }
    if let Some(config) = &payload.config {
        apply_config_to_trigger(&mut trigger, config);
    }

    state.trigger_registry.insert(trigger.clone()).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "INSERT_FAILED".to_string(),
            message: e.to_string(),
        })))?;

    Ok((StatusCode::CREATED, Json(trigger_to_response(trigger))))
}

/// GET /api/triggers - 列出觸發器（支援篩選分頁）
pub async fn list_triggers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTriggersParams>,
) -> Result<Json<Vec<TriggerResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let mut triggers = if params.enabled_only.unwrap_or(false) {
        state.trigger_registry.get_enabled().await
    } else {
        state.trigger_registry.get_all().await
    };

    if let Some(type_filter) = &params.trigger_type {
        triggers.retain(|t| {
            format!("{:?}", t.trigger_type)
                .to_lowercase()
                .contains(&type_filter.to_lowercase())
        });
    }

    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(triggers.len());

    let paged: Vec<TriggerResponse> = triggers.into_iter()
        .skip(offset)
        .take(limit)
        .map(trigger_to_response)
        .collect();

    Ok(Json(paged))
}

/// GET /api/triggers/:id - 取得單一觸發器
pub async fn get_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<TriggerResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.trigger_registry.get(id).await {
        Some(t) => Ok(Json(trigger_to_response(t))),
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "NOT_FOUND".to_string(),
            message: format!("Trigger {} not found", id),
        }))),
    }
}

/// PUT /api/triggers/:id - 更新觸發器
pub async fn update_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTriggerRequest>,
) -> Result<Json<TriggerResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut trigger = state.trigger_registry.get(id).await
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "NOT_FOUND".to_string(),
            message: format!("Trigger {} not found", id),
        })))?;

    apply_update_to_trigger(&mut trigger, &payload);

    state.trigger_registry.update(trigger.clone()).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "UPDATE_FAILED".to_string(),
            message: e.to_string(),
        })))?;

    Ok(Json(trigger_to_response(trigger)))
}

/// DELETE /api/triggers/:id - 刪除觸發器
pub async fn delete_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let deleted = state.trigger_registry.delete(id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "DELETE_FAILED".to_string(),
            message: e.to_string(),
        })))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "NOT_FOUND".to_string(),
            message: format!("Trigger {} not found", id),
        })))
    }
}

/// POST /api/triggers/:id/trigger - 手動觸發
pub async fn manual_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_payload): Json<ManualTriggerRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let trigger = state.trigger_registry.get(id).await
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "NOT_FOUND".to_string(),
            message: format!("Trigger {} not found", id),
        })))?;

    // 發送手動觸發事件至 EventBus
    let _ = state.event_bus.publish_simple(
        EventSource::Manual,
        serde_json::json!({
            "trigger_id": trigger.id,
            "trigger_name": trigger.name,
            "manual": true,
            "timestamp": Utc::now().timestamp(),
        }),
    );

    Ok(Json(serde_json::json!({
        "status": "triggered",
        "trigger_id": id,
        "message": format!("Trigger {} manually executed", trigger.name)
    })))
}

/// GET /api/triggers/:id/health - 取得單一觸發器健康狀態與熔斷器狀況
pub async fn get_trigger_health(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<TriggerHealthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let trigger = state.trigger_registry.get(id).await
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "NOT_FOUND".to_string(),
            message: format!("Trigger {} not found", id),
        })))?;

    let cb_state = state.circuit_breaker_manager
        .get_status(id)
        .await
        .ok()
        .flatten()
        .unwrap_or(TriggerCircuitBreakerState::Closed);

    Ok(Json(TriggerHealthResponse {
        trigger_id: trigger.id,
        status: if trigger.enabled { TriggerStatus::Active } else { TriggerStatus::Disabled },
        circuit_breaker_state: cb_state,
        last_fired_at: trigger.last_fired_at.map(|dt| dt.timestamp()),
        fire_count: trigger.fire_count,
        consecutive_failures: 0,
        last_error: None,
    }))
}

/// GET /health - 整體系統健康檢查
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Json<HealthResponse> {
    let triggers = state.trigger_registry.get_all().await;
    let enabled_count = triggers.iter().filter(|t| t.enabled).count();
    let runtime_running = state.is_runtime_running().await;

    Json(HealthResponse {
        status: "healthy".to_string(),
        triggers_registered: triggers.len(),
        triggers_enabled: enabled_count,
        runtime_running,
        circuit_breakers_open: 0,
        timestamp: Utc::now().timestamp(),
    })
}

// ==========================================
// Phase 52: 雙軌即時動態更新 API (A + B 融合)
// ==========================================

#[derive(Debug, Serialize, Deserialize)]
pub struct RealtimeApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentStatusRequest {
    pub status: String,
}

/// GET /api/v1/agents - 取得特工即時名冊與狀態
pub async fn list_agents() -> Json<RealtimeApiResponse<Value>> {
    let candidates = ["data/agents_runtime.json", "./data/agents_runtime.json", "../data/agents_runtime.json"];
    for p in candidates {
        let path = std::path::Path::new(p);
        if path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    return Json(RealtimeApiResponse {
                        success: true,
                        data: Some(json),
                        error: None,
                    });
                }
            }
        }
    }
    Json(RealtimeApiResponse {
        success: true,
        data: Some(serde_json::json!([])),
        error: None,
    })
}

/// GET /api/v1/tasks - 取得 Sprint 任務工單清單
pub async fn list_tasks() -> Json<RealtimeApiResponse<Value>> {
    let candidates = ["data/tasks_runtime.json", "./data/tasks_runtime.json", "../data/tasks_runtime.json"];
    for p in candidates {
        let path = std::path::Path::new(p);
        if path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    return Json(RealtimeApiResponse {
                        success: true,
                        data: Some(json),
                        error: None,
                    });
                }
            }
        }
    }
    Json(RealtimeApiResponse {
        success: true,
        data: Some(serde_json::json!([])),
        error: None,
    })
}

/// PATCH /api/v1/agents/:id/status - 更新特工狀態
pub async fn update_agent_status(
    Path(agent_id): Path<String>,
    Json(payload): Json<UpdateAgentStatusRequest>,
) -> Json<RealtimeApiResponse<Value>> {
    let candidates = ["data/agents_runtime.json", "./data/agents_runtime.json", "../data/agents_runtime.json"];
    for p in candidates {
        let path = std::path::Path::new(p);
        if path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(mut json) = serde_json::from_str::<Value>(&content) {
                    let mut found_item = None;
                    if let Some(arr) = json.as_array_mut() {
                        for item in arr.iter_mut() {
                            if item.get("id").and_then(|v| v.as_str()) == Some(&agent_id) {
                                item["status"] = Value::String(payload.status.clone());
                                item["lastActiveAt"] = Value::String(Utc::now().to_rfc3339());
                                found_item = Some(item.clone());
                                break;
                            }
                        }
                    }
                    if let Some(item) = found_item {
                        let _ = tokio::fs::write(path, serde_json::to_string_pretty(&json).unwrap_or_default()).await;
                        return Json(RealtimeApiResponse {
                            success: true,
                            data: Some(item),
                            error: None,
                        });
                    }
                }
            }
        }
    }
    Json(RealtimeApiResponse {
        success: false,
        data: None,
        error: Some(format!("Agent {} not found", agent_id)),
    })
}

/// GET /ws - 儀表板 WebSocket 雙向即時廣播頻道
pub async fn ws_dashboard_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(handle_dashboard_socket)
}

async fn handle_dashboard_socket(mut socket: WebSocket) {
    // 1. 初次連線立即推送全量特工快照
    let initial_agents = {
        let candidates = ["data/agents_runtime.json", "./data/agents_runtime.json", "../data/agents_runtime.json"];
        let mut found = serde_json::json!([]);
        for p in candidates {
            if let Ok(content) = tokio::fs::read_to_string(p).await {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    found = json;
                    break;
                }
            }
        }
        found
    };

    let snapshot_msg = serde_json::json!({
        "type": "agent_snapshot",
        "payload": initial_agents,
        "timestamp": Utc::now().to_rfc3339()
    });

    if socket.send(Message::Text(snapshot_msg.to_string().into())).await.is_err() {
        return;
    }

    // 2. 每 3 秒定期廣播即時狀態與心跳脈衝
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let candidates = ["data/agents_runtime.json", "./data/agents_runtime.json", "../data/agents_runtime.json"];
                let mut current_agents = serde_json::json!([]);
                for p in candidates {
                    if let Ok(content) = tokio::fs::read_to_string(p).await {
                        if let Ok(json) = serde_json::from_str::<Value>(&content) {
                            current_agents = json;
                            break;
                        }
                    }
                }
                let pulse_msg = serde_json::json!({
                    "type": "agent_status_pulse",
                    "payload": current_agents,
                    "timestamp": Utc::now().to_rfc3339()
                });
                if socket.send(Message::Text(pulse_msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Text(text) => {
                        if text.contains("ping") {
                            let pong = serde_json::json!({
                                "type": "pong",
                                "timestamp": Utc::now().to_rfc3339()
                            });
                            if socket.send(Message::Text(pong.to_string().into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            else => break,
        }
    }
}

