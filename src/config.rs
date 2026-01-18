use nexus::paths::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fs, hash::Hash, path::PathBuf};

use crate::json_loader::{load_tracks_from_json, EventTrack};

// === Notification Types ===

/// Identifies a specific event by track and event name
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TrackedEventId {
    pub track_name: String,
    pub event_name: String,
}

impl Hash for TrackedEventId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.track_name.hash(state);
        self.event_name.hash(state);
    }
}

impl TrackedEventId {
    pub fn new(track_name: &str, event_name: &str) -> Self {
        Self {
            track_name: track_name.to_string(),
            event_name: event_name.to_string(),
        }
    }

    pub fn display_name(&self) -> String {
        format!("{}: {}", self.track_name, self.event_name)
    }
}

/// Toast notification position anchor
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ToastPosition {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl Default for ToastPosition {
    fn default() -> Self {
        Self::TopRight
    }
}

/// A single reminder configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReminderConfig {
    /// Display name for this reminder (e.g., "Heads up!", "Starting soon!")
    pub name: String,
    /// Minutes before the event to trigger (0 = during event, repeats based on interval)
    pub minutes_before: u32,
    /// Color for the reminder text
    pub text_color: [f32; 4],
    /// For ongoing reminders (minutes_before=0): interval in minutes between notifications
    #[serde(default = "default_ongoing_interval")]
    pub ongoing_interval_minutes: u32,
}

fn default_ongoing_interval() -> u32 { 5 }

impl Default for ReminderConfig {
    fn default() -> Self {
        Self {
            name: "Starting soon!".to_string(),
            minutes_before: 5,
            text_color: [0.5, 1.0, 0.5, 1.0], // Green
            ongoing_interval_minutes: 5,
        }
    }
}

fn default_reminders() -> Vec<ReminderConfig> {
    vec![
        ReminderConfig {
            name: "Heads up!".to_string(),
            minutes_before: 10,
            text_color: [0.5, 0.8, 1.0, 1.0], // Light blue
            ongoing_interval_minutes: 5,
        },
        ReminderConfig {
            name: "Starting soon!".to_string(),
            minutes_before: 5,
            text_color: [1.0, 0.8, 0.2, 1.0], // Yellow/orange
            ongoing_interval_minutes: 5,
        },
        ReminderConfig {
            name: "Happening now!".to_string(),
            minutes_before: 0,
            text_color: [0.5, 1.0, 0.5, 1.0], // Green
            ongoing_interval_minutes: 5,
        },
    ]
}

/// Settings for the notification system
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NotificationConfig {
    #[serde(default = "default_true")]
    pub toast_enabled: bool,

    #[serde(default)]
    pub upcoming_panel_enabled: bool,

    #[serde(default = "default_reminders")]
    pub reminders: Vec<ReminderConfig>,

    #[serde(default = "default_toast_duration")]
    pub toast_duration_seconds: f32,

    #[serde(default = "default_max_toasts")]
    pub max_visible_toasts: usize,

    #[serde(default = "default_upcoming_panel_size")]
    pub upcoming_panel_size: [f32; 2],

    #[serde(default = "default_max_upcoming")]
    pub max_upcoming_events: usize,

    #[serde(default)]
    pub toast_position: ToastPosition,

    #[serde(default = "default_toast_size")]
    pub toast_size: [f32; 2],

    /// X offset from corner (percentage of screen width, 0.0 to 1.0)
    #[serde(default)]
    pub toast_offset_x: f32,

    /// Y offset from corner (percentage of screen height, 0.0 to 1.0)
    #[serde(default)]
    pub toast_offset_y: f32,

    #[serde(default = "default_toast_bg_color")]
    pub toast_bg_color: [f32; 4],

    #[serde(default = "default_toast_text_scale")]
    pub toast_text_scale: f32,

    #[serde(default = "default_toast_title_color")]
    pub toast_title_color: [f32; 4],

    #[serde(default = "default_toast_time_color")]
    pub toast_time_color: [f32; 4],

    #[serde(default = "default_toast_track_color")]
    pub toast_track_color: [f32; 4],
}

fn default_toast_duration() -> f32 { 5.0 }
fn default_max_toasts() -> usize { 3 }
fn default_upcoming_panel_size() -> [f32; 2] { [300.0, 200.0] }
fn default_max_upcoming() -> usize { 10 }
fn default_toast_size() -> [f32; 2] { [280.0, 80.0] }
fn default_toast_bg_color() -> [f32; 4] { [0.1, 0.1, 0.1, 0.95] }
fn default_toast_text_scale() -> f32 { 1.2 }
fn default_toast_title_color() -> [f32; 4] { [1.0, 0.8, 0.2, 1.0] }
fn default_toast_time_color() -> [f32; 4] { [0.5, 1.0, 0.5, 1.0] }
fn default_toast_track_color() -> [f32; 4] { [0.7, 0.7, 0.7, 1.0] }

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            toast_enabled: true,
            upcoming_panel_enabled: false,
            reminders: default_reminders(),
            toast_duration_seconds: 5.0,
            max_visible_toasts: 3,
            upcoming_panel_size: [300.0, 200.0],
            max_upcoming_events: 10,
            toast_position: ToastPosition::default(),
            toast_size: default_toast_size(),
            toast_offset_x: 0.0,
            toast_offset_y: 0.0,
            toast_bg_color: default_toast_bg_color(),
            toast_text_scale: default_toast_text_scale(),
            toast_title_color: default_toast_title_color(),
            toast_time_color: default_toast_time_color(),
            toast_track_color: default_toast_track_color(),
        }
    }
}

const USER_CONFIG_FILENAME: &str = "user_config.json";

// === Alignment Options ===

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl Default for TextAlignment {
    fn default() -> Self {
        Self::Center
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum LabelColumnPosition {
    None,
    Left,
    Right,
}

impl Default for LabelColumnPosition {
    fn default() -> Self {
        Self::None
    }
}

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
pub struct TrackOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
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
    #[serde(default)]
    pub category_header_alignment: TextAlignment,
    #[serde(default)]
    pub category_header_padding: f32,
    #[serde(default)]
    pub label_column_position: LabelColumnPosition,
    #[serde(default = "default_label_column_width")]
    pub label_column_width: f32,
    #[serde(default)]
    pub label_column_show_category: bool,
    #[serde(default = "default_true")]
    pub label_column_show_track: bool,
    #[serde(default = "default_label_text_size")]
    pub label_column_text_size: f32,
    #[serde(default)]
    pub label_column_bg_color: [f32; 4],
    #[serde(default = "default_label_text_color")]
    pub label_column_text_color: [f32; 4],
    #[serde(default = "default_label_category_color")]
    pub label_column_category_color: [f32; 4],
    #[serde(default = "default_true")]
    pub close_on_escape: bool,
    #[serde(default)]
    pub copy_with_event_name: bool,

    // === Notification Settings ===
    #[serde(default)]
    pub tracked_events: HashSet<TrackedEventId>,

    #[serde(default)]
    pub notification_config: NotificationConfig,
}

fn default_global_track_bg() -> [f32; 4] { [0.2, 0.2, 0.2, 0.2] } // #33333333
fn default_border_color() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] } // #000000FF
fn default_border_thickness() -> f32 { 1.0 }
fn default_height() -> f32 { 40.0 }
fn default_label_column_width() -> f32 { 150.0 }
fn default_label_text_size() -> f32 { 1.0 }
fn default_label_text_color() -> [f32; 4] { [1.0, 1.0, 1.0, 1.0] } // White
fn default_label_category_color() -> [f32; 4] { [0.8, 0.8, 0.2, 1.0] } // Yellow like default

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
            show_main_window: false,
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
            category_header_alignment: TextAlignment::Center,
            category_header_padding: 0.0,
            label_column_position: LabelColumnPosition::None,
            label_column_width: 150.0,
            label_column_show_category: false,
            label_column_show_track: true,
            label_column_text_size: 1.0,
            label_column_bg_color: [0.0, 0.0, 0.0, 0.0],
            label_column_text_color: [1.0, 1.0, 1.0, 1.0],
            label_column_category_color: [0.8, 0.8, 0.2, 1.0],
            close_on_escape: true,
            copy_with_event_name: false,
            tracked_events: HashSet::new(),
            notification_config: NotificationConfig::default(),
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
    pub category_header_alignment: TextAlignment,
    pub category_header_padding: f32,
    pub label_column_position: LabelColumnPosition,
    pub label_column_width: f32,
    pub label_column_show_category: bool,
    pub label_column_show_track: bool,
    pub label_column_text_size: f32,
    pub label_column_bg_color: [f32; 4],
    pub label_column_text_color: [f32; 4],
    pub label_column_category_color: [f32; 4],
    pub close_on_escape: bool,
    pub copy_with_event_name: bool,

    // === Notification Settings ===
    pub tracked_events: HashSet<TrackedEventId>,
    pub notification_config: NotificationConfig,
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
            category_header_alignment: TextAlignment::Center,
            category_header_padding: 0.0,
            label_column_position: LabelColumnPosition::None,
            label_column_width: 150.0,
            label_column_show_category: false,
            label_column_show_track: true,
            label_column_text_size: 1.0,
            label_column_bg_color: [0.0, 0.0, 0.0, 0.0],
            label_column_text_color: [1.0, 1.0, 1.0, 1.0],
            label_column_category_color: [0.8, 0.8, 0.2, 1.0],
            close_on_escape: true,
            copy_with_event_name: false,
            tracked_events: HashSet::new(),
            notification_config: NotificationConfig::default(),
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
    // Load fresh tracks from JSON (outside locks)
    let (default_tracks, categories) = load_tracks_from_json();
    
    // Scope 1: Clean up user config
    let (cleaned_custom_tracks, user_settings) = {
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
        let default_track_names: HashSet<String> = default_tracks.iter()
            .map(|t| t.name.clone())
            .collect();
        
        user_cfg.custom_tracks.retain(|track| {
            !default_track_names.contains(&track.name)
        });
        
        // Clone data we need (releases lock early)
        (
            user_cfg.custom_tracks.clone(),
            (
                user_cfg.track_overrides.clone(),
                user_cfg.category_visibility.clone(),
                user_cfg.show_main_window,
                user_cfg.is_window_locked,
                user_cfg.hide_background,
                user_cfg.show_time_ruler,
                user_cfg.show_scrollbar,
                user_cfg.timeline_width,
                user_cfg.view_range_seconds,
                user_cfg.current_time_position,
                user_cfg.show_category_headers,
                user_cfg.spacing_same_category,
                user_cfg.spacing_between_categories,
                user_cfg.category_order.clone(),
                user_cfg.global_track_background,
                user_cfg.global_track_padding,
                user_cfg.override_all_track_heights,
                user_cfg.global_track_height,
                user_cfg.draw_event_borders,
                user_cfg.event_border_color,
                user_cfg.event_border_thickness,
                user_cfg.category_header_alignment,
                user_cfg.category_header_padding,
                user_cfg.label_column_position,
                user_cfg.label_column_width,
                user_cfg.label_column_show_category,
                user_cfg.label_column_show_track,
                user_cfg.label_column_text_size,
                user_cfg.label_column_bg_color,
                user_cfg.label_column_text_color,
                user_cfg.label_column_category_color,
                user_cfg.close_on_escape,
                user_cfg.copy_with_event_name,
                user_cfg.tracked_events.clone(),
                user_cfg.notification_config.clone(),
            )
        )
    }; // user_cfg lock dropped here
    
    // Scope 2: Update runtime config
    {
        let mut runtime = RUNTIME_CONFIG.lock();
        
        // Set runtime tracks to defaults
        runtime.tracks = default_tracks;
        runtime.categories = categories;
        
        // Apply user overrides to default tracks
        for track in &mut runtime.tracks {
            if let Some(override_data) = user_settings.0.get(&track.name) {
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
        runtime.category_visibility = user_settings.1;
        runtime.show_main_window = user_settings.2;
        runtime.is_window_locked = user_settings.3;
        runtime.hide_background = user_settings.4;
        runtime.show_time_ruler = user_settings.5;
        runtime.show_scrollbar = user_settings.6;
        runtime.timeline_width = user_settings.7;
        runtime.view_range_seconds = user_settings.8;
        runtime.current_time_position = user_settings.9;
        runtime.show_category_headers = user_settings.10;
        runtime.spacing_same_category = user_settings.11;
        runtime.spacing_between_categories = user_settings.12;
        runtime.category_order = user_settings.13;
        runtime.global_track_background = user_settings.14;
        runtime.global_track_padding = user_settings.15;
        runtime.override_all_track_heights = user_settings.16;
        runtime.global_track_height = user_settings.17;
        runtime.draw_event_borders = user_settings.18;
        runtime.event_border_color = user_settings.19;
        runtime.event_border_thickness = user_settings.20;
        runtime.category_header_alignment = user_settings.21;
        runtime.category_header_padding = user_settings.22;
        runtime.label_column_position = user_settings.23;
        runtime.label_column_width = user_settings.24;
        runtime.label_column_show_category = user_settings.25;
        runtime.label_column_show_track = user_settings.26;
        runtime.label_column_text_size = user_settings.27;
        runtime.label_column_bg_color = user_settings.28;
        runtime.label_column_text_color = user_settings.29;
        runtime.label_column_category_color = user_settings.30;
        runtime.close_on_escape = user_settings.31;
        runtime.copy_with_event_name = user_settings.32;
        runtime.tracked_events = user_settings.33;
        runtime.notification_config = user_settings.34;
    } // runtime lock dropped here
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
            
            if track.visible != default_track.visible {
                override_data.visible = Some(track.visible);
                has_changes = true;
            }
            
            if (track.height - default_track.height).abs() > 0.1 {
                override_data.height = Some(track.height);
                has_changes = true;
            }
            
            for event in &track.events {
                if !event.enabled {
                    override_data.disabled_events.push(event.name.clone());
                    has_changes = true;
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
    user_cfg.category_header_alignment = runtime.category_header_alignment;
    user_cfg.category_header_padding = runtime.category_header_padding;
    user_cfg.label_column_position = runtime.label_column_position;
    user_cfg.label_column_width = runtime.label_column_width;
    user_cfg.label_column_show_category = runtime.label_column_show_category;
    user_cfg.label_column_show_track = runtime.label_column_show_track;
    user_cfg.label_column_text_size = runtime.label_column_text_size;
    user_cfg.label_column_bg_color = runtime.label_column_bg_color;
    user_cfg.label_column_text_color = runtime.label_column_text_color;
    user_cfg.label_column_category_color = runtime.label_column_category_color;
    user_cfg.close_on_escape = runtime.close_on_escape;
    user_cfg.copy_with_event_name = runtime.copy_with_event_name;
    user_cfg.category_visibility = runtime.category_visibility.clone();
    user_cfg.tracked_events = runtime.tracked_events.clone();
    user_cfg.notification_config = runtime.notification_config.clone();
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
