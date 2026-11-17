#![cfg_attr(not(windows), forbid(unsafe_code))]
mod agent;
pub mod auth;
mod auth_http;
pub mod browser_pool;
mod builtin;
mod catalog;
mod checkpoint;
mod compaction;
mod config;
mod daemon;
mod daemon_dispatch;
mod daemon_lock;
mod daemon_sub;
mod daemon_ws;
mod databricks;
mod databricks_label_grammar;
mod encrypted_vault;
mod vault_api;
mod capability_token;
mod handoff;
mod health_analyzer;
mod health_dashboard;
mod health_diagnoser;
mod health_guardian;
mod health_reporter;
mod hints;
mod hooks;
mod cron;
mod event_bus;
mod scheduler;
pub mod windows_job;

pub mod anymodelbot;
pub mod agentic_rag;
pub mod tstack;
mod llm;
mod mcp;
pub mod model_capabilities;
mod permission;
pub mod ticket_sandbox;
// pub mod triggers; // WIP by Pollen
pub mod types;
mod wire;

pub use catalog::{
    discover_databricks_models, discover_databricks_models_with_cache_dir, ModelEntry,
};
pub use config::Provider;
pub use types::AgentError;

// Browser Automation Phase 2 exports
pub use capability_token::{Capability, CapabilityToken, CapabilityChecker, TokenFactory, CapabilityError};
pub use encrypted_vault::{EncryptedVault, MasterKey, VaultConfig, VaultEntry};
pub use vault_api::{VaultApi, StoreCredentialRequest, RetrieveCredentialRequest, CredentialResponse, ListCredentialsResponse};

/// Environment keys the Windows Git Bash resolver may inspect. `spawn_one()`
/// forwards every key in this list into its otherwise-cleared MCP child; Doctor
/// uses the same contract so a ready agent can always start its shell tool.
#[cfg(windows)]
pub const WINDOWS_SHELL_RESOLUTION_ENV: &[&str] = &[
    "PATH",
    "BUZZ_SHELL",
    "GIT_BASH",
    "SystemRoot",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "LOCALAPPDATA",
];

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::BufReader;
use tokio::sync::{mpsc, watch, Mutex};

use crate::agent::RunCtx;
use crate::browser_pool::BrowserPool;
use crate::config::{Config, MAX_SYSTEM_PROMPT_BYTES, PROTOCOL_VERSION};
use crate::hints::SkillEntry;
use crate::llm::Llm;
use crate::mcp::McpRegistry;
use crate::types::{ContentBlock, HistoryItem};
use crate::wire::{
    classify, goose_session_update, Inbound, InitializeParams, SessionCancelParams,
    SessionNewParams, SessionPromptParams, SessionSetModelParams, SessionSteerParams, WireMsg,
    WireSender, INVALID_PARAMS, METHOD_NOT_FOUND, PARSE_ERROR,
};

struct App {
    cfg: Config,
    llm: Arc<Llm>,
    sessions: Mutex<HashMap<String, Session>>,
    /// ACP protocol version negotiated at `initialize`, stored for the whole
    /// connection lifetime. The `session/request_permission` wire shape derives
    /// from this value — never from a later mutable session field — so a strict
    /// client always receives exactly the shape it negotiated. Defaults to
    /// [`PROTOCOL_VERSION`] before `initialize`; no prompt (and thus no
    /// permission ask) can run before then.
    negotiated_version: AtomicU32,
    /// Owns the entire `session/request_permission` correlation lifecycle:
    /// process-wide admission, id allocation, response delivery, and abort-safe
    /// cleanup. See [`permission::PermissionBroker`].
    permissions: Arc<permission::PermissionBroker>,
    /// Browser Pool for Browser Automation
    browser_pool: Option<Arc<BrowserPool>>,
    /// Cached model catalog for Databricks providers. Populated lazily on the
    /// first successful `session/new` discovery call. Failed discovery is never
    /// cached: static-token authentication errors reject session creation, while
    /// OAuth authentication and non-auth errors use the configured model for that
    /// response and retry on the next session.
    models_cache: tokio::sync::OnceCell<Vec<ModelEntry>>,
}

struct Session {
    id: String,
    mcp: Arc<McpRegistry>,
    /// Skills discovered at session creation; used by the built-in `load_skill` tool.
    skills: Vec<SkillEntry>,
    history: Vec<HistoryItem>,
    cancel_tx: watch::Sender<bool>,
    busy: bool,
    /// Run id of the in-flight prompt, set when a prompt starts and cleared
    /// when it ends. `None` means no active run — a steer request targeting
    /// this session is rejected. Steer-capable clients learn this value from
    /// the `params.update._meta.goose.activeRunId` field on `session/update`.
    active_run_id: Option<String>,
    /// Sender for mid-turn steer messages. Created fresh per prompt (like
    /// `cancel_tx`); the running prompt loop holds the matching receiver and
    /// drains queued steers at round boundaries. `None` when no prompt is in
    /// flight.
    steer_tx: Option<mpsc::UnboundedSender<Vec<ContentBlock>>>,
    original_task: Option<String>,
    handoff_count: usize,
    ledger: crate::compaction::WorkingSetLedger,
    /// Cache-summed input tokens the provider reported for this session's most
    /// recent request, or `None` before the first response (or after a handoff
    /// resets the context). Drives the token-based handoff gate; see
    /// [`RunCtx::should_handoff`].
    last_request_input_tokens: Option<u64>,
    /// History byte size when `last_request_input_tokens` was measured, paired
    /// with it so the gate can account for history appended since.
    last_request_history_bytes: Option<usize>,
    effective_system_prompt: Arc<str>,
    /// Per-session model override set by `session/set_model`. When `Some`,
    /// overrides `App::cfg.model` for all LLM calls on this session. Persists
    /// across `session/prompt` calls until changed.
    effective_model: Option<String>,
    /// Session-cumulative input tokens across all turns. Sent in the
    /// `_goose/unstable/session/update` usage notification so buzz-acp's
    /// `UsageTracker` can compute per-turn deltas symmetrically with goose.
    /// `TurnIOState`: `Unseen` before any turn reports; `Exact(n)` while running;
    /// `Poisoned` if any turn's sum overflowed — permanently poisons the session.
    accumulated_input_tokens: crate::types::TurnIOState,
    /// Session-cumulative output tokens across all turns.
    /// Same `Unseen`/`Exact(n)`/`Poisoned` contract as `accumulated_input_tokens`.
    accumulated_output_tokens: crate::types::TurnIOState,
    /// Session-cumulative cache-served input tokens across all turns — a subset
    /// of `accumulated_input_tokens`, not an addition to it. Tri-state:
    ///
    /// - `Unseen`: no turn has ever reported this category.
    /// - `Exact(n)`: every usage-bearing response in every turn reported this
    ///   category; `n` is the cumulative sum.
    /// - `Unknown`: at least one usage-bearing response ever omitted the
    ///   category — permanently poisoned for this session.
    accumulated_cached_input_tokens: crate::types::CacheTotalState,
    /// Session-cumulative cache-written input tokens across all turns — also a
    /// subset of `accumulated_input_tokens`, not an addition to it.
    /// Same `Unseen`/`Exact`/`Unknown` tri-state contract as
    /// `accumulated_cached_input_tokens`.
    accumulated_cache_write_tokens: crate::types::CacheTotalState,
    /// Session-cumulative total-token state across all turns.
    ///
    /// Mirrors the per-turn `TurnTotalState` tri-state: starts `Unseen`,
    /// becomes `Exact(n)` as turns with genuine provider totals complete,
    /// transitions permanently to `Unknown` when any turn lacks a total or
    /// when the cumulative would otherwise decrease. Only emitted in the
    /// `usage_update` notification when `Exact`.
    accumulated_total_state: crate::types::TurnTotalState,
}

fn die(msg: String) -> ! {
    tracing::error!("{msg}");
    std::process::exit(2);
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if matches!(args.get(1).map(String::as_str), Some("auth")) {
        return tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(auth_subcommand(&args[2..]));
    }
    if matches!(args.get(1).map(String::as_str), Some("health-check")) {
        return tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(health_check_subcommand(&args[2..]));
    }
    if matches!(args.get(1).map(String::as_str), Some("health-dashboard")) {
        return tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(health_dashboard_subcommand(&args[2..]));
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())?;
    Ok(())
}

/// Authenticate to Databricks and store credentials under an optional explicit
/// cache root. `None` preserves buzz-agent's production cache location.
pub async fn authenticate_databricks_with_cache_dir(
    host: &str,
    cache_dir: Option<&std::path::Path>,
) -> Result<(), AgentError> {
    auth::PkceOAuthTokenSource::new(llm::databricks_pkce_config(
        host,
        cache_dir.map(std::path::Path::to_path_buf),
    ))?
    .interactive_login()
    .await
}

pub async fn authenticate_databricks(host: &str) -> Result<(), AgentError> {
    authenticate_databricks_with_cache_dir(host, None).await
}

/// `buzz-agent auth <provider>` — run the interactive auth flow for a
/// provider and persist the result, then exit. Today this supports Databricks
/// OAuth 2.0 PKCE. Reads `DATABRICKS_HOST` from env; needs a browser on the
/// machine.
async fn auth_subcommand(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let provider = args.first().map(String::as_str);
    match provider {
        Some("databricks" | "databricks_v2" | "databricks-v2") => {
            let host = std::env::var("DATABRICKS_HOST")
                .map_err(|_| "auth databricks: DATABRICKS_HOST required")?;
            authenticate_databricks(&host).await?;
            eprintln!("Authenticated. Token cached under ~/.config/buzz-agent/oauth/databricks/.");
            Ok(())
        }
        Some(other) => Err(format!("auth: unknown provider {other:?}").into()),
        None => Err("auth: provider required (try: buzz-agent auth databricks)".into()),
    }
}

/// `buzz-agent health-check` — run a one-time system health check and output report.
/// Supports `--json` flag for JSON output.
async fn health_check_subcommand(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let output_json = args.iter().any(|a| a == "--json");
    
    let cfg = Config::from_env().map_err(|e| format!("config: {e}"))?;
    
    let report = crate::health_guardian::phase_a_run_once(&cfg, output_json).await
        .map_err(|e| format!("health check failed: {e}"))?;
    
    if output_json {
        println!("{}", crate::health_reporter::render_json(&report)?);
    } else {
        println!("{}", crate::health_reporter::render_markdown(&report));
    }
    Ok(())
}

/// `buzz-agent health-dashboard` — start the health dashboard HTTP server.
/// Supports `--port` flag (default: 8080) and `--host` flag (default: 127.0.0.1).
async fn health_dashboard_subcommand(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut port = 8080u16;
    let mut host = "127.0.0.1";
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().map_err(|_| "invalid port")?;
                    i += 2;
                } else {
                    return Err("--port requires a value".into());
                }
            }
            "--host" => {
                if i + 1 < args.len() {
                    host = &args[i + 1];
                    i += 2;
                } else {
                    return Err("--host requires a value".into());
                }
            }
            _ => i += 1,
        }
    }
    
    let cfg = Arc::new(Config::from_env().map_err(|e| format!("config: {e}"))?);
    
    // Check if Health Guardian is enabled
    if cfg.health_guardian.is_none() {
        return Err("Health Guardian not enabled. Set BUZZ_HEALTH_GUARDIAN_ENABLED=1".into());
    }
    
    let mut dashboard_state = crate::health_dashboard::DashboardState::new(cfg.clone());
    
    // Run initial health check to populate dashboard
    let report = crate::health_guardian::phase_a_run_once(&cfg, false).await
        .map_err(|e| format!("initial health check failed: {e}"))?;
    dashboard_state.update_report(report).await;
    
    let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse()?;
    eprintln!("Starting Health Dashboard on http://{}", addr);
    
    dashboard_state.start_server(addr).await?;
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    eprintln!("Shutting down Health Dashboard...");
    dashboard_state.stop_server().await;
    
    Ok(())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let cfg = Config::from_env().unwrap_or_else(|e| die(e));
    let llm = Arc::new(Llm::new(&cfg).unwrap_or_else(|e| die(e.to_string())));
    let max_line = cfg.max_line_bytes;
    let permissions = Arc::new(permission::PermissionBroker::new(
        cfg.max_pending_permissions,
        cfg.permission_timeout,
    ));
    let browser_pool = match BrowserPool::new().await {
        Ok(pool) => {
            tracing::info!("BrowserPool successfully initialized");
            // Initialize browser automation skill registry
            browser_automation::skills::init_registry();
            tracing::info!("Browser automation skill registry initialized");
            Some(Arc::new(pool))
        }
        Err(e) => {
            tracing::warn!("BrowserPool initialization deferred (playwright runtime not ready): {e}");
            None
        }
    };
    let app = Arc::new(App {
        cfg: cfg.clone(),
        llm,
        sessions: Mutex::new(HashMap::new()),
        negotiated_version: AtomicU32::new(PROTOCOL_VERSION),
        permissions,
        browser_pool,
        models_cache: tokio::sync::OnceCell::new(),
    });

    // Start Agent Daemon for Mode B if agent keys are configured
    let mut daemon = crate::daemon::Daemon::new(Arc::new(app.cfg.clone()));
    if app.cfg.agent_keys.is_some() {
        if let Err(e) = daemon.start().await {
            tracing::error!("Failed to start agent daemon: {}", e);
        }
    }

    let (wire_tx, wire_rx) = mpsc::channel::<WireMsg>(64);
    let mut writer = tokio::spawn(wire::writer_task(wire_rx));
    // Whichever ends first drives shutdown. The reader ending is the normal
    // path (stdin EOF/error). The writer ending while the reader still runs
    // means stdout is closed/broken: no reply can ever be written, so we must
    // stop reading and cancel every session rather than leave the process
    // reading input while outstanding permission asks wait out their full
    // deadline for a response that can never arrive.
    tokio::select! {
        r = read_loop(
            BufReader::new(tokio::io::stdin()),
            app.clone(),
            wire_tx,
            max_line,
        ) => {
            if let Err(e) = r {
                tracing::error!("io: reader: {e}");
            }
            cancel_all_sessions(&app).await;
            let _ = writer.await;
        }
        _ = &mut writer => {
            tracing::error!("io: writer exited (stdout closed); shutting down connection");
            cancel_all_sessions(&app).await;
        }
    };
    Ok(())
}

/// Signal every live session to cancel. Run on connection teardown so in-flight
/// prompts — including any waiting on a `session/request_permission` response —
/// resolve promptly instead of waiting out their deadline.
async fn cancel_all_sessions(app: &Arc<App>) {
    for session in app.sessions.lock().await.values() {
        let _ = session.cancel_tx.send(true);
    }
}

async fn read_loop<R: tokio::io::AsyncBufRead + Unpin>(
    mut stdin: R,
    app: Arc<App>,
    wire_tx: WireSender,
    max_line: usize,
) -> std::io::Result<()> {
    while let Some(line) = wire::read_bounded_line(&mut stdin, max_line).await? {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(&line) {
            Ok(msg) => dispatch(&app, msg, &wire_tx).await,
            Err(e) => {
                wire::send(
                    &wire_tx,
                    wire::err(Value::Null, PARSE_ERROR, &format!("jsonrpc: parse: {e}")),
                )
                .await;
            }
        }
    }
    Ok(())
}

async fn dispatch(app: &Arc<App>, msg: Value, wire_tx: &WireSender) {
    match classify(&msg) {
        Inbound::Request { id, method, params } => {
            handle_request(app, id, method, params, wire_tx).await
        }
        Inbound::Notification { method, params } => handle_notification(app, &method, params).await,
        // Client's answer to a `session/request_permission` we issued. The
        // broker matches it to a live correlation id (waking that waiter) or
        // ignores an unknown/late id.
        Inbound::Response { id, result } => app.permissions.deliver(&id, result),
        Inbound::Invalid { id, code, message } => {
            wire::send(wire_tx, wire::err(id, code, &message)).await
        }
    }
}

async fn handle_request(
    app: &Arc<App>,
    id: Value,
    method: String,
    params: Value,
    wire_tx: &WireSender,
) {
    match method.as_str() {
        "initialize" => initialize(app, id, params, wire_tx).await,
        "session/new" => {
            let app = app.clone();
            let wire_tx = wire_tx.clone();
            tokio::spawn(async move { session_new(&app, id, params, &wire_tx).await });
        }
        "session/prompt" => spawn_prompt(app.clone(), id, params, wire_tx.clone()),
        "session/set_model" => {
            set_model_session(app, id, params, wire_tx).await;
        }
        "session/cancel" => {
            cancel_session(app, params).await;
            wire::send(wire_tx, wire::ok(id, Value::Null)).await;
        }
        // goose-compatible non-standard extension: inject user input into the
        // currently active prompt without starting a new one. Mirrors goose's
        // `_goose/unstable/session/steer` wire contract so a single client-side
        // delivery path serves both agents.
        "_goose/unstable/session/steer" => {
            steer_session(app, id, params, wire_tx).await;
        }
        _ => {
            wire::send(
                wire_tx,
                wire::err(
                    id,
                    METHOD_NOT_FOUND,
                    &format!("jsonrpc: method not found: {method}"),
                ),
            )
            .await
        }
    }
}

async fn handle_notification(app: &Arc<App>, method: &str, params: Value) {
    if method == "session/cancel" {
        cancel_session(app, params).await;
    }
}

async fn initialize(app: &Arc<App>, id: Value, params: Value, wire_tx: &WireSender) {
    let p: InitializeParams = match decode(params, "initialize") {
        Ok(p) => p,
        Err(m) => return reject(wire_tx, id, INVALID_PARAMS, &m).await,
    };
    // Honest negotiation: respond with the minimum of what the client
    // requested and what we support.
    // NOTE: gating `[Base]` injection on `protocol_version < 2` is a deliberate
    // temporary measure — we are squatting on ACP v2 ahead of the upstream ACP
    // RFD. Revisit when that RFD merges; otherwise a genuine upstream-v2 agent
    // would silently lose `[Base]`.
    let negotiated_version = p.protocol_version.min(PROTOCOL_VERSION);
    // Store the negotiated version for the connection lifetime: the
    // `session/request_permission` wire shape derives from this value, never
    // from a later mutable session field, so a strict client always receives
    // exactly the shape it negotiated at `initialize`.
    app.negotiated_version
        .store(negotiated_version, Ordering::Relaxed);
    wire::send(
        wire_tx,
        wire::ok(
            id,
            json!({
                "protocolVersion": negotiated_version,
                "agentCapabilities": {
                    "loadSession": false,
                    "promptCapabilities": { "image": false, "audio": false, "embeddedContext": false },
                    "mcpCapabilities": { "http": false, "sse": false },
                },
                "agentInfo": { "name": "buzz-agent", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
    )
    .await;
}

/// Resolve a Databricks model catalog for one `session/new` call.
///
/// The active filter is part of the result's authority: discovery failure may
/// not fall back to a configured model when it is present, because that would
/// bypass the same restriction applied to a successful catalog.
///
/// Tries to use a previously cached successful discovery result. If the cache
/// is empty, runs `discover` and — on success — populates the cache. On failure
/// the error is returned and the cell remains empty so the next session retries.
///
/// Extracted from `session_new` so tests can drive this path with an injected
/// discovery future without requiring a full `App` / transport stack.
async fn resolve_models_catalog(
    cache: &tokio::sync::OnceCell<Vec<ModelEntry>>,
    discover: impl std::future::Future<Output = Result<Vec<ModelEntry>, AgentError>>,
) -> Result<Vec<ModelEntry>, AgentError> {
    cache.get_or_try_init(|| discover).await.cloned()
}

/// Return the configured model as an unfiltered discovery fallback.
///
/// This value is never written to `models_cache`; failed discovery must be retried by
/// the next session rather than pinning degraded state for the process lifetime.
///
/// Only reached from the Databricks provider arm below, so the curated label is
/// looked up from the Databricks manifest; `id` stays the raw configured value.
fn configured_model_fallback(model: &str) -> Vec<ModelEntry> {
    let model = model.trim().to_string();
    let name = crate::model_capabilities::databricks_registry_label(&model)
        .unwrap_or_else(|| model.clone());
    vec![ModelEntry { id: model, name }]
}

/// A discovery failure may use the configured model only when no visibility
/// filter is active. Returning that model under an active filter would silently
/// bypass the operator's authoritative catalog restriction.
fn discovery_error_fallback(cfg: &Config) -> Vec<ModelEntry> {
    if cfg.databricks_model_filter.is_some() {
        Vec::new()
    } else {
        configured_model_fallback(&cfg.model)
    }
}

async fn session_new(app: &Arc<App>, id: Value, params: Value, wire_tx: &WireSender) {
    let p: SessionNewParams = match decode(params, "session/new") {
        Ok(p) => p,
        Err(m) => return reject(wire_tx, id, INVALID_PARAMS, &m).await,
    };
    if p.cwd.is_empty() || !Path::new(&p.cwd).is_absolute() {
        return reject(
            wire_tx,
            id,
            INVALID_PARAMS,
            "session/new: cwd must be an absolute path",
        )
        .await;
    }
    // Check cap without holding lock across MCP spawn (which may be slow).
    {
        let sessions = app.sessions.lock().await;
        if sessions.len() >= app.cfg.max_sessions {
            return reject(
                wire_tx,
                id,
                INVALID_PARAMS,
                "session/new: max sessions reached",
            )
            .await;
        }
    }
    let (hints_text, skills) = if app.cfg.hints_enabled {
        hints::build_hints_section(std::path::Path::new(&p.cwd))
    } else {
        (String::new(), Vec::new())
    };
    let effective_system_prompt: Arc<str> = {
        // When the harness provides a systemPrompt (base_prompt + persona), use
        // it as the primary content and suppress the default. The default is only
        // a fallback for legacy harnesses that don't send systemPrompt.
        let base = match p.system_prompt.as_deref() {
            Some(client_prompt) if !client_prompt.trim().is_empty() => client_prompt.to_owned(),
            _ => app.cfg.system_prompt.clone(),
        };
        let prompt = if hints_text.is_empty() {
            base
        } else {
            format!("{base}\n\n{hints_text}")
        };
        // Reject combined prompts exceeding 512KB.
        if prompt.len() > MAX_SYSTEM_PROMPT_BYTES {
            return reject(
                wire_tx,
                id,
                INVALID_PARAMS,
                &format!(
                    "session/new: combined system prompt exceeds {}KB limit ({} bytes)",
                    MAX_SYSTEM_PROMPT_BYTES / 1024,
                    prompt.len()
                ),
            )
            .await;
        }
        Arc::from(prompt)
    };
    // Resolve the model catalog before spawning MCP servers or registering a
    // session. A configured static credential cannot recover interactively, so
    // its authentication failure rejects before allocation. OAuth authentication
    // failures and other catalog failures use only the configured model for this
    // response, without caching, so session/prompt can run the existing PKCE flow.
    let available_models: Vec<Value> = {
        use crate::config::Provider;
        match app.cfg.provider {
            Provider::Databricks | Provider::DatabricksV2 => {
                let models = match resolve_models_catalog(
                    &app.models_cache,
                    discover_databricks_models(&app.cfg),
                )
                .await
                {
                    Ok(models) => models,
                    Err(error @ AgentError::LlmAuth(_)) if !app.cfg.api_key.is_empty() => {
                        return reject(wire_tx, id, error.json_rpc_code(), &error.to_string())
                            .await;
                    }
                    Err(error @ AgentError::LlmAuth(_)) => {
                        tracing::warn!(
                            error = %error,
                            filter_active = app.cfg.databricks_model_filter.is_some(),
                            "Databricks OAuth model catalog unavailable; using filter-aware fallback"
                        );
                        discovery_error_fallback(&app.cfg)
                    }
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            filter_active = app.cfg.databricks_model_filter.is_some(),
                            "Databricks model catalog unavailable; using filter-aware fallback"
                        );
                        discovery_error_fallback(&app.cfg)
                    }
                };
                models
                    .iter()
                    .map(|m| json!({ "modelId": m.id, "name": m.name }))
                    .collect()
            }
            _ => vec![json!({ "modelId": app.cfg.model, "name": app.cfg.model })],
        }
    };

    let mcp = match McpRegistry::spawn_all(&app.cfg, &p.mcp_servers, &p.cwd).await {
        Ok(m) => Arc::new(m),
        Err(e) => return reject(wire_tx, id, e.json_rpc_code(), &e.to_string()).await,
    };
    let session_id = match session_token() {
        Ok(t) => format!("ses_{t}"),
        Err(e) => return reject(wire_tx, id, -32000, &e).await,
    };
    let (cancel_tx, _) = watch::channel(false);
    let mut sessions = app.sessions.lock().await;
    // Re-check cap (another session may have been created while we spawned MCP).
    if sessions.len() >= app.cfg.max_sessions {
        return reject(
            wire_tx,
            id,
            INVALID_PARAMS,
            "session/new: max sessions reached",
        )
        .await;
    }
    sessions.insert(
        session_id.clone(),
        Session {
            id: session_id.clone(),
            mcp,
            skills,
            history: Vec::new(),
            cancel_tx,
            busy: false,
            active_run_id: None,
            steer_tx: None,
            original_task: None,
            handoff_count: 0,
            ledger: crate::compaction::WorkingSetLedger::default(),
            last_request_input_tokens: None,
            last_request_history_bytes: None,
            effective_system_prompt,
            effective_model: None,
            accumulated_input_tokens: crate::types::TurnIOState::Unseen,
            accumulated_output_tokens: crate::types::TurnIOState::Unseen,
            accumulated_cached_input_tokens: crate::types::CacheTotalState::Unseen,
            accumulated_cache_write_tokens: crate::types::CacheTotalState::Unseen,
            accumulated_total_state: crate::types::TurnTotalState::Unseen,
        },
    );
    drop(sessions);

    wire::send(
        wire_tx,
        wire::ok(
            id,
            json!({
                "sessionId": session_id,
                "models": {
                    "currentModelId": app.cfg.model,
                    "availableModels": available_models,
                },
            }),
        ),
    )
    .await;
}

fn decode<T: serde::de::DeserializeOwned>(params: Value, stage: &str) -> Result<T, String> {
    serde_json::from_value(params).map_err(|e| format!("{stage}: {e}"))
}

async fn reject(wire_tx: &WireSender, id: Value, code: i32, message: &str) {
    wire::send(wire_tx, wire::err(id, code, message)).await;
}

async fn cancel_session(app: &Arc<App>, params: Value) {
    if let Ok(p) = serde_json::from_value::<SessionCancelParams>(params) {
        if let Some(s) = app.sessions.lock().await.get(&p.session_id) {
            let _ = s.cancel_tx.send(true);
        }
    }
}

/// Handle `session/set_model`: apply a per-session model override immediately.
///
/// Validation:
/// - Unknown `sessionId` → `invalid_params`.
/// - Empty `modelId` → `invalid_params`.
///
/// On success: stores `model_id` on the session and responds `{ sessionId, modelId }`.
/// The override is picked up by the next `session/prompt` call on this session.
async fn set_model_session(app: &Arc<App>, id: Value, params: Value, wire_tx: &WireSender) {
    let p: SessionSetModelParams = match decode(params, "session/set_model") {
        Ok(p) => p,
        Err(m) => return reject(wire_tx, id, INVALID_PARAMS, &m).await,
    };
    if p.model_id.trim().is_empty() {
        return reject(
            wire_tx,
            id,
            INVALID_PARAMS,
            "session/set_model: modelId must not be empty",
        )
        .await;
    }
    let mut sessions = app.sessions.lock().await;
    let Some(s) = sessions.get_mut(&p.session_id) else {
        return reject(
            wire_tx,
            id,
            INVALID_PARAMS,
            "session/set_model: unknown session",
        )
        .await;
    };
    s.effective_model = Some(p.model_id.clone());
    tracing::info!(
        session_id = %p.session_id,
        model_id = %p.model_id,
        "session/set_model: model overridden"
    );
    drop(sessions);
    wire::send(
        wire_tx,
        wire::ok(
            id,
            json!({ "sessionId": p.session_id, "modelId": p.model_id }),
        ),
    )
    .await;
}

/// Handle `_goose/unstable/session/steer`: queue user input into the in-flight
/// prompt. Validation mirrors goose's `on_steer_session`:
///   - empty prompt → `invalid_params`
///   - no active run (no prompt in flight) → `invalid_params`
///   - `expectedRunId` mismatch → `invalid_params` (caller is steering a turn
///     that already ended or rotated; it must fall back to cancel+merge)
///
/// On success the message is queued for pickup at the next round boundary and
/// we reply `{ runId, messageId }`, then emit a `queuedSteer` session/update so
/// the client can correlate the accepted steer with its eventual pickup.
async fn steer_session(app: &Arc<App>, id: Value, params: Value, wire_tx: &WireSender) {
    let p: SessionSteerParams = match decode(params, "_goose/unstable/session/steer") {
        Ok(p) => p,
        Err(m) => return reject(wire_tx, id, INVALID_PARAMS, &m).await,
    };
    if p.prompt.is_empty() {
        return reject(
            wire_tx,
            id,
            INVALID_PARAMS,
            "steer: prompt must not be empty",
        )
        .await;
    }
    if p.expected_run_id.is_empty() {
        return reject(
            wire_tx,
            id,
            INVALID_PARAMS,
            "steer: expectedRunId must not be empty",
        )
        .await;
    }
    let message_id = format!("steer_{}", session_token().unwrap_or_else(|_| "x".into()));
    let run_id = {
        let sessions = app.sessions.lock().await;
        let Some(s) = sessions.get(&p.session_id) else {
            return reject(wire_tx, id, INVALID_PARAMS, "steer: unknown session").await;
        };
        let Some(active) = s.active_run_id.as_deref() else {
            return reject(wire_tx, id, INVALID_PARAMS, "steer: no active run to steer").await;
        };
        if active != p.expected_run_id {
            return reject(
                wire_tx,
                id,
                INVALID_PARAMS,
                &format!(
                    "steer: expected active run id `{}` but found `{active}`",
                    p.expected_run_id
                ),
            )
            .await;
        }
        // A live run always has a steer_tx; if the channel is gone the run is
        // tearing down — treat as no active run rather than queue into the void.
        match &s.steer_tx {
            Some(tx) if tx.send(p.prompt).is_ok() => active.to_owned(),
            _ => return reject(wire_tx, id, INVALID_PARAMS, "steer: no active run to steer").await,
        }
    };
    wire::send(
        wire_tx,
        wire::ok(id, json!({ "runId": run_id, "messageId": message_id })),
    )
    .await;
    // Best-effort correlation hint for the client; mirrors goose's
    // `send_queued_steer_update`. Not load-bearing for delivery.
    wire::send(
        wire_tx,
        wire::session_update_with_goose_meta(
            &p.session_id,
            json!({ "sessionUpdate": "session_info_update" }),
            json!({ "queuedSteer": { "messageId": message_id, "runId": run_id } }),
        ),
    )
    .await;
}

fn spawn_prompt(app: Arc<App>, id: Value, params: Value, wire_tx: WireSender) {
    tokio::spawn(async move { run_prompt(app, id, params, wire_tx).await });
}

async fn run_prompt(app: Arc<App>, id: Value, params: Value, wire_tx: WireSender) {
    let p: SessionPromptParams = match decode(params, "session/prompt") {
        Ok(p) => p,
        Err(m) => return reject(&wire_tx, id, INVALID_PARAMS, &m).await,
    };
    let (
        sid,
        mcp,
        skills,
        mut history,
        mut original_task,
        mut handoff_count,
        mut last_request_input_tokens,
        mut last_request_history_bytes,
        mut cancel_rx,
        effective_system_prompt,
        effective_model_override,
        run_id,
        mut steer_rx,
        usage_baseline,
        mut ledger,
    ) = match acquire_session(&app, &p.session_id).await {
        Ok(v) => v,
        Err(reason) => {
            return reject(
                &wire_tx,
                id,
                INVALID_PARAMS,
                &format!("session/prompt: {reason}"),
            )
            .await
        }
    };
    // Advertise the active run id so steer-capable clients can target this turn
    // via `expectedRunId`. Mirrors goose's `send_active_run_update`.
    wire::send(
        &wire_tx,
        wire::session_update_with_goose_meta(
            &sid,
            json!({ "sessionUpdate": "session_info_update" }),
            json!({ "activeRunId": run_id }),
        ),
    )
    .await;
    // Resolve effective model: session override wins over config default.
    let effective_model_str = effective_model_override
        .as_deref()
        .unwrap_or(&app.cfg.model);
    let mut turn_input_tokens: crate::types::TurnIOState = crate::types::TurnIOState::Unseen;
    let mut turn_output_tokens: crate::types::TurnIOState = crate::types::TurnIOState::Unseen;
    let mut turn_cached_input_tokens: crate::types::CacheTotalState =
        crate::types::CacheTotalState::Unseen;
    let mut turn_cache_write_tokens: crate::types::CacheTotalState =
        crate::types::CacheTotalState::Unseen;
    let mut turn_total_state = crate::types::TurnTotalState::Unseen;
    // Per-turn billing identity accumulator — three-state:
    //   None          = no usage-bearing response seen yet (initial)
    //   Some(Some(pi))= all usage-bearing responses carry the same proven identity
    //   Some(None)    = poisoned (mixed identities, unproven response, etc.)
    // Not stored in Session (not session-cumulative); used only for the final
    // end-of-turn wire emission.
    let mut turn_pricing_identity: Option<Option<crate::types::PricingIdentity>> = None;
    let mut ctx = RunCtx {
        cfg: &app.cfg,
        effective_model: effective_model_str,
        session_id: &sid,
        system_prompt: &effective_system_prompt,
        llm: &app.llm,
        mcp: &mcp,
        permissions: &app.permissions,
        protocol_version: app.negotiated_version.load(Ordering::Relaxed),
        skills: &skills,
        wire: &wire_tx,
        cancel: &mut cancel_rx,
        agent_keys: app.cfg.agent_keys.clone(),
        history_cache: Some(Arc::new(crate::types::HistoryItemCache::new(
            app.cfg.history_cache_ttl_secs,
            app.cfg.history_cache_max_entries,
        ))),
        steer: &mut steer_rx,
        history: &mut history,
        original_task: &mut original_task,
        handoff_count: &mut handoff_count,
        run_id,
        last_request_input_tokens: &mut last_request_input_tokens,
        last_request_history_bytes: &mut last_request_history_bytes,
        turn_input_tokens: &mut turn_input_tokens,
        turn_output_tokens: &mut turn_output_tokens,
        turn_cached_input_tokens: &mut turn_cached_input_tokens,
        turn_cache_write_tokens: &mut turn_cache_write_tokens,
        turn_total_state: &mut turn_total_state,
        turn_pricing_identity: &mut turn_pricing_identity,
        usage_baseline,
        ledger: &mut ledger,
        browser_pool: &app.browser_pool,
    };
    let result = ctx.run(p.prompt).await;
    if let Some(s) = app.sessions.lock().await.get_mut(&sid) {
        s.busy = false;
        // Clear run state so a late steer can't queue into a finished turn.
        s.active_run_id = None;
        s.steer_tx = None;
        s.history = history;
        s.original_task = original_task;
        s.handoff_count = handoff_count;
        s.last_request_input_tokens = last_request_input_tokens;
        s.last_request_history_bytes = last_request_history_bytes;
        s.ledger = ledger;
    }
    // Update session-cumulative token counters and emit the usage notification
    // BEFORE sending the session/prompt response. buzz-acp's UsageTracker
    // processes the notification while the turn is still in-flight (i.e. before
    // the response triggers take_turn_usage()), which is required for the
    // begin_turn gate to recognise it as publishable.
    //
    // Only emit when at least one token count was observed — a turn with no
    // provider response (validation failure, pre-response cancellation) carries
    // no information and must not produce a kind 44200 record per NIP-AM.
    if !matches!(turn_input_tokens, crate::types::TurnIOState::Unseen)
        || !matches!(turn_output_tokens, crate::types::TurnIOState::Unseen)
    {
        let accumulated = {
            let mut sessions = app.sessions.lock().await;
            if let Some(s) = sessions.get_mut(&sid) {
                // merge_session: Poisoned poisons permanently; Exact sums with
                // overflow-check → Poisoned on wrap; Unseen leaves unchanged.
                s.accumulated_input_tokens =
                    s.accumulated_input_tokens.merge_session(turn_input_tokens);
                s.accumulated_output_tokens = s
                    .accumulated_output_tokens
                    .merge_session(turn_output_tokens);
                // D1 tri-state merge: merge_session propagates Unknown when
                // the turn was poisoned (any usage-bearing round omitted the
                // category), and is a no-op when the turn was Unseen (no
                // usage-bearing response at all).
                s.accumulated_cached_input_tokens = s
                    .accumulated_cached_input_tokens
                    .merge_session(turn_cached_input_tokens);
                s.accumulated_cache_write_tokens = s
                    .accumulated_cache_write_tokens
                    .merge_session(turn_cache_write_tokens);
                // Fold the per-turn total state into the session cumulative.
                // Unknown poisons the session permanently; Exact adds to running sum;
                // Unseen (turn emitted no usage) leaves the cumulative unchanged.
                // Uses TurnTotalState::merge_session, which applies the same
                // checked-add / overflow-poisons contract as the per-response fold.
                s.accumulated_total_state =
                    s.accumulated_total_state.merge_session(turn_total_state);
                Some((
                    s.accumulated_input_tokens,
                    s.accumulated_output_tokens,
                    s.accumulated_cached_input_tokens,
                    s.accumulated_cache_write_tokens,
                    s.accumulated_total_state,
                ))
            } else {
                // Session is gone — the accumulated baseline no longer exists, so
                // there is nothing correct to emit. Skip the usage notification.
                None
            }
        };
        if let Some((
            accumulated_in,
            accumulated_out,
            accumulated_cached,
            accumulated_written,
            accumulated_total,
        )) = accumulated
        {
            // Same builder the run loop uses for its per-round reports, so the
            // final notification is shape-identical to the ones that preceded
            // it and a consumer taking the high-water mark lands on this one.
            let update = wire::usage_update_payload(
                accumulated_in.exact_value(),
                accumulated_out.exact_value(),
                accumulated_cached.exact_value(),
                accumulated_written.exact_value(),
                accumulated_total,
                effective_model_str,
                // Pass the proven per-turn identity if consistent; absent otherwise.
                turn_pricing_identity
                    .as_ref()
                    .and_then(|inner| inner.as_ref()),
            );
            wire::send(&wire_tx, goose_session_update(&sid, update)).await;
        }
    }
    match result {
        Ok(stop) => {
            wire::send(
                &wire_tx,
                wire::ok(id, json!({ "stopReason": stop.as_wire() })),
            )
            .await
        }
        Err(e) => wire::send(&wire_tx, wire::err(id, e.json_rpc_code(), &e.to_string())).await,
    }
}

async fn acquire_session(
    app: &Arc<App>,
    session_id: &str,
) -> Result<
    (
        String,
        Arc<McpRegistry>,
        Vec<SkillEntry>,
        Vec<HistoryItem>,
        Option<String>,
        usize,
        Option<u64>,
        Option<usize>,
        watch::Receiver<bool>,
        Arc<str>,
        Option<String>,
        String,
        mpsc::UnboundedReceiver<Vec<ContentBlock>>,
        crate::types::SessionUsageBaseline,
        crate::compaction::WorkingSetLedger,
    ),
    &'static str,
> {
    let mut sessions = app.sessions.lock().await;
    let s = sessions.get_mut(session_id).ok_or("unknown session")?;
    if s.busy {
        return Err("prompt already in flight");
    }
    // Generate the run id before mutating session state. On RNG failure we reject
    // the prompt cleanly: the session stays idle and the caller can retry. Generating
    // after `s.busy = true` with `?` would wedge the session permanently busy.
    let run_id = format!(
        "run_{}",
        session_token().map_err(|_| "rng failure; retry prompt")?
    );
    s.busy = true;
    let (tx, rx) = watch::channel(false);
    s.cancel_tx = tx;
    // Skills are read-only after session creation; clone the Vec so RunCtx
    // can hold a reference without holding the sessions lock.
    let skills = s.skills.clone();
    // Fresh run id + steer channel for this turn. The run id lets steer-capable
    // clients target *this* turn (rejecting steers aimed at a turn that already
    // ended); the channel carries mid-turn injections to the run loop.
    s.active_run_id = Some(run_id.clone());
    let (steer_tx, steer_rx) = mpsc::unbounded_channel();
    s.steer_tx = Some(steer_tx);
    let effective_model = s.effective_model.clone();
    Ok((
        s.id.clone(),
        s.mcp.clone(),
        skills,
        std::mem::take(&mut s.history),
        s.original_task.take(),
        s.handoff_count,
        s.last_request_input_tokens,
        s.last_request_history_bytes,
        rx,
        Arc::clone(&s.effective_system_prompt),
        effective_model,
        run_id,
        steer_rx,
        // Snapshot rather than a handle: the run loop reports cumulative usage
        // after every LLM round, and taking the sessions lock on each of those
        // would serialise concurrent sessions behind one another's provider
        // round-trips. Nothing else advances these counters while this turn
        // holds `busy`, so the snapshot cannot go stale under it.
        crate::types::SessionUsageBaseline {
            input_tokens: s.accumulated_input_tokens,
            output_tokens: s.accumulated_output_tokens,
            cached_input_tokens: s.accumulated_cached_input_tokens,
            cache_write_tokens: s.accumulated_cache_write_tokens,
            total_state: s.accumulated_total_state,
        },
        std::mem::take(&mut s.ledger),
    ))
}

fn session_token() -> Result<String, String> {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).map_err(|e| format!("rng: getrandom failed: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use crate::catalog::ModelEntry;
    use crate::types::AgentError;

    /// Regression: a discovery error must not pin the models_cache for the process lifetime.
    /// 
    /// `resolve_models_catalog` uses `get_or_try_init` so an `Err` leaves the `OnceCell`
    /// empty and the next `session/new` retries discovery. This test calls
    /// `resolve_models_catalog` directly — the same function `session_new` calls — so
    /// reverting `session_new` to `get_or_init` (or any other cache-on-error variant) would
    /// break this test, not just the standalone `OnceCell` semantics.
    #[tokio::test]
    async fn models_cache_does_not_pin_on_discovery_error() {
        let cache: tokio::sync::OnceCell<Vec<ModelEntry>> = tokio::sync::OnceCell::new();

        // First call — discovery failure is surfaced and leaves the cell empty.
        let error = crate::resolve_models_catalog(&cache, async {
            Err::<Vec<ModelEntry>, AgentError>(AgentError::Llm("transient failure".into()))
        })
        .await
        .unwrap_err();
        assert!(matches!(error, AgentError::Llm(_)));

        // Second call — discovery succeeds. Cell is now populated and returned.
        let discovered = vec![ModelEntry {
            id: "databricks-meta-llama-3-1-70b-instruct".into(),
            name: "databricks-meta-llama-3-1-70b-instruct".into(),
        }];
        let discovered_clone = discovered.clone();
        let second = crate::resolve_models_catalog(&cache, async move {
            Ok::<Vec<ModelEntry>, AgentError>(discovered_clone)
        })
        .await
        .unwrap();
        assert_eq!(
            second, discovered,
            "second call must return the discovered catalog"
        );
        assert!(
            cache.get().is_some(),
            "cell must be populated after successful discovery"
        );
        assert_eq!(
            cache.get().unwrap(),
            &discovered,
            "cache must hold the successful discovery result"
        );
    }

    #[tokio::test]
    async fn models_catalog_does_not_cache_oauth_auth_fallback() {
        let cache: tokio::sync::OnceCell<Vec<ModelEntry>> = tokio::sync::OnceCell::new();
        let error = crate::resolve_models_catalog(&cache, async {
            Err::<Vec<ModelEntry>, AgentError>(AgentError::LlmAuth("sign in again".into()))
        })
        .await
        .unwrap_err();

        assert!(matches!(error, AgentError::LlmAuth(_)));
        assert!(cache.get().is_none());

        let discovered = vec![ModelEntry {
            id: "authenticated-model".into(),
            name: "authenticated-model".into(),
        }];
        let result = crate::resolve_models_catalog(&cache, async {
            Ok::<Vec<ModelEntry>, AgentError>(discovered.clone())
        })
        .await
        .unwrap();

        assert_eq!(result, discovered);
        assert_eq!(cache.get(), Some(&discovered));
    }

    #[test]
    fn configured_model_fallback_is_trimmed_and_singular() {
        // Unknown id: trimmed, and the raw id passes through as the name.
        assert_eq!(
            crate::configured_model_fallback("  configured-model  "),
            vec![ModelEntry {
                id: "configured-model".into(),
                name: "configured-model".into(),
            }]
        );
    }

    #[test]
    fn configured_model_fallback_curates_known_databricks_id() {
        // A configured Databricks id known to the manifest gets its curated
        // label; `id` stays the raw wire/config value.
        assert_eq!(
            crate::configured_model_fallback("databricks-gpt-5-5"),
            vec![ModelEntry {
                id: "databricks-gpt-5-5".into(),
                name: "GPT-5.5".into(),
            }]
        );
    }
}

#[cfg(test)]
mod daemon_tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;

    /// Test that DaemonLock can be created with valid Redis URL
    #[tokio::test]
    #[ignore = "requires Redis"]
    async fn daemon_lock_acquire_release() {
        let lock = crate::daemon_lock::DaemonLock::new("redis://localhost:6379", "test_pubkey").await.unwrap();
        let mut lock = lock;
        lock.acquire().await.unwrap();
        lock.release().await.unwrap();
    }

    /// Test that DaemonLock prevents double acquire
    #[tokio::test]
    #[ignore = "requires Redis"]
    async fn daemon_lock_double_acquire_fails() {
        let lock1 = crate::daemon_lock::DaemonLock::new("redis://localhost:6379", "test_pubkey2").await.unwrap();
        let mut lock1 = lock1;
        lock1.acquire().await.unwrap();

        let lock2 = crate::daemon_lock::DaemonLock::new("redis://localhost:6379", "test_pubkey2").await.unwrap();
        let mut lock2 = lock2;
        assert!(lock2.acquire().await.is_err());

        lock1.release().await.unwrap();
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use serde_json::json;

    /// Test parsing reminder content from kind:30300 event
    #[test]
    fn parse_reminder_content() {
        let keys = Keys::generate();
        let target_pubkey = keys.public_key().to_hex();
        
        let reminder_content = json!({
            "target": {
                "dm_recipient_pubkey": target_pubkey,
                "channelId": "chan1",
                "preview": "test message",
                "authorPubkey": "author1"
            },
            "status": "pending",
            "note": "喝水"
        });

        let ciphertext = nostr::nips::nip44::encrypt(
            keys.secret_key(),
            &keys.public_key(),
            &reminder_content.to_string(),
            nostr::nips::nip44::Version::V2,
        ).unwrap();

        let event = EventBuilder::new(Kind::Custom(30300), ciphertext)
            .tag(Tag::parse(["d", "test123"]).unwrap())
            .tag(Tag::parse(["not_before", "1234567890"]).unwrap())
            .sign_with_keys(&keys)
            .unwrap();

        // Verify we can decrypt
        let plaintext = nostr::nips::nip44::decrypt(
            keys.secret_key(),
            &keys.public_key(),
            &event.content,
        ).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&plaintext).unwrap();
        assert_eq!(parsed["status"], "pending");
        assert_eq!(parsed["note"], "喝水");
        assert_eq!(parsed["target"]["dm_recipient_pubkey"], target_pubkey);
    }
}

#[cfg(test)]
mod ws_client_tests {
    use super::*;

    /// Test WebSocketClient creation
    #[test]
    fn ws_client_new() {
        let client = crate::daemon_ws::WebSocketClient::new("wss://relay.example.com".to_string());
        assert_eq!(client.relay_url, "wss://relay.example.com");
    }
}

#[cfg(test)]
mod subscription_manager_tests {
    use super::*;
    use nostr::Keys;

    /// Test SubscriptionManager creation and filter building
    #[test]
    fn subscription_manager_new() {
        let keys = Keys::generate();
        let manager = crate::daemon_sub::SubscriptionManager::new(keys.public_key().to_hex());
        assert!(manager.current_sub_id.is_none());
    }
}
