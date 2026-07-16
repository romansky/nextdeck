use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

#[cfg(unix)]
use std::collections::HashSet;

use chrono::{DateTime, Local};

use crate::request::RequestId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskUsageSnapshot {
    pub entries: Vec<DiskUsageEntry>,
    pub available_bytes: Option<u64>,
    pub updated_at: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskUsageEntry {
    pub label: &'static str,
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DiskScanCancellation(Arc<AtomicBool>);

impl DiskScanCancellation {
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiskUsageState {
    pub request_id: RequestId,
    pub loading: bool,
    pub snapshot: Option<DiskUsageSnapshot>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageHealth {
    Scanning,
    Failed,
    NotScanned,
    Unknown,
    Healthy,
    Low,
}

impl StorageHealth {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Scanning => "scanning",
            Self::Failed => "failed",
            Self::NotScanned => "not scanned",
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Low => "low",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiskCleanupState {
    pub request_id: RequestId,
    pub modal_open: bool,
    pub running: bool,
    pub last_result: Option<Result<(), String>>,
}

impl DiskUsageState {
    pub fn begin_scan(&mut self) -> RequestId {
        self.request_id = self.request_id.next();
        self.loading = true;
        self.error = None;
        self.request_id
    }

    pub fn apply_result(
        &mut self,
        result: Result<DiskUsageSnapshot, String>,
    ) -> Result<(), String> {
        self.loading = false;
        match result {
            Ok(snapshot) => {
                self.snapshot = Some(snapshot);
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn health(&self, low_space_threshold_bytes: u64) -> StorageHealth {
        if self.loading {
            return StorageHealth::Scanning;
        }
        if self.error.is_some() {
            return StorageHealth::Failed;
        }
        let Some(snapshot) = &self.snapshot else {
            return StorageHealth::NotScanned;
        };
        match snapshot.available_bytes {
            Some(available) if available < low_space_threshold_bytes => StorageHealth::Low,
            Some(_) => StorageHealth::Healthy,
            None => StorageHealth::Unknown,
        }
    }
}

impl DiskCleanupState {
    pub fn begin_clean(&mut self) -> bool {
        if self.running {
            return false;
        }
        self.request_id = self.request_id.next();
        self.running = true;
        self.last_result = None;
        true
    }

    pub fn apply_result(&mut self, result: Result<(), String>) -> bool {
        self.running = false;
        let ok = result.is_ok();
        self.last_result = Some(result);
        ok
    }
}

pub async fn load(
    cwd: Option<PathBuf>,
    cancellation: DiskScanCancellation,
) -> Result<Option<DiskUsageSnapshot>, String> {
    tokio::task::spawn_blocking(move || load_blocking(cwd, &cancellation))
        .await
        .map_err(|error| format!("disk scan task failed: {error}"))?
}

fn load_blocking(
    cwd: Option<PathBuf>,
    cancellation: &DiskScanCancellation,
) -> Result<Option<DiskUsageSnapshot>, String> {
    if cancellation.is_cancelled() {
        return Ok(None);
    }
    let roots = disk_roots(cwd)?;
    let available_bytes = roots
        .first()
        .and_then(|(_, path)| path.parent().or(Some(path.as_path())))
        .and_then(available_space);
    let mut entries = Vec::new();
    for (label, path) in roots {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        if path.exists() {
            let bytes = match dir_size(&path, cancellation) {
                Ok(bytes) => bytes,
                Err(_) if cancellation.is_cancelled() => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "failed to scan {} at {}: {error}",
                        label,
                        path.display()
                    ));
                }
            };
            entries.push(DiskUsageEntry { label, bytes, path });
        }
    }
    Ok(Some(DiskUsageSnapshot {
        entries,
        available_bytes,
        updated_at: SystemTime::now(),
    }))
}

fn disk_roots(cwd: Option<PathBuf>) -> Result<Vec<(&'static str, PathBuf)>, String> {
    let workspace = cwd
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "could not determine current directory".to_owned())?;
    Ok(vec![("target", workspace.join("target"))])
}

fn dir_size(path: &Path, cancellation: &DiskScanCancellation) -> io::Result<u64> {
    let mut seen = SeenFiles::default();
    dir_size_with_seen(path, &mut seen, cancellation)
}

fn dir_size_with_seen(
    path: &Path,
    seen: &mut SeenFiles,
    cancellation: &DiskScanCancellation,
) -> io::Result<u64> {
    if cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "disk scan cancelled",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !seen.should_count(&metadata) {
        return Ok(0);
    }
    if !metadata.is_dir() {
        return Ok(disk_usage_bytes(&metadata));
    }

    let mut total = disk_usage_bytes(&metadata);
    for entry in fs::read_dir(path)? {
        if cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "disk scan cancelled",
            ));
        }
        let entry = entry?;
        total += dir_size_with_seen(&entry.path(), seen, cancellation)?;
    }
    Ok(total)
}

#[cfg(unix)]
#[derive(Default)]
struct SeenFiles {
    hard_links: HashSet<FileId>,
}

#[cfg(unix)]
type FileId = (u64, u64);

#[cfg(unix)]
impl SeenFiles {
    fn should_count(&mut self, metadata: &fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;

        if metadata.is_dir() || metadata.nlink() <= 1 {
            return true;
        }
        self.hard_links.insert((metadata.dev(), metadata.ino()))
    }
}

#[cfg(not(unix))]
#[derive(Default)]
struct SeenFiles;

#[cfg(not(unix))]
impl SeenFiles {
    fn should_count(&mut self, _metadata: &fs::Metadata) -> bool {
        true
    }
}

#[cfg(unix)]
fn disk_usage_bytes(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    metadata.blocks().saturating_mul(512)
}

#[cfg(not(unix))]
fn disk_usage_bytes(metadata: &fs::Metadata) -> u64 {
    if metadata.is_file() {
        metadata.len()
    } else {
        0
    }
}

#[cfg(unix)]
fn available_space(path: &Path) -> Option<u64> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is a live, NUL-terminated C string and `stat` points to valid writable
    // storage for one `statvfs` value for the duration of the call.
    let result = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    // SAFETY: POSIX specifies that a successful `statvfs` call initializes the output struct.
    let stat = unsafe { stat.assume_init() };
    Some(
        (stat.f_bavail as u128)
            .saturating_mul(stat.f_frsize as u128)
            .min(u64::MAX as u128) as u64,
    )
}

#[cfg(not(unix))]
fn available_space(_path: &Path) -> Option<u64> {
    None
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_timestamp_local(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S %:z").to_string()
}

#[cfg(test)]
mod tests;
