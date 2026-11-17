//! Capability-Based Security Model
//! 
//! Defines capabilities for browser automation skills, enforcing least-privilege
//! access to domains, credentials, actions, and network resources.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

/// Browser action types that can be permitted or denied
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAction {
    /// Navigate to URLs
    Navigate,
    /// Click elements
    Click,
    /// Type text into inputs
    TypeText,
    /// Take screenshots
    Screenshot,
    /// Generate PDF
    Pdf,
    /// Evaluate JavaScript
    EvalJS,
    /// Wait for elements/conditions
    WaitFor,
    /// Download files
    Download,
    /// Upload files
    Upload,
    /// Manage cookies
    Cookies,
    /// Handle dialogs (alert, confirm, prompt)
    Dialogs,
    /// Access clipboard
    Clipboard,
    /// File system access (via downloads)
    FileSystem,
}

/// Network egress policy for capability enforcement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkEgressPolicy {
    /// No network access allowed
    None,
    /// Only allow specific domains
    AllowedDomains(Vec<String>),
    /// Unrestricted network access (use with caution)
    Unrestricted,
}

impl Default for NetworkEgressPolicy {
    fn default() -> Self {
        Self::None
    }
}

/// Capability token for browser automation
/// 
/// Each skill execution receives a capability token that defines
/// what actions it can perform, what domains it can access, and
/// what credentials it can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserCapability {
    /// Unique capability ID
    pub id: Uuid,
    /// Allowed domains for navigation and network requests
    pub allowed_domains: Vec<String>,
    /// Allowed credential IDs from the vault
    pub allowed_credentials: Vec<String>,
    /// Maximum execution time for a single operation
    pub max_execution_time: Duration,
    /// Allowed browser actions
    pub allowed_actions: Vec<BrowserAction>,
    /// Network egress policy
    pub network_egress: NetworkEgressPolicy,
    /// Human-readable description
    pub description: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Expiration timestamp (optional)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl BrowserCapability {
    /// Create a new capability builder
    pub fn builder() -> CapabilityBuilder {
        CapabilityBuilder::new()
    }
    
    /// Validate if a URL is allowed for navigation
    pub fn validate_navigation(&self, url: &str) -> Result<()> {
        let parsed = Url::parse(url)
            .with_context(|| format!("Invalid URL: {}", url))?;
        
        let host = parsed.host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host: {}", url))?;
        
        // Check if domain is allowed
        if !self.allowed_domains.is_empty() {
            let allowed = self.allowed_domains.iter().any(|d| {
                // Exact match or subdomain match
                host == d.as_str() || host.ends_with(&format!(".{}", d))
            });
            
            if !allowed {
                return Err(anyhow::anyhow!(
                    "Navigation to '{}' not allowed. Allowed domains: {:?}",
                    url, self.allowed_domains
                ));
            }
        }
        
        // Check if Navigate action is allowed
        if !self.allowed_actions.contains(&BrowserAction::Navigate) {
            return Err(anyhow::anyhow!("Navigate action not permitted by capability"));
        }
        
        Ok(())
    }
    
    /// Check if a domain is allowed
    pub fn allows_domain(&self, domain: &str) -> bool {
        if self.allowed_domains.is_empty() {
            return true;
        }
        self.allowed_domains.iter().any(|d| {
            domain == d.as_str() || domain.ends_with(&format!(".{}", d))
        })
    }

    /// Check if an action is allowed
    pub fn allows_action(&self, action: BrowserAction) -> bool {
        self.allowed_actions.contains(&action)
    }
    
    /// Check if a credential is allowed
    pub fn allows_credential(&self, credential_id: &str) -> bool {
        self.allowed_credentials.iter().any(|c| c == credential_id)
    }
    
    /// Check if capability has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| exp < chrono::Utc::now())
    }
    
    /// Get remaining time until expiration
    pub fn remaining_time(&self) -> Option<Duration> {
        self.expires_at
            .and_then(|exp| (exp - chrono::Utc::now()).to_std().ok())
    }
}

/// Builder for creating capabilities
#[derive(Debug, Clone, Default)]
pub struct CapabilityBuilder {
    allowed_domains: Vec<String>,
    allowed_credentials: Vec<String>,
    max_execution_time: Duration,
    allowed_actions: Vec<BrowserAction>,
    network_egress: NetworkEgressPolicy,
    description: String,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CapabilityBuilder {
    pub fn new() -> Self {
        Self {
            max_execution_time: Duration::from_secs(60),
            ..Default::default()
        }
    }
    
    /// Set allowed domains
    pub fn allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = domains;
        self
    }
    
    /// Add a single allowed domain
    pub fn allow_domain(mut self, domain: impl Into<String>) -> Self {
        self.allowed_domains.push(domain.into());
        self
    }
    
    /// Set allowed credentials
    pub fn allowed_credentials(mut self, credentials: Vec<String>) -> Self {
        self.allowed_credentials = credentials;
        self
    }
    
    /// Add a single allowed credential
    pub fn allow_credential(mut self, credential_id: impl Into<String>) -> Self {
        self.allowed_credentials.push(credential_id.into());
        self
    }
    
    /// Set maximum execution time
    pub fn max_execution_time(mut self, duration: Duration) -> Self {
        self.max_execution_time = duration;
        self
    }
    
    /// Set allowed actions
    pub fn allowed_actions(mut self, actions: Vec<BrowserAction>) -> Self {
        self.allowed_actions = actions;
        self
    }
    
    /// Allow a single action
    pub fn allow_action(mut self, action: BrowserAction) -> Self {
        self.allowed_actions.push(action);
        self
    }
    
    /// Allow multiple actions at once
    pub fn allow_actions(mut self, actions: &[BrowserAction]) -> Self {
        self.allowed_actions.extend_from_slice(actions);
        self
    }
    
    /// Set network egress policy
    pub fn network_egress(mut self, policy: NetworkEgressPolicy) -> Self {
        self.network_egress = policy;
        self
    }
    
    /// Allow unrestricted network access
    pub fn unrestricted_network(mut self) -> Self {
        self.network_egress = NetworkEgressPolicy::Unrestricted;
        self
    }
    
    /// Allow specific domains for network egress
    pub fn allowed_egress_domains(mut self, domains: Vec<String>) -> Self {
        self.network_egress = NetworkEgressPolicy::AllowedDomains(domains);
        self
    }
    
    /// Set description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
    
    /// Set expiration time
    pub fn expires_at(mut self, expires_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
    
    /// Set expiration duration from now
    pub fn expires_in(mut self, duration: Duration) -> Self {
        self.expires_at = Some(chrono::Utc::now() + chrono::Duration::from_std(duration).unwrap_or_default());
        self
    }
    
    /// Build the capability
    pub fn build(self) -> BrowserCapability {
        BrowserCapability {
            id: Uuid::new_v4(),
            allowed_domains: self.allowed_domains,
            allowed_credentials: self.allowed_credentials,
            max_execution_time: self.max_execution_time,
            allowed_actions: self.allowed_actions,
            network_egress: self.network_egress,
            description: self.description,
            created_at: chrono::Utc::now(),
            expires_at: self.expires_at,
        }
    }
}

/// A set of capabilities - typically used to represent multiple capabilities for a session
/// Can be used to check if any/all capabilities in the set allow a specific action
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    /// The capabilities in this set
    pub capabilities: Vec<BrowserCapability>,
}

impl CapabilitySet {
    /// Create a new empty capability set
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a capability set from a single capability
    pub fn from_single(capability: BrowserCapability) -> Self {
        Self {
            capabilities: vec![capability],
        }
    }
    
    /// Add a capability to the set
    pub fn add(&mut self, capability: BrowserCapability) {
        self.capabilities.push(capability);
    }
    
    /// Check if any capability in the set allows the given action
    pub fn allows_action(&self, action: BrowserAction) -> bool {
        self.capabilities.iter().any(|c| c.allows_action(action))
    }
    
    /// Check if any capability in the set allows access to the given domain
    pub fn allows_domain(&self, domain: &str) -> bool {
        self.capabilities.iter().any(|c| c.allows_domain(domain))
    }
    
    /// Check if any capability in the set allows the given credential
    pub fn allows_credential(&self, credential_id: &str) -> bool {
        self.capabilities.iter().any(|c| c.allows_credential(credential_id))
    }
    
    /// Get the minimum max_execution_time across all capabilities
    pub fn min_execution_time(&self) -> Option<Duration> {
        self.capabilities.iter()
            .map(|c| c.max_execution_time)
            .min()
    }
    
    /// Get the most permissive network egress policy
    pub fn network_egress(&self) -> NetworkEgressPolicy {
        // Priority: Unrestricted > AllowedDomains > None
        for cap in &self.capabilities {
            match &cap.network_egress {
                NetworkEgressPolicy::Unrestricted => return NetworkEgressPolicy::Unrestricted,
                _ => {}
            }
        }
        for cap in &self.capabilities {
            if let NetworkEgressPolicy::AllowedDomains(_) = &cap.network_egress {
                return cap.network_egress.clone();
            }
        }
        NetworkEgressPolicy::None
    }
    
    /// Get all allowed domains from all capabilities
    pub fn all_allowed_domains(&self) -> Vec<String> {
        self.capabilities.iter()
            .flat_map(|c| c.allowed_domains.clone())
            .collect()
    }
    
    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
    
    /// Get the number of capabilities in the set
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }
}
      
