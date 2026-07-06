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
    fn loads_legacy_editor_command_as_open_with_command() {
        let settings = serde_json::from_str::<AppSettings>(r#"{"editor_command":"idea"}"#)
            .expect("settings");

        assert_eq!(settings.open_with_command.as_deref(), Some("idea"));
    }

    #[test]
    fn global_config_path_lives_under_home_nextdeck() {
        assert_eq!(
            global_config_path(Path::new("/home/demo")),
            PathBuf::from("/home/demo/.nextdeck/config.json")
        );
    }

    #[test]
    fn config_read_paths_prefer_global_path_before_legacy_xdg_path() {
        assert_eq!(
            config_read_paths_for(
                Some(PathBuf::from("/home/demo")),
                Some(PathBuf::from("/xdg"))
            ),
            vec![
                PathBuf::from("/home/demo/.nextdeck/config.json"),
                PathBuf::from("/xdg/nextdeck/config.json"),
            ]
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
