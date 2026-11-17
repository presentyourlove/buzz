// capability_token.rs - Capability-based security model for Browser Automation Phase 2
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Capability-based security model for browser operations
/// Tokens are fine-grained authorizations, not role-based
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Allow browser navigation to URLs
    Navigate,
    /// Allow taking screenshots
    Screenshot,
    /// Allow clicking elements
    Click,
    /// Allow filling forms
    Fill,
    /// Allow extracting data from pages
    Extract,
    /// Allow waiting for elements/conditions
    Wait,
    /// Allow selecting dropdown options
    Select,
    /// Allow hovering elements
    Hover,
    /// Allow keypress simulation
    Keypress,
    /// Allow file upload
    Upload,
    /// Allow file download
    Download,
    /// Allow executing JavaScript
    ExecuteScript,
    /// Allow accessing network requests/responses
    Network,
    /// Allow cookie access
    Cookies,
    /// Allow localStorage access
    LocalStorage,
    /// Custom capability for extensibility
    Custom(String),
}

impl Capability {
    /// Get the string representation
    pub fn as_str(&self) -> &str {
        match self {
            Capability::Navigate => "navigate",
            Capability::Screenshot => "screenshot",
            Capability::Click => "click",
            Capability::Fill => "fill",
            Capability::Extract => "extract",
            Capability::Wait => "wait",
            Capability::Select => "select",
            Capability::Hover => "hover",
            Capability::Keypress => "keypress",
            Capability::Upload => "upload",
            Capability::Download => "download",
            Capability::ExecuteScript => "execute_script",
            Capability::Network => "network",
            Capability::Cookies => "cookies",
            Capability::LocalStorage => "local_storage",
            Capability::Custom(s) => s.as_str(),
        }
    }
}

/// Capability token for authorizing browser operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub token_id: Uuid,
    pub capabilities: HashSet<Capability>,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: serde_json::Value,
}

impl CapabilityToken {
    /// Create a new capability token
    pub fn new(capabilities: Vec<Capability>, expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        Self {
            token_id: Uuid::new_v4(),
            capabilities: capabilities.into_iter().collect(),
            issued_at: chrono::Utc::now(),
            expires_at,
            metadata: serde_json::Value::Null,
        }
    }

    /// Check if token has a specific capability
    pub fn has_capability(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }

    /// Check if token has any of the given capabilities
    pub fn has_any_capability(&self, capabilities: &[Capability]) -> bool {
        capabilities.iter().any(|c| self.capabilities.contains(c))
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| chrono::Utc::now() > exp)
    }

    /// Check if token is valid (not expired)
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Get remaining time until expiration
    pub fn time_until_expiry(&self) -> Option<chrono::Duration> {
        self.expires_at.map(|exp| exp - chrono::Utc::now())
    }

    /// Create a token with all capabilities (admin)
    pub fn admin(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        Self::new(
            vec![
                Capability::Navigate,
                Capability::Screenshot,
                Capability::Click,
                Capability::Fill,
                Capability::Extract,
                Capability::Wait,
                Capability::Select,
                Capability::Hover,
                Capability::Keypress,
                Capability::Upload,
                Capability::Download,
                Capability::ExecuteScript,
                Capability::Network,
                Capability::Cookies,
                Capability::LocalStorage,
            ],
            expires_at,
        )
    }

    /// Create a token with read-only capabilities
    pub fn read_only(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        Self::new(
            vec![
                Capability::Navigate,
                Capability::Screenshot,
                Capability::Extract,
                Capability::Wait,
            ],
            expires_at,
        )
    }

    /// Create a token with interaction capabilities
    pub fn interactive(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        Self::new(
            vec![
                Capability::Navigate,
                Capability::Screenshot,
                Capability::Click,
                Capability::Fill,
                Capability::Extract,
                Capability::Wait,
                Capability::Select,
                Capability::Hover,
                Capability::Keypress,
            ],
            expires_at,
        )
    }

    /// Create a token with form-filling capabilities
    pub fn form_filler(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        Self::new(
            vec![
                Capability::Navigate,
                Capability::Click,
                Capability::Fill,
                Capability::Select,
                Capability::Wait,
                Capability::Extract,
            ],
            expires_at,
        )
    }

    /// Create a token with script execution capabilities
    pub fn script_runner(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        Self::new(
            vec![
                Capability::Navigate,
                Capability::ExecuteScript,
                Capability::Extract,
                Capability::Wait,
                Capability::Network,
            ],
            expires_at,
        )
    }

    /// Add a capability to the token (returns new token)
    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    /// Remove a capability from the token (returns new token)
    pub fn without_capability(mut self, capability: &Capability) -> Self {
        self.capabilities.remove(capability);
        self
    }

    /// Merge with another token (union of capabilities)
    pub fn merge(&self, other: &CapabilityToken) -> CapabilityToken {
        let mut merged = self.capabilities.clone();
        merged.extend(other.capabilities.iter().cloned());
        CapabilityToken {
            token_id: Uuid::new_v4(),
            capabilities: merged,
            issued_at: chrono::Utc::now(),
            expires_at: self.expires_at.or(other.expires_at),
            metadata: serde_json::Value::Null,
        }
    }
}

/// Capability checker for runtime authorization
pub struct CapabilityChecker;

impl CapabilityChecker {
    /// Check if a token authorizes an operation
    pub fn check(token: &CapabilityToken, required: &Capability) -> Result<(), CapabilityError> {
        if token.is_expired() {
            return Err(CapabilityError::TokenExpired);
        }
        if !token.has_capability(required) {
            return Err(CapabilityError::MissingCapability(required.clone()));
        }
        Ok(())
    }

    /// Check multiple required capabilities (all must be present)
    pub fn check_all(token: &CapabilityToken, required: &[Capability]) -> Result<(), CapabilityError> {
        if token.is_expired() {
            return Err(CapabilityError::TokenExpired);
        }
        for cap in required {
            if !token.has_capability(cap) {
                return Err(CapabilityError::MissingCapability(cap.clone()));
            }
        }
        Ok(())
    }

    /// Check if at least one capability is present
    pub fn check_any(token: &CapabilityToken, required: &[Capability]) -> Result<(), CapabilityError> {
        if token.is_expired() {
            return Err(CapabilityError::TokenExpired);
        }
        if !token.has_any_capability(required) {
            return Err(CapabilityError::MissingAnyCapability(required.to_vec()));
        }
        Ok(())
    }
}

/// Capability-related errors
#[derive(Debug, thiserror::Error)]
pub enum CapabilityError {
    #[error("Token has expired")]
    TokenExpired,
    #[error("Missing required capability: {0:?}")]
    MissingCapability(Capability),
    #[error("Missing any of required capabilities: {0:?}")]
    MissingAnyCapability(Vec<Capability>),
    #[error("Token validation failed: {0}")]
    ValidationFailed(String),
}

/// Token factory for common use cases
pub struct TokenFactory;

impl TokenFactory {
    /// Create a token for basic browsing (navigate, screenshot, extract)
    pub fn basic_browsing(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> CapabilityToken {
        CapabilityToken::read_only(expires_at)
    }

    /// Create a token for form automation
    pub fn form_automation(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> CapabilityToken {
        CapabilityToken::form_filler(expires_at)
    }

    /// Create a token for interactive testing
    pub fn interactive_testing(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> CapabilityToken {
        CapabilityToken::interactive(expires_at)
    }

    /// Create a token for script-based automation
    pub fn script_automation(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> CapabilityToken {
        CapabilityToken::script_runner(expires_at)
    }

    /// Create a token for full access (admin)
    pub fn full_access(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> CapabilityToken {
        CapabilityToken::admin(expires_at)
    }

    /// Create a custom token with specific capabilities
    pub fn custom(capabilities: Vec<Capability>, expires_at: Option<chrono::DateTime<chrono::Utc>>) -> CapabilityToken {
        CapabilityToken::new(capabilities, expires_at)
    }
}
