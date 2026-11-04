use crate::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

// Embedded fallback JSON
pub const EMBEDDED_JSON: &str = include_str!("../event_tracks.json");

pub static JSON_LOAD_NOTICE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
static JSON_WRITE_LOCK: Mutex<()> = Mutex::new(());
static LOADED_VERSION: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new("unknown".into()));

// === Public Data Structures ===

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EventColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for EventColor {
    fn default() -> Self {
        Self {
            r: 0.2,
            g: 0.6,
            b: 0.8,
            a: 1.0,
        }
    }
}

impl EventColor {
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn from_array(arr: [f32; 4]) -> Self {
        Self {
            r: arr[0],
            g: arr[1],
            b: arr[2],
            a: arr[3],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TimelineType {
    #[serde(rename = "real_time")]
    RealTime,
    #[serde(rename = "game_time")]
    GameTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimelineEvent {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
    pub start_offset: i64,
    pub duration: i64,
    pub cycle_duration: i64,
    pub color: EventColor,
    #[serde(default)]
    pub copy_text: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl TimelineEvent {
    pub fn display_name(&self) -> &str {
        self.custom_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&self.name)
    }
}

fn default_true() -> bool {
    true
}

impl Default for TimelineEvent {
    fn default() -> Self {
        Self {
            name: "New Event".to_string(),
            custom_name: None,
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

fn default_height() -> f32 {
    40.0
}

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

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct JsonRoot {
    version: String,
    #[serde(default)]
    hash: String,
    categories: Vec<JsonCategory>,
}

pub fn parse_event_tracks_version(content: &str) -> Option<[u64; 3]> {
    let root: JsonRoot = serde_json::from_str(content).ok()?;
    if root.categories.is_empty()
        || root.categories.iter().any(|category| {
            category.tracks.iter().any(|track| {
                track
                    .schedules
                    .iter()
                    .any(|schedule| schedule.duration <= 0 || schedule.interval < 0)
                    || track
                        .events
                        .iter()
                        .any(|event| event.duration <= 0 || event.cycle_duration <= 0)
            })
        })
    {
        return None;
    }
    let mut parts = root.version.split('.');
    let version = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next().unwrap_or("0").parse().ok()?,
    ];
    if parts.next().is_some() {
        return None;
    }
    Some(version)
}

pub fn is_valid_event_tracks_json(content: &str) -> bool {
    parse_event_tracks_version(content).is_some()
}

pub fn current_loaded_version() -> String {
    LOADED_VERSION.lock().clone()
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

    let seconds_since_local_midnight =
        (current_utc_timestamp + timezone_offset).rem_euclid(seconds_per_day);

    current_utc_timestamp - seconds_since_local_midnight
}

fn get_base_time_from_calculator(calculator: &str) -> i64 {
    match calculator {
        "tyria_cycle" => calculate_tyria_base_time(),
        "cantha_cycle" => calculate_cantha_base_time(),
        "local_day_start" => calculate_local_day_start_time(),
        _ => {
            eprintln!(
                "Unknown base_time_calculator: {}, using local_day_start",
                calculator
            );
            calculate_local_day_start_time()
        }
    }
}

// === Event Expansion ===

fn expand_schedule(schedule: &JsonSchedule, cycle_minutes: i32) -> Vec<TimelineEvent> {
    if cycle_minutes <= 0 || schedule.duration <= 0 {
        return Vec::new();
    }
    if schedule.interval == 0 {
        // Single event, no repetition
        return vec![TimelineEvent {
            name: schedule.name.clone(),
            custom_name: None,
            start_offset: i64::from(schedule.offset.rem_euclid(cycle_minutes)) * 60,
            duration: i64::from(schedule.duration) * 60,
            cycle_duration: i64::from(cycle_minutes) * 60,
            color: EventColor::from_array(schedule.color),
            copy_text: schedule.copy_text.clone(),
            enabled: true,
        }];
    }

    if schedule.interval < 0 {
        return Vec::new();
    }

    // Repeating event
    let phase = schedule.offset.rem_euclid(schedule.interval);
    let mut events = Vec::new();
    let mut spawn_time = phase;
    while spawn_time < cycle_minutes {
        events.push(TimelineEvent {
            name: schedule.name.clone(),
            custom_name: None,
            start_offset: i64::from(spawn_time) * 60,
            duration: i64::from(schedule.duration) * 60,
            cycle_duration: i64::from(cycle_minutes) * 60,
            color: EventColor::from_array(schedule.color),
            copy_text: schedule.copy_text.clone(),
            enabled: true,
        });
        spawn_time += schedule.interval;
    }
    events
}

// === JSON Loading ===

pub fn get_json_path() -> Option<PathBuf> {
    get_addon_dir("event_timers").map(|p| p.join("event_tracks.json"))
}

pub fn atomic_write_event_tracks(path: &PathBuf, content: &str) -> Result<(), String> {
    let _write_guard = JSON_WRITE_LOCK.lock();
    if !is_valid_event_tracks_json(content) {
        return Err("Refusing to write invalid event_tracks.json content".to_string());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Only back up existing file if it is valid JSON (never overwrite backup with a 429 error!)
    if path.exists() {
        if let Ok(existing) = fs::read_to_string(path) {
            if is_valid_event_tracks_json(&existing) {
                let backup_path = path.with_extension("json.backup");
                let _ = fs::copy(path, backup_path);
            }
        }
    }
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, content).map_err(|e| format!("Failed to write temp file: {e}"))?;
    fs::OpenOptions::new()
        .write(true)
        .open(&tmp_path)
        .and_then(|file| file.sync_all())
        .map_err(|e| format!("Failed to flush temp file: {e}"))?;
    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Could not replace event_tracks.json: {err}"));
    }
    Ok(())
}

fn load_json_content() -> String {
    let embedded_ver = parse_event_tracks_version(EMBEDDED_JSON);

    if let Some(path) = get_json_path() {
        if path.exists() {
            if let Ok(disk_content) = fs::read_to_string(&path) {
                match (parse_event_tracks_version(&disk_content), embedded_ver) {
                    (Some(disk_ver), Some(emb_ver)) if disk_ver >= emb_ver => {
                        return disk_content;
                    }
                    (Some(_disk_ver), Some(_emb_ver)) => {
                        // Embedded JSON is newer than on-disk JSON! Upgrade on-disk file.
                        if let Err(err) = atomic_write_event_tracks(&path, EMBEDDED_JSON) {
                            *JSON_LOAD_NOTICE.lock() =
                                Some(format!("Using built-in schedule ({err})"));
                        }
                        return EMBEDDED_JSON.to_string();
                    }
                    (None, _) => {
                        // On-disk JSON was corrupted (e.g., 429 error text). Recover from backup or embedded.
                        let backup_path = path.with_extension("json.backup");
                        if let Ok(backup_content) = fs::read_to_string(&backup_path) {
                            if let (Some(bak_ver), Some(emb_ver)) =
                                (parse_event_tracks_version(&backup_content), embedded_ver)
                            {
                                if bak_ver > emb_ver {
                                    let _ = atomic_write_event_tracks(&path, &backup_content);
                                    *JSON_LOAD_NOTICE.lock() =
                                        Some("Recovered event_tracks.json from backup".to_string());
                                    return backup_content;
                                }
                            }
                        }
                        let _ = atomic_write_event_tracks(&path, EMBEDDED_JSON);
                        *JSON_LOAD_NOTICE.lock() = Some(
                            "Repaired corrupted event_tracks.json using built-in schedule"
                                .to_string(),
                        );
                        return EMBEDDED_JSON.to_string();
                    }
                    _ => return disk_content,
                }
            }
        } else {
            let _ = atomic_write_event_tracks(&path, EMBEDDED_JSON);
        }
    }

    // Fallback to embedded JSON
    EMBEDDED_JSON.to_string()
}

pub fn load_tracks_from_json() -> (Vec<EventTrack>, Vec<String>) {
    let json_content = load_json_content();

    match serde_json::from_str::<JsonRoot>(&json_content)
        .or_else(|_| serde_json::from_str::<JsonRoot>(EMBEDDED_JSON))
    {
        Ok(root) => {
            *LOADED_VERSION.lock() = root.version.clone();
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
                            "tyria_cycle" | "cantha_cycle" => 2 * 60, // 2 hours
                            "local_day_start" => 24 * 60,             // 24 hours
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

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(offset: i32, interval: i32, duration: i32) -> JsonSchedule {
        JsonSchedule {
            name: "Test".into(),
            offset,
            interval,
            duration,
            color: [1.0; 4],
            copy_text: String::new(),
        }
    }

    #[test]
    fn repeating_schedule_wraps_its_phase_across_the_day() {
        let events = expand_schedule(&schedule(1090, 120, 25), 1440);
        assert_eq!(events.len(), 12);
        assert_eq!(events.first().unwrap().start_offset, 10 * 60);
        assert_eq!(events.last().unwrap().start_offset, 1330 * 60);
        assert!(events
            .iter()
            .all(|event| event.duration == 25 * 60 && event.cycle_duration == 1440 * 60));
    }

    #[test]
    fn invalid_repeating_schedules_are_ignored() {
        assert!(expand_schedule(&schedule(0, -1, 5), 1440).is_empty());
        assert!(expand_schedule(&schedule(0, 15, 0), 1440).is_empty());
        assert!(expand_schedule(&schedule(0, 15, 5), 0).is_empty());
    }

    #[test]
    fn rejects_rate_limit_error_page_as_event_tracks_json() {
        assert!(!is_valid_event_tracks_json("429: Too Many Requests"));
        assert!(!is_valid_event_tracks_json(
            r#"{"version":"1.2.0","categories":[]}"#
        ));
        assert!(is_valid_event_tracks_json(EMBEDDED_JSON));
    }

    #[test]
    fn schedule_replacement_preserves_valid_backup_and_rejects_error_pages() {
        let dir =
            std::env::temp_dir().join(format!("event-timers-json-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("event_tracks.json");
        fs::write(&path, EMBEDDED_JSON).unwrap();
        assert!(atomic_write_event_tracks(&path, "429: Too Many Requests").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), EMBEDDED_JSON);
        atomic_write_event_tracks(&path, EMBEDDED_JSON).unwrap();
        assert_eq!(
            fs::read_to_string(path.with_extension("json.backup")).unwrap(),
            EMBEDDED_JSON
        );
        fs::write(&path, "429: Too Many Requests").unwrap();
        atomic_write_event_tracks(&path, EMBEDDED_JSON).unwrap();
        assert_eq!(
            fs::read_to_string(path.with_extension("json.backup")).unwrap(),
            EMBEDDED_JSON
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_invalid_event_cycles_and_extra_version_components() {
        let mut json: serde_json::Value = serde_json::from_str(EMBEDDED_JSON).unwrap();
        json["version"] = "1.2.0.1".into();
        assert!(!is_valid_event_tracks_json(&json.to_string()));
        json["version"] = "1.2.0".into();
        json["categories"][0]["tracks"][0]["events"] = serde_json::json!([{
            "name": "Invalid", "start_offset": 0, "duration": 60, "cycle_duration": 0,
            "color": { "r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0 }
        }]);
        assert!(!is_valid_event_tracks_json(&json.to_string()));
    }
}
