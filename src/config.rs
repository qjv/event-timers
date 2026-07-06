use nexus::paths::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
};

use crate::json_loader::{load_tracks_from_json, EventTrack};

// The serializable config types live in the nexus-free core crate so their
// tests can run natively; re-export them so `crate::config::X` paths keep
// working throughout the addon.
pub use event_timers_core::config::{
    LabelColumnPosition, NotificationConfig, ReminderConfig, Settings, TextAlignment,
    TimeRulerInterval, ToastPosition, TrackVisualConfig, TrackedEventId,
    UserConfig,
};

const USER_CONFIG_FILENAME: &str = "user_config.json";

// === Runtime Configuration ===

/// Live state read and mutated by the UI every frame: the resolved tracks
/// plus the shared Settings. Persisted through UserConfig on save.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub tracks: Vec<EventTrack>,
    pub categories: Vec<String>,
    pub settings: Settings,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        let (tracks, categories) = load_tracks_from_json();
        Self {
            tracks,
            categories,
            settings: Settings::default(),
        }
    }
}

// === Global State ===

pub static RUNTIME_CONFIG: Lazy<Mutex<RuntimeConfig>> =
    Lazy::new(|| Mutex::new(RuntimeConfig::default()));

/// Staging area for load/save only. Between load_user_config and
/// save_user_config its contents are stale: the live state is
/// RUNTIME_CONFIG, and extract_user_overrides overwrites everything here
/// at save time. Never read or write it from feature code — and when
/// holding both locks, always take RUNTIME_CONFIG first.
static USER_CONFIG: Lazy<Mutex<UserConfig>> = Lazy::new(|| Mutex::new(UserConfig::default()));
pub static SELECTED_TRACK: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));
pub static SELECTED_EVENT: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));

// === Configuration Management ===

pub fn apply_user_overrides() {
    let mut runtime = RUNTIME_CONFIG.lock();
    apply_user_overrides_to(&mut runtime);
}

/// Rebuild the given runtime state from USER_CONFIG plus the default
/// tracks. Takes the runtime as a parameter so callers already holding the
/// RUNTIME_CONFIG lock (the settings UI) don't deadlock re-locking it.
fn apply_user_overrides_to(runtime: &mut RuntimeConfig) {
    // Load fresh tracks from JSON (outside locks)
    let (default_tracks, categories) = load_tracks_from_json();

    // Scope 1: Clean up user config
    let (cleaned_custom_tracks, track_overrides, settings) = {
        let mut user_cfg = USER_CONFIG.lock();

        // Deduplicate custom tracks by name (keep first occurrence)
        let mut seen_custom_track_names: HashSet<String> = HashSet::new();
        user_cfg.custom_tracks.retain(|track| {
            if seen_custom_track_names.contains(&track.name) {
                false // Remove duplicate
            } else {
                seen_custom_track_names.insert(track.name.clone());
                true // Keep first occurrence
            }
        });

        // Remove custom tracks that now exist in default JSON
        let default_track_names: HashSet<String> =
            default_tracks.iter().map(|t| t.name.clone()).collect();

        user_cfg
            .custom_tracks
            .retain(|track| !default_track_names.contains(&track.name));

        // Clone data we need (releases lock early)
        (
            user_cfg.custom_tracks.clone(),
            user_cfg.track_overrides.clone(),
            user_cfg.settings.clone(),
        )
    }; // user_cfg lock dropped here

    // Set runtime tracks to defaults
    runtime.tracks = default_tracks;
    runtime.categories = categories;

    // Apply user overrides to default tracks
    for track in &mut runtime.tracks {
        if let Some(override_data) = track_overrides.get(&track.name) {
            if let Some(visible) = override_data.visible {
                track.visible = visible;
            }
            if let Some(height) = override_data.height {
                track.height = height;
            }

            for event in &mut track.events {
                if override_data.disabled_events.contains(&event.name) {
                    event.enabled = false;
                }
            }
        }
    }

    // Add cleaned custom tracks
    runtime.tracks.extend(cleaned_custom_tracks);

    // Apply all user settings
    runtime.settings = settings;
}

pub fn extract_user_overrides() {
    let (default_tracks, _) = load_tracks_from_json();

    let runtime = RUNTIME_CONFIG.lock();
    let mut user_cfg = USER_CONFIG.lock();

    let (track_overrides, custom_tracks) = event_timers_core::config::build_track_overrides(
        &runtime.tracks,
        &default_tracks,
        &user_cfg.track_overrides,
    );
    user_cfg.track_overrides = track_overrides;
    user_cfg.custom_tracks = custom_tracks;
    user_cfg.settings = runtime.settings.clone();
}

// === File I/O ===

pub fn get_user_config_path() -> Option<PathBuf> {
    get_addon_dir("event_timers").map(|p| p.join(USER_CONFIG_FILENAME))
}

pub fn load_user_config() {
    if let Some(path) = get_user_config_path() {
        if path.exists() {
            if let Ok(json_str) = fs::read_to_string(&path) {
                if let Ok(loaded) = serde_json::from_str::<UserConfig>(&json_str) {
                    *USER_CONFIG.lock() = loaded;
                    apply_user_overrides();
                    return;
                }
            }
        }
    }

    apply_user_overrides();
}

pub fn save_user_config() {
    extract_user_overrides();

    let user_cfg = USER_CONFIG.lock();
    if let Some(path) = get_user_config_path() {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).ok();
        }
        if let Ok(json_str) = serde_json::to_string_pretty(&*user_cfg) {
            fs::write(&path, json_str).ok();
        }
    }
}

/// Delete the config file and reset the given live state to defaults.
/// Takes the runtime guard from the caller (the settings UI holds the
/// RUNTIME_CONFIG lock while rendering); locking here would deadlock.
pub fn reset_all_settings(runtime: &mut RuntimeConfig) {
    if let Some(path) = get_user_config_path() {
        if fs::remove_file(&path).is_ok() {
            *USER_CONFIG.lock() = UserConfig::default();
            apply_user_overrides_to(runtime);
        }
    }
}

pub fn get_track_visual_config(
    track_name: &str,
    global_bg: [f32; 4],
    global_padding: f32,
) -> TrackVisualConfig {
    let user_override = {
        let user_cfg = USER_CONFIG.lock();
        user_cfg
            .track_overrides
            .get(track_name)
            .and_then(|o| o.visual.clone())
    };

    if let Some(visual) = user_override {
        return visual;
    }

    TrackVisualConfig {
        background_color: global_bg,
        padding: global_padding,
    }
}
