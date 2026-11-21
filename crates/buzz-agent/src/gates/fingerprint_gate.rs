//! Gate 3: Fingerprint & Circuit Breaker Integration
//!
//! Implements binary build hash (SHA256 Fingerprint) verification
//! and Circuit Breaker state coordination as specified in
//! PLANS/DESIGN_SPEC_B_TRIPLE_GATE.md Section 3.

use crate::health_analyzer::{read_circuit_breaker, CircuitBreakerStatus};
use crate::types::AgentError;
use crate::types::AgentError as TypesAgentError;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs;

/// Fingerprint verification verdict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintVerdict {
    Match,
    Mismatch,
    Error,
}

/// Gate 3 overall verdict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate3Verdict {
    Pass,
    Fail,
}

/// Circuit breaker state summary for evidence
#[derive(Debug, Clone)]
pub struct CircuitBreakerStateSummary {
    pub tripped: bool,
    pub tripped_at: Option<String>,
    pub reason: Option<String>,
    pub failure_count: u32,
    pub last_failure: Option<String>,
}

/// Gate 3 validation result
#[derive(Debug, Clone)]
pub struct Gate3Result {
    pub fingerprint_verdict: FingerprintVerdict,
    pub circuit_breaker_state: CircuitBreakerStateSummary,
    pub overall_verdict: Gate3Verdict,
    pub deployment_authorized: bool,
    pub evidence_refs: Vec<String>,
}

/// Configuration for FingerprintGate
#[derive(Debug, Clone)]
pub struct FingerprintGateConfig {
    pub binary_path: PathBuf,
    pub expected_fingerprint: Option<String>,
    pub circuit_breaker_path: PathBuf,
}

impl Default for FingerprintGateConfig {
    fn default() -> Self {
        let exe_name = if cfg!(target_os = "windows") { "buzz-agent.exe" } else { "buzz-agent" };
        let binary_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."))
            .parent()
            .unwrap_or(&PathBuf::from("."))
            .join(exe_name);
        let circuit_breaker_path = PathBuf::from(".buzz/WORK_LOGS/circuit_breaker.json");
        Self {
            binary_path,
            expected_fingerprint: None,
            circuit_breaker_path,
        }
    }
}

/// Errors for FingerprintGate
#[derive(Debug, Error)]
pub enum FingerprintGateError {
    #[error("Failed to read binary file: {0}")]
    BinaryReadError(#[from] std::io::Error),
    #[error("Failed to compute fingerprint: {0}")]
    FingerprintComputeError(String),
    #[error("Circuit breaker read error: {0}")]
    CircuitBreakerError(#[from] AgentError),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// FingerprintGate: Gate 3 - Binary Fingerprint & Circuit Breaker Integration
pub struct FingerprintGate {
    config: FingerprintGateConfig,
}

impl FingerprintGate {
    /// Create a new FingerprintGate with default configuration
    pub fn new() -> Self {
        Self {
            config: FingerprintGateConfig::default(),
        }
    }

    /// Create a new FingerprintGate with custom configuration
    pub fn with_config(config: FingerprintGateConfig) -> Self {
        Self { config }
    }

    /// Compute SHA256 fingerprint of the binary
    async fn compute_fingerprint(&self) -> Result<String, FingerprintGateError> {
        let bytes = fs::read(&self.config.binary_path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    /// Read circuit breaker state from file
    async fn read_circuit_breaker_state(&self) -> Result<CircuitBreakerStateSummary, FingerprintGateError> {
        let status = read_circuit_breaker(&self.config.circuit_breaker_path).await?;
        Ok(CircuitBreakerStateSummary {
            tripped: status.tripped,
            tripped_at: status.tripped_at,
            reason: status.reason,
            failure_count: status.failure_count,
            last_failure: status.last_failure,
        })
    }

    /// Validate fingerprint and circuit breaker health
    pub async fn validate(&self) -> Result<Gate3Result, FingerprintGateError> {
        let mut evidence_refs = Vec::new();

        // Compute fingerprint
        let computed_fingerprint = self.compute_fingerprint().await?;
        evidence_refs.push(format!("G3-FP-{}:{}", self.config.binary_path.display(), computed_fingerprint));

        // Verify fingerprint if expected is provided
        let fingerprint_verdict = if let Some(expected) = &self.config.expected_fingerprint {
            let matches = expected.eq_ignore_ascii_case(&computed_fingerprint);
            if matches {
                FingerprintVerdict::Match
            } else {
                FingerprintVerdict::Mismatch
            }
        } else {
            FingerprintVerdict::Match
        };

        // Read circuit breaker state
        let circuit_breaker_state = self.read_circuit_breaker_state().await?;
        evidence_refs.push(format!("G3-CB-tripped:{},failures:{}", circuit_breaker_state.tripped, circuit_breaker_state.failure_count));

        // Determine overall verdict based on fingerprint and circuit breaker health
        let overall_verdict = match fingerprint_verdict {
            FingerprintVerdict::Error => Gate3Verdict::Fail,
            FingerprintVerdict::Mismatch => Gate3Verdict::Fail,
            FingerprintVerdict::Match => {
                if circuit_breaker_state.tripped {
                    Gate3Verdict::Fail
                } else {
                    Gate3Verdict::Pass
                }
            }
        };

        let deployment_authorized = matches!(overall_verdict, Gate3Verdict::Pass);

        Ok(Gate3Result {
            fingerprint_verdict,
            circuit_breaker_state,
            overall_verdict,
            deployment_authorized,
            evidence_refs,
        })
    }
}

impl Default for FingerprintGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentError;
    use std::fs;
    use tempfile::tempdir;
    use tokio::fs as async_fs;

    #[tokio::test]
    async fn test_fingerprint_match() {
        let dir = tempdir().unwrap();
        let binary_path = dir.path().join("test_binary");
        async_fs::write(&binary_path, b"test binary content").await.unwrap();
        let expected = {
            let mut hasher = Sha256::new();
            hasher.update(b"test binary content");
            hex::encode(hasher.finalize())
        };
        let config = FingerprintGateConfig {
            binary_path: binary_path.clone(),
            expected_fingerprint: Some(expected),
            circuit_breaker_path: dir.path().join("circuit_breaker.json"),
        };
        let gate = FingerprintGate::with_config(config);
        let result = gate.validate().await.unwrap();
        assert_eq!(result.fingerprint_verdict, FingerprintVerdict::Match);
        assert_eq!(result.overall_verdict, Gate3Verdict::Pass);
        assert!(result.deployment_authorized);
    }

    #[tokio::test]
    async fn test_fingerprint_mismatch() {
        let dir = tempdir().unwrap();
        let binary_path = dir.path().join("test_binary");
        async_fs::write(&binary_path, b"test binary content").await.unwrap();
        let config = FingerprintGateConfig {
            binary_path,
            expected_fingerprint: Some("invalid_fingerprint_that_will_not_match".to_string()),
            circuit_breaker_path: dir.path().join("circuit_breaker.json"),
        };
        let gate = FingerprintGate::with_config(config);
        let result = gate.validate().await;
        assert!(result.is_ok());
        let gate3_result = result.unwrap();
        assert_eq!(gate3_result.fingerprint_verdict, FingerprintVerdict::Mismatch);
        assert_eq!(gate3_result.overall_verdict, Gate3Verdict::Fail);
        assert!(!gate3_result.deployment_authorized);
    }

    #[tokio::test]
    async fn test_circuit_breaker_not_tripped() {
        let dir = tempdir().unwrap();
        let binary_path = dir.path().join("test_binary");
        async_fs::write(&binary_path, b"test binary content").await.unwrap();
        let cb_path = dir.path().join("circuit_breaker.json");
        async_fs::write(&cb_path, r#"{"tripped": false, "failure_count": 0}"#).await.unwrap();
        let config = FingerprintGateConfig {
            binary_path,
            circuit_breaker_path: cb_path,
            ..Default::default()
        };
        let gate = FingerprintGate::with_config(config);
        let result = gate.validate().await;
        assert!(result.is_ok());
        let gate3_result = result.unwrap();
        assert!(!gate3_result.circuit_breaker_state.tripped);
        assert_eq!(gate3_result.overall_verdict, Gate3Verdict::Pass);
        assert!(gate3_result.deployment_authorized);
    }

    #[tokio::test]
    async fn test_circuit_breaker_tripped_fails() {
        let dir = tempdir().unwrap();
        let binary_path = dir.path().join("test_binary");
        async_fs::write(&binary_path, b"test binary content").await.unwrap();
        let cb_path = dir.path().join("circuit_breaker.json");
        async_fs::write(&cb_path, r#"{"tripped": true, "failure_count": 5, "reason": "test failure"}"#).await.unwrap();
        let config = FingerprintGateConfig {
            binary_path,
            circuit_breaker_path: cb_path,
            ..Default::default()
        };
        let gate = FingerprintGate::with_config(config);
        let result = gate.validate().await;
        assert!(result.is_ok());
        let gate3_result = result.unwrap();
        assert!(gate3_result.circuit_breaker_state.tripped);
        assert_eq!(gate3_result.overall_verdict, Gate3Verdict::Fail);
        assert!(!gate3_result.deployment_authorized);
    }
}
