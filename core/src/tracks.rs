use serde::{Deserialize, Serialize};

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
