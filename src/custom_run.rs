use crate::{
    field_schema::{ParameterDetails, on_off},
    input_field::{InputField, InputFieldInput},
    nextest::{
        FailFast, FilterPreset, FlakyResult, RunConfig, RunIgnored, RunOptions, RunRequest,
        RunScope,
    },
    scroll::ViewportState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomRunState {
    pub open: bool,
    pub selected: CustomRunField,
    pub editing: Option<CustomRunEditField>,
    pub input: InputField,
    pub scope: CustomRunScope,
    pub profile_index: usize,
    pub filter: CustomRunFilter,
    pub options: RunOptions,
    pub run_config: RunConfig,
    pub viewport: ViewportState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CustomRunScope {
    #[default]
    Selected,
    Workspace,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CustomRunField {
    #[default]
    Scope,
    Profile,
    Filterset,
    Ignored,
    Retries,
    FlakyResult,
    FailFast,
    MaxFail,
    NoCapture,
    Debugger,
    StressCount,
    StressDuration,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum CustomRunFilter {
    #[default]
    None,
    Preset(usize),
    Custom(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomRunEditField {
    Filterset,
    MaxFail,
    Debugger,
    StressCount,
    StressDuration,
}

impl Default for CustomRunState {
    fn default() -> Self {
        Self {
            open: false,
            selected: CustomRunField::Scope,
            editing: None,
            input: InputField::default(),
            scope: CustomRunScope::Selected,
            profile_index: 0,
            filter: CustomRunFilter::None,
            options: RunOptions::default(),
            run_config: RunConfig::default(),
            viewport: ViewportState::default(),
        }
    }
}

impl CustomRunState {
    pub fn update_run_config(&mut self, run_config: RunConfig) {
        self.run_config = run_config;
        self.profile_index = self
            .profile_index
            .min(self.run_config.profiles.len().saturating_sub(1));
        if let CustomRunFilter::Preset(index) = self.filter
            && index >= self.run_config.filter_presets.len()
        {
            self.filter = CustomRunFilter::None;
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.editing = None;
        self.selected = CustomRunField::Scope;
        self.viewport.reset();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.editing = None;
    }

    pub fn selected_profile(&self) -> Option<&str> {
        self.run_config
            .profiles
            .get(self.profile_index)
            .map(|profile| profile.name.as_str())
    }

    pub fn selected_filter_preset(&self) -> Option<&FilterPreset> {
        match self.filter {
            CustomRunFilter::Preset(index) => self.run_config.filter_presets.get(index),
            CustomRunFilter::None | CustomRunFilter::Custom(_) => None,
        }
    }

    pub fn field_value(&self, field: CustomRunField, value_width: usize) -> String {
        if field
            .edit_field()
            .is_some_and(|edit_field| self.editing == Some(edit_field))
        {
            return format!("[{}]", self.input.view(value_width.saturating_sub(2), true));
        }

        match field {
            CustomRunField::Scope => self.scope.label().to_owned(),
            CustomRunField::Profile => self.selected_profile().unwrap_or("default").to_owned(),
            CustomRunField::Filterset => self.filter_value(),
            CustomRunField::Ignored => self.options.ignored.label().to_owned(),
            CustomRunField::Retries => optional_value(self.options.retries),
            CustomRunField::FlakyResult => self
                .options
                .flaky_result
                .map(|value| value.label().to_owned())
                .unwrap_or_else(|| "profile".to_owned()),
            CustomRunField::FailFast => self.options.fail_fast.label().to_owned(),
            CustomRunField::MaxFail => self
                .options
                .max_fail
                .clone()
                .unwrap_or_else(|| "profile".to_owned()),
            CustomRunField::NoCapture => on_off(self.options.no_capture).to_owned(),
            CustomRunField::Debugger => self
                .options
                .debugger
                .clone()
                .unwrap_or_else(|| "off".to_owned()),
            CustomRunField::StressCount => self
                .options
                .stress_count
                .clone()
                .unwrap_or_else(|| "off".to_owned()),
            CustomRunField::StressDuration => self
                .options
                .stress_duration
                .clone()
                .unwrap_or_else(|| "off".to_owned()),
        }
    }

    pub fn field_details(&self, field: CustomRunField) -> ParameterDetails {
        match field {
            CustomRunField::Scope => {
                ParameterDetails::enum_values(["selected", "workspace", "failed"])
                    .with_default("selected")
            }
            CustomRunField::Profile => {
                let profiles = self
                    .run_config
                    .profiles
                    .iter()
                    .map(|profile| profile.name.clone())
                    .collect::<Vec<_>>();
                let profiles = if profiles.is_empty() {
                    vec!["default".to_owned()]
                } else {
                    profiles
                };
                ParameterDetails::enum_values(profiles).with_default("default")
            }
            CustomRunField::Filterset => {
                let mut options = vec!["none".to_owned()];
                options.extend(
                    self.run_config
                        .filter_presets
                        .iter()
                        .map(|preset| preset.name().to_owned()),
                );
                if matches!(self.filter, CustomRunFilter::Custom(_)) {
                    options.push("custom".to_owned());
                    ParameterDetails::enum_values(options).with_default("none")
                } else {
                    ParameterDetails::enum_values(options)
                        .with_default("none")
                        .custom_value()
                }
            }
            CustomRunField::Ignored => {
                ParameterDetails::enum_values(["default", "only ignored", "all"])
                    .with_default("default")
            }
            CustomRunField::Retries => ParameterDetails::number()
                .with_choices(["profile", "0..10"])
                .with_default("profile"),
            CustomRunField::FlakyResult => {
                ParameterDetails::enum_values(["profile", "pass", "fail"]).with_default("profile")
            }
            CustomRunField::FailFast => {
                ParameterDetails::enum_values(["profile", "on", "off"]).with_default("profile")
            }
            CustomRunField::MaxFail => ParameterDetails::number()
                .with_choices(["profile", "0..20"])
                .with_default("profile")
                .custom_value(),
            CustomRunField::NoCapture => ParameterDetails::bool(false),
            CustomRunField::Debugger => ParameterDetails::string()
                .with_choices(["off", "rust-lldb --args"])
                .with_default("off")
                .custom_value(),
            CustomRunField::StressCount => ParameterDetails::number()
                .with_choices(["off", "0..100"])
                .with_default("off")
                .custom_value(),
            CustomRunField::StressDuration => ParameterDetails::string()
                .with_choices(["off", "30s"])
                .with_default("off")
                .custom_value(),
        }
    }

    pub fn next_field(&mut self) {
        self.selected = self.selected.next();
    }

    pub fn previous_field(&mut self) {
        self.selected = self.selected.previous();
    }

    pub fn selected_field_line_range(&self) -> (usize, usize, usize) {
        let selected = Self::field_line_index(self.selected);
        let selected_len = Self::field_line_count(self.selected);
        let line_count = self.line_count();
        (selected, selected_len, line_count)
    }

    pub fn line_count(&self) -> usize {
        CustomRunField::ALL
            .into_iter()
            .map(Self::field_line_count)
            .sum()
    }

    fn field_line_index(field: CustomRunField) -> usize {
        CustomRunField::ALL
            .into_iter()
            .take_while(|candidate| *candidate != field)
            .map(|candidate| 1 + usize::from(candidate.has_details()))
            .sum()
    }

    fn field_line_count(field: CustomRunField) -> usize {
        1 + usize::from(field.has_details())
    }

    pub fn adjust_selected(&mut self, delta: i8) {
        match self.selected {
            CustomRunField::Scope => self.scope = self.scope.adjust(delta),
            CustomRunField::Profile => self.adjust_profile(delta),
            CustomRunField::Filterset => self.adjust_filter(delta),
            CustomRunField::Ignored => {
                self.options.ignored = adjust_ignored(self.options.ignored, delta)
            }
            CustomRunField::Retries => {
                self.options.retries = adjust_optional_u32(self.options.retries, delta, 10)
            }
            CustomRunField::FlakyResult => {
                self.options.flaky_result = adjust_flaky(self.options.flaky_result, delta)
            }
            CustomRunField::FailFast => {
                self.options.fail_fast = adjust_fail_fast(self.options.fail_fast, delta)
            }
            CustomRunField::MaxFail => {
                self.options.max_fail =
                    adjust_optional_u32_string(self.options.max_fail.take(), delta, 20)
            }
            CustomRunField::NoCapture => self.options.no_capture = !self.options.no_capture,
            CustomRunField::Debugger => {
                if self.options.debugger.is_some() {
                    self.options.debugger = None;
                } else {
                    self.options.debugger = Some(default_debugger_command());
                }
            }
            CustomRunField::StressCount => {
                self.options.stress_count =
                    adjust_optional_u32_string(self.options.stress_count.take(), delta, 100);
            }
            CustomRunField::StressDuration => {
                if self.options.stress_duration.is_some() {
                    self.options.stress_duration = None;
                } else {
                    self.options.stress_duration = Some("30s".to_owned());
                }
            }
        }
    }

    pub fn begin_edit_selected(&mut self) -> bool {
        let Some(field) = self.selected.edit_field() else {
            return false;
        };
        self.editing = Some(field);
        self.input.set_text(&self.edit_value(field));
        true
    }

    pub fn edit_input(&mut self, input: InputFieldInput) -> bool {
        self.input.input(input)
    }

    pub fn commit_edit(&mut self) {
        let Some(field) = self.editing.take() else {
            return;
        };
        let value = empty_to_none(self.input.text());
        match field {
            CustomRunEditField::Filterset => {
                self.filter = value
                    .map(CustomRunFilter::Custom)
                    .unwrap_or(CustomRunFilter::None);
            }
            CustomRunEditField::MaxFail => self.options.max_fail = value,
            CustomRunEditField::Debugger => self.options.debugger = value,
            CustomRunEditField::StressCount => self.options.stress_count = value,
            CustomRunEditField::StressDuration => self.options.stress_duration = value,
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    pub fn run_options(&self) -> RunOptions {
        let mut options = self.options.clone();
        options.profile = self.selected_profile().map(ToOwned::to_owned);
        match &self.filter {
            CustomRunFilter::None | CustomRunFilter::Preset(_) => {}
            CustomRunFilter::Custom(expression) => {
                options.filterset = Some(expression.clone());
            }
        }
        options
    }

    pub fn build_request(
        &self,
        selected_scope: RunScope,
        failed_scope: Option<RunScope>,
    ) -> Result<RunRequest, String> {
        let mut scope = match self.scope {
            CustomRunScope::Selected => selected_scope,
            CustomRunScope::Workspace => RunScope::Workspace,
            CustomRunScope::Failed => {
                failed_scope.ok_or_else(|| "No failed tests to rerun".to_owned())?
            }
        };
        let mut options = self.run_options();

        if let Some(preset) = self.selected_filter_preset() {
            match preset {
                FilterPreset::Filterset { expression, .. } => {
                    options.filterset = Some(expression.clone());
                }
                FilterPreset::IgnoredReason { reason, tests } => {
                    scope = RunScope::TestSet {
                        label: format!("ignored: {reason}"),
                        tests: tests.clone(),
                    };
                    options.ignored = RunIgnored::Only;
                }
            }
        }

        if options.debugger.is_some() && !matches!(scope, RunScope::Test(_)) {
            return Err("Debugger requires a single selected test".to_owned());
        }
        if (options.stress_count.is_some() || options.stress_duration.is_some())
            && !matches!(scope, RunScope::Test(_))
        {
            return Err("Stress runs require a single selected test".to_owned());
        }

        Ok(RunRequest { scope, options })
    }

    fn adjust_profile(&mut self, delta: i8) {
        let count = self.run_config.profiles.len();
        if count == 0 {
            self.profile_index = 0;
            return;
        }
        self.profile_index = wrap_index(self.profile_index, count, delta);
    }

    fn adjust_filter(&mut self, delta: i8) {
        let preset_count = self.run_config.filter_presets.len();
        let custom = match &self.filter {
            CustomRunFilter::Custom(expression) => Some(expression.clone()),
            CustomRunFilter::None | CustomRunFilter::Preset(_) => None,
        };
        let item_count = 1 + preset_count + usize::from(custom.is_some());
        let current = match self.filter {
            CustomRunFilter::None => 0,
            CustomRunFilter::Preset(index) => 1 + index.min(preset_count.saturating_sub(1)),
            CustomRunFilter::Custom(_) => 1 + preset_count,
        };
        let next = wrap_index(current, item_count.max(1), delta);
        self.filter = if next == 0 {
            CustomRunFilter::None
        } else if next <= preset_count {
            CustomRunFilter::Preset(next - 1)
        } else {
            CustomRunFilter::Custom(custom.unwrap_or_default())
        };
    }

    fn edit_value(&self, field: CustomRunEditField) -> String {
        match field {
            CustomRunEditField::Filterset => match &self.filter {
                CustomRunFilter::Custom(expression) => expression.clone(),
                CustomRunFilter::Preset(index) => self
                    .run_config
                    .filter_presets
                    .get(*index)
                    .and_then(|preset| match preset {
                        FilterPreset::Filterset { expression, .. } => Some(expression.clone()),
                        FilterPreset::IgnoredReason { .. } => None,
                    })
                    .unwrap_or_default(),
                CustomRunFilter::None => String::new(),
            },
            CustomRunEditField::MaxFail => self.options.max_fail.clone().unwrap_or_default(),
            CustomRunEditField::Debugger => self.options.debugger.clone().unwrap_or_default(),
            CustomRunEditField::StressCount => {
                self.options.stress_count.clone().unwrap_or_default()
            }
            CustomRunEditField::StressDuration => {
                self.options.stress_duration.clone().unwrap_or_default()
            }
        }
    }

    fn filter_value(&self) -> String {
        match &self.filter {
            CustomRunFilter::None => "none".to_owned(),
            CustomRunFilter::Custom(expression) => format!("custom: {expression}"),
            CustomRunFilter::Preset(index) => self
                .run_config
                .filter_presets
                .get(*index)
                .map(|preset| format!("preset: {}", preset.name()))
                .unwrap_or_else(|| "none".to_owned()),
        }
    }
}

impl CustomRunScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Workspace => "workspace",
            Self::Failed => "failed",
        }
    }

    const fn adjust(self, delta: i8) -> Self {
        if delta >= 0 {
            match self {
                Self::Selected => Self::Workspace,
                Self::Workspace => Self::Failed,
                Self::Failed => Self::Selected,
            }
        } else {
            match self {
                Self::Selected => Self::Failed,
                Self::Workspace => Self::Selected,
                Self::Failed => Self::Workspace,
            }
        }
    }
}

impl CustomRunField {
    pub const ALL: [Self; 12] = [
        Self::Scope,
        Self::Profile,
        Self::Filterset,
        Self::Ignored,
        Self::Retries,
        Self::FlakyResult,
        Self::FailFast,
        Self::MaxFail,
        Self::NoCapture,
        Self::Debugger,
        Self::StressCount,
        Self::StressDuration,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Scope => "scope",
            Self::Profile => "profile",
            Self::Filterset => "filterset",
            Self::Ignored => "ignored",
            Self::Retries => "retries",
            Self::FlakyResult => "flaky",
            Self::FailFast => "fail-fast",
            Self::MaxFail => "max-fail",
            Self::NoCapture => "no-capture",
            Self::Debugger => "debugger",
            Self::StressCount => "stress-count",
            Self::StressDuration => "stress-duration",
        }
    }

    fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub const fn edit_field(self) -> Option<CustomRunEditField> {
        match self {
            Self::Filterset => Some(CustomRunEditField::Filterset),
            Self::MaxFail => Some(CustomRunEditField::MaxFail),
            Self::Debugger => Some(CustomRunEditField::Debugger),
            Self::StressCount => Some(CustomRunEditField::StressCount),
            Self::StressDuration => Some(CustomRunEditField::StressDuration),
            Self::Scope
            | Self::Profile
            | Self::Ignored
            | Self::Retries
            | Self::FlakyResult
            | Self::FailFast
            | Self::NoCapture => None,
        }
    }

    const fn has_details(self) -> bool {
        true
    }
}

fn adjust_ignored(value: RunIgnored, delta: i8) -> RunIgnored {
    if delta >= 0 {
        value.next()
    } else {
        value.previous()
    }
}

fn adjust_flaky(value: Option<FlakyResult>, delta: i8) -> Option<FlakyResult> {
    match (value, delta >= 0) {
        (None, true) => Some(FlakyResult::Pass),
        (Some(FlakyResult::Pass), true) => Some(FlakyResult::Fail),
        (Some(FlakyResult::Fail), true) => None,
        (None, false) => Some(FlakyResult::Fail),
        (Some(FlakyResult::Fail), false) => Some(FlakyResult::Pass),
        (Some(FlakyResult::Pass), false) => None,
    }
}

fn adjust_fail_fast(value: FailFast, delta: i8) -> FailFast {
    if delta >= 0 {
        value.next()
    } else {
        value.previous()
    }
}

fn adjust_optional_u32(value: Option<u32>, delta: i8, max: u32) -> Option<u32> {
    match (value, delta >= 0) {
        (None, true) => Some(0),
        (Some(current), true) if current < max => Some(current + 1),
        (Some(_), true) => None,
        (None, false) => Some(max),
        (Some(0), false) => None,
        (Some(current), false) => Some(current - 1),
    }
}

fn adjust_optional_u32_string(value: Option<String>, delta: i8, max: u32) -> Option<String> {
    let parsed = value.and_then(|value| value.parse::<u32>().ok());
    adjust_optional_u32(parsed, delta, max).map(|value| value.to_string())
}

fn wrap_index(index: usize, count: usize, delta: i8) -> usize {
    if count == 0 {
        return 0;
    }
    if delta >= 0 {
        (index + 1) % count
    } else {
        (index + count - 1) % count
    }
}

fn empty_to_none(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn optional_value(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "profile".to_owned())
}

fn default_debugger_command() -> String {
    if cfg!(target_os = "macos") {
        "rust-lldb --args".to_owned()
    } else {
        "rust-gdb --args".to_owned()
    }
}
