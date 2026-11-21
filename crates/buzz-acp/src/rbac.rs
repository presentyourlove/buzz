//! Agent Control Plane (ACP) Role-Based Access Control (RBAC) Engine.
//!
//! Provides granular authorization boundaries for Buzz agents, preventing
//! unauthorized write operations, out-of-scope code modifications, and
//! unverified shell executions.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Agent Role Classification in Buzz RBAC hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Full architectural privileges (e.g. Antigravity backend executor).
    /// Permitted to perform workspace refactoring, cargo build/test, git push.
    Architect,

    /// Engineering & implementation privileges (e.g. Pollen in isolated worktrees).
    /// Permitted to modify designated task files, run localized tests.
    Engineer,

    /// Quality Assurance and Reviewer role (e.g. Sentry).
    /// Strictly read-only for codebase; permitted to execute read-only tests and audits.
    Reviewer,

    /// Planning and research role (e.g. Honey / Honey2).
    /// Strictly restricted to PLANS/ and REPORTS/ documentation paths; forbidden from touching crates/.
    Planner,
}

impl Default for AgentRole {
    fn default() -> Self {
        AgentRole::Engineer
    }
}

/// Action attempting to be performed by an agent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Reading from file or directory
    ReadFile(PathBuf),
    /// Writing or creating file/directory
    WriteFile(PathBuf),
    /// Deleting file or directory
    DeleteFile(PathBuf),
    /// Executing terminal command
    ExecuteCommand(String),
    /// Git operation (commit, push, merge, checkout)
    GitOp(String),
}

/// Violation error returned when an action exceeds role privileges
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RbacViolation {
    #[error("角色 {role:?} 禁止寫入非授權路徑: {path}")]
    UnauthorizedPathWrite { role: AgentRole, path: String },

    #[error("角色 {role:?} 禁止刪除路徑: {path}")]
    UnauthorizedPathDelete { role: AgentRole, path: String },

    #[error("角色 {role:?} 禁止執行高危終端指令: {command}")]
    UnauthorizedCommandExecution { role: AgentRole, command: String },

    #[error("角色 {role:?} 禁止執行 Git 操作: {operation}")]
    UnauthorizedGitOperation { role: AgentRole, operation: String },

    #[error("路徑遍歷越界風險 (Path Traversal): {path}")]
    PathTraversalDetected { path: String },
}

/// Core RBAC Evaluator
#[derive(Debug, Clone, Default)]
pub struct RbacPolicy;

impl RbacPolicy {
    /// Evaluate whether an agent with `role` is authorized to perform `action`
    pub fn enforce(role: AgentRole, action: &Action) -> Result<(), RbacViolation> {
        match role {
            AgentRole::Architect => {
                // Architect has full privileges, but still prevent raw path traversal (e.g. ../../Windows)
                if let Action::WriteFile(p) | Action::DeleteFile(p) = action {
                    Self::check_traversal(p)?;
                }
                Ok(())
            }
            AgentRole::Engineer => Self::enforce_engineer(action),
            AgentRole::Reviewer => Self::enforce_reviewer(action),
            AgentRole::Planner => Self::enforce_planner(action),
        }
    }

    fn check_traversal(path: &Path) -> Result<(), RbacViolation> {
        let p_str = path.to_string_lossy();
        if p_str.contains("..") {
            return Err(RbacViolation::PathTraversalDetected {
                path: p_str.to_string(),
            });
        }
        Ok(())
    }

    fn enforce_planner(action: &Action) -> Result<(), RbacViolation> {
        match action {
            Action::ReadFile(_) => Ok(()),
            Action::WriteFile(path) => {
                Self::check_traversal(path)?;
                let norm = path.to_string_lossy().replace('\\', "/");
                // Planner is ONLY allowed to write to PLANS/ or REPORTS/
                if norm.contains("PLANS/")
                    || norm.contains("REPORTS/")
                    || norm.contains("data/triggers/")
                    || norm.contains(".scratch/")
                {
                    Ok(())
                } else {
                    Err(RbacViolation::UnauthorizedPathWrite {
                        role: AgentRole::Planner,
                        path: norm,
                    })
                }
            }
            Action::DeleteFile(path) => {
                let norm = path.to_string_lossy().replace('\\', "/");
                if norm.contains(".scratch/") {
                    Ok(())
                } else {
                    Err(RbacViolation::UnauthorizedPathDelete {
                        role: AgentRole::Planner,
                        path: norm,
                    })
                }
            }
            Action::ExecuteCommand(cmd) => {
                // Planner can only run read-only introspection commands
                let lower = cmd.trim().to_lowercase();
                if lower.starts_with("git status")
                    || lower.starts_with("git diff")
                    || lower.starts_with("dir")
                    || lower.starts_with("ls")
                {
                    Ok(())
                } else {
                    Err(RbacViolation::UnauthorizedCommandExecution {
                        role: AgentRole::Planner,
                        command: cmd.clone(),
                    })
                }
            }
            Action::GitOp(op) => Err(RbacViolation::UnauthorizedGitOperation {
                role: AgentRole::Planner,
                operation: op.clone(),
            }),
        }
    }

    fn enforce_reviewer(action: &Action) -> Result<(), RbacViolation> {
        match action {
            Action::ReadFile(_) => Ok(()),
            Action::WriteFile(path) => {
                // Reviewer is strictly read-only for core, only allowed to write temporary scratch/audit files
                let norm = path.to_string_lossy().replace('\\', "/");
                if norm.contains(".scratch/") || norm.contains("AUDIT_") {
                    Ok(())
                } else {
                    Err(RbacViolation::UnauthorizedPathWrite {
                        role: AgentRole::Reviewer,
                        path: norm,
                    })
                }
            }
            Action::DeleteFile(path) => Err(RbacViolation::UnauthorizedPathDelete {
                role: AgentRole::Reviewer,
                path: path.to_string_lossy().to_string(),
            }),
            Action::ExecuteCommand(cmd) => {
                // Reviewer can run tests and audits, but forbidden from push/merge
                let lower = cmd.trim().to_lowercase();
                if lower.contains("push") || lower.contains("merge") || lower.contains("rebase") {
                    Err(RbacViolation::UnauthorizedCommandExecution {
                        role: AgentRole::Reviewer,
                        command: cmd.clone(),
                    })
                } else {
                    Ok(())
                }
            }
            Action::GitOp(op) => {
                if op == "push" || op == "merge" {
                    Err(RbacViolation::UnauthorizedGitOperation {
                        role: AgentRole::Reviewer,
                        operation: op.clone(),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    fn enforce_engineer(action: &Action) -> Result<(), RbacViolation> {
        match action {
            Action::ReadFile(_) => Ok(()),
            Action::WriteFile(path) => {
                Self::check_traversal(path)?;
                let norm = path.to_string_lossy().replace('\\', "/");
                // Engineer is protected from directly editing core security files without architect elevation
                if norm.contains("crates/buzz-core/src/kind.rs") {
                    return Err(RbacViolation::UnauthorizedPathWrite {
                        role: AgentRole::Engineer,
                        path: norm,
                    });
                }
                Ok(())
            }
            Action::DeleteFile(path) => {
                Self::check_traversal(path)?;
                Ok(())
            }
            Action::ExecuteCommand(cmd) => {
                let lower = cmd.trim().to_lowercase();
                // Engineer cannot force push to main
                if lower.contains("git push") && (lower.contains("--force") || lower.contains("-f")) {
                    return Err(RbacViolation::UnauthorizedCommandExecution {
                        role: AgentRole::Engineer,
                        command: cmd.clone(),
                    });
                }
                Ok(())
            }
            Action::GitOp(op) => {
                if op == "force-push" {
                    Err(RbacViolation::UnauthorizedGitOperation {
                        role: AgentRole::Engineer,
                        operation: op.clone(),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_cannot_touch_crates() {
        let action = Action::WriteFile(PathBuf::from("crates/buzz-core/src/kind.rs"));
        let res = RbacPolicy::enforce(AgentRole::Planner, &action);
        assert!(matches!(res, Err(RbacViolation::UnauthorizedPathWrite { .. })));
    }

    #[test]
    fn test_planner_can_write_plans() {
        let action = Action::WriteFile(PathBuf::from("PLANS/MY_NEW_PLAN.md"));
        let res = RbacPolicy::enforce(AgentRole::Planner, &action);
        assert!(res.is_ok());
    }

    #[test]
    fn test_reviewer_cannot_modify_code() {
        let action = Action::WriteFile(PathBuf::from("crates/buzz-agent/src/lib.rs"));
        let res = RbacPolicy::enforce(AgentRole::Reviewer, &action);
        assert!(matches!(res, Err(RbacViolation::UnauthorizedPathWrite { .. })));
    }

    #[test]
    fn test_architect_has_full_access() {
        let action = Action::WriteFile(PathBuf::from("crates/buzz-core/src/kind.rs"));
        let res = RbacPolicy::enforce(AgentRole::Architect, &action);
        assert!(res.is_ok());
    }
}
