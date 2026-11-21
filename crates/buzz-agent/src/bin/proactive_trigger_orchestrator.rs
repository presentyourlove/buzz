#!/usr/bin/env rust
//! proactive-trigger-orchestrator Binary
//!
//! Phase 4: 主動觸發器協調層 - 提供觸發器完整生命週期 CRUD REST API、
//! CircuitBreaker 熔斷機制、健康檢查與手動觸發介面。
//!
//! 啟動方式：
//!   cargo run --bin proactive-trigger-orchestrator -- --port 8080 --storage-dir ./data/triggers

use buzz_agent::triggers::orchestrator::{
    circuit_breaker::{CircuitBreakerConfig, TriggerCircuitBreakerManager},
    routes::create_router,
    state::AppState,
};
use buzz_agent::triggers::{
    event_bus::{EventBus, EventBusConfig},
    runtime::{TriggerRuntime, TriggerRuntimeConfig},
    trigger_registry::TriggerRegistry,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化 tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("proactive_trigger_orchestrator=info".parse()?)
            .add_directive("buzz_agent=info".parse()?))
        .init();

    info!("🚀 啟動 Proactive Trigger Orchestrator (Phase 4)");

    // 解析命令列參數
    let args: Vec<String> = std::env::args().collect();
    let mut port = 8080u16;
    let mut storage_dir = PathBuf::from("./data/triggers");
    let mut config_path: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse()?;
                    i += 2;
                } else {
                    error!("--port 需要參數");
                    return Err(anyhow::anyhow!("缺少 --port 參數"));
                }
            }
            "--storage-dir" => {
                if i + 1 < args.len() {
                    storage_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    error!("--storage-dir 需要參數");
                    return Err(anyhow::anyhow!("缺少 --storage-dir 參數"));
                }
            }
            "--config" => {
                if i + 1 < args.len() {
                    config_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    error!("--config 需要參數");
                    return Err(anyhow::anyhow!("缺少 --config 參數"));
                }
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                warn!("未知參數: {}", args[i]);
                i += 1;
            }
        }
    }

    // 環境變數覆蓋
    if let Ok(env_port) = std::env::var("ORCHESTRATOR_PORT") {
        port = env_port.parse().unwrap_or(port);
    }
    if let Ok(env_storage) = std::env::var("ORCHESTRATOR_STORAGE_DIR") {
        storage_dir = PathBuf::from(env_storage);
    }
    if let Ok(env_config) = std::env::var("ORCHESTRATOR_CONFIG") {
        config_path = Some(PathBuf::from(env_config));
    }

    info!("配置: port={}, storage_dir={:?}, config={:?}", port, storage_dir, config_path);

    // 確保儲存目錄存在
    tokio::fs::create_dir_all(&storage_dir).await?;

    // 初始化核心組件
    let trigger_registry = Arc::new(TriggerRegistry::new(storage_dir.join("triggers.json")).await?);
    info!("已載入 {} 個觸發器", trigger_registry.count().await);

    // 初始化熔斷器管理器
    let circuit_breaker_config = CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 2,
        timeout_secs: 60,
        auto_reset_after_secs: 30,
        half_open_probes: 3,
    };
    let circuit_breaker_dir = storage_dir.join("circuit_breakers");
    tokio::fs::create_dir_all(&circuit_breaker_dir).await?;
    let circuit_breaker_manager = Arc::new(TriggerCircuitBreakerManager::new(circuit_breaker_dir, circuit_breaker_config));

    // 初始化事件總線
    let event_bus = Arc::new(EventBus::new(EventBusConfig::default()));

    // 建立應用狀態
    let app_state = Arc::new(AppState::new(
        trigger_registry.clone(),
        circuit_breaker_manager.clone(),
        event_bus.clone(),
    ));

    // 建立並啟動 TriggerRuntime
    let mut trigger_runtime = TriggerRuntime::with_config(TriggerRuntimeConfig::default());
    let _ = trigger_runtime.start().await;
    info!("TriggerRuntime 已啟動");

    // 將 Runtime 設定到 AppState
    app_state.set_trigger_runtime(trigger_runtime).await;

    // 建立 Router
    let router = create_router(app_state.clone());

    // 啟動 HTTP Server
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("🌐 HTTP Server 監聽於 {}", addr);
    info!("  - 健康檢查: GET http://localhost:{}/health", port);
    info!("  - 觸發器 CRUD: /api/triggers");
    info!("  - 手動觸發: POST /api/triggers/:id/trigger");
    info!("  - 觸發器健康: GET /api/triggers/:id/health");

    // 優雅關機信號
    let shutdown_signal = async move {
        match signal::ctrl_c().await {
            Ok(()) => info!("收到 SIGINT，開始優雅關機..."),
            Err(e) => error!("關機信號錯誤: {}", e),
        }
    };

    // 運行 Server
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    info!("HTTP Server 已停止接收請求，開始清理服務...");

    // 停止 TriggerRuntime
    if let Some(runtime) = app_state.get_trigger_runtime().await {
        info!("正在停止 TriggerRuntime...");
        let _ = runtime.stop().await;
        info!("TriggerRuntime 已停止");
    }

    // 持久化觸發器註冊表
    if let Err(e) = trigger_registry.persist().await {
        error!("持久化觸發器註冊表失敗: {}", e);
    } else {
        info!("觸發器註冊表已持久化");
    }

    info!("✅ Proactive Trigger Orchestrator 已優雅關閉");
    Ok(())
}

fn print_help() {
    println!("Proactive Trigger Orchestrator - Phase 4 主動觸發器協調層");
    println!();
    println!("用法: proactive-trigger-orchestrator [選項]");
    println!();
    println!("選項:");
    println!("  --port <PORT>           HTTP 監聽埠 (預設: 8080，環境變數: ORCHESTRATOR_PORT)");
    println!("  --storage-dir <PATH>    持久化儲存目錄 (預設: ./data/triggers，環境變數: ORCHESTRATOR_STORAGE_DIR)");
    println!("  --config <PATH>         選配 TOML 配置檔路徑 (環境變數: ORCHESTRATOR_CONFIG)");
    println!("  --help, -h              顯示此說明");
    println!();
    println!("API 端點:");
    println!("  GET  /health                    服務健康檢查");
    println!("  POST /api/triggers              建立觸發器");
    println!("  GET  /api/triggers              列出觸發器 (支援分頁、過濾)");
    println!("  GET  /api/triggers/:id          取得單一觸發器");
    println!("  PUT  /api/triggers/:id          更新觸發器 (熱重載排程)");
    println!("  DELETE /api/triggers/:id        刪除觸發器");
    println!("  POST /api/triggers/:id/trigger  手動觸發");
    println!("  GET  /api/triggers/:id/health   觸發器健康檢查 (含熔斷狀態)");
}
