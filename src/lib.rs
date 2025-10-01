use nexus::{
    gui::{register_render, render, RenderType},
    keybind::register_keybind_with_string,
    AddonFlags, UpdateProvider,
};
use std::ffi::c_char;

mod config;
mod json_loader;
mod time_utils;
mod ui;

use config::{load_user_config, save_user_config, RUNTIME_CONFIG};
use ui::{render_main_window, render_settings, check_for_event_tracks_update};

extern "C-unwind" fn toggle_window_keybind(_identifier: *const c_char, is_release: bool) {
    if !is_release {
        let mut config = RUNTIME_CONFIG.lock();
        config.show_main_window = !config.show_main_window;
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
    
    // Check for event_tracks.json updates on load
    check_for_event_tracks_update();
    
    register_keybind_with_string("Toggle Event Timers", toggle_window_keybind, "ALT+E")
        .revert_on_unload();
    
    register_render(RenderType::Render, render!(|ui| {
        render_main_window(ui);
    }))
    .revert_on_unload();
    
    register_render(RenderType::OptionsRender, render!(|ui| {
        render_settings(ui);
    }))
    .revert_on_unload();
}

fn unload() {
    save_user_config();
}