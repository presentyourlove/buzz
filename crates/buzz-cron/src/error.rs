//! Error types for buzz-cron

use thiserror::Error;

/// Result type for buzz-cron operations
pub type CronResult<T> = Result<T, CronError>;

/// Errors that can occur when parsing or evaluating cron expressions
#[derive(Debug, Error)]
pub enum CronError {
    /// Invalid cron expression syntax
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),

    /// Failed to parse cron expression
    #[error("Failed to parse cron expression: {0}")]
    ParseError(#[from] cron::error::Error),

    /// Invalid timezone identifier
    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),

    /// Timezone not found in chrono-tz
    #[error("Timezone not found: {0}")]
    TimezoneNotFound(String),

    /// Failed to convert between time representations
    #[error("Time conversion error: {0}")]
    TimeConversionError(String),

    /// Schedule has no upcoming times (should not happen with valid cron)
    #[error("Schedule exhausted - no upcoming times")]
    ScheduleExhausted,

    /// DST transition ambiguity
    #[error("Ambiguous time due to DST transition: {0}")]
    AmbiguousTime(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl CronError {
    /// Create an invalid expression error
    pub fn invalid_expression(msg: impl Into<String>) -> Self {
        Self::InvalidExpression(msg.into())
    }

    /// Create a config error
    pub fn config_error(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }
}
