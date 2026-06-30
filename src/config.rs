use std::{env, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_TREE_WIDTH_PERCENT: u16 = 45;
pub const MIN_TREE_WIDTH_PERCENT: u16 = 25;
pub const MAX_TREE_WIDTH_PERCENT: u16 = 70;
pub const TREE_WIDTH_STEP_PERCENT: u16 = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppSettings {
    pub tree_width_percent: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            tree_width_percent: DEFAULT_TREE_WIDTH_PERCENT,
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.tree_width_percent = clamp_tree_width(self.tree_width_percent);
        self
    }
}

pub fn load() -> AppSettings {
    let Some(path) = config_path() else {
        return AppSettings::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    serde_json::from_str::<AppSettings>(&text)
        .map(AppSettings::normalized)
        .unwrap_or_default()
}

pub fn save(settings: AppSettings) -> io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&settings.normalized())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)?;
    Ok(())
}

pub fn clamp_tree_width(width: u16) -> u16 {
    width.clamp(MIN_TREE_WIDTH_PERCENT, MAX_TREE_WIDTH_PERCENT)
}

pub fn resize_tree_width(width: u16, delta: i16) -> u16 {
    clamp_tree_width(width.saturating_add_signed(delta))
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("cargo-test-tui").join("config.json"))
}

fn config_dir() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| home_dir().map(|home| home.join(".config")))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tree_width() {
        assert_eq!(
            AppSettings {
                tree_width_percent: 10,
            }
            .normalized()
            .tree_width_percent,
            MIN_TREE_WIDTH_PERCENT
        );
        assert_eq!(
            AppSettings {
                tree_width_percent: 90,
            }
            .normalized()
            .tree_width_percent,
            MAX_TREE_WIDTH_PERCENT
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
}
