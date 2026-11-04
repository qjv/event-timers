use nexus::imgui::{
    Condition, MenuItem, MouseButton, StyleColor, StyleVar, Ui, Window, WindowFlags,
};

use crate::config::{NotificationConfig, ToastPosition, UpcomingPanelMode, RUNTIME_CONFIG};
use crate::notification_logic::{toggle_event_favorite, toggle_event_tracking};
use crate::notifications::{ToastNotification, NOTIFICATION_STATE};
use crate::time_utils::format_time_only;

/// Calculate toast position based on config
fn calculate_toast_position(
    index: usize,
    position: ToastPosition,
    toast_size: [f32; 2],
    display_size: [f32; 2],
    offset_x: f32,
    offset_y: f32,
) -> [f32; 2] {
    let margin = 10.0;
    let spacing = 5.0;
    let stack_offset = index as f32 * (toast_size[1] + spacing);

    // Convert percentage offsets to pixels
    let x_offset_px = offset_x * display_size[0];
    let y_offset_px = offset_y * display_size[1];

    match position {
        ToastPosition::TopRight => [
            display_size[0] - toast_size[0] - margin - x_offset_px,
            margin + stack_offset + y_offset_px,
        ],
        ToastPosition::TopLeft => [margin + x_offset_px, margin + stack_offset + y_offset_px],
        ToastPosition::BottomRight => [
            display_size[0] - toast_size[0] - margin - x_offset_px,
            display_size[1] - toast_size[1] - margin - stack_offset - y_offset_px,
        ],
        ToastPosition::BottomLeft => [
            margin + x_offset_px,
            display_size[1] - toast_size[1] - margin - stack_offset - y_offset_px,
        ],
    }
}

struct ToastAction {
    copy_clicked: bool,
    dismissed: bool,
}

/// Render a single toast notification
fn render_single_toast(
    ui: &Ui,
    toast: &ToastNotification,
    position: [f32; 2],
    size: [f32; 2],
    config: &NotificationConfig,
) -> ToastAction {
    let mut action = ToastAction {
        copy_clicked: false,
        dismissed: false,
    };
    let _alpha = ui.push_style_var(StyleVar::Alpha(toast.opacity));
    let _bg = ui.push_style_color(StyleColor::WindowBg, config.toast_bg_color);

    let window_flags = WindowFlags::NO_DECORATION
        | WindowFlags::NO_MOVE
        | WindowFlags::NO_RESIZE
        | WindowFlags::NO_SAVED_SETTINGS
        | WindowFlags::NO_FOCUS_ON_APPEARING
        | WindowFlags::NO_NAV;

    Window::new(format!("##toast_{}", toast.id))
        .position(position, Condition::Always)
        .size(size, Condition::Always)
        .flags(window_flags)
        .build(ui, || {
            let scale = config.toast_text_scale;

            // Draw X button in upper right corner
            let draw_list = ui.get_window_draw_list();
            let window_pos = ui.window_pos();
            let button_size = 16.0 * scale;
            let button_margin = 4.0;
            let button_x = window_pos[0] + size[0] - button_size - button_margin;
            let button_y = window_pos[1] + button_margin;

            let mouse_pos = ui.io().mouse_pos;
            let over_x_button = mouse_pos[0] >= button_x
                && mouse_pos[0] <= button_x + button_size
                && mouse_pos[1] >= button_y
                && mouse_pos[1] <= button_y + button_size;

            let x_color = if over_x_button {
                [1.0, 0.4, 0.4, 1.0]
            } else {
                [0.6, 0.6, 0.6, 0.8]
            };

            let x_center = [button_x + button_size / 2.0, button_y + button_size / 2.0];
            let x_half = button_size / 3.0;
            draw_list
                .add_line(
                    [x_center[0] - x_half, x_center[1] - x_half],
                    [x_center[0] + x_half, x_center[1] + x_half],
                    x_color,
                )
                .thickness(2.0)
                .build();
            draw_list
                .add_line(
                    [x_center[0] + x_half, x_center[1] - x_half],
                    [x_center[0] - x_half, x_center[1] + x_half],
                    x_color,
                )
                .thickness(2.0)
                .build();

            // Event name (title)
            ui.set_window_font_scale(scale);
            ui.text_colored(config.toast_title_color, &toast.display_name);

            // Track name
            ui.set_window_font_scale(scale * 0.85);
            ui.text_colored(config.toast_track_color, &toast.event_id.track_name);

            // Reminder message and time info
            ui.set_window_font_scale(scale);
            let time_text = if toast.minutes_until > 0 {
                format!("{} ({} min)", toast.reminder_name, toast.minutes_until)
            } else if toast.minutes_until < 0 {
                format!("{} ({}m ago)", toast.reminder_name, -toast.minutes_until)
            } else {
                format!("{} (now!)", toast.reminder_name)
            };
            ui.text_colored(toast.reminder_color, &time_text);

            // Click hint if copy_text available
            if !toast.copy_text.is_empty() {
                ui.set_window_font_scale(scale * 0.7);
                ui.text_colored([0.5, 0.5, 0.5, 1.0], "Click to copy waypoint");
            }

            ui.set_window_font_scale(1.0);

            if over_x_button && ui.is_mouse_clicked(MouseButton::Left) {
                action.dismissed = true;
            } else if ui.is_window_hovered() && ui.is_mouse_clicked(MouseButton::Left) {
                action.copy_clicked = true;
            }
        });

    action
}

/// Render toast notifications (call from main render loop)
pub fn render_toast_notifications(ui: &Ui) {
    let (notification_config, copy_with_event_name) = {
        let config = RUNTIME_CONFIG.lock();
        (
            config.notification_config.clone(),
            config.copy_with_event_name,
        )
    };

    if !notification_config.toast_enabled {
        let has_preview_or_toasts = {
            let state = NOTIFICATION_STATE.lock();
            state.preview_toast.is_some() || !state.toast_queue.is_empty()
        };
        if !has_preview_or_toasts {
            return;
        }
    }

    let toast_position = notification_config.toast_position;
    let toast_size = notification_config.toast_size;
    let toast_duration = notification_config.toast_duration_seconds;
    let offset_x = notification_config.toast_offset_x;
    let offset_y = notification_config.toast_offset_y;

    // Update and render preview toast
    {
        let mut state = NOTIFICATION_STATE.lock();
        state.update_toasts(
            notification_config.toast_duration_seconds,
            notification_config.max_visible_toasts,
        );
        state.update_preview(toast_duration);

        if let Some(preview) = &state.preview_toast {
            let display_size = ui.io().display_size;
            let pos = calculate_toast_position(
                0,
                toast_position,
                toast_size,
                display_size,
                offset_x,
                offset_y,
            );
            let action = render_single_toast(ui, preview, pos, toast_size, &notification_config);
            if action.copy_clicked && !preview.copy_text.is_empty() {
                let copy_text = if copy_with_event_name {
                    format!("{}: {}", preview.display_name, preview.copy_text)
                } else {
                    preview.copy_text.clone()
                };
                ui.set_clipboard_text(&copy_text);
            }
            if action.dismissed {
                state.preview_toast = None;
            }
        }
    }

    if !notification_config.toast_enabled {
        return;
    }

    // Collect actions from clicked toasts
    let mut copy_text_to_set: Option<String> = None;
    let mut toasts_to_dismiss: Vec<u64> = Vec::new();

    {
        let state = NOTIFICATION_STATE.lock();
        let display_size = ui.io().display_size;

        let start_index = if state.preview_toast.is_some() { 1 } else { 0 };

        for (index, toast) in state.toast_queue.iter().enumerate() {
            let pos = calculate_toast_position(
                index + start_index,
                toast_position,
                toast_size,
                display_size,
                offset_x,
                offset_y,
            );
            let action = render_single_toast(ui, toast, pos, toast_size, &notification_config);
            if action.copy_clicked && !toast.copy_text.is_empty() {
                let copy_text = if copy_with_event_name {
                    format!("{}: {}", toast.display_name, toast.copy_text)
                } else {
                    toast.copy_text.clone()
                };
                copy_text_to_set = Some(copy_text);
            }
            if action.dismissed {
                toasts_to_dismiss.push(toast.id);
            }
        }
    }

    if let Some(text) = copy_text_to_set {
        ui.set_clipboard_text(&text);
    }

    if !toasts_to_dismiss.is_empty() {
        let mut state = NOTIFICATION_STATE.lock();
        for id in toasts_to_dismiss {
            if let Some(toast) = state.toast_queue.iter_mut().find(|t| t.id == id) {
                toast.dismissed = true;
            }
        }
    }
}

// Thread-local state for context menu in upcoming panel
thread_local! {
    static UPCOMING_CONTEXT_EVENT: std::cell::RefCell<Option<(crate::config::TrackedEventId, String)>> = const { std::cell::RefCell::new(None) };
    static UPCOMING_OPEN_MENU: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Render the upcoming events panel
pub fn render_upcoming_panel(ui: &Ui) {
    let (panel_enabled, panel_size, panel_mode, copy_with_event_name, tracked_set, favorite_set) = {
        let config = RUNTIME_CONFIG.lock();
        (
            config.notification_config.upcoming_panel_enabled,
            config.notification_config.upcoming_panel_size,
            config.notification_config.upcoming_panel_mode,
            config.copy_with_event_name,
            config.tracked_events.clone(),
            config.favorite_events.clone(),
        )
    };

    if !panel_enabled {
        return;
    }

    let mut copy_text_to_set: Option<String> = None;
    let mut event_to_toggle_track: Option<crate::config::TrackedEventId> = None;
    let mut event_to_toggle_fav: Option<crate::config::TrackedEventId> = None;
    let mut event_to_toggle_done: Option<crate::config::TrackedEventId> = None;
    let mut wiki_to_open: Option<String> = None;
    let upcoming_events = NOTIFICATION_STATE.lock().upcoming_events.clone();

    let mut opened = true;
    Window::new("Upcoming Events")
        .size(panel_size, Condition::FirstUseEver)
        .collapsible(true)
        .opened(&mut opened)
        .build(ui, || {
            if upcoming_events.is_empty() {
                match panel_mode {
                    UpcomingPanelMode::TrackedOnly => {
                        ui.text_disabled("No tracked events");
                        ui.text_disabled("Right-click events in timeline to track");
                    }
                    UpcomingPanelMode::TrackedAndFavorites => {
                        ui.text_disabled("No tracked or favorite events");
                        ui.text_disabled("Right-click events in timeline to track/favorite");
                    }
                    UpcomingPanelMode::AllVisibleEvents => {
                        ui.text_disabled("No upcoming events in visible tracks");
                    }
                }
                return;
            }

            for event in &upcoming_events {
                let draw_list = ui.get_window_draw_list();
                let cursor_pos = ui.cursor_screen_pos();

                // Color indicator bar
                draw_list
                    .add_rect(
                        cursor_pos,
                        [cursor_pos[0] + 4.0, cursor_pos[1] + 18.0],
                        event.color,
                    )
                    .filled(true)
                    .build();

                ui.set_cursor_pos([ui.cursor_pos()[0] + 8.0, ui.cursor_pos()[1]]);

                // Time display
                let (time_text, time_color) =
                    format_event_time(event.seconds_until, event.seconds_into);
                ui.text_colored(time_color, &time_text);
                let time_hovered = ui.is_item_hovered();

                ui.same_line();

                // Event name
                let completed = crate::completion::is_completed(
                    &event.event_id.track_name,
                    &event.event_id.event_name,
                );
                let is_favorite = favorite_set.contains(&event.event_id);

                if completed {
                    ui.text_colored([0.52, 0.52, 0.52, 1.0], &event.display_name);
                    let min = ui.item_rect_min();
                    let max = ui.item_rect_max();
                    draw_list
                        .add_line(
                            [min[0], (min[1] + max[1]) * 0.5],
                            [max[0], (min[1] + max[1]) * 0.5],
                            [0.72, 0.72, 0.72, 0.95],
                        )
                        .thickness(1.5)
                        .build();
                } else if is_favorite {
                    ui.text_colored([1.0, 0.85, 0.25, 1.0], &event.display_name);
                } else {
                    ui.text(&event.display_name);
                }

                let name_hovered = ui.is_item_hovered();
                let row_hovered = time_hovered || name_hovered;

                if row_hovered {
                    ui.tooltip(|| {
                        ui.text(format!(
                            "{}: {}",
                            event.event_id.track_name, event.display_name
                        ));
                        ui.separator();
                        ui.text(format!("Starts: {}", format_time_only(event.start_time)));
                        if completed {
                            ui.text_colored([0.5, 0.9, 0.5, 1.0], "Status: Completed today");
                        }
                        if !event.copy_text.is_empty() {
                            ui.text(format!("Waypoint: {}", event.copy_text));
                            ui.separator();
                            ui.text_disabled("Left-click to copy");
                        }
                        ui.text_disabled("Right-click for options");
                    });
                }

                if row_hovered && ui.is_mouse_clicked(MouseButton::Left) {
                    if !event.copy_text.is_empty() {
                        if copy_with_event_name {
                            copy_text_to_set =
                                Some(format!("{}: {}", event.display_name, event.copy_text));
                        } else {
                            copy_text_to_set = Some(event.copy_text.clone());
                        }
                    }
                }

                if row_hovered && ui.is_mouse_clicked(MouseButton::Right) {
                    UPCOMING_CONTEXT_EVENT.with(|e| {
                        *e.borrow_mut() =
                            Some((event.event_id.clone(), event.display_name.clone()));
                    });
                    UPCOMING_OPEN_MENU.with(|f| f.set(true));
                }

                ui.separator();
            }

            let should_open = UPCOMING_OPEN_MENU.with(|f| {
                let val = f.get();
                if val {
                    f.set(false);
                }
                val
            });

            if should_open {
                ui.open_popup("##upcoming_context_menu");
            }

            ui.popup("##upcoming_context_menu", || {
                let context_event = UPCOMING_CONTEXT_EVENT.with(|e| e.borrow().clone());
                if let Some((event_id, display_name)) = context_event {
                    ui.text_disabled(&display_name);
                    ui.separator();

                    let is_done = crate::completion::is_manually_completed(
                        &event_id.track_name,
                        &event_id.event_name,
                    );
                    let done_label = if is_done {
                        "Unmark Done Today"
                    } else {
                        "Mark as Done Today"
                    };
                    if MenuItem::new(done_label).build(ui) {
                        event_to_toggle_done = Some(event_id.clone());
                    }

                    let is_tracked = tracked_set.contains(&event_id);
                    let track_label = if is_tracked {
                        "Untrack Event"
                    } else {
                        "Track Event"
                    };
                    if MenuItem::new(track_label).build(ui) {
                        event_to_toggle_track = Some(event_id.clone());
                    }

                    let is_fav = favorite_set.contains(&event_id);
                    let fav_label = if is_fav {
                        "Unfavorite Event"
                    } else {
                        "Favorite Event"
                    };
                    if MenuItem::new(fav_label).build(ui) {
                        event_to_toggle_fav = Some(event_id.clone());
                    }

                    ui.separator();
                    if MenuItem::new("Open Wiki").build(ui) {
                        wiki_to_open = Some(event_id.event_name.clone());
                    }
                }
            });
        });

    if !opened {
        let mut config = RUNTIME_CONFIG.lock();
        config.notification_config.upcoming_panel_enabled = false;
    }

    if let Some(text) = copy_text_to_set {
        ui.set_clipboard_text(&text);
    }

    if let Some(event_id) = event_to_toggle_done {
        crate::completion::toggle_manual_completion(&event_id.track_name, &event_id.event_name);
        NOTIFICATION_STATE.lock().force_refresh();
    }

    if let Some(event_id) = event_to_toggle_track {
        toggle_event_tracking(&event_id.track_name, &event_id.event_name);
    }

    if let Some(event_id) = event_to_toggle_fav {
        toggle_event_favorite(&event_id.track_name, &event_id.event_name);
    }

    if let Some(event_name) = wiki_to_open {
        let search_query = event_name.replace(' ', "+");
        let url = format!("https://wiki.guildwars2.com/wiki/?search={}", search_query);
        let _ = open::that(url);
    }
}

/// Format event time - returns (text, color)
/// Shows time until event, or time since it started if active
fn format_event_time(seconds_until: i64, seconds_into: i64) -> (String, [f32; 4]) {
    if seconds_until <= 0 && seconds_into > 0 {
        let text = if seconds_into < 60 {
            format!("{}s ago", seconds_into)
        } else if seconds_into < 3600 {
            let mins = seconds_into / 60;
            format!("{}m ago", mins)
        } else {
            let hours = seconds_into / 3600;
            let mins = (seconds_into % 3600) / 60;
            format!("{}h {}m ago", hours, mins)
        };
        (text, [1.0, 0.8, 0.2, 1.0])
    } else if seconds_until <= 0 {
        ("NOW".to_string(), [0.5, 1.0, 0.5, 1.0])
    } else {
        let text = if seconds_until < 60 {
            format!("{}s", seconds_until)
        } else if seconds_until < 3600 {
            let mins = seconds_until / 60;
            let secs = seconds_until % 60;
            if secs > 0 {
                format!("{}m {}s", mins, secs)
            } else {
                format!("{}m", mins)
            }
        } else {
            let hours = seconds_until / 3600;
            let mins = (seconds_until % 3600) / 60;
            format!("{}h {}m", hours, mins)
        };
        (text, [0.5, 1.0, 0.5, 1.0])
    }
}
