use playwright_rs::{Playwright, Browser, BrowserContext, Page};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use thiserror::Error;
use uuid::Uuid;

/// Configuration for creating a new BrowserContext
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Whether to accept downloads
    pub accept_downloads: bool,
    /// Whether to bypass CSP
    pub bypass_csp: bool,
    /// Browser color scheme
    pub color_scheme: Option<String>,
    /// Device scale factor
    pub device_scale_factor: Option<f64>,
    /// Extra HTTP headers
    pub extra_http_headers: Option<Vec<(String, String)>>,
    /// Geolocation override
    pub geolocation: Option<(f64, f64)>,
    /// Whether to ignore HTTPS errors
    pub ignore_https_errors: bool,
    /// Whether to record HAR
    pub record_har: bool,
    /// HAR output path
    pub record_har_path: Option<String>,
    /// Locale
    pub locale: Option<String>,
    /// Offline mode
    pub offline: bool,
    /// Proxy settings
    pub proxy: Option<String>,
    /// Screen size (width, height)
    pub screen_size: Option<(u32, u32)>,
    /// Storage state (cookies, localStorage)
    pub storage_state: Option<String>,
    /// Timezone ID
    pub timezone_id: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Viewport (width, height)
    pub viewport: Option<(u32, u32)>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            accept_downloads: false,
            bypass_csp: false,
            color_scheme: None,
            device_scale_factor: None,
            extra_http_headers: None,
            geolocation: None,
            ignore_https_errors: false,
            record_har: false,
            record_har_path: None,
            locale: None,
            offline: false,
            proxy: None,
            screen_size: None,
            storage_state: None,
            timezone_id: None,
            user_agent: None,
            viewport: Some((1280, 720)),
        }
    }
}

/// Errors specific to browser context operations
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Context creation failed: {0}")]
    CreationFailed(String),
    #[error("Context already closed")]
    AlreadyClosed,
    #[error("Context not found: {0}")]
    NotFound(String),
    #[error("Browser pool error: {0}")]
    PoolError(#[from] BrowserPoolError),
}

/// RAII guard for BrowserContext - ensures cleanup on drop
pub struct ContextGuard<'a> {
    context: Option<BrowserContext>,
    pool: Option<&'a BrowserPool>,
    browser_id: Option<usize>,
    context_id: Option<String>,
}

impl<'a> ContextGuard<'a> {
    /// Create a new context guard
    pub fn new(context: BrowserContext, pool: &'a BrowserPool, browser_id: Option<usize>) -> Self {
        Self {
            context: Some(context),
            pool: Some(pool),
            browser_id,
            context_id: None,
        }
    }

    /// Get the wrapped BrowserContext
    pub fn context(&self) -> Option<&BrowserContext> {
        self.context.as_ref()
    }

    /// Get the context ID (GUID)
    pub fn context_id(&self) -> Option<&str> {
        self.context_id.as_deref()
    }

    /// Get the browser ID this context belongs to
    pub fn browser_id(&self) -> Option<usize> {
        self.browser_id
    }

    /// Check if context is closed
    pub fn is_closed(&self) -> bool {
        self.context.is_none()
    }

    /// Explicitly close the context
    pub async fn close(&mut self) -> Result<(), ContextError> {
        if let Some(context) = self.context.take() {
            context.close().await
                .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
            self.context_id = None;
            Ok(())
        } else {
            Err(ContextError::AlreadyClosed)
        }
    }

    /// Create a new page in this context
    pub async fn new_page(&self) -> Result<playwright_rs::Page, ContextError> {
        self.context
            .as_ref()
            .ok_or(ContextError::AlreadyClosed)?
            .new_page()
            .await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))
    }

    /// Get all pages in this context
    pub fn pages(&self) -> Vec<playwright_rs::Page> {
        self.context
            .as_ref()
            .map(|c| c.pages())
            .unwrap_or_default()
    }
}

impl<'a> Drop for ContextGuard<'a> {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            // Spawn a task to close the context asynchronously
            // Note: We can't await in Drop, so we spawn a detached task
            tokio::task::spawn(async move {
                let _ = context.close().await;
            });
        }
    }
}

/// BrowserPool manages a pool of Playwright browser instances for automation tasks.
/// Phase 1 Task 1.2: Browser Pool Manager with proper acquire/release semantics.
pub struct BrowserPool {
    inner: Arc<Mutex<PoolInner>>,
    config: BrowserPoolConfig,
    permits: Arc<Semaphore>,
}

struct PoolInner {
    playwright: Playwright,
    browsers: Vec<Option<PooledBrowser>>,
    available: Vec<usize>,
    next_id: usize,
    stats: PoolStats,
    shutdown: bool,
    /// Active contexts tracked by context_id for browser_open/screenshot/close tools
    contexts: HashMap<String, (BrowserContext, usize)>, // context_id -> (context, browser_id)
}

struct PooledBrowser {
    id: usize,
    browser: Browser,
    in_use: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub total: usize,
    pub available: usize,
    pub in_use: usize,
}

#[derive(Debug, Clone)]
pub struct BrowserPoolConfig {
    pub max_browsers: usize,
    pub acquire_timeout: Duration,
    pub headless: bool,
}

impl Default for BrowserPoolConfig {
    fn default() -> Self {
        Self {
            max_browsers: 4,
            acquire_timeout: Duration::from_secs(30),
            headless: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum BrowserPoolError {
    #[error("Pool exhausted: no browsers available within timeout")]
    AcquireTimeout { waited: Duration },
    #[error("Failed to launch browser: {0}")]
    LaunchFailed(String),
    #[error("Pool already shut down")]
    Shutdown,
    #[error("Playwright error: {0}")]
    Playwright(#[from] playwright_rs::Error),
}

pub struct PoolHandle {
    pool: Arc<Mutex<PoolInner>>,
    browser_id: Option<usize>,
}

impl BrowserPool {
    pub async fn new() -> Result<Self, BrowserPoolError> {
        // Install the Chromium revision matching the bundled playwright-rs driver
        // before launching, to avoid revision mismatch errors.
        playwright_rs::install_browsers(Some(&["chromium"])).await.ok();
        let playwright = Playwright::launch().await?;
        let config = BrowserPoolConfig::default();
        let permits = Arc::new(Semaphore::new(config.max_browsers));
        
        let inner = PoolInner {
            playwright,
            browsers: Vec::new(),
            available: Vec::new(),
            next_id: 0,
            stats: PoolStats::default(),
            shutdown: false,
            contexts: HashMap::new(),
        };
        
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            config,
            permits,
        })
    }

    pub async fn with_config(config: BrowserPoolConfig) -> Result<Self, BrowserPoolError> {
        // Install the Chromium revision matching the bundled playwright-rs driver
        // before launching, to avoid revision mismatch errors.
        playwright_rs::install_browsers(Some(&["chromium"])).await.ok();
        let playwright = Playwright::launch().await?;
        let permits = Arc::new(Semaphore::new(config.max_browsers));
        
        let inner = PoolInner {
            playwright,
            browsers: Vec::new(),
            available: Vec::new(),
            next_id: 0,
            stats: PoolStats::default(),
            shutdown: false,
            contexts: HashMap::new(),
        };
        
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            config,
            permits,
        })
    }

    pub async fn acquire(&self) -> Result<PoolHandle, BrowserPoolError> {
        {
            let inner = self.inner.lock().await;
            if inner.shutdown {
                return Err(BrowserPoolError::Shutdown);
            }
        }
        
        let permit = timeout(self.config.acquire_timeout, {
            self.permits.clone().acquire_owned()
        }).await
            .map_err(|_| BrowserPoolError::AcquireTimeout { waited: self.config.acquire_timeout })?
            .map_err(|_| BrowserPoolError::Shutdown)?;
        
        let mut inner = self.inner.lock().await;
        
        if inner.shutdown {
            drop(permit);
            return Err(BrowserPoolError::Shutdown);
        }
        
        // Try to get an available browser or create a new one
        let browser_id = self.acquire_or_create_browser(&mut inner).await?;
        
        inner.stats.in_use += 1;
        inner.stats.available = inner.browsers.iter().filter(|b| b.as_ref().map_or(false, |b| !b.in_use)).count();
        
        drop(permit);
        
        Ok(PoolHandle {
            pool: self.inner.clone(),
            browser_id: Some(browser_id),
        })
    }
    
    async fn acquire_or_create_browser(&self, inner: &mut PoolInner) -> Result<usize, BrowserPoolError> {
        // First, try to get an available browser
        if let Some(id) = inner.available.pop() {
            if let Some(browser) = &mut inner.browsers[id] {
                browser.in_use = true;
                return Ok(id);
            }
            // Slot exists but browser is None (shouldn't happen), fall through to create new
        }
        
        // Try to create a new browser if under limit
        if inner.browsers.len() < self.config.max_browsers {
            let browser = self.launch_browser(&inner.playwright).await?;
            let id = inner.next_id;
            inner.next_id += 1;
            inner.browsers.push(Some(PooledBrowser {
                id,
                browser,
                in_use: true,
            }));
            inner.stats.total += 1;
            return Ok(id);
        }
        
        // Pool exhausted
        Err(BrowserPoolError::AcquireTimeout { waited: self.config.acquire_timeout })
    }

    async fn launch_browser(&self, playwright: &Playwright) -> Result<Browser, BrowserPoolError> {
        let browser_type = playwright.chromium();
        let browser = browser_type
            .launch()
            .await
            .map_err(|e| BrowserPoolError::LaunchFailed(e.to_string()))?;
        Ok(browser)
    }

    pub async fn release(&self, handle: PoolHandle) {
        if let Some(browser_id) = handle.browser_id {
            let mut inner = self.inner.lock().await;
            if browser_id < inner.browsers.len() {
                if let Some(browser) = &mut inner.browsers[browser_id] {
                    browser.in_use = false;
                    inner.stats.in_use = inner.stats.in_use.saturating_sub(1);
                    inner.stats.available = inner.browsers.iter().filter(|b| b.as_ref().map_or(false, |b| !b.in_use)).count();
                    inner.available.push(browser_id);
                    self.permits.add_permits(1);
                }
            }
        }
    }

    pub async fn shutdown(&self) -> Result<(), BrowserPoolError> {
        let mut inner = self.inner.lock().await;
        inner.shutdown = true;
        for browser_opt in &mut inner.browsers {
            if let Some(browser) = browser_opt.take() {
                if !browser.in_use {
                    let _ = browser.browser.close().await;
                }
            }
        }
        inner.browsers.clear();
        inner.available.clear();
        inner.stats = PoolStats::default();
        Ok(())
    }

    pub async fn stats(&self) -> PoolStats {
        let inner = self.inner.lock().await;
        inner.stats.clone()
    }

    /// Create a new isolated BrowserContext from the pool
    /// Each context has its own cookies, localStorage, sessionStorage, and cache
    pub async fn new_context(&self, config: ContextConfig) -> Result<ContextGuard<'_>, ContextError> {
        let handle = self.acquire().await?;
        let browser_id = handle.browser_id();
        let browser = handle.browser().await.ok_or_else(|| ContextError::NotFound("Browser not found".to_string()))?;
        
        // Create a new context with the provided configuration
        let context = browser.new_context().await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        
        // Apply context configuration
        self.apply_context_config(&context, &config).await?;
        
        Ok(ContextGuard::new(context, self, browser_id))
    }

    /// Create a new context with default configuration
    pub async fn new_default_context(&self) -> Result<ContextGuard<'_>, ContextError> {
        self.new_context(ContextConfig::default()).await
    }

    /// Create a new context for browser automation tools, returning context_id
    /// This is used by browser_open builtin tool
    pub async fn create_context(&self) -> Result<String, ContextError> {
        let handle = self.acquire().await?;
        let browser_id = handle.browser_id().ok_or(ContextError::NotFound("No browser available".to_string()))?;
        let browser = handle.browser().await.ok_or_else(|| ContextError::NotFound("Browser not found".to_string()))?;
        
        let context = browser.new_context().await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        
        let context_id = Uuid::new_v4().to_string();
        
        let mut inner = self.inner.lock().await;
        inner.contexts.insert(context_id.clone(), (context, browser_id));
        
        Ok(context_id)
    }

    /// Navigate to a URL in the specified context
    pub async fn navigate(&self, context_id: &str, url: &str) -> Result<(), ContextError> {
        let mut inner = self.inner.lock().await;
        let (context, _browser_id) = inner.contexts.get_mut(context_id)
            .ok_or_else(|| ContextError::NotFound(format!("Context not found: {}", context_id)))?;
        
        let page = context.new_page().await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        
        page.goto(url, None).await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        
        Ok(())
    }

    /// Take a screenshot of the current page in the specified context
    pub async fn screenshot(&self, context_id: &str, path: &str) -> Result<(), ContextError> {
        let mut inner = self.inner.lock().await;
        let (context, _browser_id) = inner.contexts.get_mut(context_id)
            .ok_or_else(|| ContextError::NotFound(format!("Context not found: {}", context_id)))?;
        
        let pages = context.pages();
        let page = pages.first()
            .ok_or_else(|| ContextError::NotFound("No pages in context".to_string()))?;
        
        let screenshot_bytes = page.screenshot(None).await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        
        tokio::fs::write(path, screenshot_bytes).await
            .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        
        Ok(())
    }

    /// Close and remove a context by ID
    pub async fn close_context(&self, context_id: &str) -> Result<(), ContextError> {
        let mut inner = self.inner.lock().await;
        if let Some((context, browser_id)) = inner.contexts.remove(context_id) {
            // Close the context
            let _ = context.close().await;
            
            // Release the browser back to the pool
            if browser_id < inner.browsers.len() {
                if let Some(browser) = &mut inner.browsers[browser_id] {
                    browser.in_use = false;
                    inner.stats.in_use = inner.stats.in_use.saturating_sub(1);
                    inner.stats.available = inner.browsers.iter().filter(|b| b.as_ref().map_or(false, |b| !b.in_use)).count();
                    inner.available.push(browser_id);
                }
            }
            Ok(())
        } else {
            Err(ContextError::NotFound(format!("Context not found: {}", context_id)))
        }
    }

    /// Get a context by ID (returns a clone of the BrowserContext)
    pub async fn get_context(&self, context_id: &str) -> Option<BrowserContext> {
        let inner = self.inner.lock().await;
        inner.contexts.get(context_id).map(|(context, _)| context.clone())
    }

    /// Apply ContextConfig to a BrowserContext
    async fn apply_context_config(&self, context: &BrowserContext, config: &ContextConfig) -> Result<(), ContextError> {
        // Note: playwright-rs applies most config at context creation time via options
        // For now we apply what we can. Future enhancement: pass options to new_context()
        if let Some(user_agent) = &config.user_agent {
            let mut headers = HashMap::new();
            headers.insert("User-Agent".to_string(), user_agent.clone());
            context.set_extra_http_headers(headers).await
                .map_err(|e| ContextError::CreationFailed(e.to_string()))?;
        }
        if let Some(locale) = &config.locale {
            // Locale is typically set at context creation; warn if different
            tracing::warn!("Locale override requested after context creation: {}", locale);
        }
        if let Some(timezone_id) = &config.timezone_id {
            // Timezone is typically set at context creation; warn if different
            tracing::warn!("Timezone override requested after context creation: {}", timezone_id);
        }
        Ok(())
    }

    /// Execute a closure with a mutable reference to the context
    /// This allows complex operations that need to hold the context across await points
    pub async fn with_context<F, Fut, T>(&self, context_id: &str, f: F) -> Result<T, ContextError>
    where
        F: FnOnce(&mut playwright_rs::BrowserContext) -> Fut,
        Fut: std::future::Future<Output = Result<T, ContextError>>,
    {
        let mut inner = self.inner.lock().await;
        let (context, _browser_id) = inner.contexts.get_mut(context_id)
            .ok_or_else(|| ContextError::NotFound(format!("Context not found: {}", context_id)))?;
        
        f(context).await
    }
}

impl PoolHandle {
    pub fn browser_id(&self) -> Option<usize> {
        self.browser_id
    }

    pub async fn browser(&self) -> Option<Browser> {
        if let Some(browser_id) = self.browser_id {
            let inner = self.pool.lock().await;
            if browser_id < inner.browsers.len() {
                if let Some(browser) = &inner.browsers[browser_id] {
                    return Some(browser.browser.clone());
                }
            }
        }
        None
    }
}

impl Drop for PoolHandle {
    fn drop(&mut self) {
        if let Some(browser_id) = self.browser_id.take() {
            let pool = self.pool.clone();
            tokio::task::spawn(async move {
                let mut inner = pool.lock().await;
                if browser_id < inner.browsers.len() {
                    if let Some(browser) = &mut inner.browsers[browser_id] {
                        browser.in_use = false;
                        inner.stats.in_use = inner.stats.in_use.saturating_sub(1);
                        inner.stats.available = inner.browsers.iter().filter(|b| b.as_ref().map_or(false, |b| !b.in_use)).count();
                        inner.available.push(browser_id);
                    }
                }
            });
        }
    }
}
