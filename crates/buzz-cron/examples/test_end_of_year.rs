use cron::Schedule as CronScheduleInner;
use std::str::FromStr;
use chrono::{Datelike, Timelike, TimeZone, Utc};

fn main() {
    // Test end of year
    let expr = "0 0 12 * * *";
    let sched = CronScheduleInner::from_str(expr).unwrap();
    let dec31 = Utc.with_ymd_and_hms(2024, 12, 31, 11, 0, 0).unwrap();
    let next = sched.after(&dec31).next().unwrap();
    println!("Dec 31 11:00 UTC -> next: {}", next);
    println!("Year: {}, Month: {}, Day: {}, Hour: {}", next.year(), next.month(), next.day(), next.hour());
}
