use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod xtask;

pub const ENV_VAR: &str = "NEXTDECK_TEST_EVENTS";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub module: String,
    pub file: String,
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestEvent {
    pub schema_version: u32,
    pub time: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub level: Level,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceLocation>,
}

impl TestEvent {
    #[must_use]
    pub fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            time: now_millis(),
            pid: Some(std::process::id()),
            thread: current_thread_name(),
            level,
            target: None,
            message: message.into(),
            fields: BTreeMap::new(),
            source: None,
        }
    }

    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    #[must_use]
    pub fn with_source(
        mut self,
        module: impl Into<String>,
        file: impl Into<String>,
        line: u32,
    ) -> Self {
        self.source = Some(SourceLocation {
            module: module.into(),
            file: file.into(),
            line,
        });
        self
    }

    #[must_use]
    pub fn with_fields(mut self, fields: BTreeMap<String, Value>) -> Self {
        self.fields = fields;
        self
    }
}

#[must_use]
pub fn enabled() -> bool {
    event_file_path().is_some()
}

#[must_use]
pub fn event_file_path() -> Option<PathBuf> {
    std::env::var_os(ENV_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|dir| pid_event_file_path(&dir))
}

pub fn emit(event: &TestEvent) -> io::Result<()> {
    let Some(path) = event_file_path() else {
        return Ok(());
    };
    let mut line = serde_json::to_vec(event).map_err(io::Error::other)?;
    line.push(b'\n');
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&line)
}

pub fn emit_with_fields(
    level: Level,
    target: Option<String>,
    message: String,
    fields: BTreeMap<String, Value>,
    module: &'static str,
    file: &'static str,
    line: u32,
) -> io::Result<()> {
    let mut event = TestEvent::new(level, message)
        .with_fields(fields)
        .with_source(module, file, line);
    event.target = target;
    emit(&event)
}

#[must_use]
pub fn now_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[doc(hidden)]
pub fn field_value(value: impl Serialize) -> Value {
    serde_json::to_value(value).unwrap_or_else(|error| {
        Value::String(format!("<event field serialization failed: {error}>"))
    })
}

fn pid_event_file_path(dir: &Path) -> PathBuf {
    dir.join(format!("{}.jsonl", std::process::id()))
}

fn current_thread_name() -> Option<String> {
    let thread = std::thread::current();
    thread.name().map(ToOwned::to_owned)
}

#[macro_export]
macro_rules! event {
    (level: $level:expr, target: $target:expr, $message:expr; $($key:literal => $value:expr),* $(,)?) => {{
        if $crate::enabled() {
            let mut fields = std::collections::BTreeMap::new();
            $(
                fields.insert($key.to_string(), $crate::field_value(&$value));
            )*
            let _ = $crate::emit_with_fields(
                $level,
                Some(($target).to_string()),
                ($message).to_string(),
                fields,
                module_path!(),
                file!(),
                line!(),
            );
        }
    }};
    (level: $level:expr, target: $target:expr, $message:expr $(,)?) => {{
        if $crate::enabled() {
            let _ = $crate::emit_with_fields(
                $level,
                Some(($target).to_string()),
                ($message).to_string(),
                std::collections::BTreeMap::new(),
                module_path!(),
                file!(),
                line!(),
            );
        }
    }};
    ($message:expr; $($key:literal => $value:expr),* $(,)?) => {{
        if $crate::enabled() {
            let mut fields = std::collections::BTreeMap::new();
            $(
                fields.insert($key.to_string(), $crate::field_value(&$value));
            )*
            let _ = $crate::emit_with_fields(
                $crate::Level::Info,
                Some(module_path!().to_string()),
                ($message).to_string(),
                fields,
                module_path!(),
                file!(),
                line!(),
            );
        }
    }};
    ($message:expr $(,)?) => {{
        if $crate::enabled() {
            let _ = $crate::emit_with_fields(
                $crate::Level::Info,
                Some(module_path!().to_string()),
                ($message).to_string(),
                std::collections::BTreeMap::new(),
                module_path!(),
                file!(),
                line!(),
            );
        }
    }};
}

#[cfg(feature = "xtask-clap")]
#[macro_export]
macro_rules! xtask_clap_info {
    ($cli:ty) => {{
        if $crate::xtask::handle_nextdeck_info::<$cli>()? {
            return Ok(());
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serializes_schema_line() {
        let event = TestEvent::new(Level::Info, "cache hit")
            .with_target("artifact-cache")
            .with_fields(BTreeMap::from([(
                "key".to_owned(),
                Value::String("abc".to_owned()),
            )]))
            .with_source("demo::tests", "src/lib.rs", 42);

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"pid\":"));
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"target\":\"artifact-cache\""));
        assert!(json.contains("\"message\":\"cache hit\""));
        assert!(json.contains("\"module\":\"demo::tests\""));
    }

    #[test]
    fn event_target_directory_resolves_to_pid_file() {
        let dir = std::env::temp_dir().join(format!("nextdeck-test-events-{}", std::process::id()));

        assert_eq!(
            pid_event_file_path(&dir),
            dir.join(format!("{}.jsonl", std::process::id()))
        );
    }

    #[test]
    fn event_macro_field_values_do_not_move_callers_values() {
        let key = String::from("abc");

        event!("cache hit"; "key" => key);

        assert_eq!(key, "abc");
    }
}
