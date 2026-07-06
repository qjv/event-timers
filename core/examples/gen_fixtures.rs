//! Regenerates the golden fixtures used by tests/serde_format.rs.
//!
//! Run from the repo root:
//!   cargo run -p event_timers_core --example gen_fixtures
//!
//! Only do this deliberately: the fixtures pin the on-disk user_config.json
//! format. A diff in a fixture is a change to what shipped installs read
//! and write, and must be intentional.

use event_timers_core::config::UserConfig;
use std::fs;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::create_dir_all(&dir).expect("create fixtures dir");

    let default_json = serde_json::to_string_pretty(&UserConfig::default())
        .expect("serialize default UserConfig");
    fs::write(dir.join("default_user_config.json"), default_json + "\n")
        .expect("write default fixture");

    let from_empty: UserConfig =
        serde_json::from_str("{}").expect("empty object must deserialize");
    let empty_json =
        serde_json::to_string_pretty(&from_empty).expect("serialize empty-object UserConfig");
    fs::write(dir.join("empty_object_user_config.json"), empty_json + "\n")
        .expect("write empty-object fixture");

    println!("fixtures written to {}", dir.display());
}
