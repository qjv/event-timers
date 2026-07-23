// Add to lib.rs

use nexus::{
    gui::{register_render, render, RenderType},
    keybind::register_keybind_with_string,
    quick_access::add_quick_access,
    texture::load_texture_from_memory,
    AddonFlags, UpdateProvider,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::ffi::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

mod completion;
mod config;
mod json_loader;
mod notification_logic;
mod notifications;
mod time_utils;
mod ui;

use config::{load_user_config, save_user_config, RUNTIME_CONFIG};
use notification_logic::update_notifications;
use ui::{
    check_for_event_tracks_update, render_main_window, render_settings, render_toast_notifications,
    render_upcoming_panel,
};

// Embed icon files directly in the binary
const QA_ICON: &[u8] = include_bytes!("../qa_icon.png");
const QA_ICON_HOVER: &[u8] = include_bytes!("../qa_icon_hovered.png");
const NOTIFICATION_TICK_ACTIVE_MS: u64 = 250;
const NOTIFICATION_TICK_IDLE_MS: u64 = 1500;
const CONFIG_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(2);

static BG_STOP: AtomicBool = AtomicBool::new(false);
static BG_THREAD: Lazy<Mutex<Option<thread::JoinHandle<()>>>> = Lazy::new(|| Mutex::new(None));
static QA_TEXTURES_LOADED: AtomicBool = AtomicBool::new(false);
type QuickAccessRemove = Box<dyn Fn() + Send + Sync>;
static QA_REMOVE: Lazy<Mutex<Option<QuickAccessRemove>>> = Lazy::new(|| Mutex::new(None));

extern "C-unwind" fn toggle_window_keybind(_identifier: *const c_char, is_release: bool) {
    if !is_release {
        let mut config = RUNTIME_CONFIG.lock();
        config.show_main_window = !config.show_main_window;
    }
}

extern "C-unwind" fn toggle_toasts_keybind(_identifier: *const c_char, is_release: bool) {
    if !is_release {
        let mut config = RUNTIME_CONFIG.lock();
        config.notification_config.toast_enabled = !config.notification_config.toast_enabled;
    }
}

extern "C-unwind" fn toggle_upcoming_panel_keybind(_identifier: *const c_char, is_release: bool) {
    if !is_release {
        let mut config = RUNTIME_CONFIG.lock();
        config.notification_config.upcoming_panel_enabled =
            !config.notification_config.upcoming_panel_enabled;
    }
}

nexus::export! {
    name: "Event Timers",
    signature: -0x45564E54,
    load,
    unload,
    flags: AddonFlags::None,
    provider: UpdateProvider::GitHub,
    update_link: "https://github.com/qjv/event-timers",
}

fn load() {
    load_user_config();
    completion::load();
    BG_STOP.store(false, Ordering::Relaxed);

    // Check for event_tracks.json updates on load
    check_for_event_tracks_update();

    sync_quick_access();

    register_keybind_with_string("Toggle Event Timers", toggle_window_keybind, "ALT+E")
        .revert_on_unload();

    register_keybind_with_string("Toggle Toast Notifications", toggle_toasts_keybind, "")
        .revert_on_unload();

    register_keybind_with_string("Toggle Upcoming Panel", toggle_upcoming_panel_keybind, "")
        .revert_on_unload();

    register_render(
        RenderType::Render,
        render!(|ui| {
            sync_quick_access();
            render_main_window(ui);
            let (toast_enabled, upcoming_enabled) = {
                let config = RUNTIME_CONFIG.lock();
                (
                    config.notification_config.toast_enabled,
                    config.notification_config.upcoming_panel_enabled,
                )
            };
            if toast_enabled {
                render_toast_notifications(ui);
            }
            if upcoming_enabled {
                render_upcoming_panel(ui);
            }
        }),
    )
    .revert_on_unload();

    register_render(
        RenderType::OptionsRender,
        render!(|ui| {
            render_settings(ui);
        }),
    )
    .revert_on_unload();

    let handle = thread::Builder::new()
        .name("event-timers-notifications".to_string())
        .spawn(|| {
            let mut last_config_save = Instant::now();
            while !BG_STOP.load(Ordering::Relaxed) {
                let (has_targets, any_surface_enabled) = {
                    let config = RUNTIME_CONFIG.lock();
                    let has_targets =
                        !config.tracked_events.is_empty() || !config.oneshot_events.is_empty();
                    let any_surface_enabled = config.notification_config.toast_enabled
                        || config.notification_config.upcoming_panel_enabled
                        || config.show_main_window;
                    (has_targets, any_surface_enabled)
                };
                if has_targets && any_surface_enabled {
                    update_notifications();
                }
                completion::refresh_if_needed();
                if last_config_save.elapsed() >= CONFIG_AUTOSAVE_INTERVAL {
                    save_user_config();
                    last_config_save = Instant::now();
                }
                let sleep_ms = if has_targets && any_surface_enabled {
                    NOTIFICATION_TICK_ACTIVE_MS
                } else {
                    NOTIFICATION_TICK_IDLE_MS
                };
                thread::sleep(Duration::from_millis(sleep_ms));
            }
        });
    if let Ok(h) = handle {
        *BG_THREAD.lock() = Some(h);
    }
}

fn setup_quick_access() {
    if QA_REMOVE.lock().is_some() {
        return;
    }
    if !QA_TEXTURES_LOADED.swap(true, Ordering::AcqRel) {
        load_texture_from_memory("EVENT_TIMERS_QA_ICON", QA_ICON, None);
        load_texture_from_memory("EVENT_TIMERS_QA_ICON_HOVER", QA_ICON_HOVER, None);
    }

    let remove = add_quick_access(
        "EVENT_TIMERS_QA",
        "EVENT_TIMERS_QA_ICON",
        "EVENT_TIMERS_QA_ICON_HOVER",
        "Toggle Event Timers",
        "Toggle Event Timers Window",
    )
    .into_inner();
    *QA_REMOVE.lock() = Some(Box::new(remove));
}

fn sync_quick_access() {
    let should_show = RUNTIME_CONFIG.lock().show_quick_access_icon;
    if should_show {
        setup_quick_access();
    } else if let Some(remove) = QA_REMOVE.lock().take() {
        remove();
    }
}

fn unload() {
    BG_STOP.store(true, Ordering::Relaxed);
    if let Some(h) = BG_THREAD.lock().take() {
        let _ = h.join();
    }
    if let Some(remove) = QA_REMOVE.lock().take() {
        remove();
    }
    completion::save();
    save_user_config();
}
