#![cfg(feature = "kittest-ui")]

use spp_core::business_hours::{
    KITTEST_PINNED_NOW_ISO, iso_days_ago, now_iso_millis, pin_now_for_kittest, wall_minutes_between,
};

#[test]
fn pinned_clock_drives_the_relative_time_render_path() {
    assert!(
        pin_now_for_kittest(KITTEST_PINNED_NOW_ISO),
        "the golden anchor must pin (idempotent with the fixtures' pin)"
    );
    assert_eq!(now_iso_millis(), "2026-11-07T12:00:00.000Z");
    assert_eq!(iso_days_ago(1), "2026-11-06T12:00:00.000Z");
    assert_eq!(iso_days_ago(9), "2026-10-29T12:00:00.000Z");
}

#[test]
fn the_seed_helper_and_the_render_clock_report_the_same_pinned_instant() {
    assert!(pin_now_for_kittest(KITTEST_PINNED_NOW_ISO));
    let minutes = wall_minutes_between(&iso_days_ago(2), &now_iso_millis())
        .expect("the seed parses and precedes the render clock");
    assert!(
        (minutes - 2880.0).abs() < 0.001,
        "2 days = 2880 wall minutes under the pin, got {minutes}"
    );
}

#[test]
fn repinning_the_same_anchor_is_idempotent_and_a_conflicting_one_is_rejected() {
    assert!(pin_now_for_kittest(KITTEST_PINNED_NOW_ISO));
    assert!(
        !pin_now_for_kittest("2026-01-01T00:00:00.000Z"),
        "first pin wins — a second, different anchor must be rejected"
    );
    assert_eq!(now_iso_millis(), "2026-11-07T12:00:00.000Z");
}

#[test]
fn an_unparseable_anchor_is_rejected_not_swallowed() {
    assert!(pin_now_for_kittest(KITTEST_PINNED_NOW_ISO));
    assert!(!pin_now_for_kittest("not a timestamp"));
    assert_eq!(now_iso_millis(), "2026-11-07T12:00:00.000Z");
}
