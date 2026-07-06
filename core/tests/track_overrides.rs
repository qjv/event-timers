use std::collections::HashMap;

use event_timers_core::config::{build_track_overrides, TrackOverride, TrackVisualConfig};
use event_timers_core::tracks::{EventTrack, TimelineEvent};

fn track(name: &str) -> EventTrack {
    EventTrack {
        name: name.to_string(),
        events: vec![
            TimelineEvent {
                name: "Event A".to_string(),
                ..TimelineEvent::default()
            },
            TimelineEvent {
                name: "Event B".to_string(),
                ..TimelineEvent::default()
            },
        ],
        ..EventTrack::default()
    }
}

fn visual_override(padding: f32) -> TrackOverride {
    TrackOverride {
        visual: Some(TrackVisualConfig {
            padding,
            ..TrackVisualConfig::default()
        }),
        ..TrackOverride::default()
    }
}

#[test]
fn unchanged_track_produces_no_override() {
    let defaults = [track("Bosses")];
    let (overrides, custom) = build_track_overrides(&defaults, &defaults, &HashMap::new());
    assert!(overrides.is_empty());
    assert!(custom.is_empty());
}

#[test]
fn visibility_height_and_disabled_events_are_diffed() {
    let defaults = [track("Bosses")];
    let mut live = track("Bosses");
    live.visible = false;
    live.height = 55.0;
    live.events[1].enabled = false;

    let (overrides, custom) = build_track_overrides(&[live], &defaults, &HashMap::new());

    assert!(custom.is_empty());
    let data = &overrides["Bosses"];
    assert_eq!(data.visible, Some(false));
    assert_eq!(data.height, Some(55.0));
    assert_eq!(data.disabled_events, ["Event B"]);
    assert!(data.visual.is_none());
}

#[test]
fn non_default_track_becomes_custom() {
    let defaults = [track("Bosses")];
    let live = [track("Bosses"), track("My Custom")];

    let (overrides, custom) = build_track_overrides(&live, &defaults, &HashMap::new());

    assert!(overrides.is_empty());
    assert_eq!(custom.len(), 1);
    assert_eq!(custom[0].name, "My Custom");
}

/// Regression test: saving used to rebuild overrides with `visual: None`,
/// destroying hand-edited visual overrides on every clean unload.
#[test]
fn hand_edited_visual_override_survives_rebuild() {
    let defaults = [track("Bosses")];
    let mut live = track("Bosses");
    live.height = 60.0;
    let previous = HashMap::from([("Bosses".to_string(), visual_override(2.0))]);

    let (overrides, _) = build_track_overrides(&[live], &defaults, &previous);

    let data = &overrides["Bosses"];
    assert_eq!(data.height, Some(60.0));
    let visual = data.visual.as_ref().expect("visual override preserved");
    assert_eq!(visual.padding, 2.0);
}

/// A track whose only customization is a visual override must still be
/// persisted; treating it as "no changes" would also drop the visual.
#[test]
fn visual_only_override_is_kept() {
    let defaults = [track("Bosses")];
    let previous = HashMap::from([("Bosses".to_string(), visual_override(7.5))]);

    let (overrides, _) = build_track_overrides(&defaults, &defaults, &previous);

    let visual = overrides["Bosses"].visual.as_ref().expect("kept");
    assert_eq!(visual.padding, 7.5);
}

#[test]
fn stale_previous_override_for_unchanged_track_without_visual_is_dropped() {
    let defaults = [track("Bosses")];
    let previous = HashMap::from([(
        "Bosses".to_string(),
        TrackOverride {
            visible: Some(false),
            ..TrackOverride::default()
        },
    )]);

    // The live track matches defaults again, so only a visual (which lives
    // nowhere else) would justify keeping an entry.
    let (overrides, _) = build_track_overrides(&defaults, &defaults, &previous);
    assert!(overrides.is_empty());
}
