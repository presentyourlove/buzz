//! BrowserSkill Trait & Dynamic Mounting Mechanism
//! 
//! Defines the interface for all browser automation skills and provides
//! a dynamic registration mechanism for extending skill capabilities.

use anyhow::Result;
use std::any::Any;
use std::collections::HashMap;
use std::sync::OnceLock;

// Core trait for all browser skills
#[async_trait::async_trait]
pub trait BrowserSkill: Send + Sync + 'static {
    /// Returns the skill name for identification
    fn name(&self) -> &str;
    
    /// Returns whether this skill requires network access
    fn requires_network(&self) -> bool {
        false
    }
    
    /// Execute the skill with the given context
    async fn execute(&self, context: &crate::skills::SkillContext) -> Result<serde_json::Value>;
    
    /// As Any for type checking
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Generic skill stub for registry
#[derive(Debug, Clone)]
pub struct GenericSkill {
    pub name: String,
    pub requires_network: bool,
}

#[async_trait::async_trait]
impl BrowserSkill for GenericSkill {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn requires_network(&self) -> bool {
        self.requires_network
    }
    
    async fn execute(&self, _context: &crate::skills::SkillContext) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "status": "ok",
            "skill": self.name,
        }))
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// Dynamic skill registry for mounting skills at runtime
pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn BrowserSkill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        SkillRegistry {
            skills: HashMap::new(),
        }
    }

    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        for name in &[
            "navigate", "click", "fill_form", "extract", "wait_for", 
            "evaluate", "screenshot", "close", "form_fill", "structured_scrape"
        ] {
            reg.register(GenericSkill {
                name: name.to_string(),
                requires_network: true,
            });
        }
        reg
    }
    
    /// Register a skill dynamically
    pub fn register<S: BrowserSkill + 'static>(&mut self, skill: S) {
        self.skills.insert(skill.name().to_string(), Box::new(skill));
    }
    
    /// Get a skill by name
    pub fn get(&self, name: &str) -> Option<&dyn BrowserSkill> {
        self.skills.get(name).map(|s| s.as_ref())
    }
    
    /// List all registered skill names
    pub fn list(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }
    
    /// Check if a skill is registered
    pub fn contains(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }
    
    /// Get all skills as trait objects
    pub fn all(&self) -> Vec<&dyn BrowserSkill> {
        self.skills.values().map(|s| s.as_ref()).collect()
    }
}

// Global registry instance
static REGISTRY: OnceLock<SkillRegistry> = OnceLock::new();

// Initialize the global registry
pub fn init_registry() {
    let _ = REGISTRY.set(SkillRegistry::default_registry());
}

// Get the global registry (thread-safe lazy init)
pub fn registry() -> &'static SkillRegistry {
    REGISTRY.get_or_init(|| SkillRegistry::default_registry())
}

// Convenience macro for skill registration
#[macro_export]
macro_rules! register_skill {
    ($registry:expr, $skill:expr) => {
        $registry.register($skill)
    };
}
