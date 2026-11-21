//! buzz-cron - Cron expression parsing and scheduling for Buzz agents
//!
//! This crate provides:
//! - `CronSchedule`: A wrapper around `cron::Schedule` with proper error handling
//! - `BuzzSchedule`: High-level schedule with timezone and DST support
//! - Timezone-aware scheduling with `chrono-tz`

#![warn(missing_docs)]

pub mod schedule;
pub mod error;
pub mod timezone;

pub use schedule::{CronSchedule, CronScheduleConfig, BuzzSchedule};
pub use error::{CronError, CronResult};
pub use timezone::{Timezone, TimezoneConfig};
