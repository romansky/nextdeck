use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "xtask-clap")]
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
    pub fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            time: now_millis(),
            level,
            target: None,
            message: message.into(),
            fields: BTreeMap::new(),
            source: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

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

    pub fn with_fields(mut self, fields: BTreeMap<String, Value>) -> Self {
        self.fields = fields;
        self
    }
}

pub fn enabled() -> bool {
    event_file_path().is_some()
}

pub fn event_file_path() -> Option<PathBuf> {
    std::env::var_os(ENV_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn emit(event: &TestEvent) -> io::Result<()> {
    let Some(path) = event_file_path() else {
        return Ok(());
    };
    let mut line = serde_json::to_vec(event).map_err(io::Error::other)?;
    line.push(b'\n');
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

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[macro_export]
macro_rules! event {
    (level: $level:expr, target: $target:expr, $message:expr; $($key:literal => $value:expr),* $(,)?) => {{
        if $crate::enabled() {
            let mut fields = std::collections::BTreeMap::new();
            $(
                fields.insert($key.to_string(), serde_json::json!($value));
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
                fields.insert($key.to_string(), serde_json::json!($value));
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
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"target\":\"artifact-cache\""));
        assert!(json.contains("\"message\":\"cache hit\""));
        assert!(json.contains("\"module\":\"demo::tests\""));
    }
}
