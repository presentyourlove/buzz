//! Unit tests for BuzzHarness & Multi-Backend Architecture

use buzz_agent::harness::{
    native::NativeHarness, BuzzHarness, HarnessError, HarnessRegistry,
};
use std::sync::Arc;

#[tokio::test]
async fn test_harness_registry_registration_and_dispatch() {
    let mut registry = HarnessRegistry::new();

    let openrouter = Arc::new(NativeHarness::new(
        "openrouter",
        "nvidia/nemotron-3-ultra-550b-a55b:free",
    ));
    let gemini = Arc::new(NativeHarness::new(
        "gemini_proxy",
        "models/gemini-2.5-flash",
    ));
    let local = Arc::new(NativeHarness::new(
        "local_gguf",
        "qwen2.5-coder-7b-instruct",
    ));

    registry.register(openrouter);
    registry.register(gemini);
    registry.register(local);

    let backends = registry.list_backends();
    assert_eq!(backends.len(), 3);
    assert!(backends.contains(&"openrouter".to_string()));
    assert!(backends.contains(&"gemini_proxy".to_string()));
    assert!(backends.contains(&"local_gguf".to_string()));

    // 測試預設後端調度
    let default_harness = registry.get(None).expect("Default harness should exist");
    assert_eq!(default_harness.backend_name(), "openrouter");

    let out = default_harness
        .execute_turn(None, "You are helpful.", "Hello Buzz!")
        .await
        .expect("Execution should succeed");
    assert!(out.text.contains("Hello Buzz!"));
    assert_eq!(out.model_id, "nvidia/nemotron-3-ultra-550b-a55b:free");

    // 測試切換後端
    let gemini_harness = registry
        .get(Some("gemini_proxy"))
        .expect("Gemini harness should exist");
    assert_eq!(gemini_harness.backend_name(), "gemini_proxy");

    let out_gemini = gemini_harness
        .execute_turn(Some("models/gemini-1.5-pro"), "System prompt", "Test switch")
        .await
        .expect("Execution should succeed");
    assert_eq!(out_gemini.model_id, "models/gemini-1.5-pro");
}

#[tokio::test]
async fn test_harness_invalid_backend() {
    let mut registry = HarnessRegistry::new();
    let res = registry.set_default_backend("non_existent");
    assert!(matches!(res, Err(HarnessError::ModelNotFound(_))));
}

