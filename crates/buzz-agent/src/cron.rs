//! Shared Cron Scheduling Module
//! Extracted from health_guardian.rs for reuse across proactive triggers, scheduled reminders, and health checks.

use std::time::SystemTime;

use chrono::{DateTime, Duration, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};

use crate::types::AgentError;

/// Cron 排程表達式 - 支援標準 5/6 欄位 cron 表達式
/// 
/// 範例：
/// - "0 4 * * *"     - 每天 04:00 執行 (5 欄位，自動補齊秒數為 0)
/// - "0 0 4 * * *"   - 每天 04:00:00 執行 (6 欄位)
/// - "*/5 * * * *"   - 每 5 分鐘執行一次
/// - "0 */2 * * *"   - 每 2 小時執行一次
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronSchedule {
    pub expression: String,
    #[serde(skip)]
    schedule: Option<Schedule>,
}

impl CronSchedule {
    /// 建立新的 CronSchedule
    /// 
    /// # Arguments
    /// * `expression` - cron 表達式 (5 或 6 欄位)
    /// 
    /// # Example
    /// ```
    /// let cron = CronSchedule::new("0 4 * * *".to_string()); // 每天 04:00
    /// ```
    pub fn new(expression: String) -> Self {
        Self { expression, schedule: None }
    }

    /// 取得或建立 cron Schedule (延遲初始化，支援 5 欄位自動補齊秒數為 6 欄位)
    pub fn get_schedule(&mut self) -> Result<&Schedule, AgentError> {
        if self.schedule.is_none() {
            let parts: Vec<&str> = self.expression.split_whitespace().collect();
            let expr = if parts.len() == 5 {
                // 5 欄位 -> 自動補齊秒數為 0，轉為 6 欄位
                format!("0 {}", self.expression)
            } else {
                self.expression.clone()
            };
            let schedule = expr.parse::<Schedule>()
                .map_err(|e| AgentError::Other(format!("Invalid cron expression '{}': {}", self.expression, e)))?;
            self.schedule = Some(schedule);
        }
        Ok(self.schedule.as_ref().unwrap())
    }

    /// 檢查指定時間是否匹配排程
    /// 
    /// # Arguments
    /// * `now` - 要檢查的時間點
    /// 
    /// # Returns
    /// * `Ok(true)` - 時間匹配排程
    /// * `Ok(false)` - 時間不匹配
    /// * `Err` - 解析 cron 表達式失敗
    pub fn matches(&mut self, now: SystemTime) -> Result<bool, AgentError> {
        let schedule = self.get_schedule()?;
        let dt: DateTime<Utc> = now.into();
        // cron::Schedule::after() 回傳「下一個」時間點，我們檢查前一秒是否等於當前時間
        let prev = dt - Duration::seconds(1);
        Ok(schedule.after(&prev).take(1).next() == Some(dt))
    }

    /// 取得指定時間之後的下一次執行時間
    /// 
    /// # Arguments
    /// * `after` - 基準時間
    /// 
    /// # Returns
    /// * `Ok(SystemTime)` - 下一次執行時間
    /// * `Err` - 解析失敗或無法計算下一個時間
    pub fn next_after(&mut self, after: SystemTime) -> Result<SystemTime, AgentError> {
        let schedule = self.get_schedule()?;
        let after_dt: DateTime<Utc> = after.into();
        let next = schedule.after(&after_dt).next()
            .ok_or_else(|| AgentError::Other("No upcoming schedule found".into()))?;
        Ok(next.into())
    }

    /// 取得 cron 表達式字串
    pub fn expression(&self) -> &str {
        &self.expression
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_cron_schedule_5_field_auto_seconds() {
        let mut cron = CronSchedule::new("0 4 * * *".to_string()); // 5 欄位：每天 04:00
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 0).unwrap();
        let sys_time: SystemTime = t.into();
        assert!(cron.matches(sys_time).unwrap(), "5-field cron should match at 04:00:00");
        
        let t2 = Utc.with_ymd_and_hms(2026, 1, 2, 4, 0, 0).unwrap();
        let sys_time2: SystemTime = t2.into();
        assert!(cron.matches(sys_time2).unwrap(), "Should match next day at 04:00:00");
        
        let t3 = Utc.with_ymd_and_hms(2026, 1, 1, 5, 0, 0).unwrap();
        let sys_time3: SystemTime = t3.into();
        assert!(!cron.matches(sys_time3).unwrap(), "Should not match at 05:00:00");
    }

    #[test]
    fn test_cron_schedule_6_field_explicit_seconds() {
        let mut cron = CronSchedule::new("0 0 4 * * *".to_string()); // 6 欄位：每天 04:00:00
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 0).unwrap();
        let sys_time: SystemTime = t.into();
        assert!(cron.matches(sys_time).unwrap());
        
        let t2 = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 1).unwrap();
        let sys_time2: SystemTime = t2.into();
        assert!(!cron.matches(sys_time2).unwrap(), "Should not match at 04:00:01");
    }

    #[test]
    fn test_cron_schedule_every_5_minutes() {
        let mut cron = CronSchedule::new("*/5 * * * *".to_string()); // 每 5 分鐘
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let sys_time: SystemTime = t.into();
        assert!(cron.matches(sys_time).unwrap());
        
        let t2 = Utc.with_ymd_and_hms(2026, 1, 1, 12, 5, 0).unwrap();
        let sys_time2: SystemTime = t2.into();
        assert!(cron.matches(sys_time2).unwrap());
        
        let t3 = Utc.with_ymd_and_hms(2026, 1, 1, 12, 3, 0).unwrap();
        let sys_time3: SystemTime = t3.into();
        assert!(!cron.matches(sys_time3).unwrap());
    }

    #[test]
    fn test_cron_next_after() {
        let mut cron = CronSchedule::new("0 4 * * *".to_string());
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 3, 30, 0).unwrap();
        let next = cron.next_after(t.into()).unwrap();
        let expected: SystemTime = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 0).unwrap().into();
        assert_eq!(next, expected);
    }

    #[test]
    fn test_cron_next_after_cross_day() {
        let mut cron = CronSchedule::new("0 4 * * *".to_string());
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 5, 0, 0).unwrap();
        let next = cron.next_after(t.into()).unwrap();
        let expected: SystemTime = Utc.with_ymd_and_hms(2026, 1, 2, 4, 0, 0).unwrap().into();
        assert_eq!(next, expected);
    }

    #[test]
    fn test_cron_invalid_expression() {
        let mut cron = CronSchedule::new("invalid cron".to_string());
        let t = Utc.with_ymd_and_hms(2026, 1, 1, 4, 0, 0).unwrap();
        let result = cron.matches(t.into());
        assert!(result.is_err(), "Invalid cron expression should return error");
    }

    #[test]
    fn test_cron_serialization() {
        let cron = CronSchedule::new("0 4 * * *".to_string());
        let json = serde_json::to_string(&cron).unwrap();
        assert!(json.contains("0 4 * * *"));
        
        let deserialized: CronSchedule = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.expression(), "0 4 * * *");
    }
}
