use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_current_unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn calculate_tyria_time(utc_timestamp: i64) -> (i32, i32) {
    let reference_time: i64 = 1759264200; // 2025-09-30 17:30:00 UTC-3 = Tyrian 06:00
    
    // Work in seconds for precision, then convert to Tyrian minutes
    let real_seconds_elapsed = utc_timestamp - reference_time;
    
    // 1 real second = 12 Tyrian minutes / 60 seconds = 0.2 Tyrian minutes = 12 Tyrian seconds
    // So: 1 real second = 12 Tyrian seconds
    let tyria_seconds_elapsed = real_seconds_elapsed * 12;
    
    // Convert to Tyrian minutes
    let tyria_minutes_elapsed = tyria_seconds_elapsed / 60;
    
    // Start at 6:00 (360 minutes into the day)
    let total_tyria_minutes = 360 + tyria_minutes_elapsed;
    
    // Wrap around 24-hour cycle (1440 minutes)
    let tyria_minutes_in_day = total_tyria_minutes.rem_euclid(1440);
    
    let hours = (tyria_minutes_in_day / 60) as i32;
    let minutes = (tyria_minutes_in_day % 60) as i32;
    
    (hours, minutes)
}

pub fn format_time_only(timestamp: i64) -> String {
    use chrono::{DateTime, Local};
    let datetime = DateTime::from_timestamp(timestamp, 0)
        .expect("Invalid timestamp");
    datetime.with_timezone(&Local).format("%H:%M").to_string()
}