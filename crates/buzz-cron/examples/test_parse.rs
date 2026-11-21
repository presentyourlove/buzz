use cron::Schedule as CronScheduleInner;
use std::str::FromStr;

fn main() {
    // Test 5-field
    match CronScheduleInner::from_str("0 12 * * *") {
        Ok(s) => println!("5-field OK: {:?}", s),
        Err(e) => println!("5-field ERROR: {:?}", e),
    }
    
    // Test 6-field
    match CronScheduleInner::from_str("0 0 12 * * *") {
        Ok(s) => println!("6-field OK: {:?}", s),
        Err(e) => println!("6-field ERROR: {:?}", e),
    }
    
    // Test with day name
    match CronScheduleInner::from_str("0 0 12 * * MON") {
        Ok(s) => println!("6-field day name OK: {:?}", s),
        Err(e) => println!("6-field day name ERROR: {:?}", e),
    }
    
    // Test every minute 5-field
    match CronScheduleInner::from_str("* * * * *") {
        Ok(s) => println!("every min 5-field OK: {:?}", s),
        Err(e) => println!("every min 5-field ERROR: {:?}", e),
    }
    
    // Test every minute 6-field
    match CronScheduleInner::from_str("* * * * * *") {
        Ok(s) => println!("every min 6-field OK: {:?}", s),
        Err(e) => println!("every min 6-field ERROR: {:?}", e),
    }
}
