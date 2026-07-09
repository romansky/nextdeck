use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::{XtaskPreferences, XtaskState};
use crate::config;

const FILE_SCHEMA_VERSION: u32 = 1;
const STATE_FILE_NAME: &str = "xtask-state.json";

#[derive(Debug, Default)]
pub(crate) struct XtaskPersistence {
    path: Option<PathBuf>,
    workspace: Option<String>,
    attempted_revision: Option<u64>,
}

impl XtaskPersistence {
    pub(crate) async fn resolve(cwd: Option<PathBuf>) -> Self {
        let workspace = resolve_workspace_root(cwd).await;
        let path = config::app_dir().map(|dir| dir.join(STATE_FILE_NAME));
        Self::new(path, workspace)
    }

    pub(crate) fn restore(&self, state: &mut XtaskState) -> io::Result<()> {
        state.restore_preferences(self.load()?);
        Ok(())
    }

    pub(crate) fn flush(&mut self, state: &mut XtaskState) -> io::Result<bool> {
        let Some((revision, preferences)) = state.pending_preferences() else {
            return Ok(false);
        };
        if self.attempted_revision == Some(revision) {
            return Ok(false);
        }
        self.attempted_revision = Some(revision);
        self.save(preferences)?;
        state.mark_preferences_persisted(revision);
        Ok(true)
    }

    fn new(path: Option<PathBuf>, workspace: Option<PathBuf>) -> Self {
        Self {
            path,
            workspace: workspace.map(|path| path.to_string_lossy().into_owned()),
            attempted_revision: None,
        }
    }

    fn load(&self) -> io::Result<XtaskPreferences> {
        let (Some(path), Some(workspace)) = (&self.path, &self.workspace) else {
            return Ok(XtaskPreferences::default());
        };
        Ok(read_file(path)?
            .workspaces
            .remove(workspace)
            .unwrap_or_default())
    }

    fn save(&self, preferences: XtaskPreferences) -> io::Result<()> {
        let (Some(path), Some(workspace)) = (&self.path, &self.workspace) else {
            return Ok(());
        };
        let mut file = read_file(path)?;
        if preferences.is_empty() {
            file.workspaces.remove(workspace);
        } else {
            file.workspaces.insert(workspace.clone(), preferences);
        }
        write_file(path, &file)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct XtaskPreferencesFile {
    schema_version: u32,
    #[serde(default)]
    workspaces: BTreeMap<String, XtaskPreferences>,
}

#[derive(Deserialize)]
struct LocatedManifest {
    root: PathBuf,
}

impl Default for XtaskPreferencesFile {
    fn default() -> Self {
        Self {
            schema_version: FILE_SCHEMA_VERSION,
            workspaces: BTreeMap::new(),
        }
    }
}

async fn resolve_workspace_root(cwd: Option<PathBuf>) -> Option<PathBuf> {
    let cwd = cwd.or_else(|| env::current_dir().ok())?;
    let fallback = canonicalize_or_original(cwd.clone());
    let mut command = Command::new("cargo");
    command
        .args(["locate-project", "--workspace"])
        .current_dir(cwd);
    let Ok(output) = command.output().await else {
        return Some(fallback);
    };
    if !output.status.success() {
        return Some(fallback);
    }
    let Ok(manifest) = serde_json::from_slice::<LocatedManifest>(&output.stdout) else {
        return Some(fallback);
    };
    manifest
        .root
        .parent()
        .map(|root| canonicalize_or_original(root.to_path_buf()))
        .or(Some(fallback))
}

fn canonicalize_or_original(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn read_file(path: &Path) -> io::Result<XtaskPreferencesFile> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(XtaskPreferencesFile::default());
        }
        Err(error) => return Err(error),
    };
    let file = serde_json::from_slice::<XtaskPreferencesFile>(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if file.schema_version != FILE_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported xtask state schema {}, expected {}",
                file.schema_version, FILE_SCHEMA_VERSION
            ),
        ));
    }
    Ok(file)
}

fn write_file(path: &Path, file: &XtaskPreferencesFile) -> io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(file)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "xtask state path has no parent",
        ));
    };
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension(format!("tmp-{}", process::id()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut output = options.open(&temporary)?;
        #[cfg(unix)]
        output.set_permissions(fs::Permissions::from_mode(0o600))?;
        output.write_all(bytes)?;
        output.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests;
