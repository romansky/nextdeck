use std::{collections::BTreeMap, path::PathBuf, process::Stdio};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::mpsc,
};

use crate::{
    input_field::{InputField, InputFieldInput},
    output::append_bounded_text,
    output_pane::OutputPaneState,
    request::RequestId,
};

pub const INFO_COMMAND: &str = "nextdeck-info";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct XtaskManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub commands: Vec<XtaskCommandSpec>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct XtaskCommandSpec {
    pub name: String,
    #[serde(default)]
    pub about: Option<String>,
    #[serde(default)]
    pub args: Vec<XtaskArgSpec>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct XtaskArgSpec {
    pub name: String,
    #[serde(default)]
    pub long: Option<String>,
    #[serde(default)]
    pub short: Option<char>,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub required: bool,
    pub value: XtaskValueSpec,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum XtaskValueSpec {
    Bool {
        #[serde(default)]
        default: bool,
    },
    Number {
        #[serde(default)]
        default: Option<i64>,
    },
    String {
        #[serde(default)]
        default: Option<String>,
    },
    Enum {
        values: Vec<String>,
        #[serde(default)]
        default: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XtaskDetailFocus {
    #[default]
    Parameters,
    Output,
}

#[derive(Clone, Debug)]
pub struct XtaskState {
    pub modal_open: bool,
    pub detail_open: bool,
    pub load_request_id: RequestId,
    pub run_request_id: RequestId,
    pub loading: bool,
    pub running: bool,
    pub error: Option<String>,
    pub manifest: Option<XtaskManifest>,
    pub selected_command: usize,
    pub selected_arg: usize,
    pub detail_focus: XtaskDetailFocus,
    pub output: OutputPaneState,
    pub values: BTreeMap<String, BTreeMap<String, XtaskArgValue>>,
    pub editing: Option<XtaskEditState>,
    pub last_run: Option<XtaskRunOutput>,
}

impl Default for XtaskState {
    fn default() -> Self {
        Self {
            modal_open: false,
            detail_open: false,
            load_request_id: RequestId::default(),
            run_request_id: RequestId::default(),
            loading: true,
            running: false,
            error: None,
            manifest: None,
            selected_command: 0,
            selected_arg: 0,
            detail_focus: XtaskDetailFocus::default(),
            output: OutputPaneState::default(),
            values: BTreeMap::new(),
            editing: None,
            last_run: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XtaskEditState {
    pub command: String,
    pub arg: String,
    pub input: InputField,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XtaskArgValue {
    Bool(bool),
    Number(String),
    String(String),
    Enum(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XtaskRunRequest {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XtaskRunOutput {
    pub command_line: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XtaskOutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XtaskRunChunk {
    pub stream: XtaskOutputStream,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XtaskEvent {
    InfoLoaded {
        request_id: RequestId,
        result: Result<XtaskManifest, String>,
    },
    RunOutput {
        request_id: RequestId,
        chunk: XtaskRunChunk,
    },
    RunFinished {
        request_id: RequestId,
        result: Result<XtaskRunOutput, String>,
    },
}

impl XtaskManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            bail!(
                "unsupported xtask schema version {}, expected {}",
                self.schema_version,
                SCHEMA_VERSION
            );
        }
        for command in &self.commands {
            validate_name("command", &command.name)?;
            for arg in &command.args {
                validate_name("arg", &arg.name)?;
                if let XtaskValueSpec::Enum { values, default } = &arg.value {
                    if values.is_empty() {
                        bail!("enum arg {}.{} has no values", command.name, arg.name);
                    }
                    if let Some(default) = default
                        && !values.contains(default)
                    {
                        bail!(
                            "enum arg {}.{} default is not in values",
                            command.name,
                            arg.name
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

impl XtaskArgSpec {
    pub fn flag(&self) -> String {
        format!("--{}", self.long.as_deref().unwrap_or(&self.name))
    }

    pub fn default_value(&self) -> XtaskArgValue {
        match &self.value {
            XtaskValueSpec::Bool { default } => XtaskArgValue::Bool(*default),
            XtaskValueSpec::Number { default } => {
                XtaskArgValue::Number(default.map(|value| value.to_string()).unwrap_or_default())
            }
            XtaskValueSpec::String { default } => {
                XtaskArgValue::String(default.clone().unwrap_or_default())
            }
            XtaskValueSpec::Enum { values, default } => XtaskArgValue::Enum(
                default
                    .clone()
                    .or_else(|| values.first().cloned())
                    .unwrap_or_default(),
            ),
        }
    }
}

impl XtaskArgValue {
    pub fn display(&self) -> String {
        match self {
            Self::Bool(value) => {
                if *value {
                    "on".to_owned()
                } else {
                    "off".to_owned()
                }
            }
            Self::Number(value) | Self::String(value) | Self::Enum(value) => value.clone(),
        }
    }
}

impl XtaskState {
    pub fn open(&mut self) {
        self.modal_open = true;
    }

    pub fn close(&mut self) {
        self.modal_open = false;
        self.detail_open = false;
        self.detail_focus = XtaskDetailFocus::Parameters;
        self.editing = None;
        self.output.search.close_interaction();
    }

    pub fn open_detail(&mut self) -> bool {
        if self.selected_command().is_none() {
            return false;
        }
        self.detail_open = true;
        self.detail_focus = XtaskDetailFocus::Parameters;
        self.editing = None;
        self.selected_arg = 0;
        self.output.scroll_top();
        self.output.search.close_interaction();
        true
    }

    pub fn close_detail(&mut self) {
        self.detail_open = false;
        self.detail_focus = XtaskDetailFocus::Parameters;
        self.editing = None;
        self.output.search.close_interaction();
    }

    pub fn begin_load(&mut self) -> RequestId {
        self.load_request_id = self.load_request_id.next();
        self.loading = true;
        self.error = None;
        self.detail_open = false;
        self.detail_focus = XtaskDetailFocus::Parameters;
        self.editing = None;
        self.output.search.close_interaction();
        self.load_request_id
    }

    pub fn apply_event(&mut self, event: XtaskEvent) -> bool {
        match event {
            XtaskEvent::InfoLoaded { request_id, result } => {
                if request_id != self.load_request_id {
                    return false;
                }
                self.loading = false;
                match result {
                    Ok(manifest) => self.set_manifest(manifest),
                    Err(error) => {
                        self.error = Some(error);
                        self.manifest = None;
                        self.detail_open = false;
                        self.values.clear();
                    }
                }
                true
            }
            XtaskEvent::RunOutput { request_id, chunk } => {
                if request_id != self.run_request_id {
                    return false;
                }
                if let Some(output) = &mut self.last_run {
                    match chunk.stream {
                        XtaskOutputStream::Stdout => {
                            append_bounded_text(&mut output.stdout, &chunk.text);
                        }
                        XtaskOutputStream::Stderr => {
                            append_bounded_text(&mut output.stderr, &chunk.text);
                        }
                    }
                }
                self.sync_output_scroll_to_content();
                true
            }
            XtaskEvent::RunFinished { request_id, result } => {
                if request_id != self.run_request_id {
                    return false;
                }
                self.running = false;
                match result {
                    Ok(output) => {
                        self.error = None;
                        self.last_run = Some(output);
                    }
                    Err(error) => self.error = Some(error),
                }
                self.sync_output_scroll_to_content();
                true
            }
        }
    }

    pub fn set_manifest(&mut self, manifest: XtaskManifest) {
        self.error = None;
        self.selected_command = self
            .selected_command
            .min(manifest.commands.len().saturating_sub(1));
        self.selected_arg = 0;
        self.values = default_values(&manifest);
        self.manifest = Some(manifest);
        self.detail_open = false;
        self.detail_focus = XtaskDetailFocus::Parameters;
        self.editing = None;
    }

    pub fn selected_command(&self) -> Option<&XtaskCommandSpec> {
        self.manifest.as_ref()?.commands.get(self.selected_command)
    }

    pub fn selected_value(&self) -> Option<&XtaskArgValue> {
        let command = self.selected_command()?;
        let arg = command.args.get(self.selected_arg)?;
        self.values.get(&command.name)?.get(&arg.name)
    }

    pub fn select_next_command(&mut self) {
        let Some(manifest) = &self.manifest else {
            return;
        };
        if manifest.commands.is_empty() {
            return;
        }
        self.selected_command = (self.selected_command + 1) % manifest.commands.len();
        self.selected_arg = 0;
    }

    pub fn select_previous_command(&mut self) {
        let Some(manifest) = &self.manifest else {
            return;
        };
        if manifest.commands.is_empty() {
            return;
        }
        self.selected_command =
            (self.selected_command + manifest.commands.len() - 1) % manifest.commands.len();
        self.selected_arg = 0;
    }

    pub fn select_next_arg(&mut self) {
        let Some(command) = self.selected_command() else {
            return;
        };
        if command.args.is_empty() {
            self.selected_arg = 0;
            return;
        }
        self.selected_arg = (self.selected_arg + 1) % command.args.len();
    }

    pub fn select_previous_arg(&mut self) {
        let Some(command) = self.selected_command() else {
            return;
        };
        if command.args.is_empty() {
            self.selected_arg = 0;
            return;
        }
        self.selected_arg = (self.selected_arg + command.args.len() - 1) % command.args.len();
    }

    pub fn adjust_selected_arg(&mut self, delta: i8) -> bool {
        let Some(command) = self.selected_command().cloned() else {
            return false;
        };
        let Some(arg) = command.args.get(self.selected_arg).cloned() else {
            return false;
        };
        let Some(values) = self.values.get_mut(&command.name) else {
            return false;
        };
        let Some(value) = values.get_mut(&arg.name) else {
            return false;
        };
        match (&arg.value, value) {
            (_, XtaskArgValue::Bool(value)) => {
                *value = !*value;
                true
            }
            (XtaskValueSpec::Enum { values, .. }, XtaskArgValue::Enum(value)) => {
                cycle_enum(value, values, delta);
                true
            }
            (_, XtaskArgValue::Number(value)) => {
                if let Ok(number) = value.parse::<i64>() {
                    *value = number.saturating_add(i64::from(delta)).to_string();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn begin_edit_selected_arg(&mut self) -> bool {
        self.detail_focus = XtaskDetailFocus::Parameters;
        let Some(command) = self.selected_command() else {
            return false;
        };
        let Some(arg) = command.args.get(self.selected_arg) else {
            return false;
        };
        if matches!(
            arg.value,
            XtaskValueSpec::Bool { .. } | XtaskValueSpec::Enum { .. }
        ) {
            return false;
        }
        let value = self
            .selected_value()
            .map(XtaskArgValue::display)
            .unwrap_or_default();
        let mut input = InputField::default();
        input.set_text(&value);
        self.editing = Some(XtaskEditState {
            command: command.name.clone(),
            arg: arg.name.clone(),
            input,
        });
        true
    }

    pub fn edit_input(&mut self, input: InputFieldInput) {
        if let Some(editing) = &mut self.editing {
            editing.input.input(input);
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    pub fn begin_run(&mut self, command_line: String) -> RequestId {
        self.run_request_id = self.run_request_id.next();
        self.running = true;
        self.error = None;
        self.last_run = Some(XtaskRunOutput {
            command_line,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
        });
        self.output.reset_for_source_change();
        self.run_request_id
    }

    pub fn scroll_output_page_up(&mut self) {
        self.output.scroll_up(self.output.page_size);
    }

    pub fn scroll_output_page_down(&mut self) {
        self.output.scroll_down(self.output.page_size);
    }

    pub fn scroll_output_line_up(&mut self) {
        self.output.scroll_up(1);
    }

    pub fn scroll_output_line_down(&mut self) {
        self.output.scroll_down(1);
    }

    pub fn scroll_output_top(&mut self) {
        self.output.scroll_top();
        self.output.disable_snap();
    }

    pub fn scroll_output_bottom(&mut self) {
        let line_count = self.output_text().lines().count().max(1);
        self.output.snap_to_bottom(line_count);
    }

    pub fn toggle_detail_focus(&mut self) {
        self.detail_focus = match self.detail_focus {
            XtaskDetailFocus::Parameters => XtaskDetailFocus::Output,
            XtaskDetailFocus::Output => XtaskDetailFocus::Parameters,
        };
    }

    pub fn focus_output(&mut self) {
        self.detail_focus = XtaskDetailFocus::Output;
    }

    pub fn focus_parameters(&mut self) {
        self.detail_focus = XtaskDetailFocus::Parameters;
    }

    pub fn commit_edit(&mut self) -> Result<()> {
        let Some(editing) = self.editing.take() else {
            return Ok(());
        };
        let value = editing.input.text();
        let arg = self
            .manifest
            .as_ref()
            .and_then(|manifest| {
                manifest
                    .commands
                    .iter()
                    .find(|command| command.name == editing.command)
            })
            .and_then(|command| command.args.iter().find(|arg| arg.name == editing.arg))
            .context("edited xtask argument is no longer available")?;
        if matches!(arg.value, XtaskValueSpec::Number { .. }) && !value.trim().is_empty() {
            value
                .parse::<i64>()
                .with_context(|| format!("{} must be a number", arg.name))?;
        }
        let values = self
            .values
            .get_mut(&editing.command)
            .context("edited xtask command values are no longer available")?;
        let slot = values
            .get_mut(&editing.arg)
            .context("edited xtask argument value is no longer available")?;
        *slot = match slot {
            XtaskArgValue::Number(_) => XtaskArgValue::Number(value),
            XtaskArgValue::String(_) => XtaskArgValue::String(value),
            XtaskArgValue::Bool(_) | XtaskArgValue::Enum(_) => slot.clone(),
        };
        Ok(())
    }

    pub fn run_request(&self) -> Result<XtaskRunRequest> {
        let command = self
            .selected_command()
            .context("no xtask command selected")?;
        let mut args = Vec::new();
        for spec in &command.args {
            let value = self
                .values
                .get(&command.name)
                .and_then(|values| values.get(&spec.name))
                .cloned()
                .unwrap_or_else(|| spec.default_value());
            append_arg(&mut args, spec, &value)?;
        }
        Ok(XtaskRunRequest {
            command: command.name.clone(),
            args,
        })
    }

    pub fn output_text(&self) -> String {
        let mut text = String::new();
        if self.running {
            text.push_str("Running xtask...\n\n");
        }
        if let Some(error) = &self.error {
            text.push_str("Error: ");
            text.push_str(error);
            text.push_str("\n\n");
        }
        if let Some(output) = &self.last_run {
            text.push_str("$ ");
            text.push_str(&output.command_line);
            text.push('\n');
            if !self.running {
                match output.exit_code {
                    Some(code) => {
                        text.push_str("exit code: ");
                        text.push_str(&code.to_string());
                    }
                    None => text.push_str("exit code: signal/unknown"),
                }
                text.push('\n');
            }
            append_output_text_section(&mut text, "stdout", &output.stdout);
            append_output_text_section(&mut text, "stderr", &output.stderr);
        }
        if text.trim().is_empty() {
            "Run the selected xtask to see output here.".to_owned()
        } else {
            text.trim_end().to_owned()
        }
    }

    fn sync_output_scroll_to_content(&mut self) {
        let line_count = self.output_text().lines().count().max(1);
        self.output.set_line_count(line_count);
    }
}

pub async fn load(cwd: Option<PathBuf>) -> Result<XtaskManifest> {
    let mut command = cargo_xtask_command(cwd);
    command.args([INFO_COMMAND, "--format", "json"]);
    let output = command
        .output()
        .await
        .context("running cargo xtask nextdeck-info --format json")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "cargo xtask nextdeck-info exited with {}: {}",
            output.status,
            stderr.trim()
        );
    }
    let manifest = serde_json::from_slice::<XtaskManifest>(&output.stdout)
        .context("parsing xtask info JSON")?;
    manifest.validate()?;
    Ok(manifest)
}

pub async fn run_streaming(
    cwd: Option<PathBuf>,
    request: XtaskRunRequest,
    chunk_tx: mpsc::Sender<XtaskRunChunk>,
) -> Result<XtaskRunOutput> {
    let command_line = request.command_line();
    let mut command = cargo_xtask_command(cwd);
    command.arg(&request.command).args(&request.args);
    let mut child = command
        .spawn()
        .with_context(|| format!("starting {command_line}"))?;
    let stdout = child
        .stdout
        .take()
        .context("xtask stdout pipe was not available")?;
    let stderr = child
        .stderr
        .take()
        .context("xtask stderr pipe was not available")?;
    let stdout_task = tokio::spawn(read_run_stream(
        stdout,
        XtaskOutputStream::Stdout,
        chunk_tx.clone(),
    ));
    let stderr_task = tokio::spawn(read_run_stream(stderr, XtaskOutputStream::Stderr, chunk_tx));
    let status = child
        .wait()
        .await
        .with_context(|| format!("running {command_line}"))?;
    let stdout = stdout_task.await.context("joining xtask stdout reader")??;
    let stderr = stderr_task.await.context("joining xtask stderr reader")??;
    Ok(XtaskRunOutput {
        command_line,
        success: status.success(),
        exit_code: status.code(),
        stdout,
        stderr,
    })
}

impl XtaskRunRequest {
    pub fn command_line(&self) -> String {
        let mut args = vec!["cargo".to_owned(), "xtask".to_owned(), self.command.clone()];
        args.extend(self.args.clone());
        shell_command(args)
    }
}

fn cargo_xtask_command(cwd: Option<PathBuf>) -> Command {
    let mut command = Command::new("cargo");
    command.arg("xtask");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

async fn read_run_stream<R>(
    mut reader: R,
    stream: XtaskOutputStream,
    chunk_tx: mpsc::Sender<XtaskRunChunk>,
) -> Result<String>
where
    R: AsyncRead + Unpin,
{
    let mut text = String::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buffer[..read]).to_string();
        append_bounded_text(&mut text, &chunk);
        let _ = chunk_tx
            .send(XtaskRunChunk {
                stream,
                text: chunk,
            })
            .await;
    }
    Ok(text)
}

fn append_output_text_section(text: &mut String, title: &str, content: &str) {
    let content = content.trim_end();
    if content.is_empty() {
        return;
    }
    if !text.ends_with("\n\n") {
        text.push('\n');
    }
    text.push_str(title);
    text.push('\n');
    text.push_str(content);
    text.push('\n');
}

fn default_values(manifest: &XtaskManifest) -> BTreeMap<String, BTreeMap<String, XtaskArgValue>> {
    manifest
        .commands
        .iter()
        .map(|command| {
            (
                command.name.clone(),
                command
                    .args
                    .iter()
                    .map(|arg| (arg.name.clone(), arg.default_value()))
                    .collect(),
            )
        })
        .collect()
}

fn append_arg(args: &mut Vec<String>, spec: &XtaskArgSpec, value: &XtaskArgValue) -> Result<()> {
    match value {
        XtaskArgValue::Bool(true) => args.push(spec.flag()),
        XtaskArgValue::Bool(false) => {}
        XtaskArgValue::Number(value) => append_value_arg(args, spec, value)?,
        XtaskArgValue::String(value) | XtaskArgValue::Enum(value) => {
            append_value_arg(args, spec, value)?;
        }
    }
    Ok(())
}

fn append_value_arg(args: &mut Vec<String>, spec: &XtaskArgSpec, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        if spec.required {
            bail!("{} is required", spec.name);
        }
        return Ok(());
    }
    let current = match &spec.value {
        XtaskValueSpec::Number { .. } => XtaskArgValue::Number(value.to_owned()),
        XtaskValueSpec::String { .. } => XtaskArgValue::String(value.to_owned()),
        XtaskValueSpec::Enum { values, .. } => {
            if !values.iter().any(|allowed| allowed == value) {
                bail!("{} must be one of {}", spec.name, values.join(", "));
            }
            XtaskArgValue::Enum(value.to_owned())
        }
        XtaskValueSpec::Bool { .. } => return Ok(()),
    };
    if !spec.required && current == spec.default_value() {
        return Ok(());
    }
    args.push(spec.flag());
    args.push(value.to_owned());
    Ok(())
}

fn cycle_enum(current: &mut String, values: &[String], delta: i8) {
    if values.is_empty() {
        return;
    }
    let index = values
        .iter()
        .position(|value| value == current)
        .unwrap_or_default();
    let len = values.len();
    let next = if delta < 0 {
        (index + len - 1) % len
    } else {
        (index + 1) % len
    };
    *current = values[next].clone();
}

fn validate_name(kind: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("{kind} name is empty");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        bail!("{kind} name contains unsupported characters: {name}");
    }
    Ok(())
}

fn shell_command(args: Vec<String>) -> String {
    args.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests;
