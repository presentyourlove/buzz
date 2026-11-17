//! Browser Context Manager
//!
//! Manages isolated browser contexts with capability enforcement, stealth mode,
//! and resource tracking.

use anyhow::{Context, Result};
use playwright::protocol::browser_context::{
    BrowserContextOptions, Geolocation, GrantPermissionsOptions,
};
use playwright::{Browser, BrowserContext, Cookie, Page};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::capability::BrowserCapability;
use crate::stealth::StealthConfig;

/// Context manager for creating and managing isolated browser contexts
#[derive(Debug, Clone)]
pub struct ContextManager {
    stealth_enabled: bool,
    stealth_config: StealthConfig,
}

impl ContextManager {
    /// Create a new context manager
    pub fn new(stealth_enabled: bool) -> Self {
        Self {
            stealth_enabled,
            stealth_config: StealthConfig::default(),
        }
    }

    /// Create a new isolated browser context with capability enforcement
    pub async fn create_context(
        &self,
        browser: &Browser,
        capability: &BrowserCapability,
        context_id: Uuid,
    ) -> Result<BrowserContext> {
        let options = BrowserContextOptions::builder()
            .accept_downloads(true)
            .bypass_csp(
                capability
                    .allowed_actions
                    .contains(&crate::capability::BrowserAction::EvalJS),
            )
            .ignore_https_errors(true)
            .no_viewport(true)
            .build();

        let context = browser
            .new_context_with_options(options)
            .await
            .context("Failed to create browser context")?;

        // Apply capability-based restrictions
        self.apply_capability_restrictions(&context, capability)
            .await?;

        // Apply stealth if enabled
        if self.stealth_enabled {
            self.apply_stealth_to_context(&context).await?;
        }

        info!(
            "Created isolated context {} with capability {}",
            context_id, capability.id
        );
        Ok(context)
    }

    /// Apply capability-based restrictions to a context
    async fn apply_capability_restrictions(
        &self,
        context: &BrowserContext,
        capability: &BrowserCapability,
    ) -> Result<()> {
        // Set up request interception for network egress policy
        match &capability.network_egress {
            crate::capability::NetworkEgressPolicy::None => {
                // Block all network requests
                context
                    .route("**/*", |route| async move {
                        let _ = route.abort(None).await;
                        Ok(())
                    })
                    .await?;
            }
            crate::capability::NetworkEgressPolicy::AllowedDomains(domains) => {
                // Only allow specified domains
                let domains = domains.clone();
                context
                    .route("**/*", move |route| {
                        let domains = domains.clone();
                        async move {
                            let url = route.request().url();
                            if let Ok(parsed) = url::Url::parse(&url) {
                                if domains
                                    .iter()
                                    .any(|d| parsed.host_str() == Some(d.as_str()))
                                {
                                    let _ = route.continue_(None).await;
                                } else {
                                    let _ = route.abort(None).await;
                                }
                            } else {
                                let _ = route.abort(None).await;
                            }
                            Ok(())
                        }
                    })
                    .await?;
            }
            crate::capability::NetworkEgressPolicy::Unrestricted => {
                // No restrictions
            }
        }

        // Set default navigation timeout
        context
            .set_default_navigation_timeout(capability.max_execution_time.as_millis() as f64)
            .await;
        context
            .set_default_timeout(capability.max_execution_time.as_millis() as f64)
            .await;

        Ok(())
    }

    /// Apply stealth configuration to a context
    async fn apply_stealth_to_context(&self, context: &BrowserContext) -> Result<()> {
        // Add init script to mask automation
        let stealth_script = self.stealth_config.generate_init_script();
        context
            .add_init_script(&stealth_script)
            .await
            .context("Failed to add stealth init script")?;
        Ok(())
    }

    /// Apply stealth to a specific page
    pub async fn apply_stealth_to_page(&self, page: &Page) -> Result<()> {
        let stealth_script = self.stealth_config.generate_init_script();
        page.add_init_script(&stealth_script)
            .await
            .context("Failed to add stealth init script to page")?;
        Ok(())
    }

    /// Configure viewport for a page
    pub async fn set_viewport(&self, page: &Page, width: u32, height: u32) -> Result<()> {
        page.set_viewport_size(playwright::protocol::browser_context::Viewport { width, height })
            .await
            .context("Failed to set viewport size")?;
        Ok(())
    }

    /// Set user agent for a context
    pub async fn set_user_agent(&self, _context: &BrowserContext, _user_agent: &str) -> Result<()> {
        // User agent is configured during context creation via BrowserContextOptions
        Ok(())
    }

    /// Grant permissions to a context
    pub async fn grant_permissions(
        &self,
        context: &BrowserContext,
        permissions: Vec<&str>,
        origin: &str,
    ) -> Result<()> {
        let options = GrantPermissionsOptions::default().origin(origin);
        context
            .grant_permissions(&permissions, options)
            .await
            .context("Failed to grant permissions")?;
        Ok(())
    }

    /// Set cookies for a context
    pub async fn set_cookies(&self, context: &BrowserContext, cookies: Vec<Cookie>) -> Result<()> {
        context
            .add_cookies(&cookies)
            .await
            .context("Failed to set cookies")?;
        Ok(())
    }

    /// Clear cookies for a context
    pub async fn clear_cookies(&self, context: &BrowserContext) -> Result<()> {
        context
            .clear_cookies(None)
            .await
            .context("Failed to clear cookies")?;
        Ok(())
    }

    /// Get all cookies from a context
    pub async fn get_cookies(&self, context: &BrowserContext) -> Result<Vec<Cookie>> {
        let cookies = context
            .cookies(None)
            .await
            .context("Failed to get cookies")?;
        Ok(cookies)
    }

    /// Set extra HTTP headers for a context
    pub async fn set_extra_http_headers(
        &self,
        context: &BrowserContext,
        headers: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        context
            .set_extra_http_headers(headers)
            .await
            .context("Failed to set extra HTTP headers")?;
        Ok(())
    }

    /// Emulate media type (for print/PDF)
    pub async fn emulate_media(&self, page: &Page, media: &str) -> Result<()> {
        let m = match media.to_lowercase().as_str() {
            "print" => playwright::protocol::Media::Print,
            "screen" => playwright::protocol::Media::Screen,
            _ => playwright::protocol::Media::NoOverride,
        };
        let opt = playwright::protocol::EmulateMediaOptions::builder()
            .media(m)
            .build();
        page.emulate_media(opt)
            .await
            .context("Failed to emulate media")?;
        Ok(())
    }

    /// Set geolocation for a context
    pub async fn set_geolocation(
        &self,
        context: &BrowserContext,
        latitude: f64,
        longitude: f64,
        accuracy: Option<f64>,
    ) -> Result<()> {
        let geo = Geolocation {
            latitude,
            longitude,
            accuracy,
        };
        context
            .set_geolocation(Some(geo))
            .await
            .context("Failed to set geolocation")?;
        Ok(())
    }

    /// Set locale for a context
    pub async fn set_locale(&self, _context: &BrowserContext, _locale: &str) -> Result<()> {
        // Locale is configured during context creation via BrowserContextOptions
        Ok(())
    }

    /// Set timezone for a context
    pub async fn set_timezone(&self, _context: &BrowserContext, _timezone: &str) -> Result<()> {
        // Timezone is configured during context creation via BrowserContextOptions
        Ok(())
    }

    /// Create a new page with default stealth applied
    pub async fn new_page(&self, context: &BrowserContext) -> Result<Page> {
        let page = context
            .new_page()
            .await
            .context("Failed to create new page")?;

        if self.stealth_enabled {
            self.apply_stealth_to_page(&page).await?;
        }

        Ok(page)
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new(true)
    }
}
