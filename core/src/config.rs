use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::tracks::EventTrack;

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

fn default_ongoing_interval() -> u32 {
    5
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

    /// Max minutes after start that a minutes_before=0 reminder can still show.
    #[serde(default = "default_happening_now_grace_minutes")]
    pub happening_now_grace_minutes: u32,
}

fn default_toast_duration() -> f32 {
    5.0
}
fn default_max_toasts() -> usize {
    3
}
fn default_upcoming_panel_size() -> [f32; 2] {
    [300.0, 200.0]
}
fn default_max_upcoming() -> usize {
    10
}
fn default_toast_size() -> [f32; 2] {
    [280.0, 80.0]
}
fn default_toast_bg_color() -> [f32; 4] {
    [0.1, 0.1, 0.1, 0.95]
}
fn default_toast_text_scale() -> f32 {
    1.2
}
fn default_toast_title_color() -> [f32; 4] {
    [1.0, 0.8, 0.2, 1.0]
}
fn default_toast_time_color() -> [f32; 4] {
    [0.5, 1.0, 0.5, 1.0]
}
fn default_toast_track_color() -> [f32; 4] {
    [0.7, 0.7, 0.7, 1.0]
}
fn default_happening_now_grace_minutes() -> u32 {
    10
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
            toast_size: default_toast_size(),
            toast_offset_x: 0.0,
            toast_offset_y: 0.0,
            toast_bg_color: default_toast_bg_color(),
            toast_text_scale: default_toast_text_scale(),
            toast_title_color: default_toast_title_color(),
            toast_time_color: default_toast_time_color(),
            toast_track_color: default_toast_track_color(),
            happening_now_grace_minutes: default_happening_now_grace_minutes(),
        }
    }
}

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

fn default_track_bg_color() -> [f32; 4] {
    [0.2, 0.2, 0.2, 1.0]
}
fn default_track_padding() -> f32 {
    5.0
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
    #[serde(default = "default_true")]
    pub show_quick_access_icon: bool,
    #[serde(default)]
    pub setup_onboarding_seen: bool,

    // === Time Ruler Settings ===
    #[serde(default)]
    pub time_ruler_interval: TimeRulerInterval,
    #[serde(default)]
    pub time_ruler_show_current_time: bool,

    // === Notification Settings ===
    #[serde(default)]
    pub tracked_events: HashSet<TrackedEventId>,

    #[serde(default)]
    pub favorite_events: HashSet<TrackedEventId>,

    #[serde(default)]
    pub oneshot_events: HashSet<TrackedEventId>,

    #[serde(default)]
    pub notification_config: NotificationConfig,
}

fn default_global_track_bg() -> [f32; 4] {
    [0.2, 0.2, 0.2, 0.2]
} // #33333333
fn default_border_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
} // #000000FF
fn default_border_thickness() -> f32 {
    1.0
}
/// Pub only until the defaults consolidation lands: the addon's
/// `RuntimeConfig::default()` still references it.
pub fn default_height() -> f32 {
    40.0
}
fn default_label_column_width() -> f32 {
    150.0
}
fn default_label_text_size() -> f32 {
    1.0
}
fn default_label_text_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
} // White
fn default_label_category_color() -> [f32; 4] {
    [0.8, 0.8, 0.2, 1.0]
} // Yellow like default

fn default_true() -> bool {
    true
}
fn default_timeline_width() -> f32 {
    800.0
}
fn default_view_range() -> f32 {
    3600.0
}
fn default_time_position() -> f32 {
    0.5
}
fn default_spacing_same_category() -> f32 {
    0.0
}
fn default_spacing_between_categories() -> f32 {
    0.0
}

/// Time ruler marker spacing options (in minutes)
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRulerInterval {
    Minutes5 = 5,
    Minutes10 = 10,
    Minutes15 = 15,
    Minutes20 = 20,
    Minutes30 = 30,
    Minutes60 = 60,
}

impl Default for TimeRulerInterval {
    fn default() -> Self {
        Self::Minutes5
    }
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
