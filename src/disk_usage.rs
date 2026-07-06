use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiskUsageState {
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
    pub modal_open: bool,
    pub running: bool,
    pub last_result: Option<Result<(), String>>,
}

impl DiskUsageSnapshot {
    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    pub fn summary_label(&self) -> String {
        let target = self
            .entries
            .iter()
            .find(|entry| entry.label == "target")
            .map(|entry| format_bytes(entry.bytes))
            .unwrap_or_else(|| "-".to_owned());
        format!("target {target}")
    }
}

impl DiskUsageState {
    pub fn begin_scan(&mut self) {
        self.loading = true;
        self.error = None;
    }

    pub fn apply_result(&mut self, result: Result<DiskUsageSnapshot, String>) -> Result<(), String> {
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

    pub fn summary_label(&self) -> String {
        if self.loading {
            return "scanning...".to_owned();
        }
        if self.error.is_some() {
            return "scan failed".to_owned();
        }
        self.snapshot
            .as_ref()
            .map(DiskUsageSnapshot::summary_label)
            .unwrap_or_else(|| "not scanned".to_owned())
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

pub async fn load(cwd: Option<PathBuf>) -> Result<DiskUsageSnapshot, String> {
    tokio::task::spawn_blocking(move || load_blocking(cwd))
        .await
        .map_err(|error| format!("disk scan task failed: {error}"))?
}

fn load_blocking(cwd: Option<PathBuf>) -> Result<DiskUsageSnapshot, String> {
    let roots = disk_roots(cwd)?;
    let available_bytes = roots
        .first()
        .and_then(|(_, path)| path.parent().or(Some(path.as_path())))
        .and_then(available_space);
    let mut entries = Vec::new();
    for (label, path) in roots {
        if path.exists() {
            entries.push(DiskUsageEntry {
                label,
                bytes: dir_size(&path).map_err(|error| {
                    format!("failed to scan {} at {}: {error}", label, path.display())
                })?,
                path,
            });
        }
    }
    Ok(DiskUsageSnapshot {
        entries,
        available_bytes,
        updated_at: SystemTime::now(),
    })
}

fn disk_roots(cwd: Option<PathBuf>) -> Result<Vec<(&'static str, PathBuf)>, String> {
    let workspace = cwd
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "could not determine current directory".to_owned())?;
    Ok(vec![("target", workspace.join("target"))])
}

fn dir_size(path: &Path) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        total += dir_size(&entry.path())?;
    }
    Ok(total)
}

#[cfg(unix)]
fn available_space(path: &Path) -> Option<u64> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
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

pub fn format_timestamp_utc(time: SystemTime) -> String {
    let Ok(duration) = time.duration_since(UNIX_EPOCH) else {
        return "-".to_owned();
    };
    let seconds = duration.as_secs();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u64, u64) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u64, day as u64)
}

#[cfg(test)]
mod tests;
