use nexus::paths::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

use crate::json_loader::{load_tracks_from_json, EventTrack, EventColor, TimelineEvent};

const USER_CONFIG_FILENAME: &str = "user_config.json";

// === Visual Configuration ===

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrackVisualConfig {
    #[serde(default = "default_track_bg_color")]
    pub background_color: [f32; 4],
    #[serde(default = "default_track_padding")]
    pub padding: f32,
}

fn default_track_bg_color() -> [f32; 4] { [0.2, 0.2, 0.2, 1.0] }
fn default_track_padding() -> f32 { 5.0 }

impl Default for TrackVisualConfig {
    fn default() -> Self {
        Self {
            background_color: [0.2, 0.2, 0.2, 1.0],
            padding: 5.0,
        }
    }
}

// === Track Override ===

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EventOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,           // Replaces disabled_events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,         // Custom color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<i64>,       // Custom timing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,           // Custom duration
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TrackOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    
    // NEW: Per-event overrides (keyed by event name)
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub event_overrides: HashMap<String, EventOverride>,
    
    // DEPRECATED but kept for migration
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disabled_events: Vec<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<TrackVisualConfig>,
}

// === User Configuration ===

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserConfig {
    #[serde(default)]
    pub track_overrides: HashMap<String, TrackOverride>,
    #[serde(default)]
    pub custom_tracks: Vec<EventTrack>,
    #[serde(default)]
    pub category_visibility: HashMap<String, bool>,
    #[serde(default = "default_true")]
    pub show_main_window: bool,
    #[serde(default)]
    pub is_window_locked: bool,
    #[serde(default)]
    pub hide_background: bool,
    #[serde(default = "default_true")]
    pub show_time_ruler: bool,
    #[serde(default = "default_true")]
    pub show_scrollbar: bool,
    #[serde(default = "default_timeline_width")]
    pub timeline_width: f32,
    #[serde(default = "default_view_range")]
    pub view_range_seconds: f32,
    #[serde(default = "default_time_position")]
    pub current_time_position: f32,
    #[serde(default)]
    pub show_category_headers: bool,
    #[serde(default = "default_spacing_same_category")]
    pub spacing_same_category: f32,
    #[serde(default = "default_spacing_between_categories")]
    pub spacing_between_categories: f32,
    #[serde(default)]
    pub category_order: Vec<String>,
    #[serde(default = "default_global_track_bg")]
    pub global_track_background: [f32; 4],
    #[serde(default)]
    pub global_track_padding: f32,
    #[serde(default)]
    pub override_all_track_heights: bool,
    #[serde(default = "default_height")]
    pub global_track_height: f32,
    #[serde(default)]
    pub draw_event_borders: bool,
    #[serde(default = "default_border_color")]
    pub event_border_color: [f32; 4],
    #[serde(default = "default_border_thickness")]
    pub event_border_thickness: f32,
}

fn default_global_track_bg() -> [f32; 4] { [0.2, 0.2, 0.2, 0.2] } // #33333333
fn default_border_color() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] } // #000000FF
fn default_border_thickness() -> f32 { 1.0 }
fn default_height() -> f32 { 40.0 }

fn default_true() -> bool { true }
fn default_timeline_width() -> f32 { 800.0 }
fn default_view_range() -> f32 { 3600.0 }
fn default_time_position() -> f32 { 0.5 }
fn default_spacing_same_category() -> f32 { 0.0 }
fn default_spacing_between_categories() -> f32 { 0.0 }

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            track_overrides: HashMap::new(),
            custom_tracks: Vec::new(),
            category_visibility: HashMap::new(),
            show_main_window: true,
            is_window_locked: false,
            hide_background: false,
            show_time_ruler: true,
            show_scrollbar: true,
            timeline_width: 800.0,
            view_range_seconds: 3600.0,
            current_time_position: 0.5,
            show_category_headers: false,
            spacing_same_category: 0.0,
            spacing_between_categories: 0.0,
            category_order: Vec::new(),
            global_track_background: [0.2, 0.2, 0.2, 0.2],
            global_track_padding: 0.0,
            override_all_track_heights: false,
            global_track_height: default_height(),
            draw_event_borders: true,
            event_border_color: [0.0, 0.0, 0.0, 1.0],
            event_border_thickness: 1.0,
        }
    }
}

// === Runtime Configuration ===

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub tracks: Vec<EventTrack>,
    pub categories: Vec<String>,
    pub category_visibility: HashMap<String, bool>,
    pub show_main_window: bool,
    pub is_window_locked: bool,
    pub hide_background: bool,
    pub show_time_ruler: bool,
    pub show_scrollbar: bool,
    pub timeline_width: f32,
    pub view_range_seconds: f32,
    pub current_time_position: f32,
    pub show_category_headers: bool,
    pub spacing_same_category: f32,
    pub spacing_between_categories: f32,
    pub category_order: Vec<String>,
    pub global_track_background: [f32; 4],
    pub global_track_padding: f32,
    pub override_all_track_heights: bool,
    pub global_track_height: f32,
    pub draw_event_borders: bool,
    pub event_border_color: [f32; 4],
    pub event_border_thickness: f32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        let (tracks, categories) = load_tracks_from_json();
        Self {
            tracks,
            categories,
            category_visibility: HashMap::new(),
            show_main_window: false,
            is_window_locked: false,
            hide_background: false,
            show_time_ruler: false,
            show_scrollbar: true,
            timeline_width: 800.0,
            view_range_seconds: 3600.0,
            current_time_position: 0.5,
            show_category_headers: false,
            spacing_same_category: 0.0,
            spacing_between_categories: 0.0,
            category_order: Vec::new(),
            global_track_background: [0.2, 0.2, 0.2, 0.2],
            global_track_padding: 0.0,
            override_all_track_heights: false,
            global_track_height: default_height(),
            draw_event_borders: true,
            event_border_color: [0.0, 0.0, 0.0, 1.0],
            event_border_thickness: 1.0,
        }
    }
}

// === Global State ===

pub static RUNTIME_CONFIG: Lazy<Mutex<RuntimeConfig>> = Lazy::new(|| Mutex::new(RuntimeConfig::default()));
pub static USER_CONFIG: Lazy<Mutex<UserConfig>> = Lazy::new(|| Mutex::new(UserConfig::default()));
pub static SELECTED_TRACK: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));
pub static SELECTED_EVENT: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));

// === Configuration Management ===

pub fn apply_user_overrides() {
    let user_cfg = USER_CONFIG.lock();
    let mut runtime = RUNTIME_CONFIG.lock();
    
    let (default_tracks, categories) = load_tracks_from_json();
    runtime.tracks = default_tracks;
    runtime.categories = categories;
    
    for track in &mut runtime.tracks {
        if let Some(override_data) = user_cfg.track_overrides.get(&track.name) {
            // Track-level overrides
            if let Some(visible) = override_data.visible {
                track.visible = visible;
            }
            if let Some(height) = override_data.height {
                track.height = height;
            }
            
            // Event-level overrides
            for event in &mut track.events {
                if let Some(event_override) = override_data.event_overrides.get(&event.name) {
                    if let Some(enabled) = event_override.enabled {
                        event.enabled = enabled;
                    }
                    if let Some(color) = event_override.color {
                        event.color = EventColor::from_array(color);
                    }
                    if let Some(offset) = event_override.start_offset {
                        event.start_offset = offset;
                    }
                    if let Some(duration) = event_override.duration {
                        event.duration = duration;
                    }
                }
            }
        }
    }
    
    runtime.tracks.extend(user_cfg.custom_tracks.iter().cloned());
    
    runtime.show_main_window = user_cfg.show_main_window;
    runtime.is_window_locked = user_cfg.is_window_locked;
    runtime.hide_background = user_cfg.hide_background;
    runtime.show_time_ruler = user_cfg.show_time_ruler;
    runtime.show_scrollbar = user_cfg.show_scrollbar;
    runtime.timeline_width = user_cfg.timeline_width;
    runtime.view_range_seconds = user_cfg.view_range_seconds;
    runtime.current_time_position = user_cfg.current_time_position;
    runtime.show_category_headers = user_cfg.show_category_headers;
    runtime.spacing_same_category = user_cfg.spacing_same_category;
    runtime.spacing_between_categories = user_cfg.spacing_between_categories;
    runtime.category_order = user_cfg.category_order.clone();
    runtime.global_track_background = user_cfg.global_track_background;
    runtime.global_track_padding = user_cfg.global_track_padding;
    runtime.override_all_track_heights = user_cfg.override_all_track_heights;
    runtime.global_track_height = user_cfg.global_track_height;
    runtime.draw_event_borders = user_cfg.draw_event_borders;
    runtime.event_border_color = user_cfg.event_border_color;
    runtime.event_border_thickness = user_cfg.event_border_thickness;
    runtime.category_visibility = user_cfg.category_visibility.clone();
}

pub fn extract_user_overrides() {
    let runtime = RUNTIME_CONFIG.lock();
    let mut user_cfg = USER_CONFIG.lock();
    
    user_cfg.track_overrides.clear();
    user_cfg.custom_tracks.clear();
    
    let (default_tracks, _) = load_tracks_from_json();
    let default_map: HashMap<String, &EventTrack> = default_tracks
        .iter()
        .map(|t| (t.name.clone(), t))
        .collect();
    
    for track in &runtime.tracks {
        if let Some(default_track) = default_map.get(&track.name) {
            let mut override_data = TrackOverride::default();
            let mut has_changes = false;
            
            // Track-level changes
            if track.visible != default_track.visible {
                override_data.visible = Some(track.visible);
                has_changes = true;
            }
            
            if (track.height - default_track.height).abs() > 0.1 {
                override_data.height = Some(track.height);
                has_changes = true;
            }
            
            // Event-level changes
            let default_event_map: HashMap<String, &TimelineEvent> = default_track.events
                .iter()
                .map(|e| (e.name.clone(), e))
                .collect();
            
            for event in &track.events {
                if let Some(default_event) = default_event_map.get(&event.name) {
                    let mut event_override = EventOverride::default();
                    let mut event_has_changes = false;
                    
                    if event.enabled != default_event.enabled {
                        event_override.enabled = Some(event.enabled);
                        event_has_changes = true;
                    }
                    
                    if event.color.to_array() != default_event.color.to_array() {
                        event_override.color = Some(event.color.to_array());
                        event_has_changes = true;
                    }
                    
                    if event.start_offset != default_event.start_offset {
                        event_override.start_offset = Some(event.start_offset);
                        event_has_changes = true;
                    }
                    
                    if event.duration != default_event.duration {
                        event_override.duration = Some(event.duration);
                        event_has_changes = true;
                    }
                    
                    if event_has_changes {
                        override_data.event_overrides.insert(event.name.clone(), event_override);
                        has_changes = true;
                    }
                }
            }
            
            if has_changes {
                user_cfg.track_overrides.insert(track.name.clone(), override_data);
            }
        } else {
            user_cfg.custom_tracks.push(track.clone());
        }
    }
    
    user_cfg.show_main_window = runtime.show_main_window;
    user_cfg.is_window_locked = runtime.is_window_locked;
    user_cfg.hide_background = runtime.hide_background;
    user_cfg.show_time_ruler = runtime.show_time_ruler;
    user_cfg.show_scrollbar = runtime.show_scrollbar;
    user_cfg.timeline_width = runtime.timeline_width;
    user_cfg.view_range_seconds = runtime.view_range_seconds;
    user_cfg.current_time_position = runtime.current_time_position;
    user_cfg.show_category_headers = runtime.show_category_headers;
    user_cfg.spacing_same_category = runtime.spacing_same_category;
    user_cfg.spacing_between_categories = runtime.spacing_between_categories;
    user_cfg.category_order = runtime.category_order.clone();
    user_cfg.global_track_background = runtime.global_track_background;
    user_cfg.global_track_padding = runtime.global_track_padding;
    user_cfg.override_all_track_heights = runtime.override_all_track_heights;
    user_cfg.global_track_height = runtime.global_track_height;
    user_cfg.draw_event_borders = runtime.draw_event_borders;
    user_cfg.event_border_color = runtime.event_border_color;
    user_cfg.event_border_thickness = runtime.event_border_thickness;
    user_cfg.category_visibility = runtime.category_visibility.clone();
}

// === File I/O ===

fn get_user_config_path() -> Option<PathBuf> {
    get_addon_dir("event_timers").map(|p| p.join(USER_CONFIG_FILENAME))
}

pub fn load_user_config() {
    if let Some(path) = get_user_config_path() {
        if path.exists() {
            if let Ok(json_str) = fs::read_to_string(&path) {
                if let Ok(mut loaded) = serde_json::from_str::<UserConfig>(&json_str) {
                    
                    // MIGRATION: Convert disabled_events to event_overrides
                    let mut needs_save = false;
                    for track_override in loaded.track_overrides.values_mut() {
                        if !track_override.disabled_events.is_empty() {
                            for event_name in &track_override.disabled_events {
                                track_override.event_overrides
                                    .entry(event_name.clone())
                                    .or_insert(EventOverride {
                                        enabled: Some(false),
                                        ..Default::default()
                                    });
                            }
                            track_override.disabled_events.clear();
                            needs_save = true;
                        }
                    }
                    
                    *USER_CONFIG.lock() = loaded;
                    apply_user_overrides();
                    
                    if needs_save {
                        save_user_config(); // Auto-save migrated config
                    }
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