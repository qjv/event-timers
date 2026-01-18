use nexus::imgui::{
    ColorEdit, ColorEditFlags, InputFloat, InputText, Selectable, TableFlags, TreeNodeFlags, Ui, Window,
};
use std::collections::HashSet;
use parking_lot::MutexGuard;

use crate::config::{ToastPosition, TrackedEventId, RUNTIME_CONFIG, SELECTED_EVENT, SELECTED_TRACK, RuntimeConfig};
use crate::json_loader::{load_tracks_from_json, EventColor, EventTrack, TimelineEvent};
use crate::notifications::NOTIFICATION_STATE;

const GITHUB_EVENT_TRACKS_URL: &str = "https://raw.githubusercontent.com/qjv/event-timers/main/event_tracks.json";

pub fn check_for_event_tracks_update() {
    use std::thread;

    thread::spawn(|| {
        let runtime_result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        let runtime = match runtime_result {
            Ok(rt) => rt,
            Err(e) => {
                nexus::log::log(
                    nexus::log::LogLevel::Critical,
                    "Event Timers",
                    &format!("Failed to create Tokio runtime: {}", e)
                );
                return;
            }
        };

        runtime.block_on(async {
            nexus::log::log(
                nexus::log::LogLevel::Info,
                "Event Timers",
                "Checking for event_tracks.json updates from GitHub..."
            );

            match reqwest::get(GITHUB_EVENT_TRACKS_URL).await {
                Ok(response) => {
                    match response.text().await {
                        Ok(github_content) => {
                            let local_path = nexus::paths::get_addon_dir("event_timers")
                                .map(|p| p.join("event_tracks.json"));

                            if let Some(path) = local_path {
                                let needs_update = if path.exists() {
                                    match std::fs::read_to_string(&path) {
                                        Ok(local_content) => local_content != github_content,
                                        Err(_) => true,
                                    }
                                } else {
                                    true
                                };

                                if needs_update {
                                    if path.exists() {
                                        let backup_path = path.with_extension("json.backup");
                                        let _ = std::fs::copy(&path, backup_path);
                                    }

                                    match std::fs::write(&path, github_content) {
                                        Ok(_) => {
                                            nexus::log::log(
                                                nexus::log::LogLevel::Info,
                                                "Event Timers",
                                                "event_tracks.json updated! Reload addon (Ctrl+Shift+L) to apply."
                                            );
                                        }
                                        Err(e) => {
                                            nexus::log::log(
                                                nexus::log::LogLevel::Critical,
                                                "Event Timers",
                                                &format!("Failed to write file: {}", e)
                                            );
                                        }
                                    }
                                } else {
                                    nexus::log::log(
                                        nexus::log::LogLevel::Info,
                                        "Event Timers",
                                        "event_tracks.json is already up to date!"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            nexus::log::log(
                                nexus::log::LogLevel::Critical,
                                "Event Timers",
                                &format!("Failed to read response: {}", e)
                            );
                        }
                    }
                }
                Err(e) => {
                    nexus::log::log(
                        nexus::log::LogLevel::Critical,
                        "Event Timers",
                        &format!("Failed to fetch from GitHub: {}", e)
                    );
                }
            }
        });
    });
}

pub fn render_settings(ui: &Ui) {
    let mut config = RUNTIME_CONFIG.lock();

    ui.text("Event Timers Settings");
    ui.separator();

    // ==================== MAIN WINDOW ====================
    if ui.collapsing_header("Main Window", TreeNodeFlags::DEFAULT_OPEN) {
        ui.indent();

        // --- Timeline ---
        ui.text("Timeline");
        ui.checkbox("Show Time Ruler", &mut config.show_time_ruler);

        let mut view_range_minutes = config.view_range_seconds / 60.0;
        if nexus::imgui::Slider::new("View Range (minutes)", 15.0, 120.0)
            .build(ui, &mut view_range_minutes)
        {
            config.view_range_seconds = view_range_minutes * 60.0;
        }

        nexus::imgui::Slider::new("Current Time Position", 0.0, 0.5)
            .display_format("%.2f")
            .build(ui, &mut config.current_time_position);
        ui.text_disabled("0.0 = Left edge, 0.5 = Center");

        ui.spacing();

        // --- Categories ---
        ui.text("Categories");
        ui.checkbox("Show Category Headers", &mut config.show_category_headers);

        if config.show_category_headers {
            ui.same_line();
            if ui.radio_button("Left##hdr", &mut config.category_header_alignment, crate::config::TextAlignment::Left) {}
            ui.same_line();
            if ui.radio_button("Center##hdr", &mut config.category_header_alignment, crate::config::TextAlignment::Center) {}
            ui.same_line();
            if ui.radio_button("Right##hdr", &mut config.category_header_alignment, crate::config::TextAlignment::Right) {}

            nexus::imgui::Slider::new("Header Padding", 0.0, 50.0)
                .build(ui, &mut config.category_header_padding);
        }

        nexus::imgui::Slider::new("Spacing (Same Category)", 0.0, 20.0)
            .build(ui, &mut config.spacing_same_category);

        nexus::imgui::Slider::new("Spacing (Between Categories)", 0.0, 50.0)
            .build(ui, &mut config.spacing_between_categories);

        ui.spacing();

        // --- Labels ---
        ui.text("Track Labels");
        ui.same_line();
        if ui.radio_button("None##lbl", &mut config.label_column_position, crate::config::LabelColumnPosition::None) {}
        ui.same_line();
        if ui.radio_button("Left##lbl", &mut config.label_column_position, crate::config::LabelColumnPosition::Left) {}
        ui.same_line();
        if ui.radio_button("Right##lbl", &mut config.label_column_position, crate::config::LabelColumnPosition::Right) {}

        if config.label_column_position != crate::config::LabelColumnPosition::None {
            nexus::imgui::Slider::new("Label Column Width", 50.0, 300.0)
                .build(ui, &mut config.label_column_width);

            ui.checkbox("Show Category in Label", &mut config.label_column_show_category);
            ui.checkbox("Show Track Name in Label", &mut config.label_column_show_track);

            nexus::imgui::Slider::new("Label Text Size", 0.5, 2.0)
                .build(ui, &mut config.label_column_text_size);

            ColorEdit::new("Label Background", &mut config.label_column_bg_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            ColorEdit::new("Label Track Text", &mut config.label_column_text_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            ColorEdit::new("Label Category Text", &mut config.label_column_category_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);
        }

        ui.spacing();

        // --- Appearance ---
        ui.text("Appearance");

        ColorEdit::new("Track Background", &mut config.global_track_background)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui);

        nexus::imgui::Slider::new("Track Padding", 0.0, 20.0)
            .build(ui, &mut config.global_track_padding);

        ui.checkbox("Override All Track Heights", &mut config.override_all_track_heights);
        if config.override_all_track_heights {
            nexus::imgui::Slider::new("Global Track Height", 20.0, 200.0)
                .build(ui, &mut config.global_track_height);
        }

        ui.checkbox("Draw Event Borders", &mut config.draw_event_borders);
        if config.draw_event_borders {
            ColorEdit::new("Border Color", &mut config.event_border_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            nexus::imgui::Slider::new("Border Thickness", 1.0, 5.0)
                .build(ui, &mut config.event_border_thickness);
        }

        ui.spacing();

        // --- Other ---
        ui.text("Other");
        ui.checkbox("Close window with ESC", &mut config.close_on_escape);
        ui.checkbox("Include event name when copying waypoint", &mut config.copy_with_event_name);

        ui.unindent();
    }

    // ==================== NOTIFICATIONS & TRACKING ====================
    if ui.collapsing_header("Notifications & Tracking", TreeNodeFlags::DEFAULT_OPEN) {
        ui.indent();

        // --- Toast Notifications ---
        ui.text("Toast Notifications");
        ui.checkbox("Enable Toasts", &mut config.notification_config.toast_enabled);

        if config.notification_config.toast_enabled {
            nexus::imgui::Slider::new("Toast Duration (sec)", 3.0, 15.0)
                .build(ui, &mut config.notification_config.toast_duration_seconds);

            let mut max_toasts = config.notification_config.max_visible_toasts as i32;
            if nexus::imgui::Slider::new("Max Visible Toasts", 1, 5).build(ui, &mut max_toasts) {
                config.notification_config.max_visible_toasts = max_toasts as usize;
            }

            ui.text("Toast Position:");
            if ui.radio_button("Top Left##tp", &mut config.notification_config.toast_position, ToastPosition::TopLeft) {}
            ui.same_line();
            if ui.radio_button("Top Right##tp", &mut config.notification_config.toast_position, ToastPosition::TopRight) {}
            if ui.radio_button("Bottom Left##tp", &mut config.notification_config.toast_position, ToastPosition::BottomLeft) {}
            ui.same_line();
            if ui.radio_button("Bottom Right##tp", &mut config.notification_config.toast_position, ToastPosition::BottomRight) {}

            let mut x_pct = config.notification_config.toast_offset_x * 100.0;
            if nexus::imgui::Slider::new("X Offset", 0.0, 50.0)
                .display_format("%.0f%%")
                .build(ui, &mut x_pct)
            {
                config.notification_config.toast_offset_x = x_pct / 100.0;
            }
            let mut y_pct = config.notification_config.toast_offset_y * 100.0;
            if nexus::imgui::Slider::new("Y Offset", 0.0, 50.0)
                .display_format("%.0f%%")
                .build(ui, &mut y_pct)
            {
                config.notification_config.toast_offset_y = y_pct / 100.0;
            }

            nexus::imgui::Slider::new("Toast Width", 200.0, 500.0)
                .build(ui, &mut config.notification_config.toast_size[0]);
            nexus::imgui::Slider::new("Toast Height", 60.0, 150.0)
                .build(ui, &mut config.notification_config.toast_size[1]);

            nexus::imgui::Slider::new("Toast Text Scale", 0.8, 2.0)
                .build(ui, &mut config.notification_config.toast_text_scale);

            ColorEdit::new("Toast Background", &mut config.notification_config.toast_bg_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            ColorEdit::new("Toast Event Name", &mut config.notification_config.toast_title_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            ColorEdit::new("Toast Track Name", &mut config.notification_config.toast_track_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            ColorEdit::new("Toast Time Text", &mut config.notification_config.toast_time_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            if ui.button("Preview Toast") {
                let (name, color) = config.notification_config.reminders.first()
                    .map(|r| (r.name.clone(), r.text_color))
                    .unwrap_or(("Preview".to_string(), [1.0, 1.0, 1.0, 1.0]));
                NOTIFICATION_STATE.lock().show_preview(&name, color);
            }
        }

        ui.spacing();
        ui.separator();

        // --- Reminders ---
        ui.text("Reminders");
        ui.text_disabled("Configure when notifications trigger");

        let mut reminder_to_remove: Option<usize> = None;
        let reminder_count = config.notification_config.reminders.len();

        for i in 0..reminder_count {
            ui.separator();
            let _id = ui.push_id(&format!("rem_{}", i));

            let mut name = config.notification_config.reminders[i].name.clone();
            if InputText::new(ui, "##name", &mut name).hint("Reminder name").build() {
                config.notification_config.reminders[i].name = name;
            }

            let mut minutes = config.notification_config.reminders[i].minutes_before as i32;
            if nexus::imgui::Slider::new("Minutes Before", 0, 30).build(ui, &mut minutes) {
                config.notification_config.reminders[i].minutes_before = minutes as u32;
            }

            if config.notification_config.reminders[i].minutes_before == 0 {
                ui.text_disabled("0 = Repeats during event");
                let mut interval = config.notification_config.reminders[i].ongoing_interval_minutes as i32;
                if nexus::imgui::Slider::new("Repeat Interval (min)", 1, 10).build(ui, &mut interval) {
                    config.notification_config.reminders[i].ongoing_interval_minutes = interval.max(1) as u32;
                }
            }

            ColorEdit::new("Reminder Color", &mut config.notification_config.reminders[i].text_color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            if reminder_count > 1 && ui.small_button("Remove") {
                reminder_to_remove = Some(i);
            }
        }

        if let Some(idx) = reminder_to_remove {
            config.notification_config.reminders.remove(idx);
        }

        ui.separator();
        if ui.button("Add Reminder") {
            config.notification_config.reminders.push(crate::config::ReminderConfig::default());
        }

        ui.spacing();
        ui.separator();

        // --- Upcoming Events Panel ---
        ui.text("Upcoming Events Panel");
        ui.checkbox("Enable Panel", &mut config.notification_config.upcoming_panel_enabled);

        if config.notification_config.upcoming_panel_enabled {
            let mut max_upcoming = config.notification_config.max_upcoming_events as i32;
            if nexus::imgui::Slider::new("Max Events in Panel", 5, 20).build(ui, &mut max_upcoming) {
                config.notification_config.max_upcoming_events = max_upcoming as usize;
            }
        }

        ui.spacing();
        ui.separator();

        // --- Tracked Events ---
        ui.text("Tracked Events");

        thread_local! {
            static SEARCH_TEXT: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
        }

        let mut search_text = SEARCH_TEXT.with(|s| s.borrow().clone());
        let search_width = ui.content_region_avail()[0] - 50.0;
        ui.set_next_item_width(search_width);
        InputText::new(ui, "##search", &mut search_text).hint("Search events...").build();
        SEARCH_TEXT.with(|s| *s.borrow_mut() = search_text.clone());
        ui.same_line();
        if ui.small_button("X##clr") {
            SEARCH_TEXT.with(|s| s.borrow_mut().clear());
        }

        if !search_text.is_empty() {
            let search_lower = search_text.to_lowercase();
            let mut matches: Vec<(String, String)> = Vec::new();
            let mut seen: HashSet<(String, String)> = HashSet::new();

            for track in &config.tracks {
                if !track.visible { continue; }
                for event in &track.events {
                    if !event.enabled { continue; }
                    if event.name.to_lowercase().contains(&search_lower)
                        || track.name.to_lowercase().contains(&search_lower)
                    {
                        let event_id = TrackedEventId::new(&track.name, &event.name);
                        let key = (track.name.clone(), event.name.clone());
                        if !config.tracked_events.contains(&event_id) && !seen.contains(&key) {
                            seen.insert(key);
                            matches.push((track.name.clone(), event.name.clone()));
                        }
                    }
                }
            }

            if !matches.is_empty() {
                let mut to_track: Option<TrackedEventId> = None;
                if let Some(_t) = ui.begin_table_with_flags("##search_results", 2, TableFlags::SIZING_STRETCH_PROP | TableFlags::ROW_BG) {
                    for (track_name, event_name) in matches.iter().take(10) {
                        ui.table_next_row();
                        ui.table_next_column();
                        if Selectable::new(&format!("{}##{}", event_name, track_name)).build(ui) {
                            to_track = Some(TrackedEventId::new(track_name, event_name));
                        }
                        ui.table_next_column();
                        ui.text_disabled(track_name);
                    }
                }
                if let Some(id) = to_track {
                    config.tracked_events.insert(id);
                    SEARCH_TEXT.with(|s| s.borrow_mut().clear());
                }
                if matches.len() > 10 {
                    ui.text_disabled(&format!("...{} more", matches.len() - 10));
                }
            } else {
                ui.text_disabled("No matches");
            }
        }

        let tracked_count = config.tracked_events.len();
        ui.text(&format!("{} tracked", tracked_count));
        if tracked_count > 0 {
            ui.same_line();
            if ui.io().key_ctrl {
                if ui.small_button("Clear All") {
                    config.tracked_events.clear();
                }
            } else {
                ui.text_disabled("[Ctrl to clear]");
            }

            let tracked: Vec<TrackedEventId> = config.tracked_events.iter().cloned().collect();
            let mut to_remove: Vec<TrackedEventId> = Vec::new();

            if let Some(_t) = ui.begin_table_with_flags("##tracked", 3, TableFlags::SIZING_STRETCH_PROP) {
                ui.table_setup_column("Event");
                ui.table_setup_column("Track");
                ui.table_setup_column_with(nexus::imgui::TableColumnSetup {
                    name: "##x",
                    flags: nexus::imgui::TableColumnFlags::WIDTH_FIXED,
                    init_width_or_weight: 20.0,
                    user_id: Default::default(),
                });

                for event_id in &tracked {
                    ui.table_next_row();
                    ui.table_next_column();
                    ui.text(&event_id.event_name);
                    ui.table_next_column();
                    ui.text_disabled(&event_id.track_name);
                    ui.table_next_column();
                    if ui.small_button(&format!("X##{}{}", event_id.track_name, event_id.event_name)) {
                        to_remove.push(event_id.clone());
                    }
                }
            }

            for id in to_remove {
                config.tracked_events.remove(&id);
            }
        }

        ui.unindent();
    }

    // ==================== TRACK MANAGEMENT ====================
    if ui.collapsing_header("Track Management", TreeNodeFlags::empty()) {
        ui.indent();

        // --- Database ---
        ui.text("Event Database");
        if ui.button("Check for Updates") {
            check_for_event_tracks_update();
        }
        ui.same_line();
        ui.text_disabled("Downloads latest events from GitHub");

        ui.spacing();
        ui.separator();

        // --- Visibility/Reorder Controls ---
        thread_local! {
            static SHOW_VISIBILITY: std::cell::Cell<bool> = std::cell::Cell::new(true);
            static SHOW_REORDERING: std::cell::Cell<bool> = std::cell::Cell::new(false);
        }

        let mut show_vis = SHOW_VISIBILITY.get();
        let mut show_reorder = SHOW_REORDERING.get();

        ui.checkbox("Show Visibility", &mut show_vis);
        SHOW_VISIBILITY.set(show_vis);
        ui.same_line();
        ui.checkbox("Show Reorder", &mut show_reorder);
        SHOW_REORDERING.set(show_reorder);

        ui.separator();

        let categories = config.categories.clone();
        let mut all_categories = categories.clone();
        for track in &config.tracks {
            if !all_categories.contains(&track.category) && !track.category.is_empty() {
                all_categories.push(track.category.clone());
            }
        }

        if config.category_order.is_empty() {
            config.category_order = all_categories.clone();
        } else {
            for cat in &all_categories {
                if !config.category_order.contains(cat) {
                    config.category_order.push(cat.clone());
                }
            }
            config.category_order.retain(|cat| all_categories.contains(cat));
        }

        let ordered_categories = config.category_order.clone();

        for category_name in &ordered_categories {
            config.category_visibility.entry(category_name.clone()).or_insert(true);
        }

        let mut category_to_move_up = None;
        let mut category_to_move_down = None;

        for (cat_pos, category) in ordered_categories.iter().enumerate() {
            if show_vis {
                let is_visible = config.category_visibility.get_mut(category).unwrap();
                ui.checkbox(&format!("##vis_{}", category), is_visible);
                ui.same_line();
            }

            if show_reorder {
                if cat_pos > 0 && ui.small_button(&format!("^##cat_{}", category)) {
                    category_to_move_up = Some(cat_pos);
                }
                if cat_pos > 0 { ui.same_line(); }
                if cat_pos < ordered_categories.len() - 1 && ui.small_button(&format!("v##cat_{}", category)) {
                    category_to_move_down = Some(cat_pos);
                }
                if cat_pos < ordered_categories.len() - 1 { ui.same_line(); }
            }

            if ui.collapsing_header(category, TreeNodeFlags::empty()) {
                let track_indices: Vec<usize> = config.tracks.iter().enumerate()
                    .filter(|(_, t)| t.category == *category)
                    .map(|(i, _)| i)
                    .collect();

                let (default_tracks, _) = load_tracks_from_json();
                let default_names: HashSet<&str> = default_tracks.iter().map(|t| t.name.as_str()).collect();

                let mut track_to_delete = None;

                for (list_pos, &index) in track_indices.iter().enumerate() {
                    let track_name = config.tracks[index].name.clone();
                    let mut track_visible = config.tracks[index].visible;
                    let is_default = default_names.contains(track_name.as_str());

                    ui.indent();

                    if show_vis {
                        ui.checkbox(&format!("##tvis_{}", track_name), &mut track_visible);
                        config.tracks[index].visible = track_visible;
                        ui.same_line();
                    }

                    if show_reorder {
                        if list_pos > 0 && ui.small_button(&format!("^##t_{}", track_name)) {
                            config.tracks.swap(index, track_indices[list_pos - 1]);
                        }
                        if list_pos > 0 { ui.same_line(); }
                        if list_pos < track_indices.len() - 1 && ui.small_button(&format!("v##t_{}", track_name)) {
                            config.tracks.swap(index, track_indices[list_pos + 1]);
                        }
                        if list_pos < track_indices.len() - 1 { ui.same_line(); }
                    }

                    if is_default {
                        if ui.collapsing_header(&track_name, TreeNodeFlags::empty()) {
                            let mut tracked_events_clone = config.tracked_events.clone();
                            let track = &mut config.tracks[index];
                            render_default_track_editor_inline(ui, track, &mut tracked_events_clone);
                            config.tracked_events = tracked_events_clone;
                        }
                    } else {
                        ui.text(&track_name);
                        ui.same_line();
                        if ui.small_button(&format!("Edit##{}", track_name)) {
                            *SELECTED_TRACK.lock() = Some(index);
                            *SELECTED_EVENT.lock() = None;
                        }
                        ui.same_line();
                        if ui.small_button(&format!("Del##{}", track_name)) {
                            track_to_delete = Some(index);
                        }
                    }

                    ui.unindent();
                }

                if let Some(del_idx) = track_to_delete {
                    config.tracks.remove(del_idx);
                    let mut sel = SELECTED_TRACK.lock();
                    if *sel == Some(del_idx) {
                        *sel = None;
                        *SELECTED_EVENT.lock() = None;
                    } else if let Some(s) = *sel {
                        if s > del_idx { *sel = Some(s - 1); }
                    }
                }
            }
        }

        if let Some(pos) = category_to_move_up {
            config.category_order.swap(pos, pos - 1);
        } else if let Some(pos) = category_to_move_down {
            config.category_order.swap(pos, pos + 1);
        }

        ui.separator();

        if ui.button("Add Custom Track") {
            let (default_tracks, _) = load_tracks_from_json();
            let default_names: HashSet<&str> = default_tracks.iter().map(|t| t.name.as_str()).collect();
            let custom_count = config.tracks.iter().filter(|t| !default_names.contains(t.name.as_str())).count();
            let mut track = EventTrack::default();
            track.name = format!("Custom Track {}", custom_count + 1);
            track.category = "Custom".to_string();
            let new_index = config.tracks.len();
            config.tracks.push(track);
            *SELECTED_TRACK.lock() = Some(new_index);
        }

        ui.unindent();
    }

    // ==================== RESET ====================
    ui.separator();
    ui.text_colored([1.0, 0.4, 0.4, 1.0], "Reset");
    if ui.io().key_ctrl {
        if ui.button("Reset All Settings") {
            if let Some(path) = crate::config::get_user_config_path() {
                if std::fs::remove_file(&path).is_ok() {
                    *crate::config::USER_CONFIG.lock() = crate::config::UserConfig::default();
                    crate::config::apply_user_overrides();
                }
            }
        }
    } else {
        ui.text_disabled("[Hold Ctrl] Reset All Settings");
    }

    ui.separator();
    render_custom_track_editor(ui, &mut config);
}

fn render_custom_track_editor(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>) {
    let mut selected_track = SELECTED_TRACK.lock();
    let mut selected_event = SELECTED_EVENT.lock();

    if let Some(track_index) = *selected_track {
        let (default_tracks, _) = load_tracks_from_json();
        let default_names: HashSet<&str> = default_tracks.iter().map(|t| t.name.as_str()).collect();

        if track_index < config.tracks.len() {
            let is_custom = !default_names.contains(config.tracks[track_index].name.as_str());

            if is_custom {
                let mut open = true;
                Window::new("Edit Custom Track")
                    .opened(&mut open)
                    .size([500.0, 600.0], nexus::imgui::Condition::FirstUseEver)
                    .build(ui, || {
                        render_track_editor_modal(ui, config, track_index, &mut selected_event);
                    });

                if !open {
                    *selected_track = None;
                    *selected_event = None;
                }
            } else {
                *selected_track = None;
            }
        } else {
            *selected_track = None;
        }
    }
}

fn render_default_track_editor_inline(ui: &Ui, track: &mut EventTrack, tracked_events: &mut HashSet<TrackedEventId>) {
    if InputFloat::new(ui, "Track Height", &mut track.height).build() {
        track.height = track.height.max(20.0).min(200.0);
    }

    ui.separator();
    ui.text("Events");

    let mut changes: Vec<(String, Option<bool>, Option<[f32; 4]>, Option<i64>, Option<i64>)> = Vec::new();
    let mut tracking_changes: Vec<(String, bool)> = Vec::new();
    let mut seen_names = HashSet::new();

    for event in track.events.iter() {
        if seen_names.contains(&event.name) { continue; }
        seen_names.insert(event.name.clone());

        let label = if event.enabled {
            format!("+ {}", event.name)
        } else {
            format!("- {}", event.name)
        };

        if ui.collapsing_header(&label, TreeNodeFlags::empty()) {
            ui.indent();

            let mut current_enabled = event.enabled;
            let enabled_changed = ui.checkbox(&format!("Enabled##{}", event.name), &mut current_enabled);

            let event_id = TrackedEventId::new(&track.name, &event.name);
            let mut tracked = tracked_events.contains(&event_id);
            if ui.checkbox(&format!("Track##{}", event.name), &mut tracked) {
                tracking_changes.push((event.name.clone(), tracked));
            }

            let mut color = event.color.to_array();
            let color_changed = ColorEdit::new(&format!("Color##{}", event.name), &mut color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            let mut start_min = (event.start_offset / 60) as i32;
            let offset_changed = nexus::imgui::InputInt::new(ui, &format!("Start (min)##{}", event.name), &mut start_min).build();

            let mut duration_min = (event.duration / 60) as i32;
            let duration_changed = nexus::imgui::InputInt::new(ui, &format!("Duration (min)##{}", event.name), &mut duration_min).build();

            ui.text_disabled(&format!("Cycle: {}m", event.cycle_duration / 60));

            let reset_clicked = ui.button(&format!("Reset##{}", event.name));

            if enabled_changed || color_changed || offset_changed || duration_changed || reset_clicked {
                if reset_clicked {
                    let (default_tracks, _) = load_tracks_from_json();
                    if let Some(dt) = default_tracks.iter().find(|t| t.name == track.name) {
                        if let Some(de) = dt.events.iter().find(|e| e.name == event.name) {
                            changes.push((event.name.clone(), Some(de.enabled), Some(de.color.to_array()), Some(de.start_offset), Some(de.duration)));
                        }
                    }
                } else {
                    changes.push((
                        event.name.clone(),
                        if enabled_changed { Some(current_enabled) } else { None },
                        if color_changed { Some(color) } else { None },
                        if offset_changed { Some((start_min as i64) * 60) } else { None },
                        if duration_changed { Some((duration_min.max(1) as i64) * 60) } else { None },
                    ));
                }
            }

            ui.unindent();
        }
    }

    for (name, enabled, color, offset, duration) in changes {
        for e in track.events.iter_mut() {
            if e.name == name {
                if let Some(en) = enabled { e.enabled = en; }
                if let Some(c) = color { e.color = EventColor::from_array(c); }
                if let Some(o) = offset { e.start_offset = o; }
                if let Some(d) = duration { e.duration = d; }
            }
        }
    }

    for (name, should_track) in tracking_changes {
        let event_id = TrackedEventId::new(&track.name, &name);
        if should_track { tracked_events.insert(event_id); }
        else { tracked_events.remove(&event_id); }
    }
}

fn render_track_editor_modal(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>, track_index: usize, selected_event: &mut MutexGuard<Option<usize>>) {
    let track = &mut config.tracks[track_index];

    let mut name = track.name.clone();
    if InputText::new(ui, "Track Name", &mut name).build() {
        track.name = name;
    }

    let mut category = track.category.clone();
    if InputText::new(ui, "Category", &mut category).build() {
        track.category = category;
    }

    if InputFloat::new(ui, "Track Height", &mut track.height).build() {
        track.height = track.height.max(20.0).min(200.0);
    }

    ui.separator();
    ui.text("Events");

    if ui.button("Add Event") {
        track.events.push(TimelineEvent::default());
    }
    ui.separator();

    if let Some(_t) = ui.begin_table_with_flags("Events", 4, TableFlags::BORDERS | TableFlags::ROW_BG) {
        ui.table_setup_column("Name");
        ui.table_setup_column("Start");
        ui.table_setup_column("Duration");
        ui.table_setup_column("Actions");
        ui.table_headers_row();

        let mut to_remove = None;

        for (idx, event) in track.events.iter_mut().enumerate() {
            ui.table_next_row();

            ui.table_next_column();
            if Selectable::new(&event.name).build(ui) {
                **selected_event = Some(idx);
            }

            ui.table_next_column();
            ui.text(format!("{}m", event.start_offset / 60));

            ui.table_next_column();
            ui.text(format!("{}m", event.duration / 60));

            ui.table_next_column();
            if ui.small_button(&format!("Edit##ev_{}", idx)) {
                **selected_event = Some(idx);
            }
            ui.same_line();
            if ui.small_button(&format!("X##ev_{}", idx)) {
                to_remove = Some(idx);
            }
        }

        if let Some(idx) = to_remove {
            track.events.remove(idx);
            if **selected_event == Some(idx) {
                **selected_event = None;
            }
        }
    }

    if let Some(event_idx) = **selected_event {
        if let Some(event) = track.events.get_mut(event_idx) {
            ui.separator();
            render_event_editor(ui, event);
        }
    }
}

fn render_event_editor(ui: &Ui, event: &mut TimelineEvent) {
    ui.text("Event Editor");
    ui.separator();

    let mut name = event.name.clone();
    if InputText::new(ui, "Event Name", &mut name).build() {
        event.name = name;
    }

    let mut start_min = (event.start_offset / 60) as i32;
    if nexus::imgui::InputInt::new(ui, "Start (minutes)", &mut start_min).build() {
        event.start_offset = (start_min as i64) * 60;
    }

    let mut duration_min = (event.duration / 60) as i32;
    if nexus::imgui::InputInt::new(ui, "Duration (minutes)", &mut duration_min).build() {
        event.duration = (duration_min as i64) * 60;
    }

    let mut cycle_min = (event.cycle_duration / 60) as i32;
    if nexus::imgui::InputInt::new(ui, "Cycle (minutes)", &mut cycle_min).build() {
        event.cycle_duration = (cycle_min as i64) * 60;
    }

    let mut color = event.color.to_array();
    if ColorEdit::new("Color", &mut color).flags(ColorEditFlags::ALPHA_BAR).build(ui) {
        event.color = EventColor::from_array(color);
    }

    let mut copy_text = event.copy_text.clone();
    if InputText::new(ui, "Copy Text", &mut copy_text).build() {
        event.copy_text = copy_text;
    }

    ui.checkbox("Enabled", &mut event.enabled);
}
