use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskUsageSnapshot {
    pub entries: Vec<DiskUsageEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskUsageEntry {
    pub label: &'static str,
    pub path: PathBuf,
    pub bytes: u64,
}

impl DiskUsageSnapshot {
    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    pub fn summary_label(&self) -> String {
        let total = format_bytes(self.total_bytes());
        let target = self
            .entries
            .iter()
            .find(|entry| entry.label == "target")
            .map(|entry| format_bytes(entry.bytes))
            .unwrap_or_else(|| "-".to_owned());
        let cargo = self
            .entries
            .iter()
            .filter(|entry| entry.label.starts_with("cargo "))
            .map(|entry| entry.bytes)
            .sum::<u64>();
        format!("total {total}, target {target}, cargo {}", format_bytes(cargo))
    }
}

pub async fn load(cwd: Option<PathBuf>) -> Result<DiskUsageSnapshot, String> {
    tokio::task::spawn_blocking(move || load_blocking(cwd))
        .await
        .map_err(|error| format!("disk scan task failed: {error}"))?
}

fn load_blocking(cwd: Option<PathBuf>) -> Result<DiskUsageSnapshot, String> {
    let roots = disk_roots(cwd)?;
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
    Ok(DiskUsageSnapshot { entries })
}

fn disk_roots(cwd: Option<PathBuf>) -> Result<Vec<(&'static str, PathBuf)>, String> {
    let workspace = cwd
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "could not determine current directory".to_owned())?;
    let mut roots = vec![("target", workspace.join("target"))];
    if let Some(cargo_home) = cargo_home() {
        roots.push(("cargo registry", cargo_home.join("registry")));
        roots.push(("cargo git", cargo_home.join("git")));
    }
    Ok(roots)
}

fn cargo_home() -> Option<PathBuf> {
    env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_with_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MiB");
    }

    #[test]
    fn summarizes_total_target_and_cargo() {
        let snapshot = DiskUsageSnapshot {
            entries: vec![
                DiskUsageEntry {
                    label: "target",
                    path: PathBuf::from("target"),
                    bytes: 1024,
                },
                DiskUsageEntry {
                    label: "cargo registry",
                    path: PathBuf::from("registry"),
                    bytes: 2048,
                },
                DiskUsageEntry {
                    label: "cargo git",
                    path: PathBuf::from("git"),
                    bytes: 1024,
                },
            ],
        };

        assert_eq!(
            snapshot.summary_label(),
            "total 4.0 KiB, target 1.0 KiB, cargo 3.0 KiB"
        );
    }
}
