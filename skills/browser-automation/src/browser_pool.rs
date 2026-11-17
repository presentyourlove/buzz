//! Browser Pool Manager
//! 
//! Manages a pool of browser instances with lifecycle tracking,
//! capability enforcement, resource cleanup, and health monitoring.

use anyhow::{Context, Result};
use playwright::{Browser, BrowserContext, Playwright};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::interval;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::capability::BrowserCapability;
use crate::context_manager::ContextManager;

/// Pool configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Maximum number of browser instances to keep alive
    pub max_browsers: usize,
    /// Maximum contexts per browser instance
    pub max_contexts_per_browser: usize,
    /// Browser type to use
    pub browser_type: BrowserTypeConfig,
    /// Arguments to pass to browser process
    pub launch_args: Vec<String>,
    /// Enable stealth mode by default
    pub stealth_enabled: bool,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Idle context cleanup threshold
    pub max_idle_time: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_browsers: 3,
            max_contexts_per_browser: 5,
            browser_type: BrowserTypeConfig::Chromium,
            launch_args: vec![
                "--no-sandbox".to_string(),
                "--disable-dev-shm-usage".to_string(),
                "--disable-gpu".to_string(),
                "--disable-web-security".to_string(),
                "--disable-features=IsolateOrigins,site-per-process".to_string(),
            ],
            stealth_enabled: true,
            health_check_interval: Duration::from_secs(30),
            max_idle_time: Duration::from_secs(300),
        }
    }
}

/// Browser type configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowserTypeConfig {
    Chromium,
    Firefox,
    WebKit,
}

impl BrowserTypeConfig {
    pub fn as_str(&self) -> &'static str {
        match self {
            BrowserTypeConfig::Chromium => "chromium",
            BrowserTypeConfig::Firefox => "firefox",
            BrowserTypeConfig::WebKit => "webkit",
        }
    }
}

/// Managed browser instance
#[derive(Debug)]
struct ManagedBrowser {
    id: Uuid,
    browser: Browser,
    context_count: usize,
    contexts: HashMap<Uuid, BrowserContext>,
}

/// Isolated browser context with capability enforcement and automatic cleanup on drop
#[derive(Debug, Clone)]
pub struct IsolatedContext {
    /// The underlying browser context
    pub context: BrowserContext,
    /// Unique context identifier
    pub context_id: Uuid,
    /// Capability enforced on this context
    pub capability: BrowserCapability,
    /// Pool reference for automatic cleanup
    pool: BrowserPool,
}

impl IsolatedContext {
    /// Get the underlying browser context
    pub fn context(&self) -> &BrowserContext {
        &self.context
    }
    
    /// Get the context ID
    pub fn context_id(&self) -> Uuid {
        self.context_id
    }
    
    /// Get the capability enforced on this context
    pub fn capability(&self) -> &BrowserCapability {
        &self.capability
    }
}

/// Pool statistics
#[derive(Debug, Clone, Serialize)]
pub struct PoolStats {
    /// Total number of browser instances
    pub total_browsers: usize,
    /// Total number of active contexts
    pub total_contexts: usize,
    /// Maximum browsers allowed
    pub max_browsers: usize,
    /// Maximum contexts per browser
    pub max_contexts_per_browser: usize,
    /// Available semaphore permits
    pub available_permits: usize,
    /// Statistics grouped by browser type
    pub by_type: HashMap<BrowserTypeConfig, (usize, usize)>,
}

/// Browser pool for managing multiple browser instances
#[derive(Debug)]
pub struct BrowserPool {
    playwright: Arc<Playwright>,
    config: PoolConfig,
    browsers: Arc<Mutex<HashMap<BrowserTypeConfig, Vec<ManagedBrowser>>>>,
    semaphore: Arc<Semaphore>,
    context_manager: Arc<ContextManager>,
    /// Health check task handle
    health_check_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl BrowserPool {
    /// Create a new browser pool
    pub async fn new(config: PoolConfig) -> Result<Self> {
        let playwright = Playwright::launch()
            .await
            .context("Failed to initialize Playwright")?;
        
        let semaphore = Arc::new(Semaphore::new(config.max_browsers * config.max_contexts_per_browser));
        let context_manager = Arc::new(ContextManager::new(config.stealth_enabled));
        
        let pool = Self {
            playwright: Arc::new(playwright),
            config,
            browsers: Arc::new(Mutex::new(HashMap::new())),
            semaphore,
            context_manager,
            health_check_handle: Arc::new(Mutex::new(None)),
        };
        
        info!("Browser pool initialized with max {} browsers, {} contexts per browser", 
              pool.config.max_browsers, pool.config.max_contexts_per_browser);
        
        // Start health check loop
        pool.start_health_check(pool.config.health_check_interval);
        
        Ok(pool)
    }
    
    /// Get or create a browser of the specified type
    async fn get_or_create_browser(&self, browser_type: BrowserTypeConfig) -> Result<(Browser, Uuid)> {
        let mut browsers = self.browsers.lock().await;
        
        let browser_list = browsers.entry(browser_type).or_insert_with(Vec::new);
        
        // Try to find a browser with available context slots
        for managed in browser_list.iter_mut() {
            if managed.context_count < self.config.max_contexts_per_browser {
                return Ok((managed.browser.clone(), managed.id));
            }
        }
        
        // Create new browser if under limit
        if browser_list.len() < self.config.max_browsers {
            let browser = self.launch_browser(browser_type).await?;
            let id = Uuid::new_v4();
            browser_list.push(ManagedBrowser {
                id,
                browser: browser.clone(),
                context_count: 0,
                contexts: HashMap::new(),
            });
            info!("Launched new {} browser (total: {})", browser_type.as_str(), browser_list.len());
            return Ok((browser, id));
        }
        
        // Wait for a slot to become available
        drop(browsers);
        let _permit = self.semaphore.acquire().await.context("Semaphore closed")?;
        
        // Retry after acquiring permit
        let mut browsers = self.browsers.lock().await;
        let browser_list = browsers.entry(browser_type).or_insert_with(Vec::new);
        
        for managed in browser_list.iter_mut() {
            if managed.context_count < self.config.max_contexts_per_browser {
                return Ok((managed.browser.clone(), managed.id));
            }
        }
        
        // Fallback: create if needed
        let browser = self.launch_browser(browser_type).await?;
        let id = Uuid::new_v4();
        browser_list.push(ManagedBrowser {
            id,
            browser: browser.clone(),
            context_count: 0,
            contexts: HashMap::new(),
        });
        Ok((browser, id))
    }
    
    /// Launch a new browser instance
    async fn launch_browser(&self, browser_type: BrowserTypeConfig) -> Result<Browser> {
        let browser_type_api = match browser_type {
            BrowserTypeConfig::Chromium => self.playwright.chromium(),
            BrowserTypeConfig::Firefox => self.playwright.firefox(),
            BrowserTypeConfig::WebKit => self.playwright.webkit(),
        };
        
        let launch_options = playwright::LaunchOptions::new()
            .headless(true)
            .args(self.config.launch_args.clone());

        let browser = browser_type_api
            .launch_with_options(launch_options)
            .await
            .with_context(|| format!("Failed to launch {} browser", browser_type.as_str()))?;
        
        Ok(browser)
    }
    
    /// Create a new isolated browser context with capability enforcement
    pub async fn create_context(&self, capability: &BrowserCapability) -> Result<IsolatedContext> {
        let browser_type = self.config.browser_type;
        let (browser, browser_id) = self.get_or_create_browser(browser_type).await?;
        
        let context_id = Uuid::new_v4();
        let context = self.context_manager
            .create_context(&browser, capability, context_id)
            .await?;
        
        // Track context in browser pool
        let mut browsers = self.browsers.lock().await;
        if let Some(browser_list) = browsers.get_mut(&browser_type) {
            for managed in browser_list.iter_mut() {
                if managed.id == browser_id {
                    managed.context_count += 1;
                    managed.contexts.insert(context_id, context.clone());
                    break;
                }
            }
        }
        
        let _permit = self.semaphore.acquire().await.context("Semaphore closed")?;
        
        Ok(IsolatedContext {
            context,
            context_id,
            capability: capability.clone(),
            pool: self.clone(),
        })
    }
    
    /// Get the context manager for advanced operations
    pub fn context_manager(&self) -> Arc<ContextManager> {
        self.context_manager.clone()
    }
    
    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let browsers = self.browsers.lock().await;
        let mut total_browsers = 0;
        let mut total_contexts = 0;
        let mut by_type = HashMap::new();
        
        for (btype, list) in browsers.iter() {
            let browser_count = list.len();
            let context_count: usize = list.iter().map(|m| m.context_count).sum();
            total_browsers += browser_count;
            total_contexts += context_count;
            by_type.insert(*btype, (browser_count, context_count));
        }
        
        PoolStats {
            total_browsers,
            total_contexts,
            max_browsers: self.config.max_browsers,
            max_contexts_per_browser: self.config.max_contexts_per_browser,
            available_permits: self.semaphore.available_permits(),
            by_type,
        }
    }

    /// Start the health-check background task
    /// This monitors browser health and cleans up stale contexts
    pub fn start_health_check(&self, check_interval: Duration) {
        let pool = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(check_interval);
            loop {
                interval.tick().await;
                if let Err(e) = pool.run_health_check().await {
                    warn!("Health check failed: {}", e);
                }
            }
        });
    }

    /// Run a single health check iteration
    async fn run_health_check(&self) -> Result<()> {
        let mut browsers = self.browsers.lock().await;
        
        for (browser_type, browser_list) in browsers.iter_mut() {
            let mut i = 0;
            while i < browser_list.len() {
                let managed = &browser_list[i];
                
                // Check if browser is still alive by checking version
                let is_healthy = !managed.browser.version().is_empty();
                
                if !is_healthy || managed.context_count == 0 {
                    // Remove unhealthy or idle browsers
                    info!("Removing {} browser (healthy: {}, contexts: {})", 
                          browser_type.as_str(), is_healthy, managed.context_count);
                    browser_list.remove(i);
                } else {
                    i += 1;
                }
            }
        }
        
        Ok(())
    }

    /// Acquire a browser from the pool for exclusive use
    /// Returns a handle that automatically releases on drop
    pub async fn acquire_browser(&self, browser_type: BrowserTypeConfig) -> Result<BrowserHandle> {
        let (browser, browser_id) = self.get_or_create_browser(browser_type).await?;
        
        // Increment context count to mark as in-use
        {
            let mut browsers = self.browsers.lock().await;
            if let Some(browser_list) = browsers.get_mut(&browser_type) {
                for managed in browser_list.iter_mut() {
                    if managed.id == browser_id {
                        managed.context_count += 1;
                        break;
                    }
                }
            }
        }
        
        Ok(BrowserHandle {
            browser,
            browser_id,
            browser_type,
            pool: self.clone(),
            acquired_at: Instant::now(),
        })
    }

    /// Release a browser back to the pool
    async fn release_browser(&self, browser_type: BrowserTypeConfig, browser_id: Uuid) {
        let mut browsers = self.browsers.lock().await;
        if let Some(browser_list) = browsers.get_mut(&browser_type) {
            for managed in browser_list.iter_mut() {
                if managed.id == browser_id {
                    managed.context_count = managed.context_count.saturating_sub(1);
                    break;
                }
            }
        }
    }
}

/// Handle to a browser acquired from the pool
/// Automatically releases the browser back to the pool on drop
#[derive(Debug)]
pub struct BrowserHandle {
    browser: Browser,
    browser_id: Uuid,
    browser_type: BrowserTypeConfig,
    pool: BrowserPool,
    acquired_at: Instant,
}

impl BrowserHandle {
    /// Get the underlying browser instance
    pub fn browser(&self) -> &Browser {
        &self.browser
    }
    
    /// Get the browser type
    pub fn browser_type(&self) -> BrowserTypeConfig {
        self.browser_type
    }
    
    /// Get the time since acquisition
    pub fn elapsed(&self) -> Duration {
        self.acquired_at.elapsed()
    }
}

impl Drop for BrowserHandle {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let browser_type = self.browser_type;
        let browser_id = self.browser_id;
        tokio::task::spawn(async move {
            pool.release_browser(browser_type, browser_id).await;
        });
    }
}

impl Clone for BrowserPool {
    fn clone(&self) -> Self {
        Self {
            playwright: self.playwright.clone(),
            config: self.config.clone(),
            browsers: self.browsers.clone(),
            semaphore: self.semaphore.clone(),
            context_manager: self.context_manager.clone(),
            health_check_handle: self.health_check_handle.clone(),
        }
    }
}
