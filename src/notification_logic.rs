use crate::config::{TrackedEventId, RUNTIME_CONFIG};
use crate::json_loader::{EventTrack, TimelineEvent};
use crate::notifications::{UpcomingEvent, NOTIFICATION_STATE};
use crate::time_utils::get_current_unix_time;

/// Main update function - call once per frame from render loop
pub fn update_notifications() {
    let current_time = get_current_unix_time();

    let (tracked_events, notification_config, tracks) = {
        let config = RUNTIME_CONFIG.lock();
        (
            config.tracked_events.clone(),
            config.notification_config.clone(),
            config.tracks.clone(),
        )
    };

    // Early exit if no tracked events
    if tracked_events.is_empty() {
        let mut state = NOTIFICATION_STATE.lock();
        state.upcoming_events.clear();
        return;
    }

    let mut state = NOTIFICATION_STATE.lock();

    // Update toast fade/removal
    state.update_toasts(
        notification_config.toast_duration_seconds,
        notification_config.max_visible_toasts,
    );

    // Only refresh calculations once per second
    if !state.needs_refresh(current_time) {
        return;
    }
    state.set_refresh_time(current_time);

    // Clean up old notification records
    state.cleanup_old_notifications(current_time);

    let mut upcoming: Vec<UpcomingEvent> = Vec::new();

    for track in &tracks {
        if !track.visible {
            continue;
        }

        for event in &track.events {
            if !event.enabled {
                continue;
            }

            let event_id = TrackedEventId::new(&track.name, &event.name);

            // Only process tracked events
            if !tracked_events.contains(&event_id) {
                continue;
            }

            // Calculate next/current occurrence of this event
            if let Some((start_time, seconds_until, seconds_into_event, event_duration)) =
                calculate_event_timing(track, event, current_time)
            {
                // Add to upcoming events list
                upcoming.push(UpcomingEvent {
                    event_id: event_id.clone(),
                    start_time,
                    seconds_until,
                    seconds_into: if seconds_into_event >= 0 { seconds_into_event } else { 0 },
                    color: event.color.to_array(),
                    copy_text: event.copy_text.clone(),
                });

                // Check each configured reminder
                if notification_config.toast_enabled {
                    for reminder in &notification_config.reminders {
                        let reminder_seconds = (reminder.minutes_before as i64) * 60;

                        if reminder.minutes_before == 0 {
                            // "During event" reminder - triggers at configurable intervals while event is active
                            // but not on the very last interval
                            if seconds_into_event >= 0 {
                                let interval_seconds = (reminder.ongoing_interval_minutes.max(1) as i64) * 60;
                                let remaining_seconds = event_duration - seconds_into_event;
                                // Don't notify on the last interval
                                if remaining_seconds > interval_seconds {
                                    // Use interval count as the key to allow one notification per interval
                                    let interval_count = (seconds_into_event / interval_seconds) as u32;
                                    if !state.was_notified(&event_id, start_time, interval_count) {
                                        let minutes_remaining = ((remaining_seconds + 59) / 60) as i32;
                                        state.add_toast(
                                            event_id.clone(),
                                            start_time,
                                            minutes_remaining,
                                            event.copy_text.clone(),
                                            reminder.name.clone(),
                                            reminder.text_color,
                                        );
                                        state.mark_notified(&event_id, start_time, interval_count);
                                    }
                                }
                            }
                        } else {
                            // Normal "X minutes before" reminder
                            if seconds_until > 0
                                && seconds_until <= reminder_seconds
                                && !state.was_notified(&event_id, start_time, reminder.minutes_before)
                            {
                                let minutes_until = ((seconds_until + 59) / 60) as i32;
                                state.add_toast(
                                    event_id.clone(),
                                    start_time,
                                    minutes_until,
                                    event.copy_text.clone(),
                                    reminder.name.clone(),
                                    reminder.text_color,
                                );
                                state.mark_notified(&event_id, start_time, reminder.minutes_before);
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by time (soonest first)
    upcoming.sort_by_key(|e| e.seconds_until);

    // Limit to max configured
    upcoming.truncate(notification_config.max_upcoming_events);

    state.upcoming_events = upcoming;
}

/// Calculate the timing for an event
/// Returns (absolute_start_time, seconds_until_start, seconds_into_event, event_duration)
/// seconds_into_event is >= 0 if the event is currently active, < 0 otherwise
fn calculate_event_timing(
    track: &EventTrack,
    event: &TimelineEvent,
    current_time: i64,
) -> Option<(i64, i64, i64, i64)> {
    let elapsed_since_base = current_time - track.base_time;
    let time_in_cycle = elapsed_since_base.rem_euclid(event.cycle_duration);

    // Check if event is currently active
    let event_end_in_cycle = event.start_offset + event.duration;
    if time_in_cycle >= event.start_offset && time_in_cycle < event_end_in_cycle {
        // Event is active now
        let cycle_start = current_time - time_in_cycle;
        let start_time = cycle_start + event.start_offset;
        let seconds_into = time_in_cycle - event.start_offset;
        return Some((start_time, 0, seconds_into, event.duration));
    }

    // Calculate time to next occurrence
    let mut time_to_start = event.start_offset - time_in_cycle;

    // If event already passed in this cycle, get the next cycle
    if time_to_start <= 0 {
        time_to_start += event.cycle_duration;
    }

    let start_time = current_time + time_to_start;

    // Event not active yet, so seconds_into is negative (indicates not active)
    Some((start_time, time_to_start, -1, event.duration))
}

/// Helper to check if an event is currently tracked
pub fn is_event_tracked(track_name: &str, event_name: &str) -> bool {
    let config = RUNTIME_CONFIG.lock();
    let event_id = TrackedEventId::new(track_name, event_name);
    config.tracked_events.contains(&event_id)
}

/// Toggle tracking for an event
pub fn toggle_event_tracking(track_name: &str, event_name: &str) {
    let mut config = RUNTIME_CONFIG.lock();
    let event_id = TrackedEventId::new(track_name, event_name);

    if config.tracked_events.contains(&event_id) {
        config.tracked_events.remove(&event_id);
    } else {
        config.tracked_events.insert(event_id);
    }
}

/// Set tracking state for an event
pub fn set_event_tracking(track_name: &str, event_name: &str, tracked: bool) {
    let mut config = RUNTIME_CONFIG.lock();
    let event_id = TrackedEventId::new(track_name, event_name);

    if tracked {
        config.tracked_events.insert(event_id);
    } else {
        config.tracked_events.remove(&event_id);
    }
}
