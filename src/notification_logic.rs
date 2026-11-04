use crate::config::{TrackedEventId, UpcomingPanelMode, RUNTIME_CONFIG};
use crate::json_loader::{EventTrack, TimelineEvent};
use crate::notifications::{UpcomingEvent, NOTIFICATION_STATE};
use crate::time_utils::get_current_unix_time;
use std::collections::HashMap;

fn insert_upcoming(
    upcoming_by_event: &mut HashMap<TrackedEventId, UpcomingEvent>,
    upcoming: UpcomingEvent,
) {
    let event_id = upcoming.event_id.clone();
    match upcoming_by_event.get_mut(&event_id) {
        Some(existing) if upcoming.seconds_until < existing.seconds_until => {
            *existing = upcoming;
        }
        Some(existing)
            if upcoming.seconds_until == existing.seconds_until
                && upcoming.seconds_into > existing.seconds_into =>
        {
            *existing = upcoming;
        }
        None => {
            upcoming_by_event.insert(event_id, upcoming);
        }
        _ => {}
    }
}

/// Background update function - called by maintenance ticker.
pub fn update_notifications() {
    let current_time = get_current_unix_time();
    let mut config = RUNTIME_CONFIG.lock();
    let tracked_events = &config.tracked_events;
    let oneshot_events = &config.oneshot_events;
    let favorite_events = &config.favorite_events;
    let notification_config = &config.notification_config;
    let category_visibility = &config.category_visibility;
    let tracks = &config.tracks;

    let panel_mode = notification_config.upcoming_panel_mode;
    let needs_panel_scan = match panel_mode {
        UpcomingPanelMode::TrackedOnly => !tracked_events.is_empty() || !oneshot_events.is_empty(),
        UpcomingPanelMode::TrackedAndFavorites => {
            !tracked_events.is_empty() || !oneshot_events.is_empty() || !favorite_events.is_empty()
        }
        UpcomingPanelMode::AllVisibleEvents => notification_config.upcoming_panel_enabled,
    };

    if !needs_panel_scan && tracked_events.is_empty() && oneshot_events.is_empty() {
        let mut state = NOTIFICATION_STATE.lock();
        state.upcoming_events.clear();
        return;
    }

    let mut oneshot_to_remove: Vec<TrackedEventId> = Vec::new();

    let mut state = NOTIFICATION_STATE.lock();

    // Only refresh calculations once per second
    if !state.needs_refresh(current_time) {
        return;
    }
    state.set_refresh_time(current_time);

    // Clean up old notification records
    state.cleanup_old_notifications(current_time);

    let mut upcoming_by_event: HashMap<TrackedEventId, UpcomingEvent> = HashMap::new();

    for track in tracks {
        if !track.visible {
            continue;
        }
        let is_cat_visible = *category_visibility.get(&track.category).unwrap_or(&true);
        if !is_cat_visible {
            continue;
        }

        for event in &track.events {
            if !event.enabled {
                continue;
            }

            let event_id = TrackedEventId::new(&track.name, &event.name);

            let is_tracked = tracked_events.contains(&event_id);
            let is_oneshot = oneshot_events.contains(&event_id);
            let is_favorite = favorite_events.contains(&event_id);

            let include_in_upcoming = match panel_mode {
                UpcomingPanelMode::TrackedOnly => is_tracked || is_oneshot,
                UpcomingPanelMode::TrackedAndFavorites => is_tracked || is_oneshot || is_favorite,
                UpcomingPanelMode::AllVisibleEvents => {
                    is_tracked || is_oneshot || is_favorite || track.category != "Time Cycles"
                }
            };

            if !include_in_upcoming && !is_tracked && !is_oneshot {
                continue;
            }

            let is_completed = crate::completion::is_completed(&track.name, &event.name);

            if let Some((
                start_time,
                seconds_until,
                seconds_into_event,
                event_duration,
                _cycle_number,
            )) = calculate_event_timing(track, event, current_time)
            {
                let disp_name = event.display_name().to_string();

                if include_in_upcoming
                    && !(notification_config.hide_completed_in_upcoming && is_completed)
                {
                    let upcoming = UpcomingEvent {
                        start_time,
                        event_id: event_id.clone(),
                        display_name: disp_name.clone(),
                        seconds_until,
                        seconds_into: if seconds_into_event >= 0 {
                            seconds_into_event
                        } else {
                            0
                        },
                        color: event.color.to_array(),
                        copy_text: event.copy_text.clone(),
                    };
                    insert_upcoming(&mut upcoming_by_event, upcoming);
                }

                // Only fire toast reminders for tracked or one-shot events
                if !is_tracked && !is_oneshot {
                    continue;
                }

                // If completed today and mute_completed_events is enabled, suppress reminders
                if is_completed && notification_config.mute_completed_events {
                    if is_oneshot && seconds_into_event >= 0 {
                        oneshot_to_remove.push(event_id.clone());
                    }
                    continue;
                }

                if notification_config.toast_enabled {
                    let mut oneshot_fired = false;
                    for reminder in &notification_config.reminders {
                        let reminder_seconds = (reminder.minutes_before as i64) * 60;

                        if reminder.minutes_before == 0 {
                            if seconds_into_event >= 0 {
                                let interval_seconds =
                                    (reminder.ongoing_interval_minutes.max(1) as i64) * 60;
                                let grace_seconds =
                                    (notification_config.happening_now_grace_minutes as i64) * 60;
                                let remaining_seconds = event_duration - seconds_into_event;
                                if seconds_into_event <= grace_seconds
                                    && remaining_seconds > interval_seconds
                                    && state.can_add_toast(current_time)
                                    && state.can_notify_event(&event_id, current_time)
                                    && state.should_show_ongoing(
                                        &event_id,
                                        start_time,
                                        current_time,
                                        interval_seconds,
                                    )
                                {
                                    let minutes_ago = -((seconds_into_event / 60) as i32);
                                    state.add_toast(
                                        event_id.clone(),
                                        disp_name.clone(),
                                        minutes_ago,
                                        event.copy_text.clone(),
                                        reminder.name.clone(),
                                        reminder.text_color,
                                        current_time,
                                    );
                                    state.mark_ongoing_notified(
                                        &event_id,
                                        start_time,
                                        current_time,
                                    );
                                    state.mark_event_notified(&event_id, current_time);
                                    if is_oneshot {
                                        oneshot_to_remove.push(event_id.clone());
                                        oneshot_fired = true;
                                        break;
                                    }
                                }
                            }
                        } else if seconds_until > 0
                            && seconds_until <= reminder_seconds
                            && state.can_add_toast(current_time)
                            && state.can_notify_event(&event_id, current_time)
                            && !state.was_notified(&event_id, start_time, reminder.minutes_before)
                        {
                            let minutes_until = ((seconds_until + 59) / 60) as i32;
                            state.add_toast(
                                event_id.clone(),
                                disp_name.clone(),
                                minutes_until,
                                event.copy_text.clone(),
                                reminder.name.clone(),
                                reminder.text_color,
                                current_time,
                            );
                            state.mark_notified(&event_id, start_time, reminder.minutes_before);
                            state.mark_event_notified(&event_id, current_time);
                            if is_oneshot {
                                oneshot_to_remove.push(event_id.clone());
                                oneshot_fired = true;
                                break;
                            }
                        }
                    }
                    if is_oneshot && !oneshot_fired && seconds_into_event >= 0 {
                        oneshot_to_remove.push(event_id.clone());
                    }
                } else if is_oneshot && seconds_into_event >= 0 {
                    oneshot_to_remove.push(event_id.clone());
                }
            }
        }
    }

    let mut upcoming = upcoming_by_event.into_values().collect::<Vec<_>>();

    // Sort by time (active events with seconds_until == 0 first, then soonest upcoming)
    upcoming.sort_by(|a, b| {
        a.seconds_until
            .cmp(&b.seconds_until)
            .then_with(|| b.seconds_into.cmp(&a.seconds_into))
    });

    // Limit to max configured
    upcoming.truncate(notification_config.max_upcoming_events);

    state.upcoming_events = upcoming;

    // Drop state lock before modifying config
    drop(state);

    if !oneshot_to_remove.is_empty() {
        for event_id in oneshot_to_remove {
            config.oneshot_events.remove(&event_id);
        }
    }
}

/// Calculate the timing for an event
/// Returns (absolute_start_time, seconds_until_start, seconds_into_event, event_duration, cycle_number)
/// seconds_into_event is >= 0 if the event is currently active, < 0 otherwise
/// cycle_number is a stable identifier for this occurrence (used for deduplication)
pub fn calculate_event_timing(
    track: &EventTrack,
    event: &TimelineEvent,
    current_time: i64,
) -> Option<(i64, i64, i64, i64, i64)> {
    if event.cycle_duration <= 0 || event.duration <= 0 {
        return None;
    }
    let elapsed = current_time.checked_sub(track.base_time)?;
    let phase = event.start_offset.rem_euclid(event.cycle_duration);
    let since_start = elapsed.checked_sub(phase)?;
    let seconds_into = since_start.rem_euclid(event.cycle_duration);
    let cycle_number = since_start.div_euclid(event.cycle_duration);
    if seconds_into < event.duration {
        Some((
            current_time.checked_sub(seconds_into)?,
            0,
            seconds_into,
            event.duration,
            cycle_number,
        ))
    } else {
        let seconds_until = event.cycle_duration - seconds_into;
        Some((
            current_time.checked_add(seconds_until)?,
            seconds_until,
            -1,
            event.duration,
            cycle_number.checked_add(1)?,
        ))
    }
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
    drop(config);
    NOTIFICATION_STATE.lock().force_refresh();
}

/// Toggle favorite highlight for an event
pub fn toggle_event_favorite(track_name: &str, event_name: &str) {
    let mut config = RUNTIME_CONFIG.lock();
    let event_id = TrackedEventId::new(track_name, event_name);

    if config.favorite_events.contains(&event_id) {
        config.favorite_events.remove(&event_id);
    } else {
        config.favorite_events.insert(event_id);
    }
    drop(config);
    NOTIFICATION_STATE.lock().force_refresh();
}

/// Toggle one-shot tracking for an event (track next occurrence only)
pub fn toggle_oneshot_tracking(track_name: &str, event_name: &str) {
    let mut config = RUNTIME_CONFIG.lock();
    let event_id = TrackedEventId::new(track_name, event_name);

    if config.oneshot_events.contains(&event_id) {
        config.oneshot_events.remove(&event_id);
    } else {
        config.oneshot_events.insert(event_id);
    }
    drop(config);
    NOTIFICATION_STATE.lock().force_refresh();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_loader::{EventColor, TimelineType};

    fn event(cycle_duration: i64) -> TimelineEvent {
        TimelineEvent {
            name: "Event".into(),
            custom_name: None,
            start_offset: 0,
            duration: 300,
            cycle_duration,
            color: EventColor::default(),
            copy_text: String::new(),
            enabled: true,
        }
    }

    fn upcoming(seconds_until: i64) -> UpcomingEvent {
        UpcomingEvent {
            event_id: TrackedEventId::new("Track", "Event"),
            display_name: "Event".into(),
            start_time: seconds_until,
            seconds_until,
            seconds_into: 0,
            color: [1.0; 4],
            copy_text: String::new(),
        }
    }

    #[test]
    fn malformed_cycle_is_not_calculated() {
        let track = EventTrack {
            name: "Track".into(),
            timeline_type: TimelineType::RealTime,
            events: Vec::new(),
            base_time: 0,
            visible: true,
            height: 20.0,
            category: String::new(),
        };
        assert!(calculate_event_timing(&track, &event(0), 100).is_none());
    }

    #[test]
    fn event_spanning_midnight_wrap_remains_active() {
        let track = EventTrack {
            name: "Tangled Depths".into(),
            timeline_type: TimelineType::RealTime,
            events: Vec::new(),
            base_time: 0,
            visible: true,
            height: 20.0,
            category: "Heart of Thorns".into(),
        };
        let wrapped_event = TimelineEvent {
            name: "Outposts".into(),
            custom_name: None,
            start_offset: 1430 * 60,
            duration: 100 * 60,
            cycle_duration: 1440 * 60,
            color: EventColor::default(),
            copy_text: String::new(),
            enabled: true,
        };
        // 10 minutes after midnight wrap (1450 minutes since base_time)
        let timing =
            calculate_event_timing(&track, &wrapped_event, 1450 * 60).expect("should calculate");
        assert_eq!(timing.1, 0); // seconds_until == 0 (active)
        assert_eq!(timing.2, 20 * 60); // 20 minutes into the event
        assert_eq!(timing.0, 1430 * 60);
        assert_eq!(timing.4, 0);
        let end = calculate_event_timing(&track, &wrapped_event, 1530 * 60).unwrap();
        assert_eq!(end.1, 1340 * 60);
        assert_eq!(end.2, -1);
        assert_eq!(end.4, 1);
    }

    #[test]
    fn normalizes_custom_offsets_before_the_track_base() {
        let track = EventTrack::default();
        let mut custom = event(3600);
        custom.start_offset = -60;
        let active = calculate_event_timing(&track, &custom, 0).unwrap();
        assert_eq!(active, (-60, 0, 60, 300, -1));
        custom.start_offset += 7200;
        assert_eq!(calculate_event_timing(&track, &custom, 0).unwrap(), active);
    }

    #[test]
    fn upcoming_panel_keeps_only_nearest_occurrence_per_event() {
        let mut events = HashMap::new();
        insert_upcoming(&mut events, upcoming(7200));
        insert_upcoming(&mut events, upcoming(600));
        insert_upcoming(&mut events, upcoming(3600));
        assert_eq!(events.len(), 1);
        assert_eq!(events.values().next().unwrap().seconds_until, 600);
    }
}
