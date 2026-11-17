//! Integration tests for BrowserPool — Chromium launch and safe context release.
//!
//! These tests exercise the browser_pool crate-level API directly (not the ACP wire),
//! verifying that Chromium starts, contexts can be created/navigated/screenshotted,
//! and contexts are safely released back to the pool on close.

use buzz_agent::browser_pool::{BrowserPool, ContextConfig};
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper: create a temp screenshot path inside a TempDir.
fn screenshot_path(tmp: &TempDir, name: &str) -> PathBuf {
    tmp.path().join(name)
}

#[tokio::test]
async fn test_browser_pool_launch_and_shutdown() {
    let pool = BrowserPool::new().await.expect("BrowserPool::new should start Chromium");

    // Pool should start with zero in-use browsers
    let stats = pool.stats().await;
    assert_eq!(stats.in_use, 0, "pool should start with no browsers in use");

    // Shutdown should succeed
    pool.shutdown()
        .await
        .expect("BrowserPool::shutdown should succeed");
}

#[tokio::test]
async fn test_context_lifecycle_open_navigate_screenshot_close() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let pool = BrowserPool::new().await.expect("BrowserPool::new should start Chromium");

    // 1. Create a context
    let context_id = pool
        .create_context()
        .await
        .expect("create_context should return a context_id");
    assert!(!context_id.is_empty(), "context_id should not be empty");

    // Verify browser is now in use
    let stats = pool.stats().await;
    assert_eq!(stats.in_use, 1, "one browser should be in use after context creation");

    // 2. Navigate to about:blank (no network dependency)
    pool.navigate(&context_id, "about:blank")
        .await
        .expect("navigate to about:blank should succeed");

    // 3. Capture a screenshot
    let shot_path = screenshot_path(&tmp, "test_screenshot.png");
    pool.screenshot(&context_id, shot_path.to_str().unwrap())
        .await
        .expect("screenshot should succeed");
    assert!(
        shot_path.exists(),
        "screenshot file should exist at {:?}",
        shot_path
    );
    let size = std::fs::metadata(&shot_path).unwrap().len();
    assert!(size > 0, "screenshot file should be non-empty");

    // 4. Close the context — browser should be released back to pool
    pool.close_context(&context_id)
        .await
        .expect("close_context should succeed");

    let stats = pool.stats().await;
    assert_eq!(
        stats.in_use, 0,
        "no browsers should be in use after context close"
    );

    // Shutdown cleanly
    pool.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn test_safe_context_release_prevents_leak() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let pool = BrowserPool::new().await.expect("BrowserPool::new should start Chromium");

    // Create and close multiple contexts sequentially — each must release its browser
    for i in 0..3 {
        let context_id = pool
            .create_context()
            .await
            .expect("create_context should succeed");

        pool.navigate(&context_id, "about:blank")
            .await
            .expect("navigate should succeed");

        let shot_path = screenshot_path(&tmp, &format!("shot_{i}.png"));
        pool.screenshot(&context_id, shot_path.to_str().unwrap())
            .await
            .expect("screenshot should succeed");

        pool.close_context(&context_id)
            .await
            .expect("close_context should succeed");

        // After each close, no browser should be in use
        let stats = pool.stats().await;
        assert_eq!(
            stats.in_use, 0,
            "browser must be released after context {i} close — potential leak"
        );
    }

    pool.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn test_context_config_default_viewport() {
    // ContextConfig::default should provide a sensible viewport (1280x720)
    let config = ContextConfig::default();
    assert_eq!(config.viewport, Some((1280, 720)));
}
