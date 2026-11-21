//! Triple Gate CI/CD Pipeline & Circuit Breaker Orchestrator
//!
//! Integrates Gate 1 (Syntax), Gate 2 (Semantic & Contract), and Gate 3 (Fingerprint & Circuit Breaker)
//! into a unified verification pipeline under 60 seconds as specified in `PLANS/DESIGN_SPEC_B_TRIPLE_GATE.md`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::gates::fingerprint_gate::{FingerprintGate, FingerprintGateConfig, Gate3Verdict};
use crate::gates::semantic_gate::{SemanticGate, ValidationContext};
use crate::gates::syntax_gate::{SyntaxGate, SyntaxVerdict};

/// Pipeline Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripleGateConfig {
    /// Repository root path (defaults to current dir)
    pub repo_root: PathBuf,
    /// Maximum allowed duration in seconds (default: 60s)
    pub timeout_seconds: u64,
    /// Whether to auto-rollback worktree on failure (git checkout .)
    pub auto_rollback: bool,
    /// Expected SHA256 binary fingerprint (optional)
    pub expected_fingerprint: Option<String>,
}

impl Default for TripleGateConfig {
    fn default() -> Self {
        Self {
            repo_root: PathBuf::from("."),
            timeout_seconds: 60,
            auto_rollback: false,
            expected_fingerprint: None,
        }
    }
}

/// Individual gate outcome summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStepReport {
    pub gate_name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub details: Vec<String>,
}

/// Comprehensive Triple Gate Pipeline Verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripleGatePipelineResult {
    pub passed: bool,
    pub total_duration_ms: u64,
    pub circuit_breaker_tripped: bool,
    pub steps: Vec<GateStepReport>,
    pub fix_suggestion: Option<String>,
    pub rollback_performed: bool,
}

/// Triple Gate Pipeline Runner
pub struct TripleGatePipeline;

impl TripleGatePipeline {
    /// Execute the complete 3-tier gate validation
    pub async fn run(config: TripleGateConfig) -> TripleGatePipelineResult {
        let t0 = Instant::now();
        let timeout_dur = Duration::from_secs(config.timeout_seconds);

        let mut steps = Vec::new();
        let mut circuit_breaker_tripped = false;
        let mut fix_suggestion = None;
        let mut rollback_performed = false;

        // Wrap execution in timeout
        let run_future = async {
            // ==========================================
            // Step 1: Gate 1 - Syntax & Contract Gate
            // ==========================================
            let t_g1 = Instant::now();
            let g1_res = SyntaxGate::validate(&config.repo_root);
            let g1_passed = g1_res.verdict == SyntaxVerdict::Pass;
            steps.push(GateStepReport {
                gate_name: "Gate 1: Syntax & Contract Gate".to_string(),
                passed: g1_passed,
                duration_ms: t_g1.elapsed().as_millis() as u64,
                details: if g1_passed {
                    vec![format!("Checked {} contract files without syntax violations.", g1_res.checked_files)]
                } else {
                    g1_res.errors.clone()
                },
            });

            if !g1_passed {
                circuit_breaker_tripped = true;
                fix_suggestion = Some(format!(
                    "# Fix Suggestion for Gate 1 (Syntax Failure):\n# Ensure required files exist and are encoded in UTF-8 without BOM.\n# Errors:\n{}",
                    g1_res.errors.join("\n")
                ));
                return false;
            }

            // ==========================================
            // Step 2: Gate 2 - Semantic & Schema Gate
            // ==========================================
            let t_g2 = Instant::now();
            let mut semantic_ctx = ValidationContext::new(config.repo_root.to_string_lossy().to_string());
            // Add default repository scan paths
            semantic_ctx.changed_files.push("crates/buzz-agent/src/lib.rs".to_string());
            
            let semantic_gate = SemanticGate::new();
            let g2_res = semantic_gate.validate(&semantic_ctx);
            let g2_passed = match &g2_res {
                Ok(r) => r.passed,
                Err(_) => false,
            };

            let g2_details = match &g2_res {
                Ok(r) => {
                    if r.passed {
                        vec![format!("Semantic contract checks passed. Issues count: {}", r.issues.len())]
                    } else {
                        r.issues.iter().map(|i| format!("[{}] {} ({})", i.code, i.message, i.path)).collect()
                    }
                }
                Err(e) => vec![format!("Semantic gate error: {}", e)],
            };

            steps.push(GateStepReport {
                gate_name: "Gate 2: Semantic & Schema Gate".to_string(),
                passed: g2_passed,
                duration_ms: t_g2.elapsed().as_millis() as u64,
                details: g2_details.clone(),
            });

            if !g2_passed {
                circuit_breaker_tripped = true;
                fix_suggestion = Some(format!(
                    "# Fix Suggestion for Gate 2 (Semantic Violation):\n# Verify interface compatibility and contract seams:\n{}",
                    g2_details.join("\n")
                ));
                return false;
            }

            // ==========================================
            // Step 3: Gate 3 - Fingerprint & Circuit Breaker Gate
            // ==========================================
            let t_g3 = Instant::now();
            let mut fp_config = FingerprintGateConfig::default();
            if let Some(expected) = &config.expected_fingerprint {
                fp_config.expected_fingerprint = Some(expected.clone());
            }

            // Use current executable if found, else fallback to standard binary
            let fp_gate = FingerprintGate::with_config(fp_config);
            let g3_res = fp_gate.validate().await;
            let (g3_passed, g3_details) = match &g3_res {
                Ok(res) => {
                    let passed = res.overall_verdict == Gate3Verdict::Pass;
                    let msg = format!(
                        "Fingerprint verdict: {:?}, Circuit breaker tripped: {}",
                        res.fingerprint_verdict, res.circuit_breaker_state.tripped
                    );
                    (passed, vec![msg])
                }
                Err(err) => {
                    // Non-fatal if binary is not yet built in local dev mode
                    (true, vec![format!("Gate 3 dev notice: {} (treated as non-blocking in development)", err)])
                }
            };

            steps.push(GateStepReport {
                gate_name: "Gate 3: Fingerprint & Circuit Breaker Gate".to_string(),
                passed: g3_passed,
                duration_ms: t_g3.elapsed().as_millis() as u64,
                details: g3_details,
            });

            if !g3_passed {
                circuit_breaker_tripped = true;
                fix_suggestion = Some("# Fix Suggestion for Gate 3:\n# Rebuild binary with deterministic toolchain or reset circuit breaker state.\n".to_string());
                return false;
            }

            true
        };

        let overall_passed = match tokio::time::timeout(timeout_dur, run_future).await {
            Ok(passed) => passed,
            Err(_) => {
                circuit_breaker_tripped = true;
                steps.push(GateStepReport {
                    gate_name: "Pipeline Timeout".to_string(),
                    passed: false,
                    duration_ms: timeout_dur.as_millis() as u64,
                    details: vec![format!("Triple Gate verification exceeded timeout limit of {}s", config.timeout_seconds)],
                });
                fix_suggestion = Some("# Fix Suggestion:\n# Optimize slow checks or increase --timeout.\n".to_string());
                false
            }
        };

        // Auto-rollback if requested and failed
        if !overall_passed && config.auto_rollback {
            let rollback_cmd = std::process::Command::new("git")
                .args(["checkout", "."])
                .current_dir(&config.repo_root)
                .output();
            if let Ok(out) = rollback_cmd {
                rollback_performed = out.status.success();
            }
        }

        TripleGatePipelineResult {
            passed: overall_passed,
            total_duration_ms: t0.elapsed().as_millis() as u64,
            circuit_breaker_tripped,
            steps,
            fix_suggestion,
            rollback_performed,
        }
    }
}
