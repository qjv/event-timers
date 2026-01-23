use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::{HashSet, VecDeque};

use crate::config::TrackedEventId;

/// Represents a toast notification in the queue
#[derive(Debug, Clone)]
pub struct ToastNotification {
    /// Unique ID for this toast instance
    pub id: u64,
    /// The event that triggered this notification
    pub event_id: TrackedEventId,
    /// Timestamp when the event starts (unix seconds)
    pub event_start_time: i64,
    /// Time remaining until event (recalculated each frame)
    pub minutes_until: i32,
    /// When this toast was created (for fade timing)
    pub created_at: std::time::Instant,
    /// Current opacity (1.0 = fully visible, 0.0 = hidden)
    pub opacity: f32,
    /// Whether user dismissed this toast
    pub dismissed: bool,
    /// Copy text (waypoint) if available
    pub copy_text: String,
    /// Reminder message (e.g., "Starting soon!", "Happening now!")
    pub reminder_name: String,
    /// Color for the reminder text
    pub reminder_color: [f32; 4],
}

/// Key for tracking which reminders have been shown for an event occurrence
/// Uses start_time for deduplication (handles events spanning cycle boundaries)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NotifiedKey {
    pub event_id: TrackedEventId,
    /// Absolute start time of this event occurrence
    pub start_time: i64,
    pub minutes_before: u32,
}

/// Represents an upcoming event for the panel
#[derive(Debug, Clone)]
pub struct UpcomingEvent {
    pub event_id: TrackedEventId,
    /// Absolute time when event starts
    pub start_time: i64,
    /// Seconds until event starts (0 if active)
    pub seconds_until: i64,
    /// Seconds since event started (0 if not yet started)
    pub seconds_into: i64,
    /// Event color for visual matching
    pub color: [f32; 4],
    /// Copy text if available
    pub copy_text: String,
}

/// Key for tracking last ongoing notification time per event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OngoingNotificationKey {
    pub event_id: TrackedEventId,
    /// Absolute start time of this event occurrence
    pub start_time: i64,
}

/// Runtime state for the notification system
#[derive(Debug)]
pub struct NotificationState {
    /// Queue of active toast notifications
    pub toast_queue: VecDeque<ToastNotification>,

    /// Counter for generating unique toast IDs
    next_toast_id: u64,

    /// Tracks which reminders have been shown for each event occurrence
    /// This prevents duplicate notifications for the same reminder
    pub notified_reminders: HashSet<NotifiedKey>,

    /// Tracks last ongoing notification time for each event (unix timestamp)
    /// This prevents spam from ongoing notifications
    pub ongoing_last_notified: std::collections::HashMap<OngoingNotificationKey, i64>,

    /// Per-event cooldown - last time a toast was shown for each specific event
    /// This prevents spam for the same event regardless of reminder type
    pub event_last_notified: std::collections::HashMap<TrackedEventId, i64>,

    /// Global cooldown - last time ANY toast was added (prevents rapid spam)
    last_toast_time: i64,

    /// Cached list of upcoming events (refreshed each frame)
    pub upcoming_events: Vec<UpcomingEvent>,

    /// Last time we refreshed upcoming events (unix timestamp)
    last_refresh_time: i64,

    /// Preview toast (shown in settings)
    pub preview_toast: Option<ToastNotification>,
}

impl NotificationState {
    pub fn new() -> Self {
        Self {
            toast_queue: VecDeque::new(),
            next_toast_id: 0,
            notified_reminders: HashSet::new(),
            ongoing_last_notified: std::collections::HashMap::new(),
            event_last_notified: std::collections::HashMap::new(),
            last_toast_time: 0,
            upcoming_events: Vec::new(),
            last_refresh_time: 0,
            preview_toast: None,
        }
    }

    /// Check if we can add a new toast (global cooldown of 2 seconds between toasts)
    pub fn can_add_toast(&self, current_time: i64) -> bool {
        current_time - self.last_toast_time >= 2
    }

    /// Check if we can notify for this specific event (per-event cooldown of 30 seconds)
    /// This prevents spam for the same event from different reminder types
    pub fn can_notify_event(&self, event_id: &TrackedEventId, current_time: i64) -> bool {
        match self.event_last_notified.get(event_id) {
            Some(&last_time) => current_time - last_time >= 30,
            None => true,
        }
    }

    /// Mark that we just notified for this event
    pub fn mark_event_notified(&mut self, event_id: &TrackedEventId, current_time: i64) {
        self.event_last_notified.insert(event_id.clone(), current_time);
    }

    /// Show a preview toast notification
    pub fn show_preview(&mut self, reminder_name: &str, reminder_color: [f32; 4]) {
        let preview = ToastNotification {
            id: self.next_toast_id,
            event_id: TrackedEventId::new("Example Track", "Example Event"),
            event_start_time: 0,
            minutes_until: 5,
            created_at: std::time::Instant::now(),
            opacity: 1.0,
            dismissed: false,
            copy_text: "[&Example]".to_string(),
            reminder_name: reminder_name.to_string(),
            reminder_color,
        };
        self.next_toast_id += 1;
        self.preview_toast = Some(preview);
    }

    /// Update preview toast (fade out)
    pub fn update_preview(&mut self, toast_duration: f32) {
        if let Some(preview) = &mut self.preview_toast {
            let elapsed = preview.created_at.elapsed().as_secs_f32();
            let fade_start = toast_duration - 1.0;

            if elapsed > fade_start {
                preview.opacity = (toast_duration - elapsed).max(0.0);
            }

            if elapsed > toast_duration || preview.dismissed {
                self.preview_toast = None;
            }
        }
    }

    /// Add a new toast notification
    pub fn add_toast(
        &mut self,
        event_id: TrackedEventId,
        event_start_time: i64,
        minutes_until: i32,
        copy_text: String,
        reminder_name: String,
        reminder_color: [f32; 4],
        current_time: i64,
    ) {
        let toast = ToastNotification {
            id: self.next_toast_id,
            event_id,
            event_start_time,
            minutes_until,
            created_at: std::time::Instant::now(),
            opacity: 1.0,
            dismissed: false,
            copy_text,
            reminder_name,
            reminder_color,
        };
        self.next_toast_id += 1;
        self.last_toast_time = current_time;
        self.toast_queue.push_back(toast);
    }

    /// Mark a reminder as shown for an event occurrence
    pub fn mark_notified(&mut self, event_id: &TrackedEventId, start_time: i64, minutes_before: u32) {
        self.notified_reminders.insert(NotifiedKey {
            event_id: event_id.clone(),
            start_time,
            minutes_before,
        });
    }

    /// Check if a reminder was already shown for an event occurrence
    pub fn was_notified(&self, event_id: &TrackedEventId, start_time: i64, minutes_before: u32) -> bool {
        self.notified_reminders.contains(&NotifiedKey {
            event_id: event_id.clone(),
            start_time,
            minutes_before,
        })
    }

    /// Clean up old notified entries (keep entries from last 24 hours)
    pub fn cleanup_old_notifications(&mut self, current_time: i64) {
        let cutoff = current_time - 86400; // 24 hours ago
        self.notified_reminders.retain(|key| {
            key.start_time > cutoff
        });
        self.ongoing_last_notified.retain(|key, _| {
            key.start_time > cutoff
        });
        // Clean up per-event cooldown entries older than 5 minutes
        self.event_last_notified.retain(|_, &mut last_time| {
            current_time - last_time < 300
        });
    }

    /// Check if enough time has passed since last ongoing notification for this event
    /// Returns true if we should show a notification (either first time or interval has passed)
    pub fn should_show_ongoing(&self, event_id: &TrackedEventId, start_time: i64, current_time: i64, interval_seconds: i64) -> bool {
        let key = OngoingNotificationKey {
            event_id: event_id.clone(),
            start_time,
        };

        match self.ongoing_last_notified.get(&key) {
            Some(&last_time) => {
                // Check if enough time has passed since last notification
                current_time - last_time >= interval_seconds
            }
            None => {
                // First notification for this event occurrence
                true
            }
        }
    }

    /// Mark that we just showed an ongoing notification for this event
    pub fn mark_ongoing_notified(&mut self, event_id: &TrackedEventId, start_time: i64, current_time: i64) {
        let key = OngoingNotificationKey {
            event_id: event_id.clone(),
            start_time,
        };
        self.ongoing_last_notified.insert(key, current_time);
    }

    /// Update toast states (opacity, removal)
    pub fn update_toasts(&mut self, toast_duration: f32, max_visible: usize) {
        let fade_start = toast_duration - 1.0; // Start fading 1 second before end

        for toast in &mut self.toast_queue {
            let elapsed = toast.created_at.elapsed().as_secs_f32();

            if elapsed > fade_start {
                // Fade out over the last second
                toast.opacity = (toast_duration - elapsed).max(0.0);
            }

            if elapsed > toast_duration || toast.dismissed {
                toast.opacity = 0.0;
            }
        }

        // Remove fully faded toasts
        self.toast_queue.retain(|t| t.opacity > 0.0);

        // Limit visible toasts (oldest first, so we remove from front)
        while self.toast_queue.len() > max_visible {
            self.toast_queue.pop_front();
        }
    }

    /// Check if refresh is needed (called every frame, but only refreshes every second)
    pub fn needs_refresh(&self, current_time: i64) -> bool {
        current_time != self.last_refresh_time
    }

    pub fn set_refresh_time(&mut self, current_time: i64) {
        self.last_refresh_time = current_time;
    }
}

impl Default for NotificationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global notification state
pub static NOTIFICATION_STATE: Lazy<Mutex<NotificationState>> =
    Lazy::new(|| Mutex::new(NotificationState::new()));
