use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::tracks::EventTrack;

// Serde note: every config struct carries container-level
// `#[serde(default)]`, so a missing key falls back to the value from the
// struct's `Default` impl. That impl is the single source of truth for
// defaults. The only exceptions are two fields whose long-shipped missing-key
// behavior intentionally differs from `Default` (see the field comments on
// `show_main_window` and `draw_event_borders`); the golden tests in
// tests/serde_format.rs pin both sides.

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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum ToastPosition {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

/// A single reminder configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ReminderConfig {
    /// Display name for this reminder (e.g., "Heads up!", "Starting soon!")
    pub name: String,
    /// Minutes before the event to trigger (0 = during event, repeats based on interval)
    pub minutes_before: u32,
    /// Color for the reminder text
    pub text_color: [f32; 4],
    /// For ongoing reminders (minutes_before=0): interval in minutes between notifications
    pub ongoing_interval_minutes: u32,
}

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
#[serde(default)]
pub struct NotificationConfig {
    pub toast_enabled: bool,
    pub upcoming_panel_enabled: bool,
    pub reminders: Vec<ReminderConfig>,
    pub toast_duration_seconds: f32,
    pub max_visible_toasts: usize,
    pub upcoming_panel_size: [f32; 2],
    pub max_upcoming_events: usize,
    pub toast_position: ToastPosition,
    pub toast_size: [f32; 2],
    /// X offset from corner (percentage of screen width, 0.0 to 1.0)
    pub toast_offset_x: f32,
    /// Y offset from corner (percentage of screen height, 0.0 to 1.0)
    pub toast_offset_y: f32,
    pub toast_bg_color: [f32; 4],
    pub toast_text_scale: f32,
    pub toast_title_color: [f32; 4],
    pub toast_time_color: [f32; 4],
    pub toast_track_color: [f32; 4],
    /// Max minutes after start that a minutes_before=0 reminder can still show.
    pub happening_now_grace_minutes: u32,
}

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
            toast_size: [280.0, 80.0],
            toast_offset_x: 0.0,
            toast_offset_y: 0.0,
            toast_bg_color: [0.1, 0.1, 0.1, 0.95],
            toast_text_scale: 1.2,
            toast_title_color: [1.0, 0.8, 0.2, 1.0],
            toast_time_color: [0.5, 1.0, 0.5, 1.0],
            toast_track_color: [0.7, 0.7, 0.7, 1.0],
            happening_now_grace_minutes: 10,
        }
    }
}

// === Alignment Options ===

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum LabelColumnPosition {
    #[default]
    None,
    Left,
    Right,
}

// === Visual Configuration ===

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct TrackVisualConfig {
    pub background_color: [f32; 4],
    pub padding: f32,
}

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
#[serde(default)]
pub struct UserConfig {
    pub track_overrides: HashMap<String, TrackOverride>,
    pub custom_tracks: Vec<EventTrack>,
    pub category_visibility: HashMap<String, bool>,
    /// Missing key means `true` (upgrades from versions before this field
    /// existed keep the window visible), but a fresh install defaults to
    /// `false`. Long-shipped divergence — keep the field-level default.
    #[serde(default = "default_true")]
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
    /// Missing key means `false`, but a fresh install defaults to `true`.
    /// Long-shipped divergence — keep the field-level default.
    #[serde(default = "bool::default")]
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
    pub show_quick_access_icon: bool,
    pub setup_onboarding_seen: bool,

    // === Time Ruler Settings ===
    pub time_ruler_interval: TimeRulerInterval,
    pub time_ruler_show_current_time: bool,

    // === Notification Settings ===
    pub tracked_events: HashSet<TrackedEventId>,
    pub favorite_events: HashSet<TrackedEventId>,
    pub oneshot_events: HashSet<TrackedEventId>,
    pub notification_config: NotificationConfig,
}

fn default_true() -> bool {
    true
}

/// Time ruler marker spacing options (in minutes)
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeRulerInterval {
    #[default]
    Minutes5 = 5,
    Minutes10 = 10,
    Minutes15 = 15,
    Minutes20 = 20,
    Minutes30 = 30,
    Minutes60 = 60,
}

impl TimeRulerInterval {
    pub fn as_seconds(&self) -> i64 {
        (*self as i64) * 60
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Minutes5 => "5 min",
            Self::Minutes10 => "10 min",
            Self::Minutes15 => "15 min",
            Self::Minutes20 => "20 min",
            Self::Minutes30 => "30 min",
            Self::Minutes60 => "60 min",
        }
    }

    pub fn all() -> &'static [TimeRulerInterval] {
        &[
            Self::Minutes5,
            Self::Minutes10,
            Self::Minutes15,
            Self::Minutes20,
            Self::Minutes30,
            Self::Minutes60,
        ]
    }
}

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
            global_track_height: 40.0,
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
            show_quick_access_icon: true,
            setup_onboarding_seen: false,
            time_ruler_interval: TimeRulerInterval::default(),
            time_ruler_show_current_time: false,
            tracked_events: HashSet::new(),
            favorite_events: HashSet::new(),
            oneshot_events: HashSet::new(),
            notification_config: NotificationConfig::default(),
        }
    }
}
