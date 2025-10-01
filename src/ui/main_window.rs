use crate::config::{get_track_visual_config, RUNTIME_CONFIG};
use crate::json_loader::EventTrack;
use crate::time_utils::{format_time_only, get_current_unix_time};
use crate::ui::time_ruler::render_time_ruler;
use nexus::imgui::{ChildWindow, Condition, MenuItem, MouseButton, StyleVar, Ui, Window, WindowFlags};
use std::collections::HashSet;

pub fn render_main_window(ui: &Ui) {
    let mut config = RUNTIME_CONFIG.lock();

    if !config.show_main_window {
        return;
    }

    // Cache all config values ONCE at start
    let view_range = config.view_range_seconds;
    let timeline_width = config.timeline_width;
    let time_position = config.current_time_position;
    let show_headers = config.show_category_headers;
    let spacing_same = config.spacing_same_category;
    let spacing_between = config.spacing_between_categories;
    let global_bg = config.global_track_background;
    let global_padding = config.global_track_padding;
    let override_all_track_heights = config.override_all_track_heights;
    let global_track_height = config.global_track_height;
    let draw_event_borders = config.draw_event_borders;
    let event_border_color = config.event_border_color;
    let event_border_thickness = config.event_border_thickness;
    
    // Calculate time ONCE per frame
    let current_time = get_current_unix_time();
    let time_before_current = view_range * time_position;
    let time_after_current = view_range * (1.0 - time_position);

    let mut window_flags = WindowFlags::empty();
    if config.is_window_locked {
        window_flags |= WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE;
    }

    let mut window = Window::new("Event Timers");
    if config.is_window_locked {
        window = window.title_bar(false);
    }
    
    window
        .flags(window_flags)
        .draw_background(!config.hide_background)
        .scroll_bar(config.show_scrollbar)
        .size([timeline_width, 600.0], Condition::FirstUseEver)
        .title_bar(false)
        .collapsible(false)
        .build(ui, || {
            if ui.is_window_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
                ui.open_popup("window_context_menu");
            }
            ui.popup("window_context_menu", || {
                let is_locked = config.is_window_locked;
                if MenuItem::new("Lock Window").selected(is_locked).build(ui) {
                    config.is_window_locked = !is_locked;
                }
                
                let hide_bg = config.hide_background;
                if MenuItem::new("Hide Background").selected(hide_bg).build(ui) {
                    config.hide_background = !hide_bg;
                }

                let show_sb = config.show_scrollbar;
                if MenuItem::new("Show Scrollbar").selected(show_sb).build(ui) {
                    config.show_scrollbar = !show_sb;
                }
            });
            
            // Create a child window for proper scrolling and clipping
            let content_width = ui.content_region_avail()[0];
            let content_height = ui.content_region_avail()[1];
            
            if config.show_time_ruler {
                render_time_ruler(ui, current_time, view_range, time_position);
            }
            
            let _style_token = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
            
            // Build ordered category list
            let mut rendered_categories: HashSet<String> = HashSet::new();
            let ordered_categories = config.category_order.clone();
            
            // First render categories in the defined order
            for category in &ordered_categories {
                if rendered_categories.contains(category) {
                    continue;
                }
                
                render_tracks_for_category(
                    ui,
                    &config,
                    category,
                    &mut rendered_categories,
                    show_headers,
                    spacing_same,
                    spacing_between,
                    current_time,
                    time_before_current,
                    time_after_current,
                    view_range,
                    time_position,
                    global_bg,
                    global_padding,
                    override_all_track_heights,
                    global_track_height,
                    draw_event_borders,
                    event_border_color,
                    event_border_thickness,
                );
            }
            
            // Then render any tracks with categories not in the order (shouldn't happen but safety)
            for track in config.tracks.iter() {
                if !rendered_categories.contains(&track.category) && track.visible {
                    let is_category_visible = *config.category_visibility.get(&track.category).unwrap_or(&true);
                    if is_category_visible {
                        render_tracks_for_category(
                            ui,
                            &config,
                            &track.category,
                            &mut rendered_categories,
                            show_headers,
                            spacing_same,
                            spacing_between,
                            current_time,
                            time_before_current,
                            time_after_current,
                            view_range,
                            time_position,
                            global_bg,
                            global_padding,
                            override_all_track_heights,
                            global_track_height,
                            draw_event_borders,
                            event_border_color,
                            event_border_thickness,
                        );
                    }
                }
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn render_tracks_for_category(
    ui: &Ui,
    config: &parking_lot::MutexGuard<crate::config::RuntimeConfig>,
    category: &str,
    rendered_categories: &mut HashSet<String>,
    show_headers: bool,
    spacing_same: f32,
    spacing_between: f32,
    current_time: i64,
    time_before_current: f32,
    time_after_current: f32,
    view_range: f32,
    time_position: f32,
    global_bg: [f32; 4],
    global_padding: f32,
    override_all_track_heights: bool,
    global_track_height: f32,
    draw_event_borders: bool,
    event_border_color: [f32; 4],
    event_border_thickness: f32,
) {
    if rendered_categories.contains(category) {
        return;
    }
    
    let is_category_visible = *config.category_visibility.get(category).unwrap_or(&true);
    if !is_category_visible {
        rendered_categories.insert(category.to_string());
        return;
    }
    
    let mut first_visible_in_category = true;
    let needs_spacing = !rendered_categories.is_empty();

    for track in config.tracks.iter() {
        if track.category != category || !track.visible {
            continue;
        }

        if first_visible_in_category {
            if needs_spacing {
                ui.dummy([0.0, spacing_between]);
            }

            if show_headers && !category.is_empty() {
                render_category_header(ui, category);
            }
            
            first_visible_in_category = false;
        } else {
            ui.dummy([0.0, spacing_same]);
        }

        render_timeline_track(
            ui,
            track,
            current_time,
            time_before_current,
            time_after_current,
            view_range,
            time_position,
            global_bg,
            global_padding,
            override_all_track_heights,
            global_track_height,
            draw_event_borders,
            event_border_color,
            event_border_thickness,
        );
    }
    
    rendered_categories.insert(category.to_string());
}

// FIXED: Use window draw list which automatically clips to window bounds
fn render_category_header(ui: &Ui, category: &str) {
    let cursor_pos = ui.cursor_screen_pos();
    let available_width = ui.content_region_avail()[0];
    let text_size = ui.calc_text_size(category);
    
    let padding = 2.0;
    let rect_height = text_size[1] + padding * 2.0;

    // Use window draw list - it automatically clips to the child window's scroll region
    let draw_list = ui.get_window_draw_list();
    
    let rect_start = [cursor_pos[0], cursor_pos[1]];
    let rect_end = [cursor_pos[0] + available_width, cursor_pos[1] + rect_height];
    
    draw_list.add_rect(rect_start, rect_end, [0.0, 0.0, 0.0, 0.6])
        .filled(true)
        .build();

    let text_pos = [
        cursor_pos[0] + (available_width - text_size[0]) / 2.0,
        cursor_pos[1] + padding
    ];
    draw_list.add_text(text_pos, [0.9, 0.9, 0.9, 1.0], category);
    
    ui.dummy([0.0, rect_height]);
}

/// Chooses black or white text color based on the background luminance.
fn get_text_color_for_bg(bg_color: [f32; 4]) -> [f32; 4] {
    let r = bg_color[0];
    let g = bg_color[1];
    let b = bg_color[2];

    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;

    if luminance > 0.5 {
        [0.0, 0.0, 0.0, 1.0] 
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}

// Pass precalculated time values instead of recalculating
#[allow(clippy::too_many_arguments)]
fn render_timeline_track(
    ui: &Ui,
    track: &EventTrack,
    current_time: i64,
    time_before_current: f32,
    time_after_current: f32,
    view_range: f32,
    time_position: f32,
    global_bg: [f32; 4],
    global_padding: f32,
    override_all_track_heights: bool,
    global_track_height: f32,
    draw_event_borders: bool,
    event_border_color: [f32; 4],
    event_border_thickness: f32,
) {
    let draw_list = ui.get_window_draw_list();
    let cursor_pos = ui.cursor_screen_pos();
    let available_width = ui.content_region_avail()[0];

    let visual = get_track_visual_config(&track.name, global_bg, global_padding);

    let track_height = if override_all_track_heights {
        global_track_height
    } else {
        track.height
    };

    // Background
    draw_list
        .add_rect(
            [cursor_pos[0] - visual.padding, cursor_pos[1] - visual.padding],
            [cursor_pos[0] + available_width + visual.padding, cursor_pos[1] + track_height + visual.padding],
            visual.background_color,
        )
        .filled(true)
        .build();

    // Pre-calculate common values
    let elapsed_since_base = current_time - track.base_time;
    let pixels_per_second = available_width / view_range;

    for event in &track.events {
        if !event.enabled {
            continue;
        }

        let time_in_cycle = elapsed_since_base.rem_euclid(event.cycle_duration);
        let event_start_in_cycle = event.start_offset;
        let time_to_event_start = event_start_in_cycle - time_in_cycle;

        // Static array instead of Vec allocation
        let offsets = [
            time_to_event_start,
            time_to_event_start + event.cycle_duration,
            time_to_event_start - event.cycle_duration,
        ];

        for &time_offset in &offsets {
            // Early exit optimization
            if time_offset < -time_before_current as i64 - event.duration 
                || time_offset > time_after_current as i64 {
                continue;
            }

            let x_offset = (time_offset as f32 + time_before_current) * pixels_per_second;
            let event_width = event.duration as f32 * pixels_per_second;

            let event_start_x = cursor_pos[0] + x_offset;
            let event_end_x = event_start_x + event_width;

            if event_start_x >= cursor_pos[0] + available_width || event_end_x <= cursor_pos[0] {
                continue;
            }

            let is_active = time_in_cycle >= event.start_offset 
                && time_in_cycle < event.start_offset + event.duration;
            let is_this_occurrence_active = is_active && time_offset == time_to_event_start;
            
            let bar_color = if is_this_occurrence_active {
                event.color.to_array()
            } else {
                [
                    event.color.r * 0.5,
                    event.color.g * 0.5,
                    event.color.b * 0.5,
                    event.color.a,
                ]
            };
            
            let bar_min = [event_start_x.max(cursor_pos[0]), cursor_pos[1]];
            let bar_max = [
                event_end_x.min(cursor_pos[0] + available_width),
                cursor_pos[1] + track_height,
            ];

            draw_list.add_rect(bar_min, bar_max, bar_color).filled(true).build();
            
            if draw_event_borders {
                draw_list.add_rect(bar_min, bar_max, event_border_color)
                    .thickness(event_border_thickness)
                    .build();
            }
            
            // Use window bounds in screen space for clipping (accounts for scroll automatically)
            let window_pos = ui.window_pos();
            let window_size = ui.window_size();
            
            let window_clip_min = [window_pos[0], window_pos[1]];
            let window_clip_max = [window_pos[0] + window_size[0], window_pos[1] + window_size[1]];
            
            // Intersect event bar bounds with window bounds for text clipping
            let text_clip_min = [bar_min[0].max(window_clip_min[0]), bar_min[1].max(window_clip_min[1])];
            let text_clip_max = [bar_max[0].min(window_clip_max[0]), bar_max[1].min(window_clip_max[1])];
            
            draw_list.with_clip_rect(text_clip_min, text_clip_max, || {
                let text_color = get_text_color_for_bg(bar_color);
                let text_size = ui.calc_text_size(&event.name);
                let text_pos = [
                    event_start_x + 5.0,
                    cursor_pos[1] + (track_height - text_size[1]) / 2.0,
                ];
                draw_list.add_text(text_pos, text_color, &event.name);
            });
        }
    }

    // Current time line
    let current_time_x = cursor_pos[0] + (time_position * available_width);
    draw_list.add_line(
        [current_time_x, cursor_pos[1]],
        [current_time_x, cursor_pos[1] + track_height],
        [1.0, 0.0, 0.0, 1.0],
    )
    .thickness(2.0)
    .build();

    ui.dummy([available_width, track_height]);

    // Tooltip handling
    if ui.is_item_hovered() {
        handle_track_tooltip(ui, track, current_time, time_before_current, time_after_current, 
                           view_range, cursor_pos, available_width, pixels_per_second);
    }
}

// Extract tooltip logic to separate function
#[allow(clippy::too_many_arguments)]
fn handle_track_tooltip(
    ui: &Ui,
    track: &EventTrack,
    current_time: i64,
    time_before_current: f32,
    time_after_current: f32,
    _view_range: f32,
    cursor_pos: [f32; 2],
    _available_width: f32,
    pixels_per_second: f32,
) {
    let mouse_pos = ui.io().mouse_pos;
    let mouse_x = mouse_pos[0];
    let elapsed_since_base = current_time - track.base_time;

    for event in &track.events {
        if !event.enabled {
            continue;
        }

        let time_in_cycle = elapsed_since_base.rem_euclid(event.cycle_duration);
        let time_to_event_start = event.start_offset - time_in_cycle;

        let offsets = [
            time_to_event_start,
            time_to_event_start + event.cycle_duration,
            time_to_event_start - event.cycle_duration,
        ];

        for &time_offset in &offsets {
            if time_offset < -time_before_current as i64 - event.duration 
                || time_offset > time_after_current as i64 {
                continue;
            }

            let x_offset = (time_offset as f32 + time_before_current) * pixels_per_second;
            let event_width = event.duration as f32 * pixels_per_second;

            let event_start_x = cursor_pos[0] + x_offset;
            let event_end_x = event_start_x + event_width;

            if mouse_x >= event_start_x && mouse_x <= event_end_x {
                // Calculate time info for THIS specific occurrence bar
                let this_occurrence_start = current_time + time_offset;
                let this_occurrence_end = this_occurrence_start + event.duration;
                
                // Determine display text based on timing
                let (timing_text, is_active_now) = if current_time >= this_occurrence_start && current_time < this_occurrence_end {
                    // Currently active
                    let seconds_remaining = this_occurrence_end - current_time;
                    let minutes_remaining = (seconds_remaining / 60) as i32;
                    (format!("Active now ({}m remaining)", minutes_remaining), true)
                } else if this_occurrence_start > current_time {
                    // Future occurrence
                    let seconds_until = this_occurrence_start - current_time;
                    let minutes_until = (seconds_until / 60) as i32;
                    (format!("Starts: {} (in {}m)", format_time_only(this_occurrence_start), minutes_until), false)
                } else {
                    // Past occurrence
                    (format!("Ended: {}", format_time_only(this_occurrence_end)), false)
                };

                ui.tooltip(|| {
                    ui.text(format!("Track: {}", track.name));
                    ui.text(format!("Event: {}", event.name));
                    ui.separator();
                    ui.text(&timing_text);
                    if !event.copy_text.is_empty() {
                        ui.separator();
                        ui.text(format!("Click to copy: {}", event.copy_text));
                    }
                });

                if ui.is_mouse_clicked(MouseButton::Left) && !event.copy_text.is_empty() {
                    ui.set_clipboard_text(&event.copy_text);
                }

                return; // Found event, exit early
            }
        }
    }

    // No event found, show track name
    ui.tooltip_text(&track.name);
}