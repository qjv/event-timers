use nexus::imgui::{
    ColorEdit, ColorEditFlags, InputFloat, InputText, InputTextFlags, Selectable, TabBar, TabItem,
    TableFlags, TreeNodeFlags, Ui, Window,
};
use once_cell::sync::Lazy;
use parking_lot::{Mutex, MutexGuard};
use std::collections::HashSet;

use crate::config::{
    load_user_config, save_user_config, EventColorMode, RuntimeConfig, TimeRulerInterval,
    ToastPosition, TrackedEventId, UpcomingPanelMode, RUNTIME_CONFIG, SELECTED_EVENT,
    SELECTED_TRACK,
};
use crate::json_loader::{
    atomic_write_event_tracks, current_loaded_version, get_json_path, is_valid_event_tracks_json,
    load_tracks_from_json, parse_event_tracks_version, EventColor, EventTrack, TimelineEvent,
    JSON_LOAD_NOTICE,
};
use crate::notifications::NOTIFICATION_STATE;

const EVENT_TRACKS_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/qjv/event-timers/main/event_tracks.json",
    "https://cdn.jsdelivr.net/gh/qjv/event-timers@main/event_tracks.json",
];

static UPDATE_THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

pub fn wait_for_schedule_update() {
    if let Some(handle) = UPDATE_THREAD.lock().take() {
        let _ = handle.join();
    }
}

static UPDATE_STATUS: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new("Ready".to_string()));

fn event_tracks_needs_update(local_content: &str, remote_content: &str) -> bool {
    if !is_valid_event_tracks_json(remote_content) {
        return false;
    }
    match (
        parse_event_tracks_version(local_content),
        parse_event_tracks_version(remote_content),
    ) {
        (Some(local), Some(remote)) => remote > local,
        (None, Some(_)) => true,
        _ => false,
    }
}

pub fn check_for_event_tracks_update() {
    use std::thread;

    let mut worker = UPDATE_THREAD.lock();
    if worker.as_ref().is_some_and(|handle| !handle.is_finished()) {
        return;
    }
    if let Some(handle) = worker.take() {
        let _ = handle.join();
    }
    *UPDATE_STATUS.lock() = "Checking for schedule updates...".to_string();

    *worker = Some(thread::spawn(|| {
        let runtime_result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        let runtime = match runtime_result {
            Ok(rt) => rt,
            Err(e) => {
                let msg = format!("Failed to create async runtime: {e}");
                *UPDATE_STATUS.lock() = msg.clone();
                nexus::log::log(nexus::log::LogLevel::Critical, "Event Timers", &msg);
                return;
            }
        };

        runtime.block_on(async {
            nexus::log::log(
                nexus::log::LogLevel::Info,
                "Event Timers",
                "Checking for event_tracks.json updates...",
            );

            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("GW2-Nexus-Event-Timers/1.2.0")
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    *UPDATE_STATUS.lock() = format!("HTTP client error: {e}");
                    return;
                }
            };

            let mut fetched_content: Option<String> = None;
            let mut last_error = String::new();

            for &url in EVENT_TRACKS_URLS {
                match client.get(url).send().await {
                    Ok(response) => {
                        if !response.status().is_success() {
                            last_error = format!("HTTP {} from {}", response.status(), url);
                            continue;
                        }
                        match response.text().await {
                            Ok(body) if is_valid_event_tracks_json(&body) => {
                                fetched_content = Some(body);
                                break;
                            }
                            Ok(_) => {
                                last_error = format!("Invalid schedule JSON from {}", url);
                            }
                            Err(e) => {
                                last_error = format!("Read error from {}: {}", url, e);
                            }
                        }
                    }
                    Err(e) => {
                        last_error = format!("Network error ({}): {}", url, e);
                    }
                }
            }

            let Some(remote_content) = fetched_content else {
                let msg = format!("Update check failed ({last_error})");
                *UPDATE_STATUS.lock() = msg.clone();
                nexus::log::log(nexus::log::LogLevel::Warning, "Event Timers", &msg);
                return;
            };

            let Some(path) = get_json_path() else {
                *UPDATE_STATUS.lock() = "Could not resolve addon directory".to_string();
                return;
            };

            let needs_update = if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(local_content) => event_tracks_needs_update(&local_content, &remote_content),
                    Err(_) => true,
                }
            } else {
                true
            };

            if needs_update {
                // Extract overrides against the old database before replacing it.
                save_user_config();
                match atomic_write_event_tracks(&path, &remote_content) {
                    Ok(()) => {
                        load_user_config();
                        NOTIFICATION_STATE.lock().force_refresh();
                        let ver = current_loaded_version();
                        let msg = format!("Updated schedule to v{ver}");
                        *UPDATE_STATUS.lock() = msg.clone();
                        nexus::log::log(nexus::log::LogLevel::Info, "Event Timers", &msg);
                    }
                    Err(e) => {
                        *UPDATE_STATUS.lock() = e.clone();
                        nexus::log::log(nexus::log::LogLevel::Critical, "Event Timers", &e);
                    }
                }
            } else {
                let ver = current_loaded_version();
                let msg = format!("Up to date (v{ver})");
                *UPDATE_STATUS.lock() = msg;
                nexus::log::log(
                    nexus::log::LogLevel::Info,
                    "Event Timers",
                    "event_tracks.json is already up to date!",
                );
            }
        });
    }));
}

pub fn render_settings(ui: &Ui) {
    let mut config = RUNTIME_CONFIG.lock();
    let mut reset_requested = false;

    if let Some(_tab_bar) = TabBar::new("event_timers_settings_tabs").begin(ui) {
        if let Some(_tab) = TabItem::new("Tracking & Upcoming").begin(ui) {
            ui.spacing();
            render_tracking_and_upcoming_tab(ui, &mut config);
        }
        if let Some(_tab) = TabItem::new("Notifications").begin(ui) {
            ui.spacing();
            render_notifications_tab(ui, &mut config);
        }
        if let Some(_tab) = TabItem::new("Appearance & Window").begin(ui) {
            ui.spacing();
            reset_requested = render_appearance_tab(ui, &mut config);
        }
        if let Some(_tab) = TabItem::new("Tracks & Schedule").begin(ui) {
            ui.spacing();
            render_tracks_tab(ui, &mut config);
        }
        if let Some(_tab) = TabItem::new("Daily Completion").begin(ui) {
            ui.spacing();
            render_completion_tab(ui, &config);
        }
    }

    render_custom_track_editor(ui, &mut config);
    drop(config);
    if reset_requested {
        *crate::config::USER_CONFIG.lock() = crate::config::UserConfig::default();
        crate::config::apply_user_overrides();
        NOTIFICATION_STATE.lock().force_refresh();
        save_user_config();
    }
}

fn render_tracking_and_upcoming_tab(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>) {
    // --- Upcoming Events Panel ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Upcoming Events Panel");
    ui.checkbox(
        "Enable Upcoming Events Window",
        &mut config.notification_config.upcoming_panel_enabled,
    );

    ui.text("Events Shown in Panel:");
    ui.same_line();
    let modes = [
        UpcomingPanelMode::TrackedOnly,
        UpcomingPanelMode::TrackedAndFavorites,
        UpcomingPanelMode::AllVisibleEvents,
    ];
    for mode in modes {
        if ui.radio_button(
            &format!("{}##upmode", mode.label()),
            &mut config.notification_config.upcoming_panel_mode,
            mode,
        ) {
            NOTIFICATION_STATE.lock().force_refresh();
        }
        ui.same_line();
    }
    ui.new_line();

    if ui.checkbox(
        "Hide events already completed today from Upcoming Panel",
        &mut config.notification_config.hide_completed_in_upcoming,
    ) {
        NOTIFICATION_STATE.lock().force_refresh();
    }

    let mut max_upcoming = config.notification_config.max_upcoming_events as i32;
    if nexus::imgui::Slider::new("Max Events in Panel", 5, 30).build(ui, &mut max_upcoming) {
        config.notification_config.max_upcoming_events = max_upcoming as usize;
        NOTIFICATION_STATE.lock().force_refresh();
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // --- Quick Search & Track ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Quick Search & Track Events");
    ui.text_disabled("Search any event below, or right-click events directly in the timeline.");

    thread_local! {
        static SEARCH_TEXT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }

    let mut search_text = SEARCH_TEXT.with(|s| s.borrow().clone());
    let search_width = (ui.content_region_avail()[0] - 60.0).max(120.0);
    ui.set_next_item_width(search_width);
    InputText::new(ui, "##search", &mut search_text)
        .hint("Search by event or map name (e.g. Tarir, Piñata, Convergence)...")
        .build();
    SEARCH_TEXT.with(|s| *s.borrow_mut() = search_text.clone());
    ui.same_line();
    if ui.small_button("Clear##clr") {
        SEARCH_TEXT.with(|s| s.borrow_mut().clear());
    }

    if !search_text.is_empty() {
        let search_lower = search_text.to_lowercase();
        let mut matches: Vec<(String, String, String)> = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();

        for track in &config.tracks {
            if !track.visible {
                continue;
            }
            for event in &track.events {
                if !event.enabled {
                    continue;
                }
                let disp = event.display_name();
                if disp.to_lowercase().contains(&search_lower)
                    || event.name.to_lowercase().contains(&search_lower)
                    || track.name.to_lowercase().contains(&search_lower)
                {
                    let key = (track.name.clone(), event.name.clone());
                    if seen.insert(key) {
                        matches.push((track.name.clone(), event.name.clone(), disp.to_string()));
                    }
                }
            }
        }

        if !matches.is_empty() {
            let mut to_track: Option<TrackedEventId> = None;
            let mut to_oneshot: Option<TrackedEventId> = None;
            let mut to_fav: Option<TrackedEventId> = None;

            let table_flags =
                TableFlags::SIZING_STRETCH_PROP | TableFlags::ROW_BG | TableFlags::PAD_OUTER_X;
            if let Some(_t) = ui.begin_table_with_flags("##search_results", 3, table_flags) {
                ui.table_setup_column("Event");
                ui.table_setup_column("Track");
                ui.table_setup_column_with(nexus::imgui::TableColumnSetup {
                    name: "Actions",
                    flags: nexus::imgui::TableColumnFlags::WIDTH_FIXED,
                    init_width_or_weight: 185.0,
                    user_id: Default::default(),
                });

                for (track_name, event_name, disp_name) in matches.iter().take(12) {
                    ui.table_next_row();
                    ui.table_next_column();
                    ui.text(disp_name);

                    ui.table_next_column();
                    ui.text_disabled(track_name);

                    ui.table_next_column();
                    let id = TrackedEventId::new(track_name, event_name);
                    let already_tracked = config.tracked_events.contains(&id);
                    let already_fav = config.favorite_events.contains(&id);

                    if !already_tracked {
                        let _btn_color = ui.push_style_color(
                            nexus::imgui::StyleColor::Button,
                            [0.2, 0.5, 0.2, 0.85],
                        );
                        if ui.small_button(&format!("Track##{}{}", track_name, event_name)) {
                            to_track = Some(id.clone());
                        }
                        drop(_btn_color);
                        ui.same_line();

                        let _btn_color2 = ui.push_style_color(
                            nexus::imgui::StyleColor::Button,
                            [0.6, 0.4, 0.1, 0.85],
                        );
                        if ui.small_button(&format!("Next##{}{}", track_name, event_name)) {
                            to_oneshot = Some(id.clone());
                        }
                        drop(_btn_color2);
                        ui.same_line();
                    } else {
                        ui.text_disabled("Tracked");
                        ui.same_line();
                    }

                    if !already_fav {
                        let _btn_color3 = ui.push_style_color(
                            nexus::imgui::StyleColor::Button,
                            [0.55, 0.45, 0.1, 0.85],
                        );
                        if ui.small_button(&format!("Fav##{}{}", track_name, event_name)) {
                            to_fav = Some(id);
                        }
                    }
                }
            }

            if let Some(id) = to_track {
                config.tracked_events.insert(id);
                NOTIFICATION_STATE.lock().force_refresh();
            }
            if let Some(id) = to_oneshot {
                config.oneshot_events.insert(id);
                NOTIFICATION_STATE.lock().force_refresh();
            }
            if let Some(id) = to_fav {
                config.favorite_events.insert(id);
                NOTIFICATION_STATE.lock().force_refresh();
            }

            if matches.len() > 12 {
                ui.text_disabled(&format!("...and {} more matches", matches.len() - 12));
            }
        } else {
            ui.text_disabled("No matching events found.");
        }
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // --- Active Tracked / One-Shot / Favorites Table ---
    let tracked_count = config.tracked_events.len();
    let oneshot_count = config.oneshot_events.len();
    let favorite_count = config.favorite_events.len();

    ui.text_colored([0.4, 0.8, 1.0, 1.0], &format!("{} Tracked", tracked_count));
    if oneshot_count > 0 {
        ui.same_line();
        ui.text_colored(
            [1.0, 0.8, 0.4, 1.0],
            &format!(" · {} One-shot", oneshot_count),
        );
    }
    if favorite_count > 0 {
        ui.same_line();
        ui.text_colored(
            [1.0, 0.82, 0.18, 1.0],
            &format!(" · {} Favorites", favorite_count),
        );
    }

    if tracked_count > 0 || oneshot_count > 0 || favorite_count > 0 {
        ui.same_line();
        if ui.io().key_ctrl {
            if ui.small_button("Clear All") {
                config.tracked_events.clear();
                config.oneshot_events.clear();
                config.favorite_events.clear();
                NOTIFICATION_STATE.lock().force_refresh();
            }
        } else {
            ui.text_disabled("[Hold Ctrl to Clear All]");
        }

        let mut event_meta: std::collections::HashMap<(String, String), ([f32; 4], String)> =
            std::collections::HashMap::new();
        for track in &config.tracks {
            for event in &track.events {
                event_meta.insert(
                    (track.name.clone(), event.name.clone()),
                    (event.color.to_array(), event.display_name().to_string()),
                );
            }
        }

        let tracked: Vec<(TrackedEventId, bool, bool)> = config
            .tracked_events
            .iter()
            .map(|id| (id.clone(), false, config.favorite_events.contains(id)))
            .chain(
                config
                    .oneshot_events
                    .iter()
                    .map(|id| (id.clone(), true, config.favorite_events.contains(id))),
            )
            .chain(
                config
                    .favorite_events
                    .iter()
                    .filter(|id| {
                        !config.tracked_events.contains(*id) && !config.oneshot_events.contains(*id)
                    })
                    .map(|id| (id.clone(), false, true)),
            )
            .collect();

        let mut to_remove: Vec<(TrackedEventId, bool, bool)> = Vec::new();
        let mut to_toggle_done: Option<TrackedEventId> = None;

        ui.spacing();

        let table_flags = TableFlags::SIZING_STRETCH_PROP
            | TableFlags::ROW_BG
            | TableFlags::BORDERS_INNER_H
            | TableFlags::PAD_OUTER_X;

        if let Some(_t) = ui.begin_table_with_flags("##tracked_table", 4, table_flags) {
            ui.table_setup_column_with(nexus::imgui::TableColumnSetup {
                name: "##color",
                flags: nexus::imgui::TableColumnFlags::WIDTH_FIXED,
                init_width_or_weight: 8.0,
                user_id: Default::default(),
            });
            ui.table_setup_column("Event");
            ui.table_setup_column("Track & Tags");
            ui.table_setup_column_with(nexus::imgui::TableColumnSetup {
                name: "Actions",
                flags: nexus::imgui::TableColumnFlags::WIDTH_FIXED,
                init_width_or_weight: 90.0,
                user_id: Default::default(),
            });

            for (event_id, is_oneshot, is_favorite) in &tracked {
                ui.table_next_row();

                let (color, disp_name) = event_meta
                    .get(&(event_id.track_name.clone(), event_id.event_name.clone()))
                    .cloned()
                    .unwrap_or(([0.5, 0.5, 0.5, 1.0], event_id.event_name.clone()));

                // Color indicator column
                ui.table_next_column();
                let cursor_pos = ui.cursor_screen_pos();
                let draw_list = ui.get_window_draw_list();
                draw_list
                    .add_rect(
                        cursor_pos,
                        [cursor_pos[0] + 6.0, cursor_pos[1] + 18.0],
                        color,
                    )
                    .filled(true)
                    .build();
                ui.dummy([6.0, 18.0]);

                // Event name column
                ui.table_next_column();
                let completed =
                    crate::completion::is_completed(&event_id.track_name, &event_id.event_name);
                if completed {
                    ui.text_colored([0.52, 0.52, 0.52, 1.0], &disp_name);
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
                } else if *is_favorite {
                    ui.text_colored([1.0, 0.82, 0.18, 1.0], &disp_name);
                } else if *is_oneshot {
                    ui.text_colored([1.0, 0.8, 0.4, 1.0], &disp_name);
                } else {
                    ui.text_colored([1.0, 1.0, 1.0, 1.0], &disp_name);
                }

                // Track name & tags column
                ui.table_next_column();
                ui.text_colored([0.65, 0.65, 0.65, 1.0], &event_id.track_name);
                if *is_oneshot {
                    ui.same_line();
                    ui.text_colored([1.0, 0.6, 0.2, 0.85], "(next)");
                }
                if *is_favorite {
                    ui.same_line();
                    ui.text_colored([1.0, 0.82, 0.18, 0.9], "(fav)");
                }
                if completed {
                    ui.same_line();
                    ui.text_colored([0.45, 0.9, 0.45, 0.9], "(done)");
                }

                // Actions column
                ui.table_next_column();
                let manual_done = crate::completion::is_manually_completed(
                    &event_id.track_name,
                    &event_id.event_name,
                );
                let done_btn = if manual_done { "Undo" } else { "Done" };
                if ui.small_button(&format!(
                    "{}##done_{}_{}_{}",
                    done_btn, event_id.track_name, event_id.event_name, is_oneshot
                )) {
                    to_toggle_done = Some(event_id.clone());
                }
                ui.same_line();

                let _btn_color =
                    ui.push_style_color(nexus::imgui::StyleColor::Button, [0.6, 0.2, 0.2, 0.8]);
                if ui.small_button(&format!(
                    "X##{}{}{}{}",
                    event_id.track_name, event_id.event_name, is_oneshot, is_favorite
                )) {
                    to_remove.push((event_id.clone(), *is_oneshot, *is_favorite));
                }
            }
        }

        if let Some(id) = to_toggle_done {
            crate::completion::toggle_manual_completion(&id.track_name, &id.event_name);
            NOTIFICATION_STATE.lock().force_refresh();
        }
        for (id, is_oneshot, is_favorite) in to_remove {
            if is_oneshot {
                config.oneshot_events.remove(&id);
            } else if is_favorite && !config.tracked_events.contains(&id) {
                config.favorite_events.remove(&id);
            } else {
                config.tracked_events.remove(&id);
            }
            NOTIFICATION_STATE.lock().force_refresh();
        }
    } else {
        ui.spacing();
        ui.text_disabled("No events tracked or favorited yet.");
    }
}

fn render_notifications_tab(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>) {
    // --- Toast Notifications ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Toast Notifications");
    ui.checkbox(
        "Enable Toast Notifications",
        &mut config.notification_config.toast_enabled,
    );
    ui.same_line();
    if ui.button("Preview Toast") {
        let (name, color) = config
            .notification_config
            .reminders
            .first()
            .map(|r| (r.name.clone(), r.text_color))
            .unwrap_or(("Preview".to_string(), [1.0, 1.0, 1.0, 1.0]));
        NOTIFICATION_STATE.lock().show_preview(&name, color);
    }

    ui.checkbox(
        "Mute notifications for events already completed today (resumes at daily reset)",
        &mut config.notification_config.mute_completed_events,
    );
    ui.checkbox(
        "Include event name when copying waypoint to clipboard",
        &mut config.copy_with_event_name,
    );

    if config.notification_config.toast_enabled {
        nexus::imgui::Slider::new("Toast Duration (sec)", 3.0, 15.0)
            .build(ui, &mut config.notification_config.toast_duration_seconds);

        let mut max_toasts = config.notification_config.max_visible_toasts as i32;
        if nexus::imgui::Slider::new("Max Visible Toasts", 1, 5).build(ui, &mut max_toasts) {
            config.notification_config.max_visible_toasts = max_toasts as usize;
        }

        ui.text("Toast Corner:");
        ui.same_line();
        if ui.radio_button(
            "Top Left##tp",
            &mut config.notification_config.toast_position,
            ToastPosition::TopLeft,
        ) {}
        ui.same_line();
        if ui.radio_button(
            "Top Right##tp",
            &mut config.notification_config.toast_position,
            ToastPosition::TopRight,
        ) {}
        ui.same_line();
        if ui.radio_button(
            "Bottom Left##tp",
            &mut config.notification_config.toast_position,
            ToastPosition::BottomLeft,
        ) {}
        ui.same_line();
        if ui.radio_button(
            "Bottom Right##tp",
            &mut config.notification_config.toast_position,
            ToastPosition::BottomRight,
        ) {}

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

        ColorEdit::new(
            "Toast Background",
            &mut config.notification_config.toast_bg_color,
        )
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui);
        ColorEdit::new(
            "Toast Event Name",
            &mut config.notification_config.toast_title_color,
        )
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui);
        ColorEdit::new(
            "Toast Track Name",
            &mut config.notification_config.toast_track_color,
        )
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui);
        ColorEdit::new(
            "Toast Time Text",
            &mut config.notification_config.toast_time_color,
        )
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui);
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // --- Reminders ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Reminder Triggers");
    let mut grace_minutes = config.notification_config.happening_now_grace_minutes as i32;
    if nexus::imgui::Slider::new("\"Happening now\" Cutoff after Start (min)", 0, 60)
        .build(ui, &mut grace_minutes)
    {
        config.notification_config.happening_now_grace_minutes = grace_minutes.max(0) as u32;
    }
    ui.text_disabled("0 min = Only notify before the event starts, never after it has started.");

    let mut reminder_to_remove: Option<usize> = None;
    let reminder_count = config.notification_config.reminders.len();

    for i in 0..reminder_count {
        ui.separator();
        let _id = ui.push_id(&format!("rem_{}", i));

        let mut name = config.notification_config.reminders[i].name.clone();
        ui.set_next_item_width(220.0);
        if InputText::new(ui, "Label##name", &mut name)
            .hint("Reminder message")
            .build()
        {
            config.notification_config.reminders[i].name = name;
        }
        ui.same_line();
        ColorEdit::new(
            "Color",
            &mut config.notification_config.reminders[i].text_color,
        )
        .flags(ColorEditFlags::ALPHA_BAR | ColorEditFlags::NO_INPUTS)
        .build(ui);

        if reminder_count > 1 {
            ui.same_line();
            if ui.small_button("Remove") {
                reminder_to_remove = Some(i);
            }
        }

        let mut minutes = config.notification_config.reminders[i].minutes_before as i32;
        if nexus::imgui::Slider::new("Minutes Before (0 = During Event)", 0, 30)
            .build(ui, &mut minutes)
        {
            config.notification_config.reminders[i].minutes_before = minutes as u32;
        }

        if config.notification_config.reminders[i].minutes_before == 0 {
            let mut interval =
                config.notification_config.reminders[i].ongoing_interval_minutes as i32;
            if nexus::imgui::Slider::new("Repeat Interval During Event (min)", 1, 15)
                .build(ui, &mut interval)
            {
                config.notification_config.reminders[i].ongoing_interval_minutes =
                    interval.max(1) as u32;
            }
        }
    }

    if let Some(idx) = reminder_to_remove {
        config.notification_config.reminders.remove(idx);
    }

    ui.separator();
    if ui.button("Add Reminder") {
        config
            .notification_config
            .reminders
            .push(crate::config::ReminderConfig::default());
    }
}

fn render_appearance_tab(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>) -> bool {
    // --- Window & General ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Window & Behavior");
    ui.checkbox(
        "Show Main Event Timers Window",
        &mut config.show_main_window,
    );
    ui.same_line();
    ui.checkbox("Lock Window Position/Size", &mut config.is_window_locked);
    ui.checkbox("Hide Window Background", &mut config.hide_background);
    ui.same_line();
    ui.checkbox("Show Scrollbar", &mut config.show_scrollbar);
    ui.checkbox("Close Window with ESC", &mut config.close_on_escape);
    ui.same_line();
    ui.checkbox("Show Quick Access Icon", &mut config.show_quick_access_icon);

    ui.spacing();
    ui.separator();
    ui.spacing();

    // --- Timeline & Time Ruler ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Timeline & Time Ruler");
    ui.checkbox("Show Time Ruler", &mut config.show_time_ruler);
    if config.show_time_ruler {
        ui.same_line();
        ui.checkbox(
            "Sticky Time Ruler (Pin at Top When Scrolling)",
            &mut config.sticky_time_ruler,
        );
        ui.checkbox(
            "Show Current Clock Time on Ruler",
            &mut config.time_ruler_show_current_time,
        );

        ui.text("Marker Spacing:");
        ui.same_line();
        for interval in TimeRulerInterval::all() {
            ui.radio_button(
                &format!("{}##interval", interval.label()),
                &mut config.time_ruler_interval,
                *interval,
            );
            ui.same_line();
        }
        ui.new_line();
    }

    let mut view_range_minutes = config.view_range_seconds / 60.0;
    if nexus::imgui::Slider::new("View Range (minutes)", 15.0, 180.0)
        .build(ui, &mut view_range_minutes)
    {
        config.view_range_seconds = view_range_minutes * 60.0;
    }

    nexus::imgui::Slider::new("Current Time Line Position", 0.0, 0.5)
        .display_format("%.2f")
        .build(ui, &mut config.current_time_position);
    ui.text_disabled("0.0 = Left edge, 0.5 = Center of timeline");

    ui.spacing();
    ui.separator();
    ui.spacing();

    // --- Event Coloring & Track Appearance ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Event Colors & Styling");
    ui.text("Event Bar Color Mode:");
    let color_modes = [
        EventColorMode::Default,
        EventColorMode::CategoryExpansion,
        EventColorMode::MonochromeHighlight,
    ];
    for mode in color_modes {
        ui.radio_button(
            &format!("{}##cmode", mode.label()),
            &mut config.event_color_mode,
            mode,
        );
    }

    ColorEdit::new("Track Background", &mut config.global_track_background)
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui);

    nexus::imgui::Slider::new("Track Padding", 0.0, 20.0)
        .build(ui, &mut config.global_track_padding);

    ui.checkbox(
        "Override All Track Heights",
        &mut config.override_all_track_heights,
    );
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
    ui.separator();
    ui.spacing();

    // --- Categories & Track Labels ---
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Category Headers & Track Labels");
    ui.checkbox("Show Category Headers", &mut config.show_category_headers);

    if config.show_category_headers {
        ui.same_line();
        ui.radio_button(
            "Left##hdr",
            &mut config.category_header_alignment,
            crate::config::TextAlignment::Left,
        );
        ui.same_line();
        ui.radio_button(
            "Center##hdr",
            &mut config.category_header_alignment,
            crate::config::TextAlignment::Center,
        );
        ui.same_line();
        ui.radio_button(
            "Right##hdr",
            &mut config.category_header_alignment,
            crate::config::TextAlignment::Right,
        );

        nexus::imgui::Slider::new("Header Padding", 0.0, 50.0)
            .build(ui, &mut config.category_header_padding);
    }

    nexus::imgui::Slider::new("Spacing (Same Category)", 0.0, 20.0)
        .build(ui, &mut config.spacing_same_category);
    nexus::imgui::Slider::new("Spacing (Between Categories)", 0.0, 50.0)
        .build(ui, &mut config.spacing_between_categories);

    ui.spacing();
    ui.text("Track Label Column:");
    ui.same_line();
    ui.radio_button(
        "None##lbl",
        &mut config.label_column_position,
        crate::config::LabelColumnPosition::None,
    );
    ui.same_line();
    ui.radio_button(
        "Left##lbl",
        &mut config.label_column_position,
        crate::config::LabelColumnPosition::Left,
    );
    ui.same_line();
    ui.radio_button(
        "Right##lbl",
        &mut config.label_column_position,
        crate::config::LabelColumnPosition::Right,
    );

    if config.label_column_position != crate::config::LabelColumnPosition::None {
        nexus::imgui::Slider::new("Label Column Width", 50.0, 300.0)
            .build(ui, &mut config.label_column_width);
        ui.checkbox(
            "Show Category in Label",
            &mut config.label_column_show_category,
        );
        ui.same_line();
        ui.checkbox(
            "Show Track Name in Label",
            &mut config.label_column_show_track,
        );
        ColorEdit::new("Label Background", &mut config.label_column_bg_color)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui);
        ColorEdit::new("Label Track Text", &mut config.label_column_text_color)
            .flags(ColorEditFlags::ALPHA_BAR)
            .build(ui);
        ColorEdit::new(
            "Label Category Text",
            &mut config.label_column_category_color,
        )
        .flags(ColorEditFlags::ALPHA_BAR)
        .build(ui);
    }

    ui.spacing();
    ui.separator();
    ui.text_colored([1.0, 0.4, 0.4, 1.0], "Danger Zone");
    if ui.io().key_ctrl {
        ui.button("Reset All Settings to Default")
    } else {
        ui.text_disabled("[Hold Ctrl] Reset All Settings to Default");
        false
    }
}

fn render_tracks_tab(ui: &Ui, config: &mut MutexGuard<RuntimeConfig>) {
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "Event Schedule Database");
    let loaded_ver = current_loaded_version();
    ui.text(format!("Loaded Schedule Version: v{loaded_ver}"));
    ui.same_line();
    if ui.button("Check for Updates") {
        check_for_event_tracks_update();
    }
    ui.same_line();
    ui.text_disabled(&*UPDATE_STATUS.lock());

    if let Some(ref notice) = *JSON_LOAD_NOTICE.lock() {
        ui.text_colored([1.0, 0.75, 0.3, 1.0], notice);
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    thread_local! {
        static SHOW_VISIBILITY: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
        static SHOW_REORDERING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    let mut show_vis = SHOW_VISIBILITY.get();
    let mut show_reorder = SHOW_REORDERING.get();

    ui.checkbox("Show Visibility Checkboxes", &mut show_vis);
    SHOW_VISIBILITY.set(show_vis);
    ui.same_line();
    ui.checkbox("Show Reorder Arrows", &mut show_reorder);
    SHOW_REORDERING.set(show_reorder);

    ui.text_disabled(
        "Expand any track below to customize its events (rename, change waypoint, color, or timing).",
    );
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
        config
            .category_order
            .retain(|cat| all_categories.contains(cat));
    }

    let ordered_categories = config.category_order.clone();

    for category_name in &ordered_categories {
        config
            .category_visibility
            .entry(category_name.clone())
            .or_insert(true);
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
            if cat_pos > 0 {
                ui.same_line();
            }
            if cat_pos < ordered_categories.len() - 1
                && ui.small_button(&format!("v##cat_{}", category))
            {
                category_to_move_down = Some(cat_pos);
            }
            if cat_pos < ordered_categories.len() - 1 {
                ui.same_line();
            }
        }

        if ui.collapsing_header(category, TreeNodeFlags::empty()) {
            let track_indices: Vec<usize> = config
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.category == *category)
                .map(|(i, _)| i)
                .collect();

            let (default_tracks, _) = load_tracks_from_json();
            let default_names: HashSet<&str> =
                default_tracks.iter().map(|t| t.name.as_str()).collect();

            let mut track_to_delete = None;

            for (list_pos, &index) in track_indices.iter().enumerate() {
                let track_name = config.tracks[index].name.clone();
                let mut track_visible = config.tracks[index].visible;
                let is_default = default_names.contains(track_name.as_str());

                ui.indent();

                if show_vis {
                    if ui.checkbox(&format!("##tvis_{}", track_name), &mut track_visible) {
                        config.tracks[index].visible = track_visible;
                        NOTIFICATION_STATE.lock().force_refresh();
                    }
                    ui.same_line();
                }

                if show_reorder {
                    if list_pos > 0 && ui.small_button(&format!("^##t_{}", track_name)) {
                        config.tracks.swap(index, track_indices[list_pos - 1]);
                    }
                    if list_pos > 0 {
                        ui.same_line();
                    }
                    if list_pos < track_indices.len() - 1
                        && ui.small_button(&format!("v##t_{}", track_name))
                    {
                        config.tracks.swap(index, track_indices[list_pos + 1]);
                    }
                    if list_pos < track_indices.len() - 1 {
                        ui.same_line();
                    }
                }

                if is_default {
                    if ui.collapsing_header(&track_name, TreeNodeFlags::empty()) {
                        let mut tracked_events_clone = config.tracked_events.clone();
                        let mut favorite_events_clone = config.favorite_events.clone();
                        let track = &mut config.tracks[index];
                        render_default_track_editor_inline(
                            ui,
                            track,
                            &mut tracked_events_clone,
                            &mut favorite_events_clone,
                        );
                        config.tracked_events = tracked_events_clone;
                        config.favorite_events = favorite_events_clone;
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
                    if s > del_idx {
                        *sel = Some(s - 1);
                    }
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
        let custom_count = config
            .tracks
            .iter()
            .filter(|t| !default_names.contains(t.name.as_str()))
            .count();
        let mut track = EventTrack::default();
        track.name = format!("Custom Track {}", custom_count + 1);
        track.category = "Custom".to_string();
        let new_index = config.tracks.len();
        config.tracks.push(track);
        *SELECTED_TRACK.lock() = Some(new_index);
    }
}

fn render_completion_tab(ui: &Ui, config: &MutexGuard<RuntimeConfig>) {
    ui.text_colored([0.4, 0.85, 1.0, 1.0], "GW2 API Daily Completion Tracking");
    let completion_settings = crate::completion::settings();
    let mut enabled = completion_settings.enabled;
    let mut api_key = completion_settings.api_key.clone();
    let mut include_shared = completion_settings.include_shared_chests;

    let mut changed = ui.checkbox(
        "Strike out completed daily World Bosses and Hero's Choice Chests via GW2 API",
        &mut enabled,
    );
    changed |= ui.checkbox(
        "Include shared regional Hero's Choice chests (Convergences, Desolation, Vabbi, Castora)",
        &mut include_shared,
    );

    ui.set_next_item_width(360.0);
    changed |= InputText::new(ui, "GW2 API Key", &mut api_key)
        .flags(InputTextFlags::PASSWORD)
        .hint("Paste API key with account & progression permissions")
        .build();
    ui.same_line();
    if ui.button("Paste") {
        if let Some(clip) = ui.clipboard_text() {
            api_key = clip.trim().to_string();
            changed = true;
        }
    }
    if !api_key.is_empty() {
        ui.same_line();
        if ui.button("Clear Key") {
            api_key.clear();
            changed = true;
        }
    }

    ui.text_wrapped(
        "Requires an ArenaNet API key with 'account' and 'progression' permissions. Your key is stored only in your local addon directory.",
    );

    if changed {
        crate::completion::update_settings(enabled, api_key, include_shared);
        NOTIFICATION_STATE.lock().force_refresh();
    }

    if ui.button("Refresh API Completions Now") {
        crate::completion::request_refresh();
    }
    ui.same_line();
    ui.text_disabled(crate::completion::status());

    ui.spacing();
    ui.separator();
    ui.spacing();

    // --- Manual Daily Completions ---
    ui.text_colored(
        [0.4, 0.85, 1.0, 1.0],
        "Manual Daily Completions (Resets at 00:00 UTC)",
    );
    ui.text_wrapped(
        "Right-click any event in the timeline or Upcoming Events panel and choose 'Mark as Done Today' for events not covered by the GW2 API.",
    );

    let manual_list: Vec<TrackedEventId> =
        completion_settings.manual_completed.into_iter().collect();
    if manual_list.is_empty() {
        ui.text_disabled("No events manually marked as done today.");
    } else {
        if ui.button("Clear All Manual Completions") {
            crate::completion::clear_manual_completions();
            NOTIFICATION_STATE.lock().force_refresh();
        }
        ui.spacing();
        let mut to_unmark: Option<TrackedEventId> = None;
        for id in &manual_list {
            let disp = config.event_display_name(&id.track_name, &id.event_name);
            if ui.small_button(&format!("Unmark##{}_{}", id.track_name, id.event_name)) {
                to_unmark = Some(id.clone());
            }
            ui.same_line();
            ui.text(format!("{}: {}", id.track_name, disp));
        }
        if let Some(id) = to_unmark {
            crate::completion::toggle_manual_completion(&id.track_name, &id.event_name);
            NOTIFICATION_STATE.lock().force_refresh();
        }
    }
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

struct InlineEventEdit {
    event_name: String,
    enabled: Option<bool>,
    custom_name: Option<Option<String>>,
    copy_text: Option<String>,
    color: Option<[f32; 4]>,
    offset_delta: Option<i64>,
    duration: Option<i64>,
}

fn render_default_track_editor_inline(
    ui: &Ui,
    track: &mut EventTrack,
    tracked_events: &mut HashSet<TrackedEventId>,
    favorite_events: &mut HashSet<TrackedEventId>,
) {
    if InputFloat::new(ui, "Track Height", &mut track.height).build() {
        track.height = track.height.clamp(20.0, 200.0);
    }

    ui.separator();
    ui.text("Events in Track:");

    let mut edits: Vec<InlineEventEdit> = Vec::new();
    let mut tracking_changes: Vec<(String, bool)> = Vec::new();
    let mut favorite_changes: Vec<(String, bool)> = Vec::new();
    let mut seen_names = HashSet::new();

    for event in track.events.iter() {
        if !seen_names.insert(event.name.clone()) {
            continue;
        }

        let disp = event.display_name();
        let header_label = if event.enabled {
            if disp != event.name {
                format!("+ {} ({})###evhdr_{}", disp, event.name, event.name)
            } else {
                format!("+ {}###evhdr_{}", event.name, event.name)
            }
        } else {
            format!("- {}###evhdr_{}", disp, event.name)
        };

        if ui.collapsing_header(&header_label, TreeNodeFlags::empty()) {
            ui.indent();

            let mut current_enabled = event.enabled;
            let enabled_changed =
                ui.checkbox(&format!("Enabled##{}", event.name), &mut current_enabled);

            ui.same_line();
            let event_id = TrackedEventId::new(&track.name, &event.name);
            let mut tracked = tracked_events.contains(&event_id);
            if ui.checkbox(&format!("Track##{}", event.name), &mut tracked) {
                tracking_changes.push((event.name.clone(), tracked));
            }

            ui.same_line();
            let mut fav = favorite_events.contains(&event_id);
            if ui.checkbox(&format!("Favorite##{}", event.name), &mut fav) {
                favorite_changes.push((event.name.clone(), fav));
            }

            let mut custom_name_str = event
                .custom_name
                .clone()
                .unwrap_or_else(|| event.name.clone());
            let name_changed = InputText::new(
                ui,
                &format!("Display Name##{}", event.name),
                &mut custom_name_str,
            )
            .hint(&event.name)
            .build();

            let mut copy_text_str = event.copy_text.clone();
            let copy_changed = InputText::new(
                ui,
                &format!("Waypoint / Copy Text##{}", event.name),
                &mut copy_text_str,
            )
            .build();

            let mut color = event.color.to_array();
            let color_changed = ColorEdit::new(&format!("Color##{}", event.name), &mut color)
                .flags(ColorEditFlags::ALPHA_BAR)
                .build(ui);

            let mut start_min = (event.start_offset / 60) as i32;
            let offset_changed = nexus::imgui::InputInt::new(
                ui,
                &format!("Start Offset (min)##{}", event.name),
                &mut start_min,
            )
            .build();

            let mut duration_min = (event.duration / 60) as i32;
            let duration_changed = nexus::imgui::InputInt::new(
                ui,
                &format!("Duration (min)##{}", event.name),
                &mut duration_min,
            )
            .build();

            ui.text_disabled(&format!("Cycle: {}m", event.cycle_duration / 60));

            let reset_clicked = ui.button(&format!("Reset Event to Default##{}", event.name));

            if reset_clicked {
                let (default_tracks, _) = load_tracks_from_json();
                if let Some(dt) = default_tracks.iter().find(|t| t.name == track.name) {
                    if let Some(de) = dt.events.iter().find(|e| e.name == event.name) {
                        edits.push(InlineEventEdit {
                            event_name: event.name.clone(),
                            enabled: Some(de.enabled),
                            custom_name: Some(None),
                            copy_text: Some(de.copy_text.clone()),
                            color: Some(de.color.to_array()),
                            offset_delta: Some(de.start_offset - event.start_offset),
                            duration: Some(de.duration),
                        });
                    }
                }
            } else if enabled_changed
                || name_changed
                || copy_changed
                || color_changed
                || offset_changed
                || duration_changed
            {
                let new_custom_name = if name_changed {
                    let trimmed = custom_name_str.trim();
                    if trimmed.is_empty() || trimmed == event.name {
                        Some(None)
                    } else {
                        Some(Some(trimmed.to_string()))
                    }
                } else {
                    None
                };

                edits.push(InlineEventEdit {
                    event_name: event.name.clone(),
                    enabled: if enabled_changed {
                        Some(current_enabled)
                    } else {
                        None
                    },
                    custom_name: new_custom_name,
                    copy_text: if copy_changed {
                        Some(copy_text_str)
                    } else {
                        None
                    },
                    color: if color_changed { Some(color) } else { None },
                    offset_delta: if offset_changed {
                        Some((start_min as i64) * 60 - event.start_offset)
                    } else {
                        None
                    },
                    duration: if duration_changed {
                        Some((duration_min.max(1) as i64) * 60)
                    } else {
                        None
                    },
                });
            }

            ui.unindent();
        }
    }

    if !edits.is_empty() {
        for edit in edits {
            for e in track.events.iter_mut() {
                if e.name == edit.event_name {
                    if let Some(en) = edit.enabled {
                        e.enabled = en;
                    }
                    if let Some(ref cn) = edit.custom_name {
                        e.custom_name = cn.clone();
                    }
                    if let Some(ref ct) = edit.copy_text {
                        e.copy_text = ct.clone();
                    }
                    if let Some(c) = edit.color {
                        e.color = EventColor::from_array(c);
                    }
                    if let Some(delta) = edit.offset_delta {
                        e.start_offset =
                            (e.start_offset + delta).rem_euclid(e.cycle_duration.max(60));
                    }
                    if let Some(d) = edit.duration {
                        e.duration = d;
                    }
                }
            }
        }
        NOTIFICATION_STATE.lock().force_refresh();
    }

    for (name, should_track) in tracking_changes {
        let event_id = TrackedEventId::new(&track.name, &name);
        if should_track {
            tracked_events.insert(event_id);
        } else {
            tracked_events.remove(&event_id);
        }
        NOTIFICATION_STATE.lock().force_refresh();
    }

    for (name, should_fav) in favorite_changes {
        let event_id = TrackedEventId::new(&track.name, &name);
        if should_fav {
            favorite_events.insert(event_id);
        } else {
            favorite_events.remove(&event_id);
        }
        NOTIFICATION_STATE.lock().force_refresh();
    }
}

fn render_track_editor_modal(
    ui: &Ui,
    config: &mut MutexGuard<RuntimeConfig>,
    track_index: usize,
    selected_event: &mut MutexGuard<Option<usize>>,
) {
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
        track.height = track.height.clamp(20.0, 200.0);
    }

    ui.separator();
    ui.text("Events");

    if ui.button("Add Event") {
        track.events.push(TimelineEvent::default());
    }
    ui.separator();

    if let Some(_t) =
        ui.begin_table_with_flags("Events", 4, TableFlags::BORDERS | TableFlags::ROW_BG)
    {
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
        event.duration = (duration_min.max(1) as i64) * 60;
    }

    let mut cycle_min = (event.cycle_duration / 60) as i32;
    if nexus::imgui::InputInt::new(ui, "Cycle (minutes)", &mut cycle_min).build() {
        event.cycle_duration = (cycle_min.max(1) as i64) * 60;
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

#[cfg(test)]
mod tests {
    use super::event_tracks_needs_update;

    #[test]
    fn schedule_updater_never_downgrades_a_local_test_version() {
        let local = r#"{"version":"1.2.0","categories":[{"name":"Test","tracks":[]}]}"#;
        let remote = r#"{"version":"1.1.5","categories":[{"name":"Test","tracks":[]}]}"#;
        assert!(!event_tracks_needs_update(local, remote));
    }

    #[test]
    fn schedule_updater_accepts_a_newer_remote_version() {
        let local = r#"{"version":"1.1.7","categories":[{"name":"Test","tracks":[]}]}"#;
        let remote = r#"{"version":"1.2.0","categories":[{"name":"Test","tracks":[]}]}"#;
        assert!(event_tracks_needs_update(local, remote));
    }

    #[test]
    fn schedule_updater_rejects_error_text_from_github() {
        let local = r#"{"version":"1.1.7","categories":[{"name":"Test","tracks":[]}]}"#;
        let remote = "429: Too Many Requests";
        assert!(!event_tracks_needs_update(local, remote));
    }
}
