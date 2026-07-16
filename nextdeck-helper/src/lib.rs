use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod xtask;

pub const ENV_VAR: &str = "NEXTDECK_TEST_EVENTS";
pub const ENV_VALUE: &str = "stdio-v1";
pub const FRAME_PREFIX: &str = "NEXTDECK_EVENT_V1 ";
pub const SCHEMA_VERSION: u32 = 1;

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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
    pub sequence: u64,
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
            sequence: EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
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
    std::env::var(ENV_VAR).is_ok_and(|value| value == ENV_VALUE)
}

pub fn emit(event: &TestEvent) -> io::Result<()> {
    if !enabled() {
        return Ok(());
    }
    let json = serde_json::to_vec(event).map_err(io::Error::other)?;
    let mut frame = Vec::with_capacity(FRAME_PREFIX.len() + json.len() + 1);
    frame.extend_from_slice(FRAME_PREFIX.as_bytes());
    frame.extend_from_slice(&json);
    frame.push(b'\n');
    io::stdout().lock().write_all(&frame)
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

#[doc(hidden)]
pub fn emit_with_fields_best_effort(
    level: Level,
    target: Option<String>,
    message: String,
    fields: BTreeMap<String, Value>,
    module: &'static str,
    file: &'static str,
    line: u32,
) {
    match emit_with_fields(level, target, message, fields, module, file, line) {
        Ok(()) => {}
        Err(_error) => {
            // Structured events are optional telemetry and must not change test behavior.
        }
    }
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
            $crate::emit_with_fields_best_effort(
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
            $crate::emit_with_fields_best_effort(
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
            $crate::emit_with_fields_best_effort(
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
            $crate::emit_with_fields_best_effort(
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
        assert!(json.contains("\"sequence\":"));
        assert!(json.contains("\"pid\":"));
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"target\":\"artifact-cache\""));
        assert!(json.contains("\"message\":\"cache hit\""));
        assert!(json.contains("\"module\":\"demo::tests\""));
    }

    #[test]
    fn event_frame_is_versioned_json() {
        let event = TestEvent::new(Level::Warn, "slow test");
        let json = serde_json::to_string(&event).unwrap();
        let frame = format!("{FRAME_PREFIX}{json}\n");

        assert!(frame.starts_with(FRAME_PREFIX));
        assert!(frame.ends_with("\n"));
    }

    #[test]
    fn event_macro_field_values_do_not_move_callers_values() {
        let key = String::from("abc");

        event!("cache hit"; "key" => key);

        assert_eq!(key, "abc");
    }
}
