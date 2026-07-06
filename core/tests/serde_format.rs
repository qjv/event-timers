//! Golden tests pinning the on-disk user_config.json format.
//!
//! These encode the behavior shipped installs depend on: what a missing
//! field deserializes to, and what a default config serializes as. If one
//! of these fails after a refactor, the refactor changed the file format.
//! Regenerate fixtures (deliberately!) with:
//!   cargo run -p event_timers_core --example gen_fixtures

use event_timers_core::config::{ReminderConfig, UserConfig};
use serde_json::Value;

/// Serialize through a JSON string, not `serde_json::to_value`: the latter
/// widens f32 to f64 (0.2 becomes 0.20000000298…), while the string path
/// prints the shortest roundtrip form — which is also what config files on
/// disk contain.
fn to_value<T: serde::Serialize>(v: &T) -> Value {
    let s = serde_json::to_string(v).expect("serialization should not fail");
    serde_json::from_str(&s).expect("serialized JSON reparses")
}

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} missing: {e}"));
    serde_json::from_str(&content).expect("fixture is valid JSON")
}

#[test]
fn default_user_config_matches_golden() {
    assert_eq!(
        to_value(&UserConfig::default()),
        fixture("default_user_config.json")
    );
}

#[test]
fn empty_object_deserializes_to_golden() {
    let from_empty: UserConfig =
        serde_json::from_str("{}").expect("an empty/partial config file must parse");
    assert_eq!(
        to_value(&from_empty),
        fixture("empty_object_user_config.json")
    );
}

/// `UserConfig::default()` (fresh install, no file) and `{}` (existing file
/// with all keys missing) intentionally disagree on exactly two fields.
/// This is long-shipped behavior; do not "fix" one side without deciding
/// what upgrading users should see.
#[test]
fn default_and_empty_object_diverge_only_on_known_fields() {
    let default = to_value(&UserConfig::default());
    let from_empty = to_value(&serde_json::from_str::<UserConfig>("{}").unwrap());

    let (default_map, empty_map) = match (&default, &from_empty) {
        (Value::Object(d), Value::Object(e)) => (d, e),
        _ => panic!("UserConfig must serialize to an object"),
    };

    let differing: Vec<&str> = default_map
        .iter()
        .filter(|(key, value)| empty_map.get(*key) != Some(value))
        .map(|(key, _)| key.as_str())
        .collect();

    assert_eq!(differing, ["draw_event_borders", "show_main_window"]);
    assert_eq!(default_map["show_main_window"], Value::Bool(false));
    assert_eq!(empty_map["show_main_window"], Value::Bool(true));
    assert_eq!(default_map["draw_event_borders"], Value::Bool(true));
    assert_eq!(empty_map["draw_event_borders"], Value::Bool(false));
}

#[test]
fn populated_config_roundtrips_losslessly() {
    let path = format!(
        "{}/tests/fixtures/populated_user_config.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let content = std::fs::read_to_string(path).expect("populated fixture exists");

    let parsed: UserConfig = serde_json::from_str(&content).expect("fixture parses");
    let reserialized = serde_json::to_string(&parsed).expect("serializes");
    let reparsed: UserConfig = serde_json::from_str(&reserialized).expect("reparses");

    // The event sets are HashSets: serialization order is arbitrary, so
    // canonicalize those arrays before comparing.
    let mut left = to_value(&parsed);
    let mut right = to_value(&reparsed);
    for value in [&mut left, &mut right] {
        for field in ["tracked_events", "favorite_events", "oneshot_events"] {
            let Some(Value::Array(items)) = value.get_mut(field) else {
                panic!("{field} should be an array");
            };
            items.sort_by_key(|item| item.to_string());
        }
    }
    assert_eq!(left, right);
}

#[test]
fn populated_config_preserves_non_default_values() {
    let path = format!(
        "{}/tests/fixtures/populated_user_config.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let content = std::fs::read_to_string(path).expect("populated fixture exists");
    let parsed: UserConfig = serde_json::from_str(&content).expect("fixture parses");

    // Spot-check values that exercise every section of the struct.
    assert_eq!(parsed.timeline_width, 1024.0);
    assert!(parsed.is_window_locked);
    assert_eq!(parsed.category_order, ["Core Tyria", "Custom"]);
    assert_eq!(parsed.custom_tracks.len(), 1);
    assert_eq!(parsed.custom_tracks[0].events.len(), 1);
    assert_eq!(parsed.tracked_events.len(), 2);
    assert_eq!(parsed.oneshot_events.len(), 1);
    assert_eq!(parsed.favorite_events.len(), 1);
    assert_eq!(parsed.notification_config.reminders.len(), 2);
    assert_eq!(parsed.notification_config.max_visible_toasts, 5);

    let override_data = &parsed.track_overrides["World Bosses"];
    assert_eq!(override_data.visible, Some(false));
    assert_eq!(override_data.height, Some(60.0));
    assert_eq!(override_data.disabled_events, ["Shadow Behemoth"]);
    // Hand-edited visual overrides are part of the format.
    let visual = override_data.visual.as_ref().expect("visual override kept");
    assert_eq!(visual.padding, 2.0);
}

#[test]
fn unknown_fields_are_ignored() {
    let parsed: Result<UserConfig, _> =
        serde_json::from_str(r#"{ "some_future_field": 42, "timeline_width": 900.0 }"#);
    let config = parsed.expect("configs from newer addon versions must still parse");
    assert_eq!(config.timeline_width, 900.0);
}

#[test]
fn reminder_missing_interval_defaults_to_five() {
    let parsed: ReminderConfig = serde_json::from_str(
        r#"{ "name": "Soon!", "minutes_before": 3, "text_color": [1.0, 1.0, 1.0, 1.0] }"#,
    )
    .expect("reminder without ongoing_interval_minutes parses");
    assert_eq!(parsed.ongoing_interval_minutes, 5);
}
