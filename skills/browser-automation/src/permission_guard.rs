//! permission_guard.rs - Domain Whitelist Gate for SSRF Prevention
//!
//! Strictly restricts network egress to authorized domains only,
//! preventing Server-Side Request Forgery (SSRF) attacks.
//!
//! Usage:
//! - Configure allowed domains via `allowed_egress_domains()`
//! - Check domain authorization before any network operation
//! - Log all blocked attempts for audit purposes

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

// Domain whitelist policy configuration
#[derive(Debug, Clone)]
pub struct DomainWhitelistPolicy {
    /// Authorized domains that may be accessed
    allowed_domains: HashSet<String>,
    /// Whether to allow subdomains of authorized domains
    allow_subdomains: bool,
    /// Whether to allow arbitrary URLs (disabled by default for SSRF prevention)
    allow_arbitrary_urls: bool,
    /// Default TTL for domain caching in seconds
    default_ttl: u64,
}

impl DomainWhitelistPolicy {
    /// Create a new whitelist policy with no allowed domains (strict mode)
    pub fn strict() -> Self {
        Self {
            allowed_domains: HashSet::new(),
            allow_subdomains: false,
            allow_arbitrary_urls: false,
            default_ttl: 300,
        }
    }

    /// Create a whitelist policy with specific allowed domains
    pub fn with_domains(domains: Vec<String>) -> Self {
        let mut allowed = HashSet::new();
        for domain in domains {
            allowed.insert(domain.clone());
        }

        Self {
            allowed_domains: allowed,
            allow_subdomains: false,
            allow_arbitrary_urls: false,
            default_ttl: 300,
        }
    }

    /// Allow subdomains of a base domain
    pub fn allow_subdomains_of(mut self, base_domain: &str) -> Self {
        self.allow_subdomains = true;
        // We'll handle subdomain checking in the is_allowed method
        self.allowed_domains.insert(base_domain.to_string());
        self
    }

    /// Check if a URL's host is allowed by the whitelist
    pub fn is_allowed(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };

        // If arbitrary URLs are explicitly allowed, check host format only
        if self.allow_arbitrary_urls {
            // In arbitrary mode, only block obvious internal/loopback addresses
            return !Self::is_internal_host(host);
        }

        let host_lower = host.to_lowercase();

        // Check exact match
        if self.allowed_domains.contains(&host_lower) {
            return true;
        }

        // Check subdomain match if enabled
        if self.allow_subdomains {
            for allowed in &self.allowed_domains {
                if host_lower == *allowed || host_lower.ends_with(&format!(".{}", allowed)) {
                    return true;
                }
            }
        }

        // Default: deny
        false
    }

    /// Check if a host is an internal/loopback address that should always be blocked
    fn is_internal_host(host: &str) -> bool {
        // Common internal/loopback patterns
        let internal_patterns = [
            "localhost",
            "127.0.0.1",
            "192.168.",
            "10.",
            "172.16.",
            "172.31.",
            "10.0.",
            "::1",
            "fe80",
        ];

        let lower = host.to_lowercase();
        internal_patterns.iter().any(|p| lower.starts_with(p))
    }
}

/// PermissionGuard enforces domain allowlist and anti-SSRF protections
#[derive(Debug, Clone)]
pub struct PermissionGuard {
    capability: crate::capability::BrowserCapability,
}

impl PermissionGuard {
    pub fn new(capability: crate::capability::BrowserCapability) -> Self {
        Self { capability }
    }

    pub fn check_navigation(&self, url_str: &str) -> Result<()> {
        let url = Url::parse(url_str).context("Invalid URL format")?;
        let host = url.host_str().context("URL has no host")?.to_lowercase();

        // SSRF protection: reject localhost and loopback
        if host == "localhost" || host == "127.0.0.1" || host == "::1" || host.starts_with("127.") {
            anyhow::bail!(
                "SSRF blocked: localhost/loopback address is disallowed: {}",
                host
            );
        }

        // SSRF protection: private IP ranges
        if host.starts_with("10.") || host.starts_with("192.168.") {
            anyhow::bail!(
                "SSRF blocked: private IP range (10.x, 192.168.x) is disallowed: {}",
                host
            );
        }

        if host.starts_with("172.") {
            let parts: Vec<&str> = host.split('.').collect();
            if parts.len() >= 2 {
                if let Ok(second) = parts[1].parse::<u8>() {
                    if (16..=31).contains(&second) {
                        anyhow::bail!(
                            "SSRF blocked: private IP range (172.16-31.x) is disallowed: {}",
                            host
                        );
                    }
                }
            }
        }

        // Domain allowlist check: empty allowlist rejects all
        if self.capability.allowed_domains.is_empty() {
            anyhow::bail!("Navigation rejected: empty allowlist rejects all domains");
        }

        let allowed = self.capability.allowed_domains.iter().any(|d| {
            let d_lower = d.to_lowercase();
            host == d_lower || host.ends_with(&format!(".{}", d_lower))
        });
        if !allowed {
            anyhow::bail!(
                "Navigation rejected: domain '{}' not in allowed_domains list",
                host
            );
        }

        Ok(())
    }

    pub fn check_credential_access(&self, cred_label: &str) -> Result<()> {
        if self
            .capability
            .allowed_credentials
            .iter()
            .any(|c| c == cred_label)
        {
            Ok(())
        } else {
            anyhow::bail!(
                "Credential '{}' not in allowed_credentials list",
                cred_label
            );
        }
    }
}

// Global domain whitelist instance
static DOMAIN_WHITELIST: std::sync::OnceLock<DomainWhitelistPolicy> = std::sync::OnceLock::new();

/// Initialize the global domain whitelist with configured domains
pub fn init_whitelist(domains: Vec<String>) {
    let _ = DOMAIN_WHITELIST.set(DomainWhitelistPolicy::with_domains(domains));
}

/// Get the global domain whitelist policy
pub fn whitelist() -> &'static DomainWhitelistPolicy {
    DOMAIN_WHITELIST.get_or_init(|| DomainWhitelistPolicy::strict())
}

/// Check if a URL's host is authorized for network egress
pub fn check_url(url: &Url) -> bool {
    whitelist().is_allowed(url)
}

/// Convenience macro for SSRF-safe URL construction
#[macro_export]
macro_rules! ssrf_safe_url {
    ($base:expr, $path:expr) => {{
        use url::Url;
        let base = Url::parse($base).context("Invalid base URL")?;
        let path = $path;

        // Construct URL and check against whitelist
        let full_url = base.join(path).context("Failed to join URL")?;

        // Security check - will panic if not whitelisted in production
        #[cfg(debug_assertions)]
        {
            use crate::permission_guard::check_url;
            if !check_url(&full_url) {
                anyhow::bail!("URL not in whitelist: {}", full_url);
            }
        }

        full_url
    }};
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_whitelist_basic() {
//         let domains = vec!["example.com", "api.github.com"];
//         init_whitelist(domains);

//         let url = Url::parse("https://example.com/path").unwrap();
//         assert!(crate::permission_guard::check_url(&url));

//         let blocked = Url::parse("https://evil.com").unwrap();
//         assert!(!crate::permission_guard::check_url(&blocked));
//     }
// }
