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
fn reports_storage_health_from_available_space() {
    let state = DiskUsageState {
        snapshot: Some(DiskUsageSnapshot {
            entries: Vec::new(),
            available_bytes: Some(11 * 1024 * 1024 * 1024),
            updated_at: UNIX_EPOCH,
        }),
        ..DiskUsageState::default()
    };
    assert_eq!(
        state.health(10 * 1024 * 1024 * 1024),
        StorageHealth::Healthy
    );

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
    assert_eq!(
        DiskUsageState::default().health(10),
        StorageHealth::NotScanned
    );
}

#[cfg(unix)]
#[test]
fn dir_size_counts_hard_links_once() {
    let root = env::temp_dir().join(format!(
        "nextdeck-disk-usage-hard-links-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("artifact");
    let link = root.join("artifact-link");
    fs::write(&file, vec![1; 4096]).unwrap();
    fs::hard_link(&file, &link).unwrap();

    let root_bytes = disk_usage_bytes(&fs::symlink_metadata(&root).unwrap());
    let file_bytes = disk_usage_bytes(&fs::symlink_metadata(&file).unwrap());

    assert_eq!(
        dir_size(&root, &DiskScanCancellation::default()).unwrap(),
        root_bytes + file_bytes
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancelled_scan_stops_before_traversal() {
    let cancellation = DiskScanCancellation::default();
    cancellation.cancel();

    let result = load_blocking(None, &cancellation).expect("cancellation is not an error");

    assert_eq!(result, None);
}
