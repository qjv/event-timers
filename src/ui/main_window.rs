use crate::config::{
    category_expansion_color, get_track_visual_config, EventColorMode, LabelColumnPosition,
    TextAlignment, TrackedEventId, RUNTIME_CONFIG,
};
use crate::json_loader::EventTrack;
use crate::notification_logic::{
    toggle_event_favorite, toggle_event_tracking, toggle_oneshot_tracking,
};
use crate::notifications::NOTIFICATION_STATE;
use crate::time_utils::{format_time_only, get_current_unix_time};
use crate::ui::time_ruler::render_time_ruler;
use nexus::imgui::{
    ChildWindow, Condition, Key, MenuItem, MouseButton, StyleVar, Ui, Window, WindowFlags,
    WindowHoveredFlags,
};
use std::cell::RefCell;
use std::collections::{HashMap as StdHashMap, HashSet, HashSet as StdHashSet};

#[derive(Clone)]
struct ContextEventInfo {
    track_name: String,
    event_name: String,
    display_name: String,
    is_tracked: bool,
    is_oneshot: bool,
    is_favorite: bool,
    is_done_today: bool,
}

thread_local! {
    static CONTEXT_EVENT: RefCell<Option<ContextEventInfo>> = const { RefCell::new(None) };
    static OPEN_EVENT_MENU: RefCell<bool> = const { RefCell::new(false) };
    static PENDING_TRACK_TOGGLE: RefCell<Option<(String, String, bool)>> = const { RefCell::new(None) };
    static PENDING_FAVORITE_TOGGLE: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    static PENDING_DONE_TOGGLE: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    static PENDING_WIKI_OPEN: RefCell<Option<String>> = const { RefCell::new(None) };
    static CACHED_TRACKED_BY_TRACK: RefCell<StdHashMap<String, StdHashSet<String>>> = RefCell::new(StdHashMap::new());
    static CACHED_ONESHOT_BY_TRACK: RefCell<StdHashMap<String, StdHashSet<String>>> = RefCell::new(StdHashMap::new());
    static CACHED_FAVORITE_BY_TRACK: RefCell<StdHashMap<String, StdHashSet<String>>> = RefCell::new(StdHashMap::new());
    static CACHED_COPY_WITH_EVENT_NAME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CACHED_COLOR_MODE: std::cell::Cell<EventColorMode> = const { std::cell::Cell::new(EventColorMode::Default) };
    static ESC_WAS_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn render_main_window(ui: &Ui) {
    let pending = PENDING_TRACK_TOGGLE.with(|p| p.borrow_mut().take());
    if let Some((track_name, event_name, is_oneshot)) = pending {
        if is_oneshot {
            toggle_oneshot_tracking(&track_name, &event_name);
        } else {
            toggle_event_tracking(&track_name, &event_name);
        }
    }

    let pending_favorite = PENDING_FAVORITE_TOGGLE.with(|p| p.borrow_mut().take());
    if let Some((track_name, event_name)) = pending_favorite {
        toggle_event_favorite(&track_name, &event_name);
    }

    let pending_done = PENDING_DONE_TOGGLE.with(|p| p.borrow_mut().take());
    if let Some((track_name, event_name)) = pending_done {
        crate::completion::toggle_manual_completion(&track_name, &event_name);
        NOTIFICATION_STATE.lock().force_refresh();
    }

    let wiki_event = PENDING_WIKI_OPEN.with(|p| p.borrow_mut().take());
    if let Some(event_name) = wiki_event {
        let search_query = event_name.replace(' ', "+");
        let url = format!("https://wiki.guildwars2.com/wiki/?search={}", search_query);
        let _ = open::that(url);
    }

    // Always update ESC_WAS_DOWN every frame so releasing ESC after closing is never missed
    let esc_down = ui.is_key_down(Key::Escape);
    let was_down = ESC_WAS_DOWN.with(|c| {
        let prev = c.get();
        c.set(esc_down);
        prev
    });

    let mut config = RUNTIME_CONFIG.lock();

    if config.close_on_escape && config.show_main_window && esc_down && !was_down {
        config.show_main_window = false;
    }

    if !config.show_main_window {
        return;
    }

    CACHED_TRACKED_BY_TRACK.with(|c| {
        *c.borrow_mut() = to_track_event_map(&config.tracked_events);
    });
    CACHED_ONESHOT_BY_TRACK.with(|c| {
        *c.borrow_mut() = to_track_event_map(&config.oneshot_events);
    });
    CACHED_FAVORITE_BY_TRACK.with(|c| {
        *c.borrow_mut() = to_track_event_map(&config.favorite_events);
    });

    CACHED_COPY_WITH_EVENT_NAME.with(|c| c.set(config.copy_with_event_name));
    CACHED_COLOR_MODE.with(|c| c.set(config.event_color_mode));

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
    let use_sticky_ruler = config.show_time_ruler && config.sticky_time_ruler;

    let current_time = get_current_unix_time();
    let time_before_current = view_range * time_position;
    let time_after_current = view_range * (1.0 - time_position);

    let mut window_flags = WindowFlags::empty();
    if config.is_window_locked {
        window_flags |= WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE;
    }
    if use_sticky_ruler {
        window_flags |= WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SCROLL_WITH_MOUSE;
    }

    let mut window = Window::new("Event Timers");
    if config.is_window_locked {
        window = window.title_bar(false);
    }

    window
        .flags(window_flags)
        .draw_background(!config.hide_background)
        .scroll_bar(!use_sticky_ruler && config.show_scrollbar)
        .size([timeline_width, 600.0], Condition::FirstUseEver)
        .title_bar(false)
        .collapsible(false)
        .build(ui, || {
            if !config.setup_onboarding_seen {
                ui.open_popup("event_timers_onboarding");
            }

            if config.show_time_ruler {
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

            let render_body = |ui: &Ui| {
                let _style_token = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));

                match label_column_pos {
                    LabelColumnPosition::None => {
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
                            false,
                        );
                    }
                    LabelColumnPosition::Left => {
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
            };

            if use_sticky_ruler {
                let mut child_flags = WindowFlags::empty();
                if config.is_window_locked {
                    child_flags |= WindowFlags::NO_MOVE;
                }
                ChildWindow::new("timeline_scroll_region")
                    .size([0.0, 0.0])
                    .flags(child_flags)
                    .scroll_bar(config.show_scrollbar)
                    .build(ui, || {
                        render_body(ui);
                    });
            } else {
                render_body(ui);
            }

            // Handle context menu triggers after rendering tracks so event right-clicks take priority
            let should_open_event_menu = OPEN_EVENT_MENU.with(|f| {
                let val = *f.borrow();
                *f.borrow_mut() = false;
                val
            });

            if should_open_event_menu {
                ui.open_popup("event_track_menu");
            } else if ui.is_window_hovered_with_flags(WindowHoveredFlags::CHILD_WINDOWS)
                && ui.is_mouse_clicked(MouseButton::Right)
            {
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

                let show_ruler = config.show_time_ruler;
                if MenuItem::new("Show Time Ruler")
                    .selected(show_ruler)
                    .build(ui)
                {
                    config.show_time_ruler = !show_ruler;
                }

                if config.show_time_ruler {
                    let sticky = config.sticky_time_ruler;
                    if MenuItem::new("Sticky Time Ruler")
                        .selected(sticky)
                        .build(ui)
                    {
                        config.sticky_time_ruler = !sticky;
                    }
                }

                let show_sb = config.show_scrollbar;
                if MenuItem::new("Show Scrollbar").selected(show_sb).build(ui) {
                    config.show_scrollbar = !show_sb;
                }

                ui.separator();

                let upcoming = config.notification_config.upcoming_panel_enabled;
                if MenuItem::new("Show Upcoming Events Panel")
                    .selected(upcoming)
                    .build(ui)
                {
                    config.notification_config.upcoming_panel_enabled = !upcoming;
                }
            });

            ui.popup("event_track_menu", || {
                CONTEXT_EVENT.with(|e| {
                    if let Some(info) = e.borrow().clone() {
                        ui.text_disabled(format!("{}: {}", info.track_name, info.display_name));
                        ui.separator();

                        let label = if info.is_tracked {
                            format!("Untrack: {}", info.display_name)
                        } else {
                            format!("Track: {}", info.display_name)
                        };

                        if MenuItem::new(&label).build(ui) {
                            PENDING_TRACK_TOGGLE.with(|p| {
                                *p.borrow_mut() =
                                    Some((info.track_name.clone(), info.event_name.clone(), false));
                            });
                        }

                        if !info.is_tracked {
                            let oneshot_label = if info.is_oneshot {
                                format!("Cancel One-shot: {}", info.display_name)
                            } else {
                                format!("Track Next Only: {}", info.display_name)
                            };

                            if MenuItem::new(&oneshot_label).build(ui) {
                                PENDING_TRACK_TOGGLE.with(|p| {
                                    *p.borrow_mut() = Some((
                                        info.track_name.clone(),
                                        info.event_name.clone(),
                                        true,
                                    ));
                                });
                            }
                        }

                        let favorite_label = if info.is_favorite {
                            format!("Unfavorite: {}", info.display_name)
                        } else {
                            format!("Favorite: {}", info.display_name)
                        };

                        if MenuItem::new(&favorite_label).build(ui) {
                            PENDING_FAVORITE_TOGGLE.with(|p| {
                                *p.borrow_mut() =
                                    Some((info.track_name.clone(), info.event_name.clone()));
                            });
                        }

                        let done_label = if info.is_done_today {
                            "Unmark Done Today"
                        } else {
                            "Mark as Done Today"
                        };
                        if MenuItem::new(done_label).build(ui) {
                            PENDING_DONE_TOGGLE.with(|p| {
                                *p.borrow_mut() =
                                    Some((info.track_name.clone(), info.event_name.clone()));
                            });
                        }

                        ui.separator();

                        if MenuItem::new(format!("Open Wiki: {}", info.event_name)).build(ui) {
                            PENDING_WIKI_OPEN.with(|p| {
                                *p.borrow_mut() = Some(info.event_name.clone());
                            });
                        }
                    }
                });
            });

            ui.popup("event_timers_onboarding", || {
                ui.text("Event Timers setup");
                ui.separator();
                ui.text("Right-click events to track, favorite, or mark them done for today.");
                ui.text("Left-click an event to copy its waypoint to the clipboard.");
                ui.text("Tracked events can show notifications and appear in the upcoming panel.");
                ui.text("Favorites get a gold highlight in the main timeline.");
                ui.spacing();
                if ui.button("Got it") {
                    config.setup_onboarding_seen = true;
                    ui.close_current_popup();
                }
            });
        });
}

fn to_track_event_map(
    source: &StdHashSet<TrackedEventId>,
) -> StdHashMap<String, StdHashSet<String>> {
    let mut map: StdHashMap<String, StdHashSet<String>> = StdHashMap::new();
    for id in source {
        map.entry(id.track_name.clone())
            .or_default()
            .insert(id.event_name.clone());
    }
    map
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
    label_column_active: bool,
) {
    let mut rendered_categories: HashSet<String> = HashSet::new();
    let ordered_categories = config.category_order.clone();

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

    for track in config.tracks.iter() {
        if !rendered_categories.contains(&track.category) && track.visible {
            let is_category_visible = *config
                .category_visibility
                .get(&track.category)
                .unwrap_or(&true);
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
    ui.columns(2, "label_timeline_cols", false);
    ui.set_column_width(0, label_column_width);

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
        true,
    );

    ui.columns(1, "", false);
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
    let timeline_width = (available_width - label_column_width).max(50.0);

    ui.columns(2, "timeline_label_cols", false);
    ui.set_column_width(0, timeline_width);

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
        true,
    );

    ui.next_column();

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

    ui.columns(1, "", false);
}

#[allow(clippy::too_many_arguments)]
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

    for track in config.tracks.iter() {
        if !rendered_categories.contains(&track.category) && track.visible {
            let is_category_visible = *config
                .category_visibility
                .get(&track.category)
                .unwrap_or(&true);
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

#[allow(clippy::too_many_arguments)]
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
                let cursor_pos = ui.cursor_screen_pos();
                let available_width = ui.content_region_avail()[0];
                let text_size = ui.calc_text_size(category);
                let header_height = text_size[1] + 10.0;

                if label_bg_color[3] > 0.0 {
                    draw_list
                        .add_rect(
                            cursor_pos,
                            [
                                cursor_pos[0] + available_width,
                                cursor_pos[1] + header_height,
                            ],
                            label_bg_color,
                        )
                        .filled(true)
                        .build();
                }

                if label_show_category {
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

        let track_height = if override_all_track_heights {
            global_track_height
        } else {
            track.height
        };

        let cursor_pos = ui.cursor_screen_pos();
        let available_width = ui.content_region_avail()[0];

        if label_bg_color[3] > 0.0 {
            draw_list
                .add_rect(
                    cursor_pos,
                    [
                        cursor_pos[0] + available_width,
                        cursor_pos[1] + track_height,
                    ],
                    label_bg_color,
                )
                .filled(true)
                .build();
        }

        if label_show_track {
            let text_size = ui.calc_text_size(&track.name);
            let text_y_offset = (track_height - text_size[1]) / 2.0;
            let text_pos = [cursor_pos[0] + 5.0, cursor_pos[1] + text_y_offset];
            draw_list.add_text(text_pos, label_text_color, &track.name);
        }

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
    label_column_active: bool,
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

            if show_headers && !category.is_empty() && !label_column_active {
                render_category_header(ui, category, header_alignment, header_padding);
            } else if show_headers && !category.is_empty() && label_column_active {
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

    let x_offset = match alignment {
        TextAlignment::Left => padding,
        TextAlignment::Center => (available_width - text_size[0]) / 2.0,
        TextAlignment::Right => available_width - text_size[0] - padding,
    };

    let draw_list = ui.get_window_draw_list();
    let cursor_pos = ui.cursor_screen_pos();
    let header_height = text_size[1] + 10.0;

    draw_list
        .add_rect(
            cursor_pos,
            [
                cursor_pos[0] + available_width,
                cursor_pos[1] + header_height,
            ],
            [0.15, 0.15, 0.15, 0.8],
        )
        .filled(true)
        .build();

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

    draw_list
        .add_rect(
            [
                cursor_pos[0] - visual.padding,
                cursor_pos[1] - visual.padding,
            ],
            [
                cursor_pos[0] + available_width + visual.padding,
                cursor_pos[1] + track_height + visual.padding,
            ],
            visual.background_color,
        )
        .filled(true)
        .build();

    let elapsed_since_base = current_time - track.base_time;
    let pixels_per_second = available_width / view_range;
    let color_mode = CACHED_COLOR_MODE.with(|c| c.get());

    for event in &track.events {
        if !event.enabled || event.cycle_duration <= 0 || event.duration <= 0 {
            continue;
        }

        let is_tracked = CACHED_TRACKED_BY_TRACK.with(|c| {
            c.borrow()
                .get(&track.name)
                .is_some_and(|set| set.contains(event.name.as_str()))
        });
        let is_oneshot = CACHED_ONESHOT_BY_TRACK.with(|c| {
            c.borrow()
                .get(&track.name)
                .is_some_and(|set| set.contains(event.name.as_str()))
        });
        let is_favorite = CACHED_FAVORITE_BY_TRACK.with(|c| {
            c.borrow()
                .get(&track.name)
                .is_some_and(|set| set.contains(event.name.as_str()))
        });
        let completed = crate::completion::is_completed(&track.name, &event.name);
        let disp_name = event.display_name();

        let time_in_cycle = elapsed_since_base.rem_euclid(event.cycle_duration);
        let event_start_in_cycle = event.start_offset.rem_euclid(event.cycle_duration);
        let time_to_event_start = event_start_in_cycle - time_in_cycle;

        let offsets = [
            time_to_event_start,
            time_to_event_start + event.cycle_duration,
            time_to_event_start - event.cycle_duration,
        ];

        let base_color = match color_mode {
            EventColorMode::Default => event.color.to_array(),
            EventColorMode::CategoryExpansion => {
                if track.category == "Time Cycles" {
                    event.color.to_array()
                } else {
                    category_expansion_color(&track.category)
                }
            }
            EventColorMode::MonochromeHighlight => {
                if is_tracked || is_oneshot || is_favorite {
                    event.color.to_array()
                } else {
                    let lum =
                        (0.299 * event.color.r + 0.587 * event.color.g + 0.114 * event.color.b)
                            .clamp(0.22, 0.42);
                    [lum, lum, lum, event.color.a]
                }
            }
        };

        for &time_offset in &offsets {
            if time_offset < -time_before_current as i64 - event.duration
                || time_offset > time_after_current as i64
            {
                continue;
            }

            let x_offset = (time_offset as f32 + time_before_current) * pixels_per_second;
            let event_width = event.duration as f32 * pixels_per_second;

            let event_start_x = cursor_pos[0] + x_offset;
            let event_end_x = event_start_x + event_width;

            if event_start_x >= cursor_pos[0] + available_width || event_end_x <= cursor_pos[0] {
                continue;
            }

            // Works seamlessly even when an event spans across 00:00 cycle wrap
            let is_this_occurrence_active = time_offset <= 0 && time_offset + event.duration > 0;

            let mut bar_color = if is_this_occurrence_active {
                base_color
            } else {
                [
                    base_color[0] * 0.5,
                    base_color[1] * 0.5,
                    base_color[2] * 0.5,
                    base_color[3],
                ]
            };
            if completed {
                bar_color = [
                    bar_color[0] * 0.45,
                    bar_color[1] * 0.45,
                    bar_color[2] * 0.45,
                    bar_color[3],
                ];
            }

            let bar_min = [event_start_x.max(cursor_pos[0]), cursor_pos[1]];
            let bar_max = [
                event_end_x.min(cursor_pos[0] + available_width),
                cursor_pos[1] + track_height,
            ];

            draw_list
                .add_rect(bar_min, bar_max, bar_color)
                .filled(true)
                .build();

            if draw_event_borders {
                draw_list
                    .add_rect(bar_min, bar_max, event_border_color)
                    .thickness(event_border_thickness)
                    .build();
            }

            if is_tracked || is_oneshot {
                let highlight_color = if is_oneshot {
                    [1.0, 0.65, 0.2, 1.0]
                } else {
                    [0.35, 0.8, 1.0, 1.0]
                };
                draw_list
                    .add_rect(bar_min, bar_max, highlight_color)
                    .thickness(event_border_thickness.max(2.0))
                    .build();
            }

            if is_favorite && bar_max[0] - bar_min[0] > 6.0 && bar_max[1] - bar_min[1] > 6.0 {
                draw_list
                    .add_rect(
                        [bar_min[0] + 2.0, bar_min[1] + 2.0],
                        [bar_max[0] - 2.0, bar_max[1] - 2.0],
                        [1.0, 0.82, 0.18, 1.0],
                    )
                    .thickness(event_border_thickness.max(2.5))
                    .build();
            }

            let window_pos = ui.window_pos();
            let window_size = ui.window_size();

            let window_clip_min = [window_pos[0], window_pos[1]];
            let window_clip_max = [
                window_pos[0] + window_size[0],
                window_pos[1] + window_size[1],
            ];

            let text_clip_min = [
                bar_min[0].max(window_clip_min[0]),
                bar_min[1].max(window_clip_min[1]),
            ];
            let text_clip_max = [
                bar_max[0].min(window_clip_max[0]),
                bar_max[1].min(window_clip_max[1]),
            ];

            draw_list.with_clip_rect(text_clip_min, text_clip_max, || {
                let text_color = if completed {
                    [0.72, 0.72, 0.72, 1.0]
                } else {
                    get_text_color_for_bg(bar_color)
                };
                let text_size = ui.calc_text_size(disp_name);
                let text_pos = [
                    event_start_x + 5.0,
                    cursor_pos[1] + (track_height - text_size[1]) / 2.0,
                ];
                draw_list.add_text(text_pos, text_color, disp_name);
                if completed {
                    let strike_start = text_pos[0].max(text_clip_min[0]);
                    let strike_end = (text_pos[0] + text_size[0]).min(text_clip_max[0]);
                    if strike_end > strike_start {
                        let strike_y = text_pos[1] + text_size[1] * 0.5;
                        draw_list
                            .add_line(
                                [strike_start, strike_y],
                                [strike_end, strike_y],
                                [0.82, 0.82, 0.82, 0.95],
                            )
                            .thickness(1.5)
                            .build();
                    }
                }
            });
        }
    }

    // Current time line
    let current_time_x = cursor_pos[0] + (time_position * available_width);
    draw_list
        .add_line(
            [current_time_x, cursor_pos[1]],
            [current_time_x, cursor_pos[1] + track_height],
            [1.0, 0.0, 0.0, 1.0],
        )
        .thickness(2.0)
        .build();

    ui.dummy([available_width, track_height]);

    if ui.is_item_hovered() {
        handle_track_tooltip(
            ui,
            track,
            current_time,
            time_before_current,
            time_after_current,
            view_range,
            cursor_pos,
            available_width,
            pixels_per_second,
        );
    }
}

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
        if !event.enabled || event.cycle_duration <= 0 || event.duration <= 0 {
            continue;
        }

        let time_in_cycle = elapsed_since_base.rem_euclid(event.cycle_duration);
        let time_to_event_start =
            event.start_offset.rem_euclid(event.cycle_duration) - time_in_cycle;

        let offsets = [
            time_to_event_start,
            time_to_event_start + event.cycle_duration,
            time_to_event_start - event.cycle_duration,
        ];

        for &time_offset in &offsets {
            if time_offset < -time_before_current as i64 - event.duration
                || time_offset > time_after_current as i64
            {
                continue;
            }

            let x_offset = (time_offset as f32 + time_before_current) * pixels_per_second;
            let event_width = event.duration as f32 * pixels_per_second;

            let event_start_x = cursor_pos[0] + x_offset;
            let event_end_x = event_start_x + event_width;

            if mouse_x >= event_start_x && mouse_x <= event_end_x {
                let this_occurrence_start = current_time + time_offset;
                let this_occurrence_end = this_occurrence_start + event.duration;
                let disp_name = event.display_name();
                let completed = crate::completion::is_completed(&track.name, &event.name);

                let timing_text = if current_time >= this_occurrence_start
                    && current_time < this_occurrence_end
                {
                    let seconds_remaining = this_occurrence_end - current_time;
                    let minutes_remaining = (seconds_remaining / 60) as i32;
                    format!("Active now ({}m remaining)", minutes_remaining)
                } else if this_occurrence_start > current_time {
                    let seconds_until = this_occurrence_start - current_time;
                    let minutes_until = (seconds_until / 60) as i32;
                    format!(
                        "Starts: {} (in {}m)",
                        format_time_only(this_occurrence_start),
                        minutes_until
                    )
                } else {
                    format!("Ended: {}", format_time_only(this_occurrence_end))
                };

                ui.tooltip(|| {
                    ui.text(format!("Track: {}", track.name));
                    ui.text(format!("Event: {}", disp_name));
                    ui.separator();
                    ui.text(&timing_text);
                    if completed {
                        ui.text_colored([0.5, 0.9, 0.5, 1.0], "Completed today");
                    }
                    if !event.copy_text.is_empty() {
                        ui.separator();
                        ui.text(format!("Waypoint: {}", event.copy_text));
                        ui.text_disabled("Left-click to copy");
                    }
                    ui.text_disabled("Right-click for options");
                });

                if ui.is_mouse_clicked(MouseButton::Left) && !event.copy_text.is_empty() {
                    let copy_text = CACHED_COPY_WITH_EVENT_NAME.with(|c| {
                        if c.get() {
                            format!("{}: {}", disp_name, event.copy_text)
                        } else {
                            event.copy_text.clone()
                        }
                    });
                    ui.set_clipboard_text(&copy_text);
                }

                if ui.is_mouse_clicked(MouseButton::Right) {
                    let is_tracked = CACHED_TRACKED_BY_TRACK.with(|c| {
                        c.borrow()
                            .get(&track.name)
                            .is_some_and(|set| set.contains(event.name.as_str()))
                    });
                    let is_oneshot = CACHED_ONESHOT_BY_TRACK.with(|c| {
                        c.borrow()
                            .get(&track.name)
                            .is_some_and(|set| set.contains(event.name.as_str()))
                    });
                    let is_favorite = CACHED_FAVORITE_BY_TRACK.with(|c| {
                        c.borrow()
                            .get(&track.name)
                            .is_some_and(|set| set.contains(event.name.as_str()))
                    });
                    let is_done_today =
                        crate::completion::is_manually_completed(&track.name, &event.name);

                    CONTEXT_EVENT.with(|e| {
                        *e.borrow_mut() = Some(ContextEventInfo {
                            track_name: track.name.clone(),
                            event_name: event.name.clone(),
                            display_name: disp_name.to_string(),
                            is_tracked,
                            is_oneshot,
                            is_favorite,
                            is_done_today,
                        });
                    });
                    OPEN_EVENT_MENU.with(|f| {
                        *f.borrow_mut() = true;
                    });
                }

                return;
            }
        }
    }

    ui.tooltip_text(&track.name);
}

fn get_text_color_for_bg(bg_color: [f32; 4]) -> [f32; 4] {
    let luminance = 0.299 * bg_color[0] + 0.587 * bg_color[1] + 0.114 * bg_color[2];
    if luminance > 0.5 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}
