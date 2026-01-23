use crate::config::{get_track_visual_config, LabelColumnPosition, TextAlignment, RUNTIME_CONFIG};
use crate::json_loader::EventTrack;
use crate::notification_logic::{toggle_event_tracking, toggle_oneshot_tracking};
use crate::time_utils::{format_time_only, get_current_unix_time};
use crate::ui::time_ruler::render_time_ruler;
use nexus::imgui::{Condition, Key, MenuItem, MouseButton, StyleVar, Ui, Window, WindowFlags};
use std::cell::RefCell;
use std::collections::HashSet;

use std::collections::HashSet as StdHashSet;
use crate::config::TrackedEventId;

// Thread-local storage for right-clicked event info
// Stores (track_name, event_name, is_currently_tracked, is_oneshot_tracked)
thread_local! {
    static CONTEXT_EVENT: RefCell<Option<(String, String, bool, bool)>> = const { RefCell::new(None) };
    static OPEN_EVENT_MENU: RefCell<bool> = const { RefCell::new(false) };
    static PENDING_TRACK_TOGGLE: RefCell<Option<(String, String, bool)>> = const { RefCell::new(None) }; // (track, event, is_oneshot)
    static PENDING_WIKI_OPEN: RefCell<Option<String>> = const { RefCell::new(None) };
    // Cached tracked events for the current frame (to avoid re-locking)
    static CACHED_TRACKED_EVENTS: RefCell<StdHashSet<TrackedEventId>> = RefCell::new(StdHashSet::new());
    static CACHED_ONESHOT_EVENTS: RefCell<StdHashSet<TrackedEventId>> = RefCell::new(StdHashSet::new());
    // Cached copy setting for the current frame
    static CACHED_COPY_WITH_EVENT_NAME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // Track ESC key state for debouncing
    static ESC_WAS_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn render_main_window(ui: &Ui) {
    // Handle any pending track toggle (must be done before locking config)
    let pending = PENDING_TRACK_TOGGLE.with(|p| p.borrow_mut().take());
    if let Some((track_name, event_name, is_oneshot)) = pending {
        if is_oneshot {
            toggle_oneshot_tracking(&track_name, &event_name);
        } else {
            toggle_event_tracking(&track_name, &event_name);
        }
    }

    // Handle pending wiki open
    let wiki_event = PENDING_WIKI_OPEN.with(|p| p.borrow_mut().take());
    if let Some(event_name) = wiki_event {
        let search_query = event_name.replace(' ', "+");
        let url = format!("https://wiki.guildwars2.com/wiki/?search={}", search_query);
        let _ = open::that(url);
    }

    let mut config = RUNTIME_CONFIG.lock();

    // Handle ESC key to close window (check globally, with debouncing)
    if config.close_on_escape && config.show_main_window {
        let esc_down = ui.is_key_down(Key::Escape);
        let was_down = ESC_WAS_DOWN.with(|c| c.get());

        if esc_down && !was_down {
            config.show_main_window = false;
        }

        ESC_WAS_DOWN.with(|c| c.set(esc_down));
    }

    if !config.show_main_window {
        return;
    }

    // Cache tracked events for this frame (to avoid re-locking in tooltip handler)
    CACHED_TRACKED_EVENTS.with(|c| {
        *c.borrow_mut() = config.tracked_events.clone();
    });
    CACHED_ONESHOT_EVENTS.with(|c| {
        *c.borrow_mut() = config.oneshot_events.clone();
    });

    // Cache copy setting for this frame
    CACHED_COPY_WITH_EVENT_NAME.with(|c| {
        c.set(config.copy_with_event_name);
    });

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
    let header_alignment = config.category_header_alignment;
    let header_padding = config.category_header_padding;
    let label_column_pos = config.label_column_position;
    let label_column_width = config.label_column_width;
    let label_show_category = config.label_column_show_category;
    let label_show_track = config.label_column_show_track;
    let label_text_size = config.label_column_text_size;
    let label_bg_color = config.label_column_bg_color;
    let label_text_color = config.label_column_text_color;
    let label_category_color = config.label_column_category_color;

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
            // Check if we need to open the event tracking menu (set by tooltip handler)
            let should_open_event_menu = OPEN_EVENT_MENU.with(|f| {
                let val = *f.borrow();
                *f.borrow_mut() = false; // Reset flag
                val
            });

            if should_open_event_menu {
                ui.open_popup("event_track_menu");
            } else if ui.is_window_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
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

            // Event tracking context menu
            ui.popup("event_track_menu", || {
                CONTEXT_EVENT.with(|e| {
                    if let Some((track_name, event_name, was_tracked, was_oneshot)) = e.borrow().clone() {
                        // Track/Untrack option
                        let label = if was_tracked {
                            format!("Untrack: {}", event_name)
                        } else {
                            format!("Track: {}", event_name)
                        };

                        if MenuItem::new(&label).build(ui) {
                            PENDING_TRACK_TOGGLE.with(|p| {
                                *p.borrow_mut() = Some((track_name.clone(), event_name.clone(), false));
                            });
                        }

                        // Track Next Only option (only show if not already tracked)
                        if !was_tracked {
                            let oneshot_label = if was_oneshot {
                                format!("Cancel One-shot: {}", event_name)
                            } else {
                                format!("Track Next Only: {}", event_name)
                            };

                            if MenuItem::new(&oneshot_label).build(ui) {
                                PENDING_TRACK_TOGGLE.with(|p| {
                                    *p.borrow_mut() = Some((track_name.clone(), event_name.clone(), true));
                                });
                            }
                        }

                        ui.separator();

                        // Open Wiki option
                        if MenuItem::new(format!("Open Wiki: {}", event_name)).build(ui) {
                            PENDING_WIKI_OPEN.with(|p| {
                                *p.borrow_mut() = Some(event_name.clone());
                            });
                        }
                    }
                });
            });
            
            if config.show_time_ruler {
                // Calculate label offset for time ruler alignment
                let label_offset = match label_column_pos {
                    LabelColumnPosition::Left => label_column_width,
                    _ => 0.0,
                };
                render_time_ruler(
                    ui,
                    current_time,
                    view_range,
                    time_position,
                    label_offset,
                    config.time_ruler_interval,
                    config.time_ruler_show_current_time,
                );
            }
            
            let _style_token = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
            
            // Determine layout based on label column position
            match label_column_pos {
                LabelColumnPosition::None => {
                    // Normal rendering without label column
                    render_timeline_content(
                        ui,
                        &config,
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
                        header_alignment,
                        header_padding,
                        false, // label_column_active = false
                    );
                }
                LabelColumnPosition::Left => {
                    // Label column on left, timeline on right
                    render_with_label_column_left(
                        ui,
                        &config,
                        label_column_width,
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
                        header_alignment,
                        header_padding,
                        label_show_category,
                        label_show_track,
                        label_text_size,
                        label_bg_color,
                        label_text_color,
                        label_category_color,
                    );
                }
                LabelColumnPosition::Right => {
                    // Timeline on left, label column on right
                    render_with_label_column_right(
                        ui,
                        &config,
                        label_column_width,
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
                        header_alignment,
                        header_padding,
                        label_show_category,
                        label_show_track,
                        label_text_size,
                        label_bg_color,
                        label_text_color,
                        label_category_color,
                    );
                }
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn render_timeline_content(
    ui: &Ui,
    config: &parking_lot::MutexGuard<crate::config::RuntimeConfig>,
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
    header_alignment: TextAlignment,
    header_padding: f32,
    label_column_active: bool, // NEW PARAMETER
) {
    let mut rendered_categories: HashSet<String> = HashSet::new();
    let ordered_categories = config.category_order.clone();
    
    // First render categories in the defined order
    for category in &ordered_categories {
        if rendered_categories.contains(category) {
            continue;
        }
        
        render_tracks_for_category(
            ui,
            config,
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
            header_alignment,
            header_padding,
            label_column_active,
        );
    }
    
    // Then render any tracks with categories not in the order
    for track in config.tracks.iter() {
        if !rendered_categories.contains(&track.category) && track.visible {
            let is_category_visible = *config.category_visibility.get(&track.category).unwrap_or(&true);
            if is_category_visible {
                render_tracks_for_category(
                    ui,
                    config,
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
                    header_alignment,
                    header_padding,
                    label_column_active,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_with_label_column_left(
    ui: &Ui,
    config: &parking_lot::MutexGuard<crate::config::RuntimeConfig>,
    label_column_width: f32,
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
    header_alignment: TextAlignment,
    header_padding: f32,
    label_show_category: bool,
    label_show_track: bool,
    label_text_size: f32,
    label_bg_color: [f32; 4],
    label_text_color: [f32; 4],
    label_category_color: [f32; 4],
) {
    // Use columns for side-by-side layout without breaking scrolling
    ui.columns(2, "label_timeline_cols", false);
    ui.set_column_width(0, label_column_width);
    
    // Label column (first column)
    render_label_column(
        ui,
        config,
        show_headers,
        spacing_same,
        spacing_between,
        override_all_track_heights,
        global_track_height,
        label_show_category,
        label_show_track,
        label_text_size,
        label_bg_color,
        label_text_color,
        label_category_color,
    );
    
    ui.next_column();
    
    // Timeline (second column)
    render_timeline_content(
        ui,
        config,
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
        header_alignment,
        header_padding,
        true, // label_column_active = true
    );
    
    ui.columns(1, "", false); // Reset to single column
}

#[allow(clippy::too_many_arguments)]
fn render_with_label_column_right(
    ui: &Ui,
    config: &parking_lot::MutexGuard<crate::config::RuntimeConfig>,
    label_column_width: f32,
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
    header_alignment: TextAlignment,
    header_padding: f32,
    label_show_category: bool,
    label_show_track: bool,
    label_text_size: f32,
    label_bg_color: [f32; 4],
    label_text_color: [f32; 4],
    label_category_color: [f32; 4],
) {
    let available_width = ui.content_region_avail()[0];
    let timeline_width = available_width - label_column_width;
    
    // Use columns for side-by-side layout without breaking scrolling
    ui.columns(2, "timeline_label_cols", false);
    ui.set_column_width(0, timeline_width);
    
    // Timeline (first column)
    render_timeline_content(
        ui,
        config,
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
        header_alignment,
        header_padding,
        true, // label_column_active = true
    );
    
    ui.next_column();
    
    // Label column (second column)
    render_label_column(
        ui,
        config,
        show_headers,
        spacing_same,
        spacing_between,
        override_all_track_heights,
        global_track_height,
        label_show_category,
        label_show_track,
        label_text_size,
        label_bg_color,
        label_text_color,
        label_category_color,
    );
    
    ui.columns(1, "", false); // Reset to single column
}

fn render_label_column(
    ui: &Ui,
    config: &parking_lot::MutexGuard<crate::config::RuntimeConfig>,
    show_headers: bool,
    spacing_same: f32,
    spacing_between: f32,
    override_all_track_heights: bool,
    global_track_height: f32,
    label_show_category: bool,
    label_show_track: bool,
    label_text_size: f32,
    label_bg_color: [f32; 4],
    label_text_color: [f32; 4],
    label_category_color: [f32; 4],
) {
    let mut rendered_categories: HashSet<String> = HashSet::new();
    let ordered_categories = config.category_order.clone();
    let mut needs_spacing = false;
    
    // Render in order
    for category in &ordered_categories {
        if rendered_categories.contains(category) {
            continue;
        }
        
        render_label_column_for_category(
            ui,
            config,
            category,
            &mut rendered_categories,
            show_headers,
            spacing_same,
            spacing_between,
            override_all_track_heights,
            global_track_height,
            &mut needs_spacing,
            label_show_category,
            label_show_track,
            label_text_size,
            label_bg_color,
            label_text_color,
            label_category_color,
        );
    }
    
    // Render remaining categories
    for track in config.tracks.iter() {
        if !rendered_categories.contains(&track.category) && track.visible {
            let is_category_visible = *config.category_visibility.get(&track.category).unwrap_or(&true);
            if is_category_visible {
                render_label_column_for_category(
                    ui,
                    config,
                    &track.category,
                    &mut rendered_categories,
                    show_headers,
                    spacing_same,
                    spacing_between,
                    override_all_track_heights,
                    global_track_height,
                    &mut needs_spacing,
                    label_show_category,
                    label_show_track,
                    label_text_size,
                    label_bg_color,
                    label_text_color,
                    label_category_color,
                );
            }
        }
    }
}

fn render_label_column_for_category(
    ui: &Ui,
    config: &parking_lot::MutexGuard<crate::config::RuntimeConfig>,
    category: &str,
    rendered_categories: &mut HashSet<String>,
    show_headers: bool,
    spacing_same: f32,
    spacing_between: f32,
    override_all_track_heights: bool,
    global_track_height: f32,
    needs_spacing: &mut bool,
    label_show_category: bool,
    label_show_track: bool,
    _label_text_size: f32,
    label_bg_color: [f32; 4],
    label_text_color: [f32; 4],
    label_category_color: [f32; 4],
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
    let draw_list = ui.get_window_draw_list();
    
    for track in config.tracks.iter() {
        if track.category != category || !track.visible {
            continue;
        }
        
        if first_visible_in_category {
            if *needs_spacing {
                ui.dummy([0.0, spacing_between]);
            }
            
            if show_headers && !category.is_empty() {
                // Category header with same height as timeline header
                let cursor_pos = ui.cursor_screen_pos();
                let available_width = ui.content_region_avail()[0];
                let text_size = ui.calc_text_size(category);
                let header_height = text_size[1] + 10.0;
                
                // Background for category (if enabled)
                if label_bg_color[3] > 0.0 {
                    draw_list.add_rect(
                        cursor_pos,
                        [cursor_pos[0] + available_width, cursor_pos[1] + header_height],
                        label_bg_color,
                    ).filled(true).build();
                }
                
                // Category text (if enabled) - uses separate category color
                if label_show_category {
                    // Note: Font scaling in nexus imgui is limited, using regular text
                    let text_pos = [cursor_pos[0] + 5.0, cursor_pos[1] + 5.0];
                    draw_list.add_text(text_pos, label_category_color, category);
                }
                
                ui.dummy([0.0, header_height]);
            }
            
            first_visible_in_category = false;
            *needs_spacing = true;
        } else {
            ui.dummy([0.0, spacing_same]);
        }
        
        // Track label - match exact height of timeline track
        let track_height = if override_all_track_heights {
            global_track_height
        } else {
            track.height
        };
        
        let cursor_pos = ui.cursor_screen_pos();
        let available_width = ui.content_region_avail()[0];
        
        // Draw background matching track background
        if label_bg_color[3] > 0.0 {
            draw_list.add_rect(
                cursor_pos,
                [cursor_pos[0] + available_width, cursor_pos[1] + track_height],
                label_bg_color,
            ).filled(true).build();
        }
        
        // Draw track name (if enabled) - vertically centered
        if label_show_track {
            // Note: Font scaling in nexus imgui is limited, using regular text
            let text_size = ui.calc_text_size(&track.name);
            let text_y_offset = (track_height - text_size[1]) / 2.0;
            let text_pos = [cursor_pos[0] + 5.0, cursor_pos[1] + text_y_offset];
            draw_list.add_text(text_pos, label_text_color, &track.name);
        }
        
        // Dummy with EXACT track height to match timeline
        ui.dummy([available_width, track_height]);
    }
    
    rendered_categories.insert(category.to_string());
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
    header_alignment: TextAlignment,
    header_padding: f32,
    label_column_active: bool, // NEW PARAMETER
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

            // Only show header if label column is NOT active
            if show_headers && !category.is_empty() && !label_column_active {
                render_category_header(ui, category, header_alignment, header_padding);
            } else if show_headers && !category.is_empty() && label_column_active {
                // Just add spacing to match the label column's category header height
                let text_size = ui.calc_text_size(category);
                let header_height = text_size[1] + 10.0;
                ui.dummy([0.0, header_height]);
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

fn render_category_header(ui: &Ui, category: &str, alignment: TextAlignment, padding: f32) {
    let available_width = ui.content_region_avail()[0];
    let text_size = ui.calc_text_size(category);
    
    // Calculate X position based on alignment
    let x_offset = match alignment {
        TextAlignment::Left => padding,
        TextAlignment::Center => (available_width - text_size[0]) / 2.0,
        TextAlignment::Right => available_width - text_size[0] - padding,
    };
    
    // Draw using background draw list for full width coverage
    let draw_list = ui.get_window_draw_list();
    let cursor_pos = ui.cursor_screen_pos();
    let header_height = text_size[1] + 10.0;
    
    // Semi-transparent background
    draw_list
        .add_rect(
            cursor_pos,
            [cursor_pos[0] + available_width, cursor_pos[1] + header_height],
            [0.15, 0.15, 0.15, 0.8],
        )
        .filled(true)
        .build();
    
    // Category text with alignment
    let text_pos = [cursor_pos[0] + x_offset, cursor_pos[1] + 5.0];
    draw_list.add_text(text_pos, [0.8, 0.8, 0.2, 1.0], category);
    
    ui.dummy([available_width, header_height]);
}

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
    let visual = get_track_visual_config(&track.name, global_bg, global_padding);
    let draw_list = ui.get_window_draw_list();
    let cursor_pos = ui.cursor_screen_pos();
    let available_width = ui.content_region_avail()[0];

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
                let (timing_text, _is_active_now) = if current_time >= this_occurrence_start && current_time < this_occurrence_end {
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
                    let copy_text = CACHED_COPY_WITH_EVENT_NAME.with(|c| {
                        if c.get() {
                            format!("{}: {}", event.name, event.copy_text)
                        } else {
                            event.copy_text.clone()
                        }
                    });
                    ui.set_clipboard_text(&copy_text);
                }

                // Right-click to track/untrack event
                if ui.is_mouse_clicked(MouseButton::Right) {
                    // Check tracked status from cached value (avoids deadlock)
                    let event_id = TrackedEventId::new(&track.name, &event.name);
                    let is_tracked = CACHED_TRACKED_EVENTS.with(|c| {
                        c.borrow().contains(&event_id)
                    });
                    let is_oneshot = CACHED_ONESHOT_EVENTS.with(|c| {
                        c.borrow().contains(&event_id)
                    });
                    CONTEXT_EVENT.with(|e| {
                        *e.borrow_mut() = Some((track.name.clone(), event.name.clone(), is_tracked, is_oneshot));
                    });
                    OPEN_EVENT_MENU.with(|f| {
                        *f.borrow_mut() = true;
                    });
                }

                return; // Found event, exit early
            }
        }
    }

    // No event found, show track name
    ui.tooltip_text(&track.name);
}

fn get_text_color_for_bg(bg_color: [f32; 4]) -> [f32; 4] {
    let luminance = 0.299 * bg_color[0] + 0.587 * bg_color[1] + 0.114 * bg_color[2];
    if luminance > 0.5 {
        [0.0, 0.0, 0.0, 1.0] // Black text on bright background
    } else {
        [1.0, 1.0, 1.0, 1.0] // White text on dark background
    }
}