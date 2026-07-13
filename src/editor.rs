use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorConfig {
    command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: Option<usize>,
}

impl EditorConfig {
    pub fn resolve(cli_editor: Option<String>, settings_editor: Option<String>) -> Self {
        let command = cli_editor
            .filter(|value| !value.trim().is_empty())
            .or_else(|| settings_editor.filter(|value| !value.trim().is_empty()))
            .or_else(|| env_string("NEXTDECK_EDITOR"))
            .or_else(|| env_string("VISUAL"))
            .or_else(|| env_string("EDITOR"))
            .unwrap_or_else(default_editor_command);
        Self { command }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn open_source(&self, location: &SourceLocation) -> io::Result<()> {
        self.spawn_for_path(&location.path, location.line)?;
        Ok(())
    }

    pub fn open_text(&self, title: &str, text: &str) -> io::Result<PathBuf> {
        let path = temp_text_path(title);
        fs::write(&path, text)?;
        self.spawn_for_path(&path, Some(1))?;
        Ok(path)
    }

    fn spawn_for_path(&self, path: &Path, line: Option<usize>) -> io::Result<()> {
        let (program, mut args) = editor_command_parts(&self.command);
        let line = line.unwrap_or(1);
        let path_string = path.to_string_lossy().to_string();
        if args
            .iter()
            .any(|arg| arg.contains("{file}") || arg.contains("{line}"))
        {
            for arg in &mut args {
                *arg = arg
                    .replace("{file}", &path_string)
                    .replace("{line}", &line.to_string());
            }
        } else {
            append_editor_args(&program, &mut args, &path_string, line);
        }

        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }
}

fn append_editor_args(program: &str, args: &mut Vec<String>, path: &str, line: usize) {
    match editor_name(program).as_str() {
        "idea" | "idea64" | "intellij" | "webstorm" | "rustrover" => {
            args.extend(["--line".to_owned(), line.to_string(), path.to_owned()]);
        }
        "code" | "cursor" | "windsurf" => {
            args.extend(["-g".to_owned(), format!("{path}:{line}")]);
        }
        "subl" | "sublime_text" | "zed" => {
            args.push(format!("{path}:{line}"));
        }
        "open" => {
            args.push(path.to_owned());
        }
        _ => {
            args.push(path.to_owned());
        }
    }
}

fn editor_command_parts(command: &str) -> (String, Vec<String>) {
    let mut parts = split_command(command).into_iter();
    let program = parts.next().unwrap_or_else(|| "open".to_owned());
    let args = parts.collect();
    (program, args)
}

fn split_command(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for char in command.chars() {
        match (quote, char) {
            (Some(active), char) if char == active => quote = None,
            (None, '"' | '\'') => quote = Some(char),
            (None, char) if char.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(char),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn editor_name(program: &str) -> String {
    Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase()
}

fn temp_text_path(title: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "nextdeck-{}-{timestamp}.txt",
        sanitize_title(title)
    ))
}

fn sanitize_title(title: &str) -> String {
    let sanitized = title
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '-' | '_') {
                char
            } else {
                '-'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('-').chars().take(64).collect()
}

fn env_string(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn default_editor_command() -> String {
    if cfg!(target_os = "macos") {
        "open".to_owned()
    } else {
        "xdg-open".to_owned()
    }
}

#[cfg(test)]
mod tests;
