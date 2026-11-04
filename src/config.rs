use crate::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::Hash,
    path::PathBuf,
};

use crate::json_loader::{load_tracks_from_json, EventColor, EventTrack};

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
}

/// Toast notification position anchor
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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

/// Which events populate the Upcoming Events panel
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpcomingPanelMode {
    TrackedOnly,
    TrackedAndFavorites,
    AllVisibleEvents,
}

impl Default for UpcomingPanelMode {
    fn default() -> Self {
        Self::TrackedOnly
    }
}

impl UpcomingPanelMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TrackedOnly => "Tracked Only",
            Self::TrackedAndFavorites => "Tracked + Favorites",
            Self::AllVisibleEvents => "All Visible Events",
        }
    }
}

/// How event bars are colored in the timeline
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventColorMode {
    Default,
    CategoryExpansion,
    MonochromeHighlight,
}

impl Default for EventColorMode {
    fn default() -> Self {
        Self::Default
    }
}

impl EventColorMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default (Per-Event)",
            Self::CategoryExpansion => "By Expansion / Category",
            Self::MonochromeHighlight => "Monochrome (Highlight Tracked & Favorites)",
        }
    }
}

pub fn category_expansion_color(category: &str) -> [f32; 4] {
    match category {
        "Seasonal Events" => [0.26, 0.78, 0.85, 1.0], // Festive Cyan
        "Time Cycles" => [0.55, 0.68, 0.88, 1.0],     // Sky Blue
        "Instances" => [0.72, 0.45, 0.88, 1.0],       // Mists Purple
        "Base Game" => [0.85, 0.32, 0.28, 1.0],       // Core Krytan Red
        "Living World Season 1" => [0.92, 0.48, 0.18, 1.0], // Scarlet Orange
        "Living World Season 2" => [0.85, 0.68, 0.28, 1.0], // Dry Top Sand Gold
        "Heart of Thorns" => [0.24, 0.72, 0.34, 1.0], // Maguuma Jungle Green
        "Living World Season 3" => [0.76, 0.26, 0.48, 1.0], // Bloodstone Crimson
        "Path of Fire" => [0.88, 0.24, 0.56, 1.0],    // Elonian Magenta
        "Living World Season 4" => [0.64, 0.34, 0.86, 1.0], // Branded Violet
        "Icebrood Saga" => [0.28, 0.68, 0.92, 1.0],   // Shiverpeaks Ice Blue
        "End of Dragons" => [0.14, 0.78, 0.62, 1.0],  // Canthan Jade Teal
        "Secrets of the Obscure" => [0.92, 0.76, 0.26, 1.0], // Astral Gold
        "Janthir Wilds" => [0.78, 0.54, 0.26, 1.0],   // Janthir Amber/Ochre
        "Visions of Eternity" => [0.94, 0.42, 0.26, 1.0], // Castora Coral/Orange
        _ => [0.45, 0.62, 0.78, 1.0],
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

    #[serde(default)]
    pub upcoming_panel_mode: UpcomingPanelMode,

    #[serde(default = "default_true")]
    pub mute_completed_events: bool,

    #[serde(default)]
    pub hide_completed_in_upcoming: bool,

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
            upcoming_panel_mode: UpcomingPanelMode::default(),
            mute_completed_events: true,
            hide_completed_in_upcoming: false,
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

const USER_CONFIG_FILENAME: &str = "user_config.json";

// === Alignment Options ===

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

// === Track & Event Overrides ===

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct EventOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_delta_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
}

impl EventOverride {
    pub fn is_empty(&self) -> bool {
        self.custom_name.is_none()
            && self.copy_text.is_none()
            && self.color.is_none()
            && self.offset_delta_seconds.is_none()
            && self.duration.is_none()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TrackOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disabled_events: Vec<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub event_overrides: HashMap<String, EventOverride>,
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
    pub sticky_time_ruler: bool,
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
    #[serde(default)]
    pub track_order: HashMap<String, Vec<String>>,
    #[serde(default = "default_global_track_bg")]
    pub global_track_background: [f32; 4],
    #[serde(default)]
    pub global_track_padding: f32,
    #[serde(default)]
    pub override_all_track_heights: bool,
    #[serde(default = "default_height")]
    pub global_track_height: f32,
    #[serde(default)]
    pub event_color_mode: EventColorMode,
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
fn default_height() -> f32 {
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
            sticky_time_ruler: true,
            show_scrollbar: true,
            timeline_width: 800.0,
            view_range_seconds: 3600.0,
            current_time_position: 0.5,
            show_category_headers: false,
            spacing_same_category: 0.0,
            spacing_between_categories: 0.0,
            category_order: Vec::new(),
            track_order: HashMap::new(),
            global_track_background: [0.2, 0.2, 0.2, 0.2],
            global_track_padding: 0.0,
            override_all_track_heights: false,
            global_track_height: default_height(),
            event_color_mode: EventColorMode::Default,
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
    pub sticky_time_ruler: bool,
    pub show_scrollbar: bool,
    pub timeline_width: f32,
    pub view_range_seconds: f32,
    pub current_time_position: f32,
    pub show_category_headers: bool,
    pub spacing_same_category: f32,
    pub spacing_between_categories: f32,
    pub category_order: Vec<String>,
    pub track_order: HashMap<String, Vec<String>>,
    pub global_track_background: [f32; 4],
    pub global_track_padding: f32,
    pub override_all_track_heights: bool,
    pub global_track_height: f32,
    pub event_color_mode: EventColorMode,
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

impl RuntimeConfig {
    pub fn event_display_name(&self, track_name: &str, event_name: &str) -> String {
        for track in &self.tracks {
            if track.name == track_name {
                for event in &track.events {
                    if event.name == event_name {
                        return event.display_name().to_string();
                    }
                }
            }
        }
        event_name.to_string()
    }
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
            show_time_ruler: true,
            sticky_time_ruler: true,
            show_scrollbar: true,
            timeline_width: 800.0,
            view_range_seconds: 3600.0,
            current_time_position: 0.5,
            show_category_headers: false,
            spacing_same_category: 0.0,
            spacing_between_categories: 0.0,
            category_order: Vec::new(),
            track_order: HashMap::new(),
            global_track_background: [0.2, 0.2, 0.2, 0.2],
            global_track_padding: 0.0,
            override_all_track_heights: false,
            global_track_height: default_height(),
            event_color_mode: EventColorMode::Default,
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

// === Global State ===

pub static RUNTIME_CONFIG: Lazy<Mutex<RuntimeConfig>> =
    Lazy::new(|| Mutex::new(RuntimeConfig::default()));
pub static USER_CONFIG: Lazy<Mutex<UserConfig>> = Lazy::new(|| Mutex::new(UserConfig::default()));
pub static SELECTED_TRACK: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));
pub static SELECTED_EVENT: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));
static CONFIG_IO_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static LAST_SAVED_CONFIG: Lazy<Mutex<Option<Vec<u8>>>> = Lazy::new(|| Mutex::new(None));

// === Configuration Management ===

pub fn apply_user_overrides() {
    let (default_tracks, categories) = load_tracks_from_json();

    let (cleaned_custom_tracks, user_snapshot) = {
        let mut user_cfg = USER_CONFIG.lock();

        let mut seen_custom_track_names: HashSet<String> = HashSet::new();
        user_cfg.custom_tracks.retain(|track| {
            if seen_custom_track_names.contains(&track.name) {
                false
            } else {
                seen_custom_track_names.insert(track.name.clone());
                true
            }
        });

        let default_track_names: HashSet<String> =
            default_tracks.iter().map(|t| t.name.clone()).collect();

        user_cfg
            .custom_tracks
            .retain(|track| !default_track_names.contains(&track.name));

        (user_cfg.custom_tracks.clone(), user_cfg.clone())
    };

    {
        let mut runtime = RUNTIME_CONFIG.lock();

        runtime.tracks = default_tracks;
        runtime.categories = categories;

        // Apply user overrides to default tracks
        for track in &mut runtime.tracks {
            if let Some(override_data) = user_snapshot.track_overrides.get(&track.name) {
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
                    if let Some(ev_override) = override_data.event_overrides.get(&event.name) {
                        if let Some(ref custom_name) = ev_override.custom_name {
                            if !custom_name.trim().is_empty() {
                                event.custom_name = Some(custom_name.clone());
                            }
                        }
                        if let Some(ref copy_text) = ev_override.copy_text {
                            event.copy_text = copy_text.clone();
                        }
                        if let Some(color) = ev_override.color {
                            event.color = EventColor::from_array(color);
                        }
                        if let Some(delta) = ev_override.offset_delta_seconds {
                            event.start_offset = (event.start_offset + delta)
                                .rem_euclid(event.cycle_duration.max(60));
                        }
                        if let Some(duration) = ev_override.duration {
                            event.duration = duration.max(60);
                        }
                    }
                }
            }
        }

        // Add cleaned custom tracks
        runtime.tracks.extend(cleaned_custom_tracks);

        // Apply saved per-category track ordering
        if !user_snapshot.track_order.is_empty() {
            for (category, ordered_names) in &user_snapshot.track_order {
                let mut cat_tracks: Vec<EventTrack> = Vec::new();
                let mut indices: Vec<usize> = Vec::new();
                for (i, t) in runtime.tracks.iter().enumerate() {
                    if &t.category == category {
                        indices.push(i);
                        cat_tracks.push(t.clone());
                    }
                }
                if indices.len() > 1 {
                    cat_tracks.sort_by_key(|t| {
                        ordered_names
                            .iter()
                            .position(|name| name == &t.name)
                            .unwrap_or(usize::MAX)
                    });
                    for (slot, track) in indices.into_iter().zip(cat_tracks) {
                        runtime.tracks[slot] = track;
                    }
                }
            }
        }

        runtime.category_visibility = user_snapshot.category_visibility;
        runtime.show_main_window = user_snapshot.show_main_window;
        runtime.is_window_locked = user_snapshot.is_window_locked;
        runtime.hide_background = user_snapshot.hide_background;
        runtime.show_time_ruler = user_snapshot.show_time_ruler;
        runtime.sticky_time_ruler = user_snapshot.sticky_time_ruler;
        runtime.show_scrollbar = user_snapshot.show_scrollbar;
        runtime.timeline_width = user_snapshot.timeline_width;
        runtime.view_range_seconds = user_snapshot.view_range_seconds;
        runtime.current_time_position = user_snapshot.current_time_position;
        runtime.show_category_headers = user_snapshot.show_category_headers;
        runtime.spacing_same_category = user_snapshot.spacing_same_category;
        runtime.spacing_between_categories = user_snapshot.spacing_between_categories;
        runtime.category_order = user_snapshot.category_order;
        runtime.track_order = user_snapshot.track_order;
        runtime.global_track_background = user_snapshot.global_track_background;
        runtime.global_track_padding = user_snapshot.global_track_padding;
        runtime.override_all_track_heights = user_snapshot.override_all_track_heights;
        runtime.global_track_height = user_snapshot.global_track_height;
        runtime.event_color_mode = user_snapshot.event_color_mode;
        runtime.draw_event_borders = user_snapshot.draw_event_borders;
        runtime.event_border_color = user_snapshot.event_border_color;
        runtime.event_border_thickness = user_snapshot.event_border_thickness;
        runtime.category_header_alignment = user_snapshot.category_header_alignment;
        runtime.category_header_padding = user_snapshot.category_header_padding;
        runtime.label_column_position = user_snapshot.label_column_position;
        runtime.label_column_width = user_snapshot.label_column_width;
        runtime.label_column_show_category = user_snapshot.label_column_show_category;
        runtime.label_column_show_track = user_snapshot.label_column_show_track;
        runtime.label_column_text_size = user_snapshot.label_column_text_size;
        runtime.label_column_bg_color = user_snapshot.label_column_bg_color;
        runtime.label_column_text_color = user_snapshot.label_column_text_color;
        runtime.label_column_category_color = user_snapshot.label_column_category_color;
        runtime.close_on_escape = user_snapshot.close_on_escape;
        runtime.copy_with_event_name = user_snapshot.copy_with_event_name;
        runtime.show_quick_access_icon = user_snapshot.show_quick_access_icon;
        runtime.time_ruler_interval = user_snapshot.time_ruler_interval;
        runtime.time_ruler_show_current_time = user_snapshot.time_ruler_show_current_time;
        runtime.tracked_events = user_snapshot.tracked_events;
        runtime.favorite_events = user_snapshot.favorite_events;
        runtime.oneshot_events = user_snapshot.oneshot_events;
        runtime.notification_config = user_snapshot.notification_config;
        runtime.setup_onboarding_seen = user_snapshot.setup_onboarding_seen;
    }
}

pub fn extract_user_overrides() {
    let runtime = RUNTIME_CONFIG.lock();
    let mut user_cfg = USER_CONFIG.lock();

    // Preserve any existing visual overrides per track
    let existing_visuals: HashMap<String, TrackVisualConfig> = user_cfg
        .track_overrides
        .iter()
        .filter_map(|(k, v)| v.visual.clone().map(|vis| (k.clone(), vis)))
        .collect();

    user_cfg.track_overrides.clear();
    user_cfg.custom_tracks.clear();

    let (default_tracks, _) = load_tracks_from_json();
    let default_map: HashMap<String, &EventTrack> =
        default_tracks.iter().map(|t| (t.name.clone(), t)).collect();

    // Record current intra-category track ordering
    let mut current_track_order: HashMap<String, Vec<String>> = HashMap::new();
    for track in &runtime.tracks {
        if !track.category.is_empty() {
            current_track_order
                .entry(track.category.clone())
                .or_default()
                .push(track.name.clone());
        }
    }
    user_cfg.track_order = current_track_order;

    for track in &runtime.tracks {
        if let Some(default_track) = default_map.get(&track.name) {
            let mut override_data = TrackOverride {
                visual: existing_visuals.get(&track.name).cloned(),
                ..TrackOverride::default()
            };
            let mut has_changes = override_data.visual.is_some();

            if track.visible != default_track.visible {
                override_data.visible = Some(track.visible);
                has_changes = true;
            }

            if (track.height - default_track.height).abs() > 0.1 {
                override_data.height = Some(track.height);
                has_changes = true;
            }

            let mut seen_disabled: HashSet<String> = HashSet::new();
            let mut seen_event_override: HashSet<String> = HashSet::new();

            for event in &track.events {
                if !event.enabled && seen_disabled.insert(event.name.clone()) {
                    override_data.disabled_events.push(event.name.clone());
                    has_changes = true;
                }

                if seen_event_override.insert(event.name.clone()) {
                    if let Some(def_ev) =
                        default_track.events.iter().find(|de| de.name == event.name)
                    {
                        let mut ev_override = EventOverride::default();

                        if let Some(ref custom_name) = event.custom_name {
                            let trimmed = custom_name.trim();
                            if !trimmed.is_empty() && trimmed != def_ev.name {
                                ev_override.custom_name = Some(trimmed.to_string());
                            }
                        }

                        if event.copy_text != def_ev.copy_text {
                            ev_override.copy_text = Some(event.copy_text.clone());
                        }

                        let c = event.color.to_array();
                        let dc = def_ev.color.to_array();
                        if (c[0] - dc[0]).abs() > 0.005
                            || (c[1] - dc[1]).abs() > 0.005
                            || (c[2] - dc[2]).abs() > 0.005
                            || (c[3] - dc[3]).abs() > 0.005
                        {
                            ev_override.color = Some(c);
                        }

                        if event.start_offset != def_ev.start_offset {
                            ev_override.offset_delta_seconds =
                                Some(event.start_offset - def_ev.start_offset);
                        }

                        if event.duration != def_ev.duration {
                            ev_override.duration = Some(event.duration);
                        }

                        if !ev_override.is_empty() {
                            override_data
                                .event_overrides
                                .insert(event.name.clone(), ev_override);
                            has_changes = true;
                        }
                    }
                }
            }

            if has_changes {
                user_cfg
                    .track_overrides
                    .insert(track.name.clone(), override_data);
            }
        } else {
            user_cfg.custom_tracks.push(track.clone());
        }
    }

    user_cfg.show_main_window = runtime.show_main_window;
    user_cfg.is_window_locked = runtime.is_window_locked;
    user_cfg.hide_background = runtime.hide_background;
    user_cfg.show_time_ruler = runtime.show_time_ruler;
    user_cfg.sticky_time_ruler = runtime.sticky_time_ruler;
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
    user_cfg.event_color_mode = runtime.event_color_mode;
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
    user_cfg.show_quick_access_icon = runtime.show_quick_access_icon;
    user_cfg.setup_onboarding_seen = runtime.setup_onboarding_seen;
    user_cfg.time_ruler_interval = runtime.time_ruler_interval;
    user_cfg.time_ruler_show_current_time = runtime.time_ruler_show_current_time;
    user_cfg.category_visibility = runtime.category_visibility.clone();
    user_cfg.tracked_events = runtime.tracked_events.clone();
    user_cfg.favorite_events = runtime.favorite_events.clone();
    user_cfg.oneshot_events = runtime.oneshot_events.clone();
    user_cfg.notification_config = runtime.notification_config.clone();
}

// === File I/O ===

pub fn get_user_config_path() -> Option<PathBuf> {
    get_addon_dir("event_timers").map(|p| p.join(USER_CONFIG_FILENAME))
}

pub fn load_user_config() {
    if let Some(path) = get_user_config_path() {
        let _io = CONFIG_IO_LOCK.lock();
        let candidates = [
            path.clone(),
            path.with_extension("json.tmp"),
            path.with_extension("json.backup"),
        ];
        for candidate in candidates {
            if let Ok(json) = fs::read(&candidate) {
                if let Ok(loaded) = serde_json::from_slice::<UserConfig>(&json) {
                    *LAST_SAVED_CONFIG.lock() = if candidate == path {
                        serde_json::to_vec_pretty(&loaded).ok()
                    } else {
                        None
                    };
                    *USER_CONFIG.lock() = loaded;
                    drop(_io);
                    apply_user_overrides();
                    return;
                }
            }
        }
    }

    *LAST_SAVED_CONFIG.lock() = None;
    apply_user_overrides();
}

pub fn save_user_config() {
    extract_user_overrides();

    let json = {
        let user_cfg = USER_CONFIG.lock();
        match serde_json::to_vec_pretty(&*user_cfg) {
            Ok(json) => json,
            Err(_) => return,
        }
    };
    if LAST_SAVED_CONFIG.lock().as_deref() == Some(json.as_slice()) {
        return;
    }

    if let Some(path) = get_user_config_path() {
        let _io = CONFIG_IO_LOCK.lock();
        if let Some(dir) = path.parent() {
            if fs::create_dir_all(dir).is_err() {
                return;
            }
        }
        let temporary = path.with_extension("json.tmp");
        let backup = path.with_extension("json.backup");
        if fs::write(&temporary, &json).is_err() {
            return;
        }
        if path.exists() {
            let _ = fs::copy(&path, &backup);
            if fs::remove_file(&path).is_err() {
                return;
            }
        }
        if fs::rename(&temporary, &path).is_ok() {
            *LAST_SAVED_CONFIG.lock() = Some(json);
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
