use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const DEFAULT_TREE_WIDTH_PERCENT: u16 = 45;
pub const MIN_TREE_WIDTH_PERCENT: u16 = 25;
pub const MAX_TREE_WIDTH_PERCENT: u16 = 70;
pub const TREE_WIDTH_STEP_PERCENT: u16 = 5;
pub const DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB: u16 = 10;
pub const MIN_STORAGE_LOW_SPACE_THRESHOLD_GB: u16 = 1;
pub const MAX_STORAGE_LOW_SPACE_THRESHOLD_GB: u16 = 1024;
pub const STORAGE_LOW_SPACE_THRESHOLD_STEP_GB: u16 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TreeDurationMode {
    #[default]
    Wall,
    Aggregate,
}

impl TreeDurationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Wall => "wall",
            Self::Aggregate => "aggregate",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Wall => Self::Aggregate,
            Self::Aggregate => Self::Wall,
        }
    }

    pub const fn previous(self) -> Self {
        self.next()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreference {
    #[default]
    Auto,
    Dark,
    Light,
}

impl ThemePreference {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Auto => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::Auto,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Auto => Self::Light,
            Self::Dark => Self::Auto,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub tree_width_percent: u16,
    pub tree_duration_mode: TreeDurationMode,
    #[serde(alias = "editor_command")]
    pub open_with_command: Option<String>,
    pub theme_mode: ThemePreference,
    pub color_blind_mode: bool,
    pub storage_low_space_threshold_gb: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            tree_width_percent: DEFAULT_TREE_WIDTH_PERCENT,
            tree_duration_mode: TreeDurationMode::Wall,
            open_with_command: None,
            theme_mode: ThemePreference::Auto,
            color_blind_mode: false,
            storage_low_space_threshold_gb: DEFAULT_STORAGE_LOW_SPACE_THRESHOLD_GB,
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.tree_width_percent = clamp_tree_width(self.tree_width_percent);
        self.storage_low_space_threshold_gb =
            clamp_storage_low_space_threshold(self.storage_low_space_threshold_gb);
        self.open_with_command = self.open_with_command.and_then(non_empty_trimmed);
        self
    }

    pub fn open_with_label(&self) -> &str {
        self.open_with_command.as_deref().unwrap_or("env/default")
    }

    pub fn storage_low_space_threshold_bytes(&self) -> u64 {
        u64::from(self.storage_low_space_threshold_gb) * 1024 * 1024 * 1024
    }
}

pub fn load() -> AppSettings {
    config_read_paths()
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<AppSettings>(&text).ok())
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

pub fn clamp_storage_low_space_threshold(threshold_gb: u16) -> u16 {
    threshold_gb.clamp(
        MIN_STORAGE_LOW_SPACE_THRESHOLD_GB,
        MAX_STORAGE_LOW_SPACE_THRESHOLD_GB,
    )
}

pub fn resize_storage_low_space_threshold(threshold_gb: u16, delta: i16) -> u16 {
    clamp_storage_low_space_threshold(threshold_gb.saturating_add_signed(delta))
}

fn config_path() -> Option<PathBuf> {
    home_dir().map(|home| global_config_path(&home))
}

pub fn debug_log_path() -> Option<PathBuf> {
    home_dir().map(|home| global_debug_log_path(&home))
}

fn global_config_dir(home: &Path) -> PathBuf {
    home.join(".nextdeck")
}

fn global_config_path(home: &Path) -> PathBuf {
    global_config_dir(home).join("config.json")
}

fn global_debug_log_path(home: &Path) -> PathBuf {
    global_config_dir(home).join("debug.log")
}

fn config_read_paths() -> Vec<PathBuf> {
    config_read_paths_for(home_dir(), xdg_config_dir())
}

fn config_read_paths_for(home: Option<PathBuf>, xdg_config_home: Option<PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = home.as_ref() {
        paths.push(global_config_path(home));
    }
    paths.extend(legacy_config_path_for(xdg_config_home, home));
    paths
}

fn legacy_config_path_for(
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    xdg_config_home
        .or_else(|| home.map(|home| home.join(".config")))
        .map(|dir| dir.join("nextdeck").join("config.json"))
}

fn xdg_config_dir() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests;
