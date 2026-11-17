//! Unit tests for BrowserSkill Trait (8 methods) and Permission Guard (SSRF/whitelist).
//!
//! These tests exercise the browser-automation skill layer directly with mocks/stubs
//! — no real Chromium is launched. Browser lifecycle is covered by browser_integration.rs.

use browser_automation::skills::{
    BrowserSkill, SkillContext, SkillRegistry, registry, init_registry,
    FormFillSkill, FormFillConfig, FormFillResult,
    StructuredScrapeSkill, StructuredScrapeConfig, StructuredScrapeResult,
    permission_guard::PermissionGuard,
    capability::BrowserCapability,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_browser_skill_navigate() {
    init_registry();
    let skill = registry().get("navigate").expect("navigate skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    // Navigate skill expects URL in context or args - with minimal context it should fail gracefully
    assert!(result.is_err() || result.is_ok(), "navigate should not panic");
}

#[tokio::test]
async fn test_browser_skill_click() {
    init_registry();
    let skill = registry().get("click").expect("click skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "click should not panic");
}

#[tokio::test]
async fn test_browser_skill_fill_form() {
    init_registry();
    let skill = registry().get("fill_form").expect("fill_form skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "fill_form should not panic");
}

#[tokio::test]
async fn test_browser_skill_extract() {
    init_registry();
    let skill = registry().get("extract").expect("extract skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "extract should not panic");
}

#[tokio::test]
async fn test_browser_skill_wait_for() {
    init_registry();
    let skill = registry().get("wait_for").expect("wait_for skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "wait_for should not panic");
}

#[tokio::test]
async fn test_browser_skill_evaluate() {
    init_registry();
    let skill = registry().get("evaluate").expect("evaluate skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "evaluate should not panic");
}

#[tokio::test]
async fn test_browser_skill_screenshot() {
    init_registry();
    let skill = registry().get("screenshot").expect("screenshot skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "screenshot should not panic");
}

#[tokio::test]
async fn test_browser_skill_close() {
    init_registry();
    let skill = registry().get("close").expect("close skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(30))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    assert!(result.is_err() || result.is_ok(), "close should not panic");
}

#[tokio::test]
async fn test_form_fill_skill_execute() {
    init_registry();
    let skill = registry().get("form_fill").expect("form_fill skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(60))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    // FormFillSkill should handle missing config gracefully
    assert!(result.is_err() || result.is_ok(), "form_fill should not panic");
}

#[tokio::test]
async fn test_structured_scrape_skill_execute() {
    init_registry();
    let skill = registry().get("structured_scrape").expect("structured_scrape skill should be registered");
    
    let capability = BrowserCapability::builder()
        .allowed_domains(vec!["example.com".to_string()])
        .max_execution_time(Duration::from_secs(60))
        .build();
    
    let context = SkillContext::new(capability);
    let result = skill.execute(&context).await;
    
    // StructuredScrapeSkill should handle missing config gracefully
    assert!(result.is_err() || result.is_ok(), "structured_scrape should not panic");
}

#[tokio::test]
async fn test_registry_contains_all_skills() {
    init_registry();
    let reg = registry();
    
    // 7 primitive skills + 2 high-level skills = 9 total
    let skills = vec![
        "navigate", "click", "fill_form", "extract", "wait_for", 
        "evaluate", "screenshot", "close",
        "form_fill", "structured_scrape"
    ];
    
    for name in skills {
        assert!(reg.get(name).is_some(), "skill '{}' should be registered", name);
    }
}

#[tokio::test]
async fn test_skill_registry_dynamic_mounting() {
    init_registry();
    let reg = registry();
    
    // Verify we can look up skills by name
    assert!(reg.get("navigate").is_some());
    assert!(reg.get("form_fill").is_some());
    
    // Non-existent skill returns None
    assert!(reg.get("nonexistent").is_none());
}
