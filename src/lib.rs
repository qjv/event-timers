// Add to lib.rs

use nexus::{
    gui::{register_render, render, RenderType},
    keybind::register_keybind_with_string,
    quick_access::add_quick_access,
    texture::load_texture_from_memory,
    AddonFlags, UpdateProvider,
};
use std::ffi::c_char;

mod config;
mod json_loader;
mod time_utils;
mod ui;

use config::{load_user_config, save_user_config, RUNTIME_CONFIG};
use ui::{render_main_window, render_settings, check_for_event_tracks_update};

// Embed icon files directly in the binary
const QA_ICON: &[u8] = include_bytes!("../qa_icon.png");
const QA_ICON_HOVER: &[u8] = include_bytes!("../qa_icon_hovered.png");

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
    
    // Setup Quick Access icon
    setup_quick_access();
    
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

fn setup_quick_access() {
    // Load textures from embedded bytes
    load_texture_from_memory("EVENT_TIMERS_QA_ICON", QA_ICON, None);
    load_texture_from_memory("EVENT_TIMERS_QA_ICON_HOVER", QA_ICON_HOVER, None);
    
    // Add quick access button
    add_quick_access(
        "EVENT_TIMERS_QA",
        "EVENT_TIMERS_QA_ICON",
        "EVENT_TIMERS_QA_ICON_HOVER",
        "Toggle Event Timers",
        "Toggle Event Timers Window"
    )
    .revert_on_unload();
}

fn unload() {
    save_user_config();
}