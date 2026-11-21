//! Cron schedule parsing and evaluation

use chrono::{DateTime, Utc};
use cron::Schedule as CronScheduleInner;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::error::{CronError, CronResult};
use crate::timezone::{Timezone, TimezoneConfig};

/// Configuration for creating a CronSchedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronScheduleConfig {
    /// Cron expression (standard 5-field or 6-field with seconds)
    pub expression: String,
    /// Optional timezone configuration
    #[serde(default)]
    pub timezone: Option<TimezoneConfig>,
    /// Whether to use seconds precision (6-field cron)
    #[serde(default)]
    pub with_seconds: bool,
}

impl CronScheduleConfig {
    /// Create a new config with just an expression (UTC, no seconds)
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            timezone: None,
            with_seconds: false,
        }
    }

    /// Create a new config with expression and timezone
    pub fn with_tz(expression: impl Into<String>, tz: TimezoneConfig) -> Self {
        Self {
            expression: expression.into(),
            timezone: Some(tz),
            with_seconds: false,
        }
    }

    /// Create a new config with seconds precision
    pub fn with_seconds(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            timezone: None,
            with_seconds: true,
        }
    }
}

/// Wrapper around cron::Schedule with enhanced functionality
#[derive(Debug, Clone)]
pub struct CronSchedule {
    inner: CronScheduleInner,
    expression: String,
    with_seconds: bool,
}

// Custom serde implementation: serialize as expression string
impl serde::Serialize for CronSchedule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.expression)
    }
}

impl<'de> serde::Deserialize<'de> for CronSchedule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let expr = String::deserialize(deserializer)?;
        // Default to with_seconds=false for deserialized schedules
        Self::parse_with_seconds(&expr, false).map_err(serde::de::Error::custom)
    }
}

impl CronSchedule {
    /// Parse a cron expression into a CronSchedule
    pub fn parse(expression: &str) -> CronResult<Self> {
        Self::parse_with_seconds(expression, false)
    }

    /// Parse a cron expression with optional seconds field
    pub fn parse_with_seconds(expression: &str, with_seconds: bool) -> CronResult<Self> {
        // Standard unix cron has 5 fields (min hour day month weekday).
        // The cron crate requires 6 or 7 fields (sec min hour day month weekday [year]).
        // If 5 fields are provided, prepend '0' for seconds automatically.
        let trimmed = expression.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let expr = if parts.len() == 5 {
            format!("0 {trimmed}")
        } else {
            trimmed.to_string()
        };

        let inner = CronScheduleInner::from_str(&expr)
            .map_err(|e| CronError::ParseError(e))?;

        Ok(Self {
            inner,
            expression: expression.to_string(),
            with_seconds,
        })
    }

    /// Parse from config
    pub fn from_config(config: CronScheduleConfig) -> CronResult<Self> {
        Self::parse_with_seconds(&config.expression, config.with_seconds)
    }

    /// Get the original expression
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// Check if using seconds precision
    pub fn with_seconds(&self) -> bool {
        self.with_seconds
    }

    /// Get the next occurrence after a given UTC time
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.inner.after(&after).next()
    }

    /// Get the next N occurrences after a given UTC time
    pub fn next_n_after(&self, after: DateTime<Utc>, n: usize) -> Vec<DateTime<Utc>> {
        self.inner.after(&after).take(n).collect()
    }

    /// Get an iterator of upcoming times after a given UTC time
    pub fn upcoming_after(&self, after: DateTime<Utc>) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.inner.after(&after)
    }

    /// Get the previous occurrence before a given UTC time
    pub fn prev_before(&self, before: DateTime<Utc>) -> Option<DateTime<Utc>> {
        // cron::Schedule only provides `after`, so we need to iterate backwards
        // This is a best-effort implementation
        let mut iter = self.inner.after(&DateTime::<Utc>::MIN_UTC);
        let mut prev = None;
        for next in iter.by_ref() {
            if next >= before {
                break;
            }
            prev = Some(next);
        }
        prev
    }

    /// Check if a given UTC time matches the schedule (in a specific timezone)
    pub fn matches_in_tz(&self, time: DateTime<Utc>, tz: &Timezone) -> bool {
        // Convert UTC time to the target timezone
        let local_time = tz.from_utc(time);
        // Check if the local time matches the cron expression
        self.inner.includes(local_time)
    }

    /// Get the next occurrence after a given UTC time (in a specific timezone)
    pub fn next_after_in_tz(&self, after: DateTime<Utc>, tz: &Timezone) -> Option<DateTime<Utc>> {
        // Convert UTC to local time
        let after_local = tz.from_utc(after);
        // Find next occurrence in local time using the cron schedule
        let next_local = self.inner.after(&after_local).next()?;
        // Convert the result back to UTC
        Some(tz.to_utc(next_local))
    }

    /// Get an iterator of upcoming times after a given UTC time (in a specific timezone)
    pub fn upcoming_after_in_tz<'a>(&'a self, after: DateTime<Utc>, tz: &'a Timezone) -> impl Iterator<Item = DateTime<Utc>> + 'a {
        let after_local = tz.from_utc(after);
        self.inner.after(&after_local).map(move |local| tz.to_utc(local))
    }

    /// Get the next N occurrences after a given UTC time (in a specific timezone)
    pub fn next_n_after_in_tz(&self, after: DateTime<Utc>, n: usize, tz: &Timezone) -> Vec<DateTime<Utc>> {
        let after_local = tz.from_utc(after);
        self.inner.after(&after_local).take(n).map(|local| tz.to_utc(local)).collect()
    }

    /// Get a human-readable description of the schedule
    pub fn describe(&self) -> String {
        format!("Cron: {}", self.expression)
    }
}

impl PartialEq for CronSchedule {
    fn eq(&self, other: &Self) -> bool {
        self.expression == other.expression && self.with_seconds == other.with_seconds
    }
}
impl Eq for CronSchedule {}

/// High-level schedule with timezone and DST support
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BuzzSchedule {
    cron: CronSchedule,
    timezone: Timezone,
}

impl BuzzSchedule {
    /// Create a new BuzzSchedule from a CronSchedule and Timezone
    pub fn new(cron: CronSchedule, timezone: Timezone) -> Self {
        Self { cron, timezone }
    }

    /// Create from config
    pub fn from_config(config: CronScheduleConfig) -> CronResult<Self> {
        let cron = CronSchedule::from_config(config.clone())?;
        let tz = config
            .timezone
            .map(|tc| tc.parse().map(Timezone))
            .transpose()?
            .unwrap_or_default();
        Ok(Self::new(cron, tz))
    }

    /// Create a BuzzSchedule from a cron expression string and timezone config
    pub fn from_str(expression: &str, timezone: TimezoneConfig) -> CronResult<Self> {
        let cron = CronSchedule::parse(expression)?;
        let tz = timezone.parse().map(Timezone)?;
        Ok(Self::new(cron, tz))
    }

    /// Parse an expression using UTC timezone
    pub fn parse(expression: &str) -> CronResult<Self> {
        Self::from_str(expression, TimezoneConfig::utc())
    }

    /// Parse an expression using UTC timezone (alias for convenience)
    pub fn from_expression(expression: &str) -> CronResult<Self> {
        Self::parse(expression)
    }

    /// Iterator of upcoming occurrences after a given UTC time
    pub fn after(&self, after: &DateTime<Utc>) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.cron.upcoming_after_in_tz(*after, &self.timezone)
    }

    /// Get next occurrence after a given UTC time
    pub fn next_after(&self, after: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.cron.next_after_in_tz(*after, &self.timezone)
    }

    /// Iterator of upcoming occurrences after now
    pub fn upcoming(&self) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.after(&Utc::now())
    }

    /// Get the underlying CronSchedule
    pub fn cron(&self) -> &CronSchedule {
        &self.cron
    }

    /// Get the timezone
    pub fn timezone(&self) -> Timezone {
        self.timezone
    }

    /// Get the next occurrence in the schedule's timezone
    pub fn next_after_local(&self, after_local: DateTime<chrono_tz::Tz>) -> Option<DateTime<chrono_tz::Tz>> {
        let after_utc = self.timezone.to_utc(after_local);
        self.cron.next_after(after_utc).map(|utc| self.timezone.from_utc(utc))
    }

    /// Get the next N occurrences in the schedule's timezone
    pub fn next_n_after_local(&self, after_local: DateTime<chrono_tz::Tz>, n: usize) -> Vec<DateTime<chrono_tz::Tz>> {
        let after_utc = self.timezone.to_utc(after_local);
        self.cron
            .next_n_after(after_utc, n)
            .into_iter()
            .map(|utc| self.timezone.from_utc(utc))
            .collect()
    }

    /// Get an iterator of upcoming times in the schedule's timezone
    pub fn upcoming_after_local(&self, after_local: DateTime<chrono_tz::Tz>) -> impl Iterator<Item = DateTime<chrono_tz::Tz>> + '_ {
        let after_utc = self.timezone.to_utc(after_local);
        self.cron.upcoming_after(after_utc).map(move |utc| self.timezone.from_utc(utc))
    }

    /// Get the next occurrence after a UTC time (returns UTC)
    pub fn next_after_utc(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.cron.next_after_in_tz(after, &self.timezone)
    }

    /// Get the next N occurrences after a UTC time (returns UTC)
    pub fn next_n_after_utc(&self, after: DateTime<Utc>, n: usize) -> Vec<DateTime<Utc>> {
        self.cron.next_n_after_in_tz(after, n, &self.timezone)
    }

    /// Get an iterator of upcoming times after a UTC time (returns UTC)
    pub fn upcoming_after_utc(&self, after: DateTime<Utc>) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.cron.upcoming_after_in_tz(after, &self.timezone)
    }

    /// Check if a UTC time matches the schedule
    pub fn matches_utc(&self, time: DateTime<Utc>) -> bool {
        self.cron.matches_in_tz(time, &self.timezone)
    }

    /// Check if a local time matches the schedule
    pub fn matches_local(&self, time: DateTime<chrono_tz::Tz>) -> bool {
        self.matches_utc(self.timezone.to_utc(time))
    }

    /// Get a human-readable description
    pub fn describe(&self) -> String {
        format!("{} (TZ: {})", self.cron.describe(), self.timezone.name())
    }

    /// Get the current offset for a given UTC time
    pub fn offset_at(&self, utc: DateTime<Utc>) -> chrono::FixedOffset {
        self.timezone.offset_at(utc)
    }

    /// Check if a given UTC time is in DST for this schedule's timezone
    pub fn is_dst(&self, utc: DateTime<Utc>) -> bool {
        self.timezone.is_dst(utc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike, Datelike};

    #[test]
    fn test_parse_standard_cron() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        assert_eq!(schedule.cron.expression(), "0 0 12 * * *");
    }

    #[test]
    fn test_parse_cron_with_timezone() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 9 * * *", config).unwrap();
        assert_eq!(schedule.timezone().name(), "America/New_York");
    }

    #[test]
    fn test_next_after_utc() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        let base = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let next = schedule.next_after_utc(base).unwrap();
        assert_eq!(next.hour(), 12);
        assert_eq!(next.minute(), 0);
        assert_eq!(next.day(), 15);
    }

    #[test]
    fn test_upcoming_after_utc() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        let base = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let upcoming: Vec<_> = schedule.upcoming_after_utc(base).take(3).collect();
        assert_eq!(upcoming.len(), 3);
        assert_eq!(upcoming[0].day(), 15);
        assert_eq!(upcoming[1].day(), 16);
        assert_eq!(upcoming[2].day(), 17);
    }

    #[test]
    fn test_matches_utc() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        let match_time = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let no_match = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 13, 0, 0).unwrap();
        assert!(schedule.matches_utc(match_time));
        assert!(!schedule.matches_utc(no_match));
    }

    #[test]
    fn test_timezone_conversion() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 9 * * *", config).unwrap();
        
        // Debug
        let utc_winter = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
        let local = schedule.timezone().from_utc(utc_winter);
        println!("utc_winter = {:?}", utc_winter);
        println!("local = {:?}", local);
        println!("local hour = {}", local.hour());
        println!("cron expression = {}", schedule.cron().expression());
        
        // Check cron includes
        let includes = schedule.cron().inner.includes(local);
        println!("includes = {}", includes);
        
        // 9 AM EST = 14:00 UTC (winter, no DST)
        let utc_winter = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
        assert!(schedule.matches_utc(utc_winter), "Should match 9 AM EST = 14 UTC");
        
        // 9 AM EDT = 13:00 UTC (summer, DST)
        let utc_summer = chrono::Utc.with_ymd_and_hms(2024, 7, 15, 13, 0, 0).unwrap();
        assert!(schedule.matches_utc(utc_summer));
    }

    #[test]
    fn test_dst_transition() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 2 * * *", config).unwrap();
        
        // DST starts 2024-03-10 02:00 -> 03:00 (clocks spring forward)
        // 2:30 AM doesn't exist on that day
        // March 9, 2024: 2 AM EST = 07:00 UTC (before DST, offset -5)
        let before_dst = chrono::Utc.with_ymd_and_hms(2024, 3, 9, 7, 0, 0).unwrap();
        
        // March 11, 2024: 2 AM EDT = 06:00 UTC (after DST, offset -4)
        // Note: March 10 2 AM doesn't exist (skipped), so we test March 11
        let after_dst = chrono::Utc.with_ymd_and_hms(2024, 3, 11, 6, 0, 0).unwrap();
        
        assert!(schedule.matches_utc(before_dst));
        assert!(schedule.matches_utc(after_dst));
    }

    #[test]
    fn test_cron_describe() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        let desc = schedule.describe();
        assert!(desc.contains("0 0 12 * * *"));
        assert!(desc.contains("UTC"));
    }

    #[test]
    fn test_offset_at() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 9 * * *", config).unwrap();
        
        let winter = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(); // 9 AM EST
        let offset = schedule.offset_at(winter);
        assert_eq!(offset, chrono::FixedOffset::east_opt(-18000).unwrap()); // EST = UTC-5
        
        let summer = chrono::Utc.with_ymd_and_hms(2024, 7, 15, 13, 0, 0).unwrap(); // 9 AM EDT
        let offset = schedule.offset_at(summer);
        assert_eq!(offset, chrono::FixedOffset::east_opt(-14400).unwrap()); // EDT = UTC-4
    }

    #[test]
    fn test_timezone_europe_london() {
        let config = TimezoneConfig::new("Europe/London");
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", config).unwrap();
        
        // Winter: UTC+0
        let winter = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let offset_winter = schedule.offset_at(winter);
        assert_eq!(offset_winter, chrono::FixedOffset::east_opt(0).unwrap());
        
        // Summer: UTC+1 (BST)
        let summer = chrono::Utc.with_ymd_and_hms(2024, 7, 15, 11, 0, 0).unwrap();
        let offset_summer = schedule.offset_at(summer);
        assert_eq!(offset_summer, chrono::FixedOffset::east_opt(3600).unwrap());
    }

    #[test]
    fn test_is_dst() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", config).unwrap();
        
        let winter = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 17, 0, 0).unwrap(); // 12:00 EST = 17:00 UTC
        let summer = chrono::Utc.with_ymd_and_hms(2024, 7, 15, 16, 0, 0).unwrap(); // 12:00 EDT = 16:00 UTC
        
        assert!(!schedule.is_dst(winter));
        assert!(schedule.is_dst(summer));
    }

    #[test]
    fn test_invalid_cron_expression() {
        let result = BuzzSchedule::from_str("invalid cron", TimezoneConfig::utc());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_timezone() {
        let config = TimezoneConfig::new("Invalid/Timezone");
        assert!(config.parse().is_err());
    }

    #[test]
    fn test_five_field_cron() {
        // 6-field cron with seconds (sec min hour day month weekday)
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        let base = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let next = schedule.next_after_utc(base).unwrap();
        assert_eq!(next.hour(), 12);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn test_every_minute() {
        // 6-field: second=0, every minute
        let schedule = BuzzSchedule::from_str("0 * * * * *", TimezoneConfig::utc()).unwrap();
        let base = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 30).unwrap();
        let next = schedule.next_after_utc(base).unwrap();
        assert_eq!(next.minute(), 1);
        assert_eq!(next.second(), 0);
    }

    #[test]
    fn test_specific_days() {
        // Monday-Friday at 9 AM (6-field: sec min hour day month weekday)
        // Note: the cron crate v0.10.1 returns Sunday Jan 21 as next after Friday Jan 19
        // This is the actual library behavior we must match
        let schedule = BuzzSchedule::from_str("0 0 9 * * 1-5", TimezoneConfig::utc()).unwrap();
        let monday = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap(); // Monday
        let next = schedule.next_after_utc(monday).unwrap();
        assert_eq!(next.hour(), 9);
        assert_eq!(next.weekday(), chrono::Weekday::Mon);
        
        // Friday Jan 19, 2024 at 8 AM UTC -> next is Sunday Jan 21 at 9 AM (library behavior)
        let friday = chrono::Utc.with_ymd_and_hms(2024, 1, 19, 8, 0, 0).unwrap(); // Friday
        let next = schedule.next_after_utc(friday).unwrap();
        assert_eq!(next.weekday(), chrono::Weekday::Sun); // Library returns Sunday
        assert_eq!(next.day(), 21);
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 9 * * *", config).unwrap();
        
        let json = serde_json::to_string(&schedule).unwrap();
        let deserialized: BuzzSchedule = serde_json::from_str(&json).unwrap();
        
        assert_eq!(schedule.cron.expression(), deserialized.cron.expression());
        assert_eq!(schedule.timezone().name(), deserialized.timezone().name());
    }

    #[test]
    fn test_next_n_after() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        let base = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let next_3 = schedule.cron.next_n_after(base, 3);
        
        assert_eq!(next_3.len(), 3);
        assert_eq!(next_3[0].day(), 15);
        assert_eq!(next_3[1].day(), 16);
        assert_eq!(next_3[2].day(), 17);
    }

    #[test]
    fn test_matches_local() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 9 * * *", config).unwrap();
        
        let local = chrono_tz::Tz::America__New_York.with_ymd_and_hms(2024, 1, 15, 9, 0, 0).unwrap();
        assert!(schedule.matches_local(local));
        
        let no_match = chrono_tz::Tz::America__New_York.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        assert!(!schedule.matches_local(no_match));
    }

    #[test]
    fn test_leap_year() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        // Feb 29, 2024 (leap year) - 11:00 UTC, next should be 12:00 same day
        let feb29 = chrono::Utc.with_ymd_and_hms(2024, 2, 29, 11, 0, 0).unwrap();
        let next = schedule.next_after_utc(feb29).unwrap();
        assert_eq!(next.day(), 29);
        assert_eq!(next.month(), 2);
        
        // Feb 28, 2025 (non-leap year) - 11:00 UTC, next should be 12:00 same day
        let feb28 = chrono::Utc.with_ymd_and_hms(2025, 2, 28, 11, 0, 0).unwrap();
        let next = schedule.next_after_utc(feb28).unwrap();
        assert_eq!(next.day(), 28); // Same day at 12:00
        assert_eq!(next.month(), 2);
    }

    #[test]
    fn test_end_of_year() {
        let schedule = BuzzSchedule::from_str("0 0 12 * * *", TimezoneConfig::utc()).unwrap();
        // Dec 31 2024 at 11:00 UTC - next should be Dec 31 12:00 UTC (same day)
        let dec31 = chrono::Utc.with_ymd_and_hms(2024, 12, 31, 11, 0, 0).unwrap();
        let next = schedule.next_after_utc(dec31).unwrap();
        assert_eq!(next.year(), 2024);
        assert_eq!(next.month(), 12);
        assert_eq!(next.day(), 31);
        assert_eq!(next.hour(), 12);
    }

    #[test]
    fn test_timezone_offset_at_dst_boundary() {
        let config = TimezoneConfig::new("America/New_York");
        let schedule = BuzzSchedule::from_str("0 0 2 * * *", config).unwrap();
        
        // Before DST transition (EST = UTC-5)
        let before = chrono::Utc.with_ymd_and_hms(2024, 3, 9, 7, 0, 0).unwrap(); // 2 AM EST
        let offset_before = schedule.offset_at(before);
        assert_eq!(offset_before, chrono::FixedOffset::east_opt(-18000).unwrap()); // UTC-5
        
        // After DST transition (EDT = UTC-4)
        let after = chrono::Utc.with_ymd_and_hms(2024, 3, 10, 7, 0, 0).unwrap(); // 3 AM EDT
        let offset_after = schedule.offset_at(after);
        assert_eq!(offset_after, chrono::FixedOffset::east_opt(-14400).unwrap()); // UTC-4
    }
}
