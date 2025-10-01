use nexus::paths::get_addon_dir;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

// Embedded fallback JSON
const EMBEDDED_JSON: &str = include_str!("../event_tracks.json");

// === Public Data Structures ===

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for EventColor {
    fn default() -> Self {
        Self { r: 0.2, g: 0.6, b: 0.8, a: 1.0 }
    }
}

impl EventColor {
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    
    pub fn from_array(arr: [f32; 4]) -> Self {
        Self { r: arr[0], g: arr[1], b: arr[2], a: arr[3] }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TimelineType {
    #[serde(rename = "real_time")]
    RealTime,
    #[serde(rename = "game_time")]
    GameTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimelineEvent {
    pub name: String,
    pub start_offset: i64,
    pub duration: i64,
    pub cycle_duration: i64,
    pub color: EventColor,
    #[serde(default)]
    pub copy_text: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

impl Default for TimelineEvent {
    fn default() -> Self {
        Self {
            name: "New Event".to_string(),
            start_offset: 0,
            duration: 300,
            cycle_duration: 7200,
            color: EventColor::default(),
            copy_text: String::new(),
            enabled: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventTrack {
    pub name: String,
    pub timeline_type: TimelineType,
    pub events: Vec<TimelineEvent>,
    pub base_time: i64,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_height")]
    pub height: f32,
    #[serde(default)]
    pub category: String,
}

fn default_height() -> f32 { 40.0 }

impl Default for EventTrack {
    fn default() -> Self {
        Self {
            name: "New Track".to_string(),
            timeline_type: TimelineType::GameTime,
            events: Vec::new(),
            base_time: 0,
            visible: true,
            height: 40.0,
            category: String::new(),
        }
    }
}

// === JSON File Structures ===

#[derive(Deserialize, Debug)]
struct JsonSchedule {
    name: String,
    offset: i32,
    #[serde(default)]
    interval: i32,
    duration: i32,
    color: [f32; 4],
    #[serde(default)]
    copy_text: String,
}

#[derive(Deserialize, Debug)]
struct JsonTrack {
    name: String,
    timeline_type: TimelineType,
    base_time_calculator: String,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default = "default_height")]
    height: f32,
    #[serde(default)]
    schedules: Vec<JsonSchedule>,
    #[serde(default)]
    events: Vec<TimelineEvent>,
}

#[derive(Deserialize, Debug)]
struct JsonCategory {
    name: String,
    tracks: Vec<JsonTrack>,
}

#[derive(Deserialize, Debug)]
struct JsonRoot {
    version: String,
    #[serde(default)]
    hash: String,
    categories: Vec<JsonCategory>,
}

// === Time Calculators ===

fn calculate_tyria_base_time() -> i64 {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    // Reference: 2025-09-30 17:00:00 UTC-3 = Tyrian 00:00
    let reference_time: i64 = 1759262400;
    let cycle_duration = 120 * 60; // 120 minutes in seconds
    
    let time_since_reference = current_time - reference_time;
    let cycles_elapsed = time_since_reference / cycle_duration;
    let current_cycle_start = reference_time + (cycles_elapsed * cycle_duration);
    
    current_cycle_start
}

fn calculate_cantha_base_time() -> i64 {
    calculate_tyria_base_time()
}

fn calculate_local_day_start_time() -> i64 {
    let current_utc_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let seconds_per_day = 24 * 60 * 60;
    let timezone_offset = -3 * 60 * 60; // UTC-3

    let seconds_since_local_midnight = (current_utc_timestamp + timezone_offset)
        .rem_euclid(seconds_per_day);

    current_utc_timestamp - seconds_since_local_midnight
}

fn get_base_time_from_calculator(calculator: &str) -> i64 {
    match calculator {
        "tyria_cycle" => calculate_tyria_base_time(),
        "cantha_cycle" => calculate_cantha_base_time(),
        "local_day_start" => calculate_local_day_start_time(),
        _ => {
            eprintln!("Unknown base_time_calculator: {}, using local_day_start", calculator);
            calculate_local_day_start_time()
        }
    }
}

// === Event Expansion ===

fn expand_schedule(schedule: &JsonSchedule, cycle_minutes: i32) -> Vec<TimelineEvent> {
    if schedule.interval == 0 {
        // Single event, no repetition
        return vec![TimelineEvent {
            name: schedule.name.clone(),
            start_offset: (schedule.offset * 60) as i64,
            duration: (schedule.duration * 60) as i64,
            cycle_duration: (cycle_minutes * 60) as i64,
            color: EventColor::from_array(schedule.color),
            copy_text: schedule.copy_text.clone(),
            enabled: true,
        }];
    }
    
    // Repeating event
    let repetitions = cycle_minutes / schedule.interval;
    (0..repetitions)
        .map(|i| {
            let spawn_time = schedule.offset + i * schedule.interval;
            TimelineEvent {
                name: schedule.name.clone(),
                start_offset: (spawn_time * 60) as i64,
                duration: (schedule.duration * 60) as i64,
                cycle_duration: (cycle_minutes * 60) as i64,
                color: EventColor::from_array(schedule.color),
                copy_text: schedule.copy_text.clone(),
                enabled: true,
            }
        })
        .collect()
}

// === JSON Loading ===

fn get_json_path() -> Option<PathBuf> {
    get_addon_dir("event_timers").map(|p| p.join("event_tracks.json"))
}

fn load_json_content() -> String {
    if let Some(path) = get_json_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                return content;
            }
        }
    }
    
    // Fallback to embedded JSON
    EMBEDDED_JSON.to_string()
}

pub fn load_tracks_from_json() -> (Vec<EventTrack>, Vec<String>) {
    let json_content = load_json_content();
    
    match serde_json::from_str::<JsonRoot>(&json_content) {
        Ok(root) => {
            let mut all_tracks = Vec::new();
            let mut category_names = Vec::new();
            
            for category in root.categories {
                category_names.push(category.name.clone());
                
                for json_track in category.tracks {
                    let base_time = get_base_time_from_calculator(&json_track.base_time_calculator);
                    
                    let mut events = json_track.events;
                    
                    // Expand schedules into events
                    for schedule in &json_track.schedules {
                        let cycle_minutes = match json_track.base_time_calculator.as_str() {
                            "tyria_cycle" | "cantha_cycle" => 2 * 60,  // 2 hours
                            "local_day_start" => 24 * 60,              // 24 hours
                            _ => 24 * 60,
                        };
                        events.extend(expand_schedule(schedule, cycle_minutes));
                    }
                    
                    all_tracks.push(EventTrack {
                        name: json_track.name,
                        timeline_type: json_track.timeline_type,
                        events,
                        base_time,
                        visible: json_track.visible,
                        height: json_track.height,
                        category: category.name.clone(),
                    });
                }
            }
            
            (all_tracks, category_names)
        }
        Err(e) => {
            eprintln!("Failed to parse event_tracks.json: {}", e);
            eprintln!("Using empty track list");
            (Vec::new(), Vec::new())
        }
    }
}