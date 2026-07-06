use std::{env, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_TREE_WIDTH_PERCENT: u16 = 45;
pub const MIN_TREE_WIDTH_PERCENT: u16 = 25;
pub const MAX_TREE_WIDTH_PERCENT: u16 = 70;
pub const TREE_WIDTH_STEP_PERCENT: u16 = 5;

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
    pub editor_command: Option<String>,
    pub theme_mode: ThemePreference,
    pub color_blind_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            tree_width_percent: DEFAULT_TREE_WIDTH_PERCENT,
            editor_command: None,
            theme_mode: ThemePreference::Auto,
            color_blind_mode: false,
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.tree_width_percent = clamp_tree_width(self.tree_width_percent);
        self.editor_command = self.editor_command.and_then(non_empty_trimmed);
        self
    }

    pub fn editor_label(&self) -> &str {
        self.editor_command.as_deref().unwrap_or("env/default")
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

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn normalizes_empty_editor_command() {
        let settings = AppSettings {
            editor_command: Some("  ".to_owned()),
            ..AppSettings::default()
        }
        .normalized();

        assert_eq!(settings.editor_command, None);
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
