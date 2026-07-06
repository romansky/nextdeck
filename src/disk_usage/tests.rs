    use super::*;

    #[test]
    fn formats_bytes_with_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MiB");
    }

    #[test]
    fn formats_timestamps_as_utc() {
        assert_eq!(
            format_timestamp_utc(UNIX_EPOCH + std::time::Duration::from_secs(86_400)),
            "1970-01-02 00:00:00 UTC"
        );
    }

    #[test]
    fn summarizes_target_usage() {
        let snapshot = DiskUsageSnapshot {
            entries: vec![
                DiskUsageEntry {
                    label: "target",
                    path: PathBuf::from("target"),
                    bytes: 1024,
                },
            ],
            available_bytes: Some(4096),
            updated_at: UNIX_EPOCH,
        };

        assert_eq!(snapshot.summary_label(), "target 1.0 KiB");
    }

    #[test]
    fn reports_storage_health_from_available_space() {
        let state = DiskUsageState {
            snapshot: Some(DiskUsageSnapshot {
                entries: Vec::new(),
                available_bytes: Some(11 * 1024 * 1024 * 1024),
                updated_at: UNIX_EPOCH,
            }),
            ..DiskUsageState::default()
        };
        assert_eq!(state.health(10 * 1024 * 1024 * 1024), StorageHealth::Healthy);

        let state = DiskUsageState {
            snapshot: Some(DiskUsageSnapshot {
                entries: Vec::new(),
                available_bytes: Some(9 * 1024 * 1024 * 1024),
                updated_at: UNIX_EPOCH,
            }),
            ..DiskUsageState::default()
        };
        assert_eq!(state.health(10 * 1024 * 1024 * 1024), StorageHealth::Low);
    }

    #[test]
    fn reports_storage_health_for_transient_states() {
        assert_eq!(
            DiskUsageState {
                loading: true,
                ..DiskUsageState::default()
            }
            .health(10),
            StorageHealth::Scanning
        );
        assert_eq!(
            DiskUsageState {
                error: Some("boom".to_owned()),
                ..DiskUsageState::default()
            }
            .health(10),
            StorageHealth::Failed
        );
        assert_eq!(DiskUsageState::default().health(10), StorageHealth::NotScanned);
    }
