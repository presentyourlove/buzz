//! Timezone handling for cron scheduling

use chrono::{DateTime, Datelike, FixedOffset, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

use crate::error::{CronError, CronResult};

/// Timezone configuration for scheduling
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimezoneConfig {
    /// IANA timezone identifier (e.g., "Asia/Taipei", "UTC", "America/New_York")
    pub iana_name: String,
    /// Whether to use system timezone as fallback
    #[serde(default = "default_fallback")]
    pub fallback_to_system: bool,
}

fn default_fallback() -> bool {
    true
}

impl Default for TimezoneConfig {
    fn default() -> Self {
        Self {
            iana_name: "UTC".to_string(),
            fallback_to_system: true,
        }
    }
}

impl TimezoneConfig {
    /// Create a new timezone config from IANA name
    pub fn new(iana_name: impl Into<String>) -> Self {
        Self {
            iana_name: iana_name.into(),
            fallback_to_system: true,
        }
    }

    /// Create UTC timezone config
    pub fn utc() -> Self {
        Self::new("UTC")
    }

    /// Parse and validate the timezone
    pub fn parse(&self) -> CronResult<Tz> {
        Tz::from_str(&self.iana_name).map_err(|_| CronError::InvalidTimezone(self.iana_name.clone()))
    }

    /// Get the timezone, falling back to UTC if parsing fails and fallback is enabled
    pub fn get_or_utc(&self) -> Tz {
        self.parse().unwrap_or(chrono_tz::UTC)
    }
}

use std::fmt;

/// Wrapper around chrono_tz::Tz with additional functionality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timezone(pub Tz);

impl Timezone {
    /// Create a new Timezone from a Tz
    pub const fn new(tz: Tz) -> Self {
        Self(tz)
    }

    /// Get UTC timezone
    pub const fn utc() -> Self {
        Self(chrono_tz::UTC)
    }

    /// Convert a UTC DateTime to this timezone
    pub fn from_utc(&self, dt: DateTime<Utc>) -> DateTime<Tz> {
        self.0.from_utc_datetime(&dt.naive_utc())
    }

    /// Convert a DateTime in this timezone to UTC
    pub fn to_utc(&self, dt: DateTime<Tz>) -> DateTime<Utc> {
        dt.with_timezone(&Utc)
    }

    /// Get the current offset for a given UTC datetime
    pub fn offset_at(&self, dt: DateTime<Utc>) -> FixedOffset {
        self.0.offset_from_utc_datetime(&dt.naive_utc()).fix()
    }

    /// Check if a given UTC datetime is in DST for this timezone
    pub fn is_dst(&self, dt: DateTime<Utc>) -> bool {
        let offset = self.offset_at(dt);
        // Compare with standard offset (non-DST) - use January 1st as reference
        let standard_time = chrono::NaiveDate::from_ymd_opt(dt.year(), 1, 1).unwrap().and_hms_opt(12, 0, 0).unwrap();
        let standard_offset = self.0.offset_from_utc_datetime(&standard_time).fix();
        offset != standard_offset
    }

    /// Get the IANA name
    pub fn name(&self) -> &'static str {
        self.0.name()
    }
}

impl FromStr for Timezone {
    type Err = CronError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Tz::from_str(s)
            .map(Timezone)
            .map_err(|_| CronError::InvalidTimezone(s.to_string()))
    }
}

impl fmt::Display for Timezone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.name())
    }
}

impl Default for Timezone {
    fn default() -> Self {
        Self::utc()
    }
}

// Custom serde implementation: serialize as IANA name string
impl serde::Serialize for Timezone {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.name())
    }
}

impl<'de> serde::Deserialize<'de> for Timezone {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Tz::from_str(&s)
            .map(Timezone)
            .map_err(serde::de::Error::custom)
    }
}
