use super::*;
use std::path::{Path, PathBuf};

#[test]
fn normalizes_tree_width() {
    assert_eq!(
        AppSettings {
            tree_width_percent: 10,
            ..AppSettings::default()
        }
        .normalized()
        .tree_width_percent,
        MIN_TREE_WIDTH_PERCENT
    );
    assert_eq!(
        AppSettings {
            tree_width_percent: 90,
            ..AppSettings::default()
        }
        .normalized()
        .tree_width_percent,
        MAX_TREE_WIDTH_PERCENT
    );
}

#[test]
fn defaults_tree_duration_mode_to_wall_time() {
    assert_eq!(
        AppSettings::default().tree_duration_mode,
        TreeDurationMode::Wall
    );

    let expected = AppSettings {
        tree_duration_mode: TreeDurationMode::Aggregate,
        ..AppSettings::default()
    };
    let json = serde_json::to_string(&expected).expect("serialize settings");
    let settings = serde_json::from_str::<AppSettings>(&json).expect("settings");

    assert_eq!(settings, expected);
}

#[test]
fn normalizes_empty_open_with_command() {
    let settings = AppSettings {
        open_with_command: Some("  ".to_owned()),
        ..AppSettings::default()
    }
    .normalized();

    assert_eq!(settings.open_with_command, None);
}

#[test]
fn normalizes_storage_low_space_threshold() {
    assert_eq!(
        AppSettings {
            storage_low_space_threshold_gb: 0,
            ..AppSettings::default()
        }
        .normalized()
        .storage_low_space_threshold_gb,
        MIN_STORAGE_LOW_SPACE_THRESHOLD_GB
    );
    assert_eq!(
        AppSettings::default().storage_low_space_threshold_bytes(),
        u64::from(DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB) * 1024 * 1024 * 1024
    );
}

#[test]
fn normalizes_test_output_poll_interval() {
    assert_eq!(
        AppSettings {
            test_output_poll_interval_ms: 1,
            ..AppSettings::default()
        }
        .normalized()
        .test_output_poll_interval_ms,
        MIN_TEST_OUTPUT_POLL_INTERVAL_MS
    );
    assert_eq!(
        AppSettings {
            test_output_poll_interval_ms: u16::MAX,
            ..AppSettings::default()
        }
        .normalized()
        .test_output_poll_interval_ms,
        MAX_TEST_OUTPUT_POLL_INTERVAL_MS
    );
    assert_eq!(
        AppSettings::default().test_output_poll_interval(),
        std::time::Duration::from_millis(u64::from(DEFAULT_TEST_OUTPUT_POLL_INTERVAL_MS))
    );
}

#[test]
fn global_config_path_lives_under_home_nextdeck() {
    assert_eq!(
        global_config_path(Path::new("/home/demo")),
        PathBuf::from("/home/demo/.nextdeck/config.json")
    );
}

#[test]
fn resizes_tree_width_with_clamping() {
    assert_eq!(resize_tree_width(45, TREE_WIDTH_STEP_PERCENT as i16), 50);
    assert_eq!(
        resize_tree_width(MIN_TREE_WIDTH_PERCENT, -(TREE_WIDTH_STEP_PERCENT as i16)),
        MIN_TREE_WIDTH_PERCENT
    );
    assert_eq!(
        resize_tree_width(MAX_TREE_WIDTH_PERCENT, TREE_WIDTH_STEP_PERCENT as i16),
        MAX_TREE_WIDTH_PERCENT
    );
}

#[test]
fn resizes_storage_low_space_threshold_with_clamping() {
    assert_eq!(
        resize_storage_low_space_threshold(DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB, 1),
        DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB + 1
    );
    assert_eq!(
        resize_storage_low_space_threshold(MIN_STORAGE_LOW_SPACE_THRESHOLD_GB, -1),
        MIN_STORAGE_LOW_SPACE_THRESHOLD_GB
    );
}

#[test]
fn resizes_test_output_poll_interval_with_clamping() {
    assert_eq!(
        resize_test_output_poll_interval(
            DEFAULT_TEST_OUTPUT_POLL_INTERVAL_MS,
            TEST_OUTPUT_POLL_INTERVAL_STEP_MS as i16
        ),
        DEFAULT_TEST_OUTPUT_POLL_INTERVAL_MS + TEST_OUTPUT_POLL_INTERVAL_STEP_MS
    );
    assert_eq!(
        resize_test_output_poll_interval(
            MIN_TEST_OUTPUT_POLL_INTERVAL_MS,
            -(TEST_OUTPUT_POLL_INTERVAL_STEP_MS as i16)
        ),
        MIN_TEST_OUTPUT_POLL_INTERVAL_MS
    );
}
