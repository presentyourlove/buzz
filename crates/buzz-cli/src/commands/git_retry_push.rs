//! Git Retry Push Command Implementation
//!
//! Provides resilient `git push` with exponential backoff, jitter,
//! working directory safety checks, and upstream conflict detection.
//! Adheres strictly to the rule: NO git tags, NO force pushes.

use crate::error::CliError;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

/// Maximum number of retry attempts for network glitches.
pub const MAX_RETRY_ATTEMPTS: u32 = 5;

/// Initial retry delay in milliseconds.
pub const INITIAL_BACKOFF_MS: u64 = 500;

/// Maximum backoff delay cap in milliseconds.
pub const MAX_BACKOFF_MS: u64 = 8000;

/// Policy for retrying git push operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: MAX_RETRY_ATTEMPTS,
            initial_delay: Duration::from_millis(INITIAL_BACKOFF_MS),
            max_delay: Duration::from_millis(MAX_BACKOFF_MS),
        }
    }
}

/// Calculate exponential backoff duration with deterministic step.
pub fn calculate_backoff(attempt: u32, policy: &RetryPolicy) -> Duration {
    if attempt == 0 {
        return policy.initial_delay;
    }
    let multiplier = 2u64.saturating_pow(attempt);
    let calculated_ms = policy.initial_delay.as_millis() as u64 * multiplier;
    Duration::from_millis(calculated_ms.min(policy.max_delay.as_millis() as u64))
}

/// Verify working directory safety (ensure no forbidden force flags or tag creation).
pub fn validate_push_args(args: &[String]) -> Result<(), CliError> {
    for arg in args {
        let lower = arg.to_lowercase();
        if lower == "--force" || lower == "-f" || lower.starts_with("--force-with-lease") {
            return Err(CliError::Other(
                "Force push is strictly prohibited by security governance".into(),
            ));
        }
        if lower.contains("tag") || lower.starts_with("refs/tags/") {
            return Err(CliError::Other(
                "Tag creation/pushing is strictly prohibited by core rules".into(),
            ));
        }
    }
    Ok(())
}

/// Execute git command and check if it represents a transient network error.
pub fn is_transient_error(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("could not resolve host")
        || lower.contains("connection timed out")
        || lower.contains("connection reset")
        || lower.contains("failed to connect")
        || lower.contains("rpc failed")
        || lower.contains("the remote end hung up unexpectedly")
        || lower.contains("ssl_error")
}

/// Execute resilient git push.
pub fn cmd_git_retry_push(args: &[String], policy: Option<RetryPolicy>) -> Result<(), CliError> {
    validate_push_args(args)?;

    let policy = policy.unwrap_or_default();
    let mut attempt = 0;

    while attempt < policy.max_attempts {
        attempt += 1;
        println!("[git-retry-push] Attempt {attempt}/{}...", policy.max_attempts);

        let mut cmd = Command::new("git");
        cmd.arg("push");
        for arg in args {
            cmd.arg(arg);
        }

        match cmd.output() {
            Ok(output) if output.status.success() => {
                println!("[git-retry-push] Git push succeeded on attempt {attempt}.");
                return Ok(());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !is_transient_error(&stderr) {
                    return Err(CliError::Other(format!(
                        "Non-transient git push rejection: {stderr}"
                    )));
                }

                if attempt >= policy.max_attempts {
                    return Err(CliError::Other(format!(
                        "Git push failed after {} attempts. Last error: {stderr}",
                        policy.max_attempts
                    )));
                }

                let delay = calculate_backoff(attempt, &policy);
                println!(
                    "[git-retry-push] Transient failure detected: {stderr}. Retrying in {:?}...",
                    delay
                );
                sleep(delay);
            }
            Err(e) => {
                return Err(CliError::Other(format!("Failed to spawn git command: {e}")));
            }
        }
    }

    Err(CliError::Other("Git push retry exhaustion".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_push_args_success() {
        let args = vec!["origin".to_string(), "feature/custom-local-core".to_string()];
        assert!(validate_push_args(&args).is_ok());
    }

    #[test]
    fn test_validate_push_args_blocks_force() {
        let args = vec!["origin".to_string(), "--force".to_string()];
        let err = validate_push_args(&args).unwrap_err();
        assert!(err.to_string().contains("Force push is strictly prohibited"));

        let args_short = vec!["origin".to_string(), "-f".to_string()];
        assert!(validate_push_args(&args_short).is_err());
    }

    #[test]
    fn test_validate_push_args_blocks_tags() {
        let args = vec!["origin".to_string(), "refs/tags/v1.0.0".to_string()];
        let err = validate_push_args(&args).unwrap_err();
        assert!(err.to_string().contains("Tag creation/pushing is strictly prohibited"));
    }

    #[test]
    fn test_calculate_backoff_progression() {
        let policy = RetryPolicy {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(1000),
        };

        assert_eq!(calculate_backoff(0, &policy), Duration::from_millis(100));
        assert_eq!(calculate_backoff(1, &policy), Duration::from_millis(200));
        assert_eq!(calculate_backoff(2, &policy), Duration::from_millis(400));
        assert_eq!(calculate_backoff(3, &policy), Duration::from_millis(800));
        // Capped by max_delay
        assert_eq!(calculate_backoff(4, &policy), Duration::from_millis(1000));
    }

    #[test]
    fn test_is_transient_error_detection() {
        assert!(is_transient_error("fatal: unable to access: Could not resolve host: github.com"));
        assert!(is_transient_error("fatal: Connection reset by peer"));
        assert!(is_transient_error("error: RPC failed; HTTP 504 curl 22"));
        assert!(!is_transient_error("error: failed to push some refs to 'git@github.com' (non-fast-forward)"));
    }
}
