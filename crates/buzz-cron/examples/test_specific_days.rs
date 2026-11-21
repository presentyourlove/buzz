use cron::Schedule as CronScheduleInner;
use std::str::FromStr;
use chrono::{Datelike, Timelike, TimeZone, Utc};

fn main() {
    // Test specific days - Monday-Friday at 9 AM
    let expr = "0 0 9 * * 1-5";
    let sched = CronScheduleInner::from_str(expr).unwrap();
    let friday = Utc.with_ymd_and_hms(2024, 1, 19, 8, 0, 0).unwrap(); // Friday Jan 19, 2024
    println!("Friday Jan 19, 2024 at 8:00 UTC -> weekday: {}", friday.weekday());
    let next = sched.after(&friday).next().unwrap();
    println!("Next: {}", next);
    println!("Weekday: {}, Day: {}", next.weekday(), next.day());
}
