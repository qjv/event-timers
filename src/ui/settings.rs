use nexus::imgui::{
    ColorEdit, ColorEditFlags, InputFloat, InputText, Selectable, TableFlags, TreeNodeFlags, Ui, Window,
};
use std::collections::HashSet;
use parking_lot::MutexGuard;

use crate::config::{RUNTIME_CONFIG, SELECTED_EVENT, SELECTED_TRACK, RuntimeConfig};
use crate::json_loader::{load_tracks_from_json, EventColor, EventTrack, TimelineEvent};

const GITHUB_EVENT_TRACKS_URL: &str = "https://raw.githubusercontent.com/qjv/event-timers/main/event_tracks.json";

pub fn check_for_event_tracks_update() {
    use std::thread;
    
    thread::spawn(|| {
        // CRITICAL: Use a separate thread-local runtime to avoid conflicts
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
            
            // Use async reqwest instead of blocking
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
    
    ui.checkbox("Show Time Ruler", &mut config.show_time_ruler);

    let mut view_range_minutes = config.view_range_seconds / 60.0;
    if nexus::imgui::Slider::new("View Range (minutes)", 15.0, 120.0)
        .build(ui, &mut view_range_minutes)
    {
        config.view_range_seconds = view_range_minutes * 60.0;
    }
    
    if nexus::imgui::Slider::new("Timeline Position", 0.0, 0.5)
        .display_format("%.2f")
        .build(ui, &mut config.current_time_position)
    {
    }
    ui.text_disabled("0.0 = Current time at left, 0.5 = Center");
    
    ui.separator();
    
    ui.checkbox("Show Category Headers", &mut config.show_category_headers);

    if config.show_category_headers {
        ui.text("Category Header Alignment:");
        ui.same_line();
        if ui.radio_button("Left", &mut config.category_header_alignment, crate::config::TextAlignment::Left) {}
        ui.same_line();
        if ui.radio_button("Center", &mut config.category_header_alignment, crate::config::TextAlignment::Center) {}
        ui.same_line();
        if ui.radio_button("Right", &mut config.category_header_alignment, crate::config::TextAlignment::Right) {}
        
        if nexus::imgui::Slider::new("Header Padding", 0.0, 50.0)
            .build(ui, &mut config.category_header_padding)
        {
        }
    }
    
    if nexus::imgui::Slider::new("Spacing (Same Category)", 0.0, 20.0)
        .build(ui, &mut config.spacing_same_category)
    {
    }
    
    if nexus::imgui::Slider::new("Spacing (Between Categories)", 0.0, 50.0)
        .build(ui, &mut config.spacing_between_categories)
    {
    }
    
    ui.separator();

    ui.text("Track/Category Labels");
    ui.text("Show labels in:");
    ui.same_line();
    if ui.radio_button("None##label_pos", &mut config.label_column_position, crate::config::LabelColumnPosition::None) {}
    ui.same_line();
    if ui.radio_button("Left##label_pos", &mut config.label_column_position, crate::config::LabelColumnPosition::Left) {}
    ui.same_line();
    if ui.radio_button("Right##label_pos", &mut config.label_column_position, crate::config::LabelColumnPosition::Right) {}

    if config.label_column_position != crate::config::LabelColumnPosition::None {
        ui.separator();
        ui.text("Label Column Settings");
        
        if nexus::imgui::Slider::new("Label Column Width", 50.0, 300.0)
            .build(ui, &mut config.label_column_width)
        {
        }
        
        ui.checkbox("Show Category Names", &mut config.label_column_show_category);
        ui.checkbox("Show Track Names", &mut config.label_column_show_track);
        
        if nexus::imgui::Slider::new("Text Size##label_size", 0.5, 2.0)
            .build(ui, &mut config.label_column_text_size)
        {
        }
        
        if ColorEdit::new("Background Color##label_bg", &mut config.label_column_bg_color)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui)
        {
        }
        
        if ColorEdit::new("Category Text Color##label_cat", &mut config.label_column_category_color)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui)
        {
        }
        
        if ColorEdit::new("Track Text Color##label_text", &mut config.label_column_text_color)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui)
        {
        }
    }

    ui.separator();

    ui.text("Window Behavior");
    ui.checkbox("Close window with ESC key", &mut config.close_on_escape);

    ui.separator();
    
    ui.text("Global Track Appearance");
    
    if ColorEdit::new("Background Color", &mut config.global_track_background)
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui)
    {
    }
    
    if nexus::imgui::Slider::new("Background Padding", 0.0, 20.0)
        .build(ui, &mut config.global_track_padding)
    {
    }

    ui.checkbox("Override All Track Heights", &mut config.override_all_track_heights);
    if config.override_all_track_heights {
        if nexus::imgui::Slider::new("Global Track Height", 20.0, 200.0)
            .build(ui, &mut config.global_track_height)
        {
        }
    }
    
    ui.separator();

    ui.text("Global Event Appearance");
    ui.checkbox("Draw Event Borders", &mut config.draw_event_borders);
    
    if config.draw_event_borders {
        if ColorEdit::new("Border Color", &mut config.event_border_color)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui)
        {
        }

        if nexus::imgui::Slider::new("Border Thickness", 1.0, 5.0)
            .build(ui, &mut config.event_border_thickness)
        {
        }
    }
    
    ui.separator();
    
    // Event tracks update section
    ui.text("Event Tracks Database");
    
    if ui.button("Check for Updates") {
        check_for_event_tracks_update();
    }
    ui.same_line();
    ui.text_disabled("Compare local file with GitHub version");
    
    ui.separator();
    
    // Toggle buttons for cleaner UI
    ui.text("Track Management");
    
    thread_local! {
        static SHOW_VISIBILITY: std::cell::Cell<bool> = std::cell::Cell::new(true);
        static SHOW_REORDERING: std::cell::Cell<bool> = std::cell::Cell::new(false);
    }
    
    let mut show_vis = SHOW_VISIBILITY.get();
    let mut show_reorder = SHOW_REORDERING.get();
    
    if ui.checkbox("Show Visibility Toggles", &mut show_vis) {
        SHOW_VISIBILITY.set(show_vis);
    }
    ui.same_line();
    if ui.checkbox("Show Reordering Buttons", &mut show_reorder) {
        SHOW_REORDERING.set(show_reorder);
    }
    
    ui.separator();
    
    let categories = config.categories.clone();
    
    // Add any custom categories not in the default list
    let mut all_categories = categories.clone();
    for track in &config.tracks {
        if !all_categories.contains(&track.category) && !track.category.is_empty() {
            all_categories.push(track.category.clone());
        }
    }
    
    // Initialize category_order if empty
    if config.category_order.is_empty() {
        config.category_order = all_categories.clone();
    } else {
        // Add any new categories to the order
        for cat in &all_categories {
            if !config.category_order.contains(cat) {
                config.category_order.push(cat.clone());
            }
        }
        // Remove categories that no longer exist
        config.category_order.retain(|cat| all_categories.contains(cat));
    }
    
    // Use the ordered categories
    let ordered_categories = config.category_order.clone();
    
    for category_name in &ordered_categories {
        config.category_visibility.entry(category_name.clone()).or_insert(true);
    }
    
    // Get default categories for tracking
    let _default_category_count = categories.len();
    
    let mut category_to_move_up = None;
    let mut category_to_move_down = None;
    
    for (cat_pos, category) in ordered_categories.iter().enumerate() {
        if show_vis {
            let is_visible = config.category_visibility.get_mut(category).unwrap();
            ui.checkbox(&format!("##vis_{}", category), is_visible);
            if ui.is_item_hovered() {
                ui.tooltip_text("Toggle visibility for all tracks in this category");
            }
            ui.same_line();
        }
        
        // Category reorder buttons (only if enabled)
        if show_reorder {
            if cat_pos > 0 {
                if ui.small_button(&format!("Up##cat_up_{}", category)) {
                    category_to_move_up = Some(cat_pos);
                }
            } else {
                ui.dummy([ui.calc_text_size("Up")[0] + 8.0, 0.0]);
            }
            
            ui.same_line();
            
            if cat_pos < ordered_categories.len() - 1 {
                if ui.small_button(&format!("Dn##cat_down_{}", category)) {
                    category_to_move_down = Some(cat_pos);
                }
            } else {
                ui.dummy([ui.calc_text_size("Dn")[0] + 8.0, 0.0]);
            }
            
            ui.same_line();
        }

        if ui.collapsing_header(category, TreeNodeFlags::empty()) {
            // Remove the reorder buttons that were inside
            // ui.indent();
            // ...reorder buttons code removed...
            // ui.unindent();
            // ui.separator();
            
            // Get all tracks in this category (both default and custom)
            let track_indices: Vec<usize> = config.tracks.iter().enumerate()
                .filter(|(_, t)| t.category == *category)
                .map(|(i, _)| i)
                .collect();

            // Cache default names to check if this is a custom track
            let (default_tracks, _) = load_tracks_from_json();
            let default_names: HashSet<&str> = default_tracks
                .iter()
                .map(|t| t.name.as_str())
                .collect();

            let mut track_to_delete = None;

            for (list_pos, &index) in track_indices.iter().enumerate() {
                // Clone the track name first to avoid borrow issues
                let track_name = config.tracks[index].name.clone();
                let mut track_visible = config.tracks[index].visible;
                let is_default_track = default_names.contains(track_name.as_str());
                
                ui.indent();

                if show_vis {
                    ui.checkbox(&format!("##vis_{}", track_name), &mut track_visible);
                    if ui.is_item_hovered() {
                        ui.tooltip_text("Toggle visibility for this track");
                    }
                    
                    // Apply visibility change
                    config.tracks[index].visible = track_visible;
                    
                    ui.same_line();
                }
                
                // Reorder buttons for all tracks within category - only if enabled
                if show_reorder {
                    if list_pos > 0 {
                        if ui.small_button(&format!("Up##up_{}", track_name)) {
                            let prev_index = track_indices[list_pos - 1];
                            config.tracks.swap(index, prev_index);
                        }
                    } else {
                        ui.dummy([ui.calc_text_size("Up")[0] + 8.0, 0.0]);
                    }
                    
                    ui.same_line();
                    
                    if list_pos < track_indices.len() - 1 {
                        if ui.small_button(&format!("Dn##down_{}", track_name)) {
                            let next_index = track_indices[list_pos + 1];
                            config.tracks.swap(index, next_index);
                        }
                    } else {
                        ui.dummy([ui.calc_text_size("Dn")[0] + 8.0, 0.0]);
                    }
                    
                    ui.same_line();
                }
                
                if is_default_track {
                    // Default tracks use collapsing header inline editing
                    if ui.collapsing_header(&track_name, TreeNodeFlags::empty()) {
                        let track = &mut config.tracks[index];
                        render_default_track_editor_inline(ui, track);
                    }
                } else {
                    // Custom tracks show name + Edit + Delete buttons
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
            
            // Handle track deletion with confirmation
            if let Some(_delete_index) = track_to_delete {
                ui.open_popup(&format!("Delete Track##confirm_{}", category));
            }
            
            if let Some(delete_index) = track_to_delete {
                let track_name = config.tracks[delete_index].name.clone();
                ui.popup_modal(&format!("Delete Track##confirm_{}", category)).build(ui, || {
                    ui.text(&format!("Delete track '{}'?", track_name));
                    ui.text("This action cannot be undone.");
                    ui.separator();
                    
                    if ui.button("Delete") {
                        config.tracks.remove(delete_index);
                        
                        // Close the editor if we're editing this track
                        let mut selected_track = SELECTED_TRACK.lock();
                        if *selected_track == Some(delete_index) {
                            *selected_track = None;
                            *SELECTED_EVENT.lock() = None;
                        } else if let Some(sel_idx) = *selected_track {
                            // Adjust selected index if it's after the deleted track
                            if sel_idx > delete_index {
                                *selected_track = Some(sel_idx - 1);
                            }
                        }
                        
                        ui.close_current_popup();
                    }
                    ui.same_line();
                    if ui.button("Cancel") {
                        ui.close_current_popup();
                    }
                });
            }
        }
    }
    
    // Apply category reordering
    if let Some(pos) = category_to_move_up {
        config.category_order.swap(pos, pos - 1);
    } else if let Some(pos) = category_to_move_down {
        config.category_order.swap(pos, pos + 1);
    }
    
    ui.separator();
    
    if ui.button("Add Custom Track") {
        let (default_tracks, _) = load_tracks_from_json();
        let default_names: HashSet<&str> = default_tracks
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        
        let custom_count = config.tracks.iter()
            .filter(|t| !default_names.contains(t.name.as_str()))
            .count();
        let mut track = EventTrack::default();
        track.name = format!("Custom Track {}", custom_count + 1);
        track.category = "Custom".to_string();
        let new_index = config.tracks.len();
        config.tracks.push(track);
        *SELECTED_TRACK.lock() = Some(new_index);
    }
    
    ui.separator();
    
    // Render the custom track editor modal
    render_custom_track_editor(ui, &mut config);
}

fn render_custom_track_editor(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>) {
    let mut selected_track = SELECTED_TRACK.lock();
    let mut selected_event = SELECTED_EVENT.lock();
    
    if let Some(track_index) = *selected_track {
        // Cache default names to check if this is a custom track
        let (default_tracks, _) = load_tracks_from_json();
        let default_names: HashSet<&str> = default_tracks
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        
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

fn render_default_track_editor_inline(ui: &Ui, track: &mut EventTrack) {
    // Allow height editing for default tracks
    if InputFloat::new(ui, "Track Height", &mut track.height).build() {
        track.height = track.height.max(20.0).min(200.0);
    }
    
    ui.separator();
    ui.text("Events");
    
    // Collect changes to apply after iteration
    let mut changes: Vec<(String, Option<bool>, Option<[f32; 4]>, Option<i64>, Option<i64>)> = Vec::new();
    
    // Use collapsing headers instead of a table for more space
    let mut seen_names = HashSet::new();
    
    for event in track.events.iter() {
        if seen_names.contains(&event.name) { continue; }
        seen_names.insert(event.name.clone());
        
        let node_label = if event.enabled {
            format!("✓ {}", event.name)
        } else {
            format!("✗ {}", event.name)
        };
        
        if ui.collapsing_header(&node_label, TreeNodeFlags::DEFAULT_OPEN) {
            ui.indent();
            
            // Enable/Disable checkbox
            let mut current_enabled = event.enabled;
            let enabled_changed = ui.checkbox(&format!("Enabled##{}", event.name), &mut current_enabled);
            
            // Color picker
            let mut color = event.color.to_array();
            let color_changed = ColorEdit::new(&format!("Color##{}", event.name), &mut color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);
            
            // Start offset editor
            let mut start_min = (event.start_offset / 60) as i32;
            let offset_changed = nexus::imgui::InputInt::new(ui, &format!("Start Offset (min)##{}", event.name), &mut start_min).build();
            
            // Duration editor
            let mut duration_min = (event.duration / 60) as i32;
            let duration_changed = nexus::imgui::InputInt::new(ui, &format!("Duration (min)##{}", event.name), &mut duration_min).build();
            
            ui.text_disabled(&format!("Cycle: {}m", event.cycle_duration / 60));
            
            // Reset button
            let reset_clicked = ui.button(&format!("Reset to Default##{}", event.name));
            
            // Collect changes
            if enabled_changed || color_changed || offset_changed || duration_changed || reset_clicked {
                if reset_clicked {
                    // Load defaults and schedule reset
                    let (default_tracks, _) = load_tracks_from_json();
                    if let Some(default_track) = default_tracks.iter().find(|t| t.name == track.name) {
                        if let Some(default_event) = default_track.events.iter().find(|e| e.name == event.name) {
                            changes.push((
                                event.name.clone(),
                                Some(default_event.enabled),
                                Some(default_event.color.to_array()),
                                Some(default_event.start_offset),
                                Some(default_event.duration),
                            ));
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
    
    // Apply all collected changes
    for (event_name, new_enabled, new_color, new_offset, new_duration) in changes {
        for e in track.events.iter_mut() {
            if e.name == event_name {
                if let Some(enabled) = new_enabled {
                    e.enabled = enabled;
                }
                if let Some(color) = new_color {
                    e.color = EventColor::from_array(color);
                }
                if let Some(offset) = new_offset {
                    e.start_offset = offset;
                }
                if let Some(duration) = new_duration {
                    e.duration = duration;
                }
            }
        }
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
    ui.text_disabled("Category determines grouping and headers in the main window");
    
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
            if ui.small_button(&format!("Edit##event_{}", idx)) {
                **selected_event = Some(idx);
            }
            ui.same_line();
            if ui.small_button(&format!("X##event_{}", idx)) {
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
    
    let _id = ui.push_id("event_editor");
    
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
    if ColorEdit::new("Color", &mut color)
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui)
    {
        event.color = EventColor::from_array(color);
    }
    
    let mut copy_text = event.copy_text.clone();
    if InputText::new(ui, "Copy Text", &mut copy_text).build() {
        event.copy_text = copy_text;
    }
    
    ui.checkbox("Enabled", &mut event.enabled);
}