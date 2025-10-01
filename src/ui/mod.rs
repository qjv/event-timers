pub mod main_window;
pub mod settings;
pub mod time_ruler;

pub use main_window::render_main_window;
pub use settings::{render_settings, check_for_event_tracks_update};