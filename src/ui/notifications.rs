use nexus::imgui::{Condition, MenuItem, MouseButton, StyleColor, StyleVar, Ui, Window, WindowFlags};

use crate::config::{NotificationConfig, ToastPosition, RUNTIME_CONFIG};
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
        ToastPosition::TopLeft => [
            margin + x_offset_px,
            margin + stack_offset + y_offset_px,
        ],
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

/// Render a single toast notification
fn render_single_toast(
    ui: &Ui,
    toast: &ToastNotification,
    position: [f32; 2],
    size: [f32; 2],
    config: &NotificationConfig,
) -> bool {
    let mut clicked = false;
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

            // Event name (title)
            ui.set_window_font_scale(scale);
            ui.text_colored(config.toast_title_color, &toast.event_id.event_name);

            // Track name
            ui.set_window_font_scale(scale * 0.85);
            ui.text_colored(config.toast_track_color, &toast.event_id.track_name);

            // Reminder message and time remaining
            ui.set_window_font_scale(scale);
            let time_text = if toast.minutes_until > 0 {
                format!("{} ({} min)", toast.reminder_name, toast.minutes_until)
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

            // Check for click
            if ui.is_window_hovered() && ui.is_mouse_clicked(MouseButton::Left) {
                clicked = true;
            }
        });

    clicked
}

/// Render toast notifications (call from main render loop)
pub fn render_toast_notifications(ui: &Ui) {
    let notification_config = {
        let config = RUNTIME_CONFIG.lock();
        config.notification_config.clone()
    };

    let toast_position = notification_config.toast_position;
    let toast_size = notification_config.toast_size;
    let toast_duration = notification_config.toast_duration_seconds;
    let offset_x = notification_config.toast_offset_x;
    let offset_y = notification_config.toast_offset_y;

    // Update and render preview toast
    {
        let mut state = NOTIFICATION_STATE.lock();
        state.update_preview(toast_duration);

        if let Some(preview) = &state.preview_toast {
            let display_size = ui.io().display_size;
            let pos = calculate_toast_position(0, toast_position, toast_size, display_size, offset_x, offset_y);
            let clicked = render_single_toast(ui, preview, pos, toast_size, &notification_config);
            if clicked && !preview.copy_text.is_empty() {
                ui.set_clipboard_text(&preview.copy_text);
            }
        }
    }

    if !notification_config.toast_enabled {
        return;
    }

    // Collect copy text for clicked toasts
    let mut copy_text_to_set: Option<String> = None;

    {
        let state = NOTIFICATION_STATE.lock();
        let display_size = ui.io().display_size;

        // Determine starting index (1 if preview is showing, 0 otherwise)
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
            let clicked = render_single_toast(ui, toast, pos, toast_size, &notification_config);
            if clicked && !toast.copy_text.is_empty() {
                copy_text_to_set = Some(toast.copy_text.clone());
            }
        }
    }

    // Copy to clipboard outside of lock
    if let Some(text) = copy_text_to_set {
        ui.set_clipboard_text(&text);
    }
}

// Thread-local state for context menu in upcoming panel
thread_local! {
    static UPCOMING_CONTEXT_EVENT: std::cell::RefCell<Option<crate::config::TrackedEventId>> = std::cell::RefCell::new(None);
    static UPCOMING_OPEN_MENU: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Render the upcoming events panel
pub fn render_upcoming_panel(ui: &Ui) {
    let (panel_enabled, panel_size, copy_with_event_name) = {
        let config = RUNTIME_CONFIG.lock();
        (
            config.notification_config.upcoming_panel_enabled,
            config.notification_config.upcoming_panel_size,
            config.copy_with_event_name,
        )
    };

    if !panel_enabled {
        return;
    }

    // Collect actions to perform outside of lock
    let mut copy_text_to_set: Option<String> = None;
    let mut event_to_untrack: Option<crate::config::TrackedEventId> = None;

    {
        let state = NOTIFICATION_STATE.lock();

        let mut opened = true;
        Window::new("Upcoming Events")
            .size(panel_size, Condition::FirstUseEver)
            .collapsible(true)
            .opened(&mut opened)
            .build(ui, || {
                if state.upcoming_events.is_empty() {
                    ui.text_disabled("No tracked events");
                    ui.text_disabled("Right-click events in timeline to track");
                    return;
                }

                for event in &state.upcoming_events {
                    // Event row with color indicator
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

                    // Time display - show time until or time since started
                    let (time_text, time_color) = format_event_time(event.seconds_until, event.seconds_into);
                    ui.text_colored(time_color, &time_text);

                    // Check for clicks on time text
                    let time_hovered = ui.is_item_hovered();

                    ui.same_line();

                    // Event name
                    ui.text(&event.event_id.event_name);

                    // Check for clicks on event name
                    let name_hovered = ui.is_item_hovered();

                    let row_hovered = time_hovered || name_hovered;

                    // Tooltip with full info
                    if row_hovered {
                        ui.tooltip(|| {
                            ui.text(&event.event_id.display_name());
                            ui.separator();
                            ui.text(format!("Starts: {}", format_time_only(event.start_time)));
                            if !event.copy_text.is_empty() {
                                ui.text(format!("Waypoint: {}", event.copy_text));
                                ui.separator();
                                ui.text_disabled("Left-click to copy");
                            }
                            ui.text_disabled("Right-click for options");
                        });
                    }

                    // Left-click: copy waypoint (respects copy_with_event_name setting)
                    if row_hovered && ui.is_mouse_clicked(MouseButton::Left) {
                        if !event.copy_text.is_empty() {
                            if copy_with_event_name {
                                copy_text_to_set = Some(format!("{}: {}", event.event_id.event_name, event.copy_text));
                            } else {
                                copy_text_to_set = Some(event.copy_text.clone());
                            }
                        }
                    }

                    // Right-click: open context menu
                    if row_hovered && ui.is_mouse_clicked(MouseButton::Right) {
                        UPCOMING_CONTEXT_EVENT.with(|e| {
                            *e.borrow_mut() = Some(event.event_id.clone());
                        });
                        UPCOMING_OPEN_MENU.with(|f| f.set(true));
                    }

                    ui.separator();
                }

                // Render context menu
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
                    if let Some(event_id) = context_event {
                        ui.text_disabled(&event_id.event_name);
                        ui.separator();

                        if MenuItem::new("Untrack Event").build(ui) {
                            event_to_untrack = Some(event_id);
                        }
                    }
                });
            });

        // If user closed the panel, update config
        if !opened {
            drop(state); // Release state lock first
            let mut config = RUNTIME_CONFIG.lock();
            config.notification_config.upcoming_panel_enabled = false;
        }
    }

    // Copy to clipboard outside of lock
    if let Some(text) = copy_text_to_set {
        ui.set_clipboard_text(&text);
    }

    // Untrack event outside of lock
    if let Some(event_id) = event_to_untrack {
        let mut config = RUNTIME_CONFIG.lock();
        config.tracked_events.remove(&event_id);
    }
}

/// Format event time - returns (text, color)
/// Shows time until event, or time since it started if active
fn format_event_time(seconds_until: i64, seconds_into: i64) -> (String, [f32; 4]) {
    if seconds_until <= 0 && seconds_into > 0 {
        // Event is active - show time since it started
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
        // Yellow/orange color for active events
        (text, [1.0, 0.8, 0.2, 1.0])
    } else if seconds_until <= 0 {
        // Just started
        ("NOW".to_string(), [0.5, 1.0, 0.5, 1.0])
    } else {
        // Event upcoming
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
        // Green color for upcoming events
        (text, [0.5, 1.0, 0.5, 1.0])
    }
}
