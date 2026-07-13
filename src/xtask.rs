use std::{path::PathBuf, process::Stdio};

use anyhow::{Context, Result, bail};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::mpsc,
};

use crate::{
    field_schema::ParameterDetails,
    input_field::{InputField, InputFieldInput},
    output::append_bounded_text,
    output_pane::OutputPaneState,
    request::RequestId,
    scroll::ViewportState,
};
pub use nextdeck_test_events::xtask::{
    INFO_COMMAND, SCHEMA_VERSION, XtaskArg as XtaskArgSpec, XtaskCommand as XtaskCommandSpec,
    XtaskManifest, XtaskValue as XtaskValueSpec,
};

mod persistence;
mod preferences;

pub(crate) use persistence::XtaskPersistence;
use preferences::{XtaskArgValue, XtaskPreferences};

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
    pub parameters_viewport: ViewportState,
    pub detail_focus: XtaskDetailFocus,
    pub output: OutputPaneState,
    pub editing: Option<XtaskEditState>,
    pub last_run: Option<XtaskRunOutput>,
    preferences: XtaskPreferences,
    preferences_revision: u64,
    persisted_preferences_revision: u64,
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
            parameters_viewport: ViewportState::default(),
            detail_focus: XtaskDetailFocus::default(),
            output: OutputPaneState::default(),
            editing: None,
            last_run: None,
            preferences: XtaskPreferences::default(),
            preferences_revision: 0,
            persisted_preferences_revision: 0,
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
pub struct XtaskRunRequest {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XtaskRunOutput {
    pub command_line: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub combined: String,
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

pub(crate) trait XtaskManifestExt {
    fn validate(&self) -> Result<()>;
}

impl XtaskManifestExt for XtaskManifest {
    fn validate(&self) -> Result<()> {
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

pub(crate) trait XtaskArgSpecExt {
    fn flag(&self) -> String;
    fn parameter_line_count(&self) -> usize;
}

impl XtaskArgSpecExt for XtaskArgSpec {
    fn flag(&self) -> String {
        format!("--{}", self.long.as_deref().unwrap_or(&self.name))
    }

    fn parameter_line_count(&self) -> usize {
        2
    }
}

trait XtaskArgDefaultExt {
    fn default_value(&self) -> XtaskArgValue;
}

impl XtaskArgDefaultExt for XtaskArgSpec {
    fn default_value(&self) -> XtaskArgValue {
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

pub(crate) trait XtaskValueSpecExt {
    fn parameter_details(&self) -> ParameterDetails;
}

impl XtaskValueSpecExt for XtaskValueSpec {
    fn parameter_details(&self) -> ParameterDetails {
        match self {
            XtaskValueSpec::Bool { default } => ParameterDetails::bool(*default),
            XtaskValueSpec::Number {
                default: Some(default),
            } => ParameterDetails::number().with_default(default.to_string()),
            XtaskValueSpec::Number { default: None } => ParameterDetails::number(),
            XtaskValueSpec::String {
                default: Some(default),
            } if !default.trim().is_empty() => {
                ParameterDetails::string().with_default(default.clone())
            }
            XtaskValueSpec::String { .. } => ParameterDetails::string(),
            XtaskValueSpec::Enum { values, default } => {
                let details = ParameterDetails::enum_values(values.clone());
                if let Some(default) = default {
                    details.with_default(default.clone())
                } else {
                    details
                }
            }
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
        self.parameters_viewport.reset();
        self.ensure_selected_parameter_visible();
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
                        if let Some(manifest) = self.manifest.as_ref() {
                            self.preferences = self.preferences.overrides_for(manifest);
                        }
                        self.manifest = None;
                        self.detail_open = false;
                    }
                }
                true
            }
            XtaskEvent::RunOutput { request_id, chunk } => {
                if request_id != self.run_request_id {
                    return false;
                }
                if let Some(output) = &mut self.last_run {
                    append_bounded_text(&mut output.combined, &chunk.text);
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
                    Ok(mut output) => {
                        if let Some(live_output) = &self.last_run
                            && live_output.command_line == output.command_line
                        {
                            output.combined = live_output.combined.clone();
                        }
                        if output.combined.is_empty() {
                            output.combined =
                                combined_output_fallback(&output.stdout, &output.stderr);
                        }
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
        let previous_preferences = self.persistable_preferences();
        self.error = None;
        self.selected_command = self
            .selected_command
            .min(manifest.commands.len().saturating_sub(1));
        self.selected_arg = 0;
        self.preferences.reconcile(&manifest);
        let next_preferences = self.preferences.overrides_for(&manifest);
        if next_preferences != previous_preferences {
            self.mark_preferences_dirty();
        }
        self.manifest = Some(manifest);
        self.detail_open = false;
        self.detail_focus = XtaskDetailFocus::Parameters;
        self.parameters_viewport.reset();
        self.editing = None;
    }

    pub fn selected_command(&self) -> Option<&XtaskCommandSpec> {
        self.manifest.as_ref()?.commands.get(self.selected_command)
    }

    fn selected_value(&self) -> Option<&XtaskArgValue> {
        let command = self.selected_command()?;
        let arg = command.args.get(self.selected_arg)?;
        self.preferences.value(&command.name, &arg.name)
    }

    pub(crate) fn arg_value_display(&self, command: &str, arg: &str) -> String {
        self.preferences
            .value(command, arg)
            .map(XtaskArgValue::display)
            .unwrap_or_default()
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
        self.parameters_viewport.reset();
        self.ensure_selected_parameter_visible();
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
        self.parameters_viewport.reset();
        self.ensure_selected_parameter_visible();
    }

    pub fn select_next_arg(&mut self) {
        let Some(command) = self.selected_command() else {
            return;
        };
        if command.args.is_empty() {
            self.selected_arg = 0;
            self.parameters_viewport.reset();
            return;
        }
        self.selected_arg = (self.selected_arg + 1) % command.args.len();
        self.ensure_selected_parameter_visible();
    }

    pub fn select_previous_arg(&mut self) {
        let Some(command) = self.selected_command() else {
            return;
        };
        if command.args.is_empty() {
            self.selected_arg = 0;
            self.parameters_viewport.reset();
            return;
        }
        self.selected_arg = (self.selected_arg + command.args.len() - 1) % command.args.len();
        self.ensure_selected_parameter_visible();
    }

    pub fn apply_parameters_viewport_metrics(&mut self, page_size: usize) {
        let previous_page_size = self.parameters_viewport.page_size();
        self.parameters_viewport.set_page_size(page_size);
        self.sync_parameters_viewport_content();
        if self.parameters_viewport.page_size() != previous_page_size {
            self.ensure_selected_parameter_visible();
        }
    }

    pub fn sync_parameters_viewport_content(&mut self) {
        let line_count = self
            .selected_command()
            .map(XtaskCommandSpecExt::parameter_panel_line_count)
            .unwrap_or(1);
        self.parameters_viewport.set_content_len(line_count);
    }

    pub fn adjust_selected_arg(&mut self, delta: i8) -> bool {
        let Some(command) = self.selected_command().cloned() else {
            return false;
        };
        let Some(arg) = command.args.get(self.selected_arg).cloned() else {
            return false;
        };
        let Some(value) = self.preferences.value_mut(&command.name, &arg.name) else {
            return false;
        };
        let changed = match (&arg.value, value) {
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
        };
        if changed {
            self.mark_preferences_dirty();
        }
        changed
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
            combined: String::new(),
            stdout: String::new(),
            stderr: String::new(),
        });
        self.output.reset_for_source_change();
        self.run_request_id
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
        let slot = self
            .preferences
            .value_mut(&editing.command, &editing.arg)
            .context("edited xtask argument value is no longer available")?;
        let next = match slot {
            XtaskArgValue::Number(_) => XtaskArgValue::Number(value),
            XtaskArgValue::String(_) => XtaskArgValue::String(value),
            XtaskArgValue::Bool(_) | XtaskArgValue::Enum(_) => slot.clone(),
        };
        if *slot != next {
            *slot = next;
            self.mark_preferences_dirty();
        }
        Ok(())
    }

    pub fn run_request(&self) -> Result<XtaskRunRequest> {
        let command = self
            .selected_command()
            .context("no xtask command selected")?;
        let mut args = Vec::new();
        for spec in &command.args {
            let value = self
                .preferences
                .value(&command.name, &spec.name)
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
            text.push_str(&output.combined);
        }
        if text.trim().is_empty() {
            "Run the selected xtask to see output here.".to_owned()
        } else {
            text.trim_end().to_owned()
        }
    }

    fn sync_output_scroll_to_content(&mut self) {
        let line_count = self.output.output_view(&self.output_text()).line_count();
        self.output.apply_content_len(line_count);
    }

    fn ensure_selected_parameter_visible(&mut self) {
        let Some((selected_line, selected_len, line_count)) = self.selected_parameter_range()
        else {
            self.parameters_viewport.reset();
            return;
        };
        self.parameters_viewport.set_content_len(line_count);
        self.parameters_viewport
            .ensure_range_visible(selected_line, selected_len);
    }

    fn selected_parameter_range(&self) -> Option<(usize, usize, usize)> {
        let command = self.selected_command()?;
        let selected_len = command
            .args
            .get(self.selected_arg)
            .map(XtaskArgSpec::parameter_line_count)
            .unwrap_or(1);
        Some((
            command.parameter_line_index(self.selected_arg),
            selected_len,
            command.parameter_panel_line_count(),
        ))
    }

    fn restore_preferences(&mut self, preferences: XtaskPreferences) {
        self.preferences = preferences;
        self.preferences_revision = 0;
        self.persisted_preferences_revision = 0;
        if let Some(manifest) = self.manifest.clone() {
            let restored = self.preferences.clone();
            self.preferences.reconcile(&manifest);
            if self.preferences.overrides_for(&manifest) != restored {
                self.mark_preferences_dirty();
            }
        }
    }

    fn pending_preferences(&self) -> Option<(u64, XtaskPreferences)> {
        (self.preferences_revision != self.persisted_preferences_revision)
            .then(|| (self.preferences_revision, self.persistable_preferences()))
    }

    fn mark_preferences_persisted(&mut self, revision: u64) {
        if self.preferences_revision == revision {
            self.persisted_preferences_revision = revision;
        }
    }

    fn persistable_preferences(&self) -> XtaskPreferences {
        self.manifest
            .as_ref()
            .map(|manifest| self.preferences.overrides_for(manifest))
            .unwrap_or_else(|| self.preferences.clone())
    }

    fn mark_preferences_dirty(&mut self) {
        self.preferences_revision = self.preferences_revision.wrapping_add(1);
    }
}

trait XtaskCommandSpecExt {
    fn parameter_line_count(&self) -> usize;
    fn parameter_panel_line_count(&self) -> usize;
    fn parameter_line_index(&self, selected_arg: usize) -> usize;
}

impl XtaskCommandSpecExt for XtaskCommandSpec {
    fn parameter_line_count(&self) -> usize {
        if self.args.is_empty() {
            return 1;
        }
        self.args
            .iter()
            .map(XtaskArgSpec::parameter_line_count)
            .sum()
    }

    fn parameter_panel_line_count(&self) -> usize {
        let has_about = self
            .about
            .as_deref()
            .map(str::trim)
            .is_some_and(|about| !about.is_empty());
        self.parameter_line_count() + 1 + usize::from(has_about) + 1
    }

    fn parameter_line_index(&self, selected_arg: usize) -> usize {
        self.args
            .iter()
            .take(selected_arg.min(self.args.len()))
            .map(XtaskArgSpec::parameter_line_count)
            .sum()
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
        combined: String::new(),
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

fn combined_output_fallback(stdout: &str, stderr: &str) -> String {
    let mut combined = String::new();
    append_bounded_text(&mut combined, stdout);
    if !combined.is_empty() && !combined.ends_with('\n') && !stderr.is_empty() {
        append_bounded_text(&mut combined, "\n");
    }
    append_bounded_text(&mut combined, stderr);
    combined
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
