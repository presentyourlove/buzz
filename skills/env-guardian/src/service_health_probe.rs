//! 服務健康探針模組

use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// 單一端點健康探測結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthItem {
    pub name: String,
    pub endpoint: String,
    pub reachable: bool,
    pub required: bool,
}

/// 服務健康總體報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthReport {
    pub all_healthy: bool,
    pub results: Vec<ServiceHealthItem>,
}

/// 服務健康探針
#[derive(Debug, Clone)]
pub struct ServiceHealthProbe {
    endpoints: Vec<(&'static str, &'static str, bool)>,
}

impl Default for ServiceHealthProbe {
    fn default() -> Self {
        Self {
            endpoints: vec![
                ("local-relay", "127.0.0.1:3000", false),
                ("thought-proxy", "127.0.0.1:8888", false),
            ],
        }
    }
}

impl ServiceHealthProbe {
    /// 探測所有設定端點的連通性
    pub async fn check_all(&self) -> ServiceHealthReport {
        let mut results = Vec::new();
        let mut all_healthy = true;

        for &(name, endpoint, required) in &self.endpoints {
            let reachable = endpoint
                .parse::<SocketAddr>()
                .ok()
                .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(200)).ok())
                .is_some();

            if required && !reachable {
                all_healthy = false;
            }
            results.push(ServiceHealthItem {
                name: name.to_string(),
                endpoint: endpoint.to_string(),
                reachable,
                required,
            });
        }

        ServiceHealthReport {
            all_healthy,
            results,
        }
    }
}
