use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    input::InputEvent,
    input_field::{InputFieldInput, InputFieldKey},
    scroll::ScrollAction,
    test_events::TestEventsFocus,
    xtask::XtaskDetailFocus,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppCommand {
    Noop,
    Quit,
    Resize,
    StopRun,
    ToggleFocus,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ToggleSelected,
    MoveHome,
    MoveEnd,
    Scroll(ScrollAction),
    PageUp,
    PageDown,
    ActivateSelected,
    NarrowTestsPane,
    WidenTestsPane,
    RefreshTests,
    RunSelected,
    OpenCustomRun,
    CloseCustomRun,
    CustomRunNext,
    CustomRunPrevious,
    CustomRunAdjustLeft,
    CustomRunAdjustRight,
    CustomRunActivate,
    CustomRunEdit(InputFieldInput),
    CommitCustomRunEdit,
    CancelCustomRunEdit,
    RunCustom,
    CaptureTestSnapshot,
    OpenSource,
    OpenOutput,
    CloseTestDetails,
    OpenSettings,
    CloseSettings,
    SettingsNext,
    SettingsPrevious,
    SettingsAdjustLeft,
    SettingsAdjustRight,
    SettingsActivate,
    SettingsOpenWithEdit(InputFieldInput),
    CommitOpenWithSetting,
    CancelOpenWithSetting,
    RefreshDiskUsage,
    OpenDiskCleanup,
    CloseDiskCleanup,
    RunCargoClean,
    OpenTestEvents,
    CloseTestEvents,
    ToggleTestEventsFocus,
    TestEventsNextRun,
    TestEventsPreviousRun,
    OpenXtasks,
    CloseXtasks,
    RefreshXtasks,
    OpenSelectedXtask,
    CloseXtaskDetails,
    XtaskNextCommand,
    XtaskPreviousCommand,
    XtaskNextArg,
    XtaskPreviousArg,
    XtaskAdjustLeft,
    XtaskAdjustRight,
    XtaskActivateArg,
    ToggleXtaskDetailFocus,
    XtaskEdit(InputFieldInput),
    CommitXtaskEdit,
    CancelXtaskEdit,
    RunXtask,
    ToggleShowSuccess,
    ToggleShowFailed,
    ToggleShowIgnored,
    ToggleShowSkipped,
    SelectNextFailed,
    SelectPreviousFailed,
    StartOutputSearch,
    OpenOutputSearchModal,
    OutputSearchEdit(InputFieldInput),
    ClearOutputSearch,
    ApplyOutputSearch,
    CancelOutputSearch,
    SearchModalNextControl,
    SearchModalPreviousControl,
    SearchModalActivate,
    FindNextOutputMatch,
    FindPreviousOutputMatch,
    ToggleOutputFilter,
    ToggleOutputRegex,
    ToggleOutputCaseSensitive,
    ToggleOutputSnap,
    ReportStatus(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandGroup {
    Navigation,
    Runs,
    View,
    Output,
    Global,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKind {
    Quit,
    StopRun,
    ToggleFocus,
    MoveUpDown,
    MoveLeftRight,
    ActivateSelected,
    ToggleSelected,
    MoveHomeEnd,
    PageUpDown,
    NarrowTestsPane,
    WidenTestsPane,
    RefreshTests,
    RunSelected,
    OpenCustomRun,
    OpenSource,
    OpenOutput,
    OpenSettings,
    OpenXtasks,
    RefreshDiskUsage,
    OpenDiskCleanup,
    OpenTestEvents,
    ToggleShowSuccess,
    ToggleShowFailed,
    ToggleShowIgnored,
    ToggleShowSkipped,
    SelectFailed,
    ToggleOutputSnap,
    StartOutputSearch,
    FindOutputMatch,
    ToggleOutputFilter,
    ToggleOutputRegex,
    ToggleOutputCaseSensitive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandInfo {
    pub kind: CommandKind,
    pub group: CommandGroup,
    pub keys: &'static str,
    pub label: &'static str,
    pub ticker: &'static str,
}

const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        kind: CommandKind::MoveUpDown,
        group: CommandGroup::Navigation,
        keys: "Up/Down",
        label: "move selection",
        ticker: "move",
    },
    CommandInfo {
        kind: CommandKind::PageUpDown,
        group: CommandGroup::Navigation,
        keys: "PageUp/PageDown",
        label: "page active pane",
        ticker: "page",
    },
    CommandInfo {
        kind: CommandKind::MoveHomeEnd,
        group: CommandGroup::Navigation,
        keys: "Home/End",
        label: "first or last row",
        ticker: "jump",
    },
    CommandInfo {
        kind: CommandKind::MoveLeftRight,
        group: CommandGroup::Navigation,
        keys: "Left/Right",
        label: "collapse or expand",
        ticker: "fold",
    },
    CommandInfo {
        kind: CommandKind::ActivateSelected,
        group: CommandGroup::Navigation,
        keys: "Enter",
        label: "open selected details",
        ticker: "activate",
    },
    CommandInfo {
        kind: CommandKind::ToggleSelected,
        group: CommandGroup::Navigation,
        keys: "Space",
        label: "toggle selected branch",
        ticker: "toggle",
    },
    CommandInfo {
        kind: CommandKind::ToggleFocus,
        group: CommandGroup::Navigation,
        keys: "Tab",
        label: "switch tree/output focus",
        ticker: "focus",
    },
    CommandInfo {
        kind: CommandKind::NarrowTestsPane,
        group: CommandGroup::Navigation,
        keys: "Shift+Left/[",
        label: "narrow tests pane",
        ticker: "narrow tests",
    },
    CommandInfo {
        kind: CommandKind::WidenTestsPane,
        group: CommandGroup::Navigation,
        keys: "Shift+Right/]",
        label: "widen tests pane",
        ticker: "widen tests",
    },
    CommandInfo {
        kind: CommandKind::RefreshTests,
        group: CommandGroup::Runs,
        keys: "u",
        label: "update test list",
        ticker: "update tests",
    },
    CommandInfo {
        kind: CommandKind::RunSelected,
        group: CommandGroup::Runs,
        keys: "r",
        label: "run selected scope",
        ticker: "run",
    },
    CommandInfo {
        kind: CommandKind::OpenCustomRun,
        group: CommandGroup::Runs,
        keys: "R",
        label: "run custom",
        ticker: "run-custom",
    },
    CommandInfo {
        kind: CommandKind::OpenSource,
        group: CommandGroup::Runs,
        keys: "o",
        label: "open selected test source",
        ticker: "open source",
    },
    CommandInfo {
        kind: CommandKind::RefreshDiskUsage,
        group: CommandGroup::Global,
        keys: "r",
        label: "refresh disk usage",
        ticker: "disk refresh",
    },
    CommandInfo {
        kind: CommandKind::OpenDiskCleanup,
        group: CommandGroup::Global,
        keys: "D",
        label: "open disk cleanup",
        ticker: "disk cleanup",
    },
    CommandInfo {
        kind: CommandKind::OpenTestEvents,
        group: CommandGroup::Global,
        keys: "E",
        label: "open test events",
        ticker: "events",
    },
    CommandInfo {
        kind: CommandKind::OpenXtasks,
        group: CommandGroup::Global,
        keys: "X",
        label: "open xtasks",
        ticker: "xtasks",
    },
    CommandInfo {
        kind: CommandKind::SelectFailed,
        group: CommandGroup::Runs,
        keys: "j/J",
        label: "next or previous failure",
        ticker: "failed",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowSuccess,
        group: CommandGroup::View,
        keys: "p",
        label: "toggle passed tests",
        ticker: "toggle success",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowFailed,
        group: CommandGroup::View,
        keys: "f",
        label: "toggle failed tests",
        ticker: "toggle failed",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowIgnored,
        group: CommandGroup::View,
        keys: "i",
        label: "toggle ignored tests",
        ticker: "toggle ignored",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowSkipped,
        group: CommandGroup::View,
        keys: "s",
        label: "toggle skipped tests",
        ticker: "toggle skipped",
    },
    CommandInfo {
        kind: CommandKind::ToggleOutputSnap,
        group: CommandGroup::Output,
        keys: "s",
        label: "toggle output snap",
        ticker: "snap",
    },
    CommandInfo {
        kind: CommandKind::StartOutputSearch,
        group: CommandGroup::Output,
        keys: "/",
        label: "search output",
        ticker: "search output",
    },
    CommandInfo {
        kind: CommandKind::FindOutputMatch,
        group: CommandGroup::Output,
        keys: "n/N",
        label: "next or previous output match",
        ticker: "match",
    },
    CommandInfo {
        kind: CommandKind::ToggleOutputFilter,
        group: CommandGroup::Output,
        keys: "f",
        label: "toggle output match filter",
        ticker: "output filter",
    },
    CommandInfo {
        kind: CommandKind::ToggleOutputRegex,
        group: CommandGroup::Output,
        keys: "r",
        label: "toggle output regex",
        ticker: "regex",
    },
    CommandInfo {
        kind: CommandKind::ToggleOutputCaseSensitive,
        group: CommandGroup::Output,
        keys: "c",
        label: "toggle output case sensitivity",
        ticker: "case",
    },
    CommandInfo {
        kind: CommandKind::OpenOutput,
        group: CommandGroup::Output,
        keys: "o",
        label: "open output as text file",
        ticker: "open output",
    },
    CommandInfo {
        kind: CommandKind::OpenSettings,
        group: CommandGroup::Global,
        keys: ",",
        label: "open global settings",
        ticker: "settings",
    },
    CommandInfo {
        kind: CommandKind::StopRun,
        group: CommandGroup::Global,
        keys: "Ctrl+C",
        label: "stop running tests",
        ticker: "stop run",
    },
    CommandInfo {
        kind: CommandKind::Quit,
        group: CommandGroup::Global,
        keys: "Q",
        label: "quit",
        ticker: "quit",
    },
];

impl AppCommand {
    pub fn kind(&self) -> Option<CommandKind> {
        match self {
            Self::Noop | Self::Resize | Self::ReportStatus(_) => None,
            Self::Quit => Some(CommandKind::Quit),
            Self::StopRun => Some(CommandKind::StopRun),
            Self::ToggleFocus => Some(CommandKind::ToggleFocus),
            Self::MoveUp | Self::MoveDown => Some(CommandKind::MoveUpDown),
            Self::MoveLeft | Self::MoveRight => Some(CommandKind::MoveLeftRight),
            Self::ActivateSelected => Some(CommandKind::ActivateSelected),
            Self::ToggleSelected => Some(CommandKind::ToggleSelected),
            Self::MoveHome | Self::MoveEnd => Some(CommandKind::MoveHomeEnd),
            Self::PageUp | Self::PageDown => Some(CommandKind::PageUpDown),
            Self::Scroll(ScrollAction::PageUp | ScrollAction::PageDown) => {
                Some(CommandKind::PageUpDown)
            }
            Self::NarrowTestsPane => Some(CommandKind::NarrowTestsPane),
            Self::WidenTestsPane => Some(CommandKind::WidenTestsPane),
            Self::RefreshTests => Some(CommandKind::RefreshTests),
            Self::RunSelected => Some(CommandKind::RunSelected),
            Self::OpenCustomRun => Some(CommandKind::OpenCustomRun),
            Self::OpenSource => Some(CommandKind::OpenSource),
            Self::OpenOutput => Some(CommandKind::OpenOutput),
            Self::OpenSettings => Some(CommandKind::OpenSettings),
            Self::OpenXtasks => Some(CommandKind::OpenXtasks),
            Self::RefreshDiskUsage => Some(CommandKind::RefreshDiskUsage),
            Self::OpenDiskCleanup => Some(CommandKind::OpenDiskCleanup),
            Self::OpenTestEvents => Some(CommandKind::OpenTestEvents),
            Self::ToggleShowSuccess => Some(CommandKind::ToggleShowSuccess),
            Self::ToggleShowFailed => Some(CommandKind::ToggleShowFailed),
            Self::ToggleShowIgnored => Some(CommandKind::ToggleShowIgnored),
            Self::ToggleShowSkipped => Some(CommandKind::ToggleShowSkipped),
            Self::SelectNextFailed | Self::SelectPreviousFailed => Some(CommandKind::SelectFailed),
            Self::StartOutputSearch => Some(CommandKind::StartOutputSearch),
            Self::ToggleOutputSnap => Some(CommandKind::ToggleOutputSnap),
            Self::OpenOutputSearchModal
            | Self::CaptureTestSnapshot
            | Self::CloseCustomRun
            | Self::CustomRunNext
            | Self::CustomRunPrevious
            | Self::CustomRunAdjustLeft
            | Self::CustomRunAdjustRight
            | Self::CustomRunActivate
            | Self::CustomRunEdit(_)
            | Self::CommitCustomRunEdit
            | Self::CancelCustomRunEdit
            | Self::RunCustom
            | Self::CloseTestDetails
            | Self::CloseSettings
            | Self::SettingsNext
            | Self::SettingsPrevious
            | Self::SettingsAdjustLeft
            | Self::SettingsAdjustRight
            | Self::SettingsActivate
            | Self::SettingsOpenWithEdit(_)
            | Self::CommitOpenWithSetting
            | Self::CancelOpenWithSetting
            | Self::CloseDiskCleanup
            | Self::RunCargoClean
            | Self::CloseTestEvents
            | Self::ToggleTestEventsFocus
            | Self::TestEventsNextRun
            | Self::TestEventsPreviousRun
            | Self::CloseXtasks
            | Self::RefreshXtasks
            | Self::OpenSelectedXtask
            | Self::CloseXtaskDetails
            | Self::XtaskNextCommand
            | Self::XtaskPreviousCommand
            | Self::XtaskNextArg
            | Self::XtaskPreviousArg
            | Self::XtaskAdjustLeft
            | Self::XtaskAdjustRight
            | Self::XtaskActivateArg
            | Self::ToggleXtaskDetailFocus
            | Self::XtaskEdit(_)
            | Self::CommitXtaskEdit
            | Self::CancelXtaskEdit
            | Self::RunXtask
            | Self::ApplyOutputSearch
            | Self::SearchModalNextControl
            | Self::SearchModalPreviousControl
            | Self::SearchModalActivate => None,
            Self::OutputSearchEdit(_) | Self::ClearOutputSearch | Self::CancelOutputSearch => None,
            Self::FindNextOutputMatch | Self::FindPreviousOutputMatch => {
                Some(CommandKind::FindOutputMatch)
            }
            Self::ToggleOutputFilter => Some(CommandKind::ToggleOutputFilter),
            Self::ToggleOutputRegex => Some(CommandKind::ToggleOutputRegex),
            Self::ToggleOutputCaseSensitive => Some(CommandKind::ToggleOutputCaseSensitive),
            Self::Scroll(_) => None,
        }
    }

    pub fn info(&self) -> Option<&'static CommandInfo> {
        let kind = self.kind()?;
        COMMANDS.iter().find(|info| info.kind == kind)
    }

    pub fn ticker_label(&self) -> Option<&'static str> {
        match self {
            Self::Noop => None,
            Self::Resize => Some("resize"),
            Self::OutputSearchEdit(_) => Some("search edit"),
            Self::ClearOutputSearch => Some("search clear"),
            Self::OpenOutputSearchModal => Some("search modal"),
            Self::ApplyOutputSearch => Some("search apply"),
            Self::CancelOutputSearch => Some("search cancel"),
            Self::CloseTestDetails => Some("close details"),
            Self::SearchModalNextControl => Some("search focus"),
            Self::SearchModalPreviousControl => Some("search focus"),
            Self::SearchModalActivate => Some("search action"),
            Self::CloseSettings => Some("settings close"),
            Self::SettingsNext | Self::SettingsPrevious => Some("settings select"),
            Self::SettingsAdjustLeft | Self::SettingsAdjustRight => Some("settings adjust"),
            Self::SettingsActivate => Some("settings edit"),
            Self::SettingsOpenWithEdit(_) => Some("settings input"),
            Self::CommitOpenWithSetting => Some("settings save"),
            Self::CancelOpenWithSetting => Some("settings cancel"),
            Self::RunCargoClean => Some("cargo clean"),
            Self::CustomRunNext | Self::CustomRunPrevious => Some("custom select"),
            Self::CloseCustomRun => Some("custom back"),
            Self::CustomRunAdjustLeft | Self::CustomRunAdjustRight => Some("custom adjust"),
            Self::CustomRunActivate => Some("custom edit"),
            Self::CustomRunEdit(_) => Some("custom input"),
            Self::CommitCustomRunEdit => Some("custom save"),
            Self::CancelCustomRunEdit => Some("custom cancel"),
            Self::RunCustom => Some("custom run"),
            Self::CaptureTestSnapshot => Some("sample stacks"),
            Self::CloseTestEvents => Some("events close"),
            Self::ToggleTestEventsFocus => Some("events focus"),
            Self::TestEventsNextRun | Self::TestEventsPreviousRun => Some("events run"),
            Self::CloseXtasks => Some("xtasks close"),
            Self::RefreshXtasks => Some("xtasks refresh"),
            Self::OpenSelectedXtask => Some("xtasks open"),
            Self::CloseXtaskDetails => Some("xtasks back"),
            Self::XtaskNextCommand | Self::XtaskPreviousCommand => Some("xtasks command"),
            Self::XtaskNextArg | Self::XtaskPreviousArg => Some("xtasks arg"),
            Self::XtaskAdjustLeft | Self::XtaskAdjustRight => Some("xtasks adjust"),
            Self::XtaskActivateArg => Some("xtasks edit"),
            Self::ToggleXtaskDetailFocus => Some("xtasks focus"),
            Self::XtaskEdit(_) => Some("xtasks input"),
            Self::CommitXtaskEdit => Some("xtasks save"),
            Self::CancelXtaskEdit => Some("xtasks cancel"),
            Self::RunXtask => Some("xtasks run"),
            Self::Scroll(_) => Some("scroll"),
            Self::ReportStatus(_) => Some("status"),
            _ => self.info().map(|info| info.ticker),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CommandFocus {
    #[default]
    Tests,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandContext {
    pub input: InputMode,
    pub overlay: Option<OverlayMode>,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self::normal(CommandFocus::default())
    }
}

impl CommandContext {
    pub const fn normal(focus: CommandFocus) -> Self {
        Self {
            input: InputMode::Normal(focus),
            overlay: None,
        }
    }

    pub const fn normal_focus(self) -> Option<CommandFocus> {
        match (self.overlay, self.input) {
            (None, InputMode::Normal(focus)) => Some(focus),
            _ => None,
        }
    }

    pub const fn pane_focus_suppressed(self) -> bool {
        self.normal_focus().is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputMode {
    Normal(CommandFocus),
    DiscoveryRunning,
    SettingsOpenWith,
    CustomRunInput,
    CustomRunModal,
    SettingsModal,
    DiskCleanupModal,
    XtaskInput,
    XtaskModal,
    XtaskCommandModal(XtaskDetailFocus),
    TestEventsModal(TestEventsFocus),
    TestDetailsModal,
    OutputSearchModal,
    OutputSearchInline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayMode {
    Discovery,
    DiscoveryError,
    Settings,
    DiskCleanup,
    Xtasks,
    TestEvents,
    TestDetails,
    OutputSearch,
}

pub fn command_for_input(event: &InputEvent, context: CommandContext) -> AppCommand {
    match event {
        InputEvent::Terminal(Event::Resize(_, _)) => AppCommand::Resize,
        InputEvent::Terminal(Event::Key(key)) if key.kind != KeyEventKind::Press => {
            AppCommand::Noop
        }
        InputEvent::Terminal(Event::Key(key)) if is_stop_key(key.code, key.modifiers) => {
            AppCommand::StopRun
        }
        InputEvent::Terminal(Event::Key(key)) => {
            command_for_input_mode(key.code, key.modifiers, context.input)
        }
        InputEvent::Terminal(_) => AppCommand::Noop,
        InputEvent::Error(error) => AppCommand::ReportStatus(format!("Input error: {error}")),
    }
}

fn command_for_input_mode(code: KeyCode, modifiers: KeyModifiers, input: InputMode) -> AppCommand {
    match input {
        InputMode::DiscoveryRunning => command_for_discovery_running(code),
        InputMode::SettingsOpenWith => command_for_settings_open_with_input(code, modifiers),
        InputMode::CustomRunInput => command_for_custom_run_input(code, modifiers),
        InputMode::CustomRunModal => command_for_custom_run_modal(code),
        InputMode::SettingsModal => command_for_settings_modal(code),
        InputMode::DiskCleanupModal => command_for_disk_cleanup_modal(code),
        InputMode::XtaskInput => command_for_xtask_input(code, modifiers),
        InputMode::XtaskModal => command_for_xtask_modal(code),
        InputMode::XtaskCommandModal(focus) => {
            command_for_xtask_command_modal(code, modifiers, focus)
        }
        InputMode::TestEventsModal(focus) => command_for_test_events_modal(code, modifiers, focus),
        InputMode::TestDetailsModal => command_for_test_details_modal(code),
        InputMode::OutputSearchModal => command_for_output_search_modal(code, modifiers),
        InputMode::OutputSearchInline => command_for_output_search_input(code, modifiers),
        InputMode::Normal(focus) => command_for_key(code, modifiers, focus),
    }
}

fn command_for_output_search_input(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelOutputSearch,
        code if is_output_search_apply_key(code, modifiers) => AppCommand::ApplyOutputSearch,
        KeyCode::Enter if is_advanced_search_modifier(modifiers) => {
            AppCommand::OpenOutputSearchModal
        }
        KeyCode::Enter => AppCommand::ApplyOutputSearch,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        _ => input_field_input_for_key(code, modifiers)
            .map(AppCommand::OutputSearchEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn is_advanced_search_modifier(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::SHIFT)
}

fn command_for_discovery_running(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Char('Q') => AppCommand::Quit,
        _ => AppCommand::Noop,
    }
}

fn command_for_settings_open_with_input(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelOpenWithSetting,
        KeyCode::Enter => AppCommand::CommitOpenWithSetting,
        _ => input_field_input_for_key(code, modifiers)
            .map(AppCommand::SettingsOpenWithEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn command_for_custom_run_input(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelCustomRunEdit,
        KeyCode::Enter => AppCommand::CommitCustomRunEdit,
        _ => input_field_input_for_key(code, modifiers)
            .map(AppCommand::CustomRunEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn command_for_settings_modal(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CloseSettings,
        KeyCode::Up => AppCommand::SettingsPrevious,
        KeyCode::Down => AppCommand::SettingsNext,
        KeyCode::Left => AppCommand::SettingsAdjustLeft,
        KeyCode::Right => AppCommand::SettingsAdjustRight,
        KeyCode::Enter => AppCommand::SettingsActivate,
        _ => AppCommand::Noop,
    }
}

fn command_for_disk_cleanup_modal(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CloseDiskCleanup,
        KeyCode::Char('c') => AppCommand::RunCargoClean,
        KeyCode::Char('r') | KeyCode::Char('d') => AppCommand::RefreshDiskUsage,
        _ => AppCommand::Noop,
    }
}

fn command_for_xtask_input(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelXtaskEdit,
        KeyCode::Enter => AppCommand::CommitXtaskEdit,
        _ => input_field_input_for_key(code, modifiers)
            .map(AppCommand::XtaskEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn command_for_xtask_modal(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CloseXtasks,
        KeyCode::Char('u') => AppCommand::RefreshXtasks,
        KeyCode::Up => AppCommand::XtaskPreviousCommand,
        KeyCode::Down => AppCommand::XtaskNextCommand,
        KeyCode::Enter | KeyCode::Right => AppCommand::OpenSelectedXtask,
        _ => AppCommand::Noop,
    }
}

fn command_for_xtask_command_modal(
    code: KeyCode,
    modifiers: KeyModifiers,
    focus: XtaskDetailFocus,
) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CloseXtaskDetails,
        KeyCode::Tab | KeyCode::BackTab => AppCommand::ToggleXtaskDetailFocus,
        KeyCode::Char('r') if !modifiers.contains(KeyModifiers::CONTROL) => AppCommand::RunXtask,
        _ => match focus {
            XtaskDetailFocus::Parameters => command_for_xtask_params(code),
            XtaskDetailFocus::Output => command_for_xtask_output(code, modifiers),
        },
    }
}

fn command_for_xtask_params(code: KeyCode) -> AppCommand {
    if let Some(command) = scroll_key(code, ScrollKeys::Page) {
        return command;
    }
    match code {
        KeyCode::Up => AppCommand::XtaskPreviousArg,
        KeyCode::Down => AppCommand::XtaskNextArg,
        KeyCode::Left => AppCommand::XtaskAdjustLeft,
        KeyCode::Right => AppCommand::XtaskAdjustRight,
        KeyCode::Enter => AppCommand::XtaskActivateArg,
        _ => AppCommand::Noop,
    }
}

fn command_for_xtask_output(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    if let Some(command) = scroll_key(code, ScrollKeys::LineAndPage) {
        return command;
    }
    match code {
        KeyCode::Char('/') => AppCommand::StartOutputSearch,
        KeyCode::Char('n') => AppCommand::FindNextOutputMatch,
        KeyCode::Char('N') => AppCommand::FindPreviousOutputMatch,
        KeyCode::Char('f') => AppCommand::ToggleOutputFilter,
        KeyCode::Char('s') => AppCommand::ToggleOutputSnap,
        KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ToggleOutputRegex
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        KeyCode::Char('c') => AppCommand::ToggleOutputCaseSensitive,
        KeyCode::Char('o') => AppCommand::OpenOutput,
        _ => AppCommand::Noop,
    }
}

fn command_for_test_events_modal(
    code: KeyCode,
    modifiers: KeyModifiers,
    focus: TestEventsFocus,
) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CloseTestEvents,
        KeyCode::Tab | KeyCode::BackTab => AppCommand::ToggleTestEventsFocus,
        _ => match focus {
            TestEventsFocus::Runs => command_for_test_event_runs(code),
            TestEventsFocus::Events => command_for_test_event_output(code, modifiers),
        },
    }
}

fn command_for_test_event_runs(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Up => AppCommand::TestEventsPreviousRun,
        KeyCode::Down => AppCommand::TestEventsNextRun,
        KeyCode::Enter | KeyCode::Right => AppCommand::ToggleTestEventsFocus,
        _ => AppCommand::Noop,
    }
}

fn command_for_test_event_output(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    if let Some(command) = scroll_key(code, ScrollKeys::LineAndPage) {
        return command;
    }
    match code {
        KeyCode::Left => AppCommand::ToggleTestEventsFocus,
        KeyCode::Char('/') => AppCommand::StartOutputSearch,
        KeyCode::Char('n') => AppCommand::FindNextOutputMatch,
        KeyCode::Char('N') => AppCommand::FindPreviousOutputMatch,
        KeyCode::Char('f') => AppCommand::ToggleOutputFilter,
        KeyCode::Char('s') => AppCommand::ToggleOutputSnap,
        KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ToggleOutputRegex
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        KeyCode::Char('c') => AppCommand::ToggleOutputCaseSensitive,
        KeyCode::Char('o') => AppCommand::OpenOutput,
        _ => AppCommand::Noop,
    }
}

fn command_for_test_details_modal(code: KeyCode) -> AppCommand {
    if let Some(command) = scroll_key(code, ScrollKeys::Page) {
        return command;
    }
    match code {
        KeyCode::Esc => AppCommand::CloseTestDetails,
        KeyCode::Char('R') => AppCommand::OpenCustomRun,
        KeyCode::Char('s') => AppCommand::CaptureTestSnapshot,
        _ => AppCommand::Noop,
    }
}

fn command_for_custom_run_modal(code: KeyCode) -> AppCommand {
    if let Some(command) = scroll_key(code, ScrollKeys::Page) {
        return command;
    }
    match code {
        KeyCode::Esc => AppCommand::CloseCustomRun,
        KeyCode::Up => AppCommand::CustomRunPrevious,
        KeyCode::Down => AppCommand::CustomRunNext,
        KeyCode::Left => AppCommand::CustomRunAdjustLeft,
        KeyCode::Right => AppCommand::CustomRunAdjustRight,
        KeyCode::Enter => AppCommand::CustomRunActivate,
        KeyCode::Char('r') => AppCommand::RunCustom,
        _ => AppCommand::Noop,
    }
}

fn command_for_output_search_modal(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelOutputSearch,
        KeyCode::Tab => AppCommand::SearchModalNextControl,
        KeyCode::BackTab => AppCommand::SearchModalPreviousControl,
        code if is_output_search_apply_key(code, modifiers) => AppCommand::ApplyOutputSearch,
        KeyCode::Enter => AppCommand::SearchModalActivate,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ToggleOutputFilter
        }
        KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ToggleOutputRegex
        }
        _ => input_field_input_for_key(code, modifiers)
            .map(AppCommand::OutputSearchEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn is_output_search_apply_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
        KeyCode::Enter => modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER),
        KeyCode::Char('\n' | '\r') => true,
        KeyCode::Char('j' | 'm') => modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

fn input_field_input_for_key(code: KeyCode, modifiers: KeyModifiers) -> Option<InputFieldInput> {
    let reserved_modifiers = KeyModifiers::CONTROL
        | KeyModifiers::ALT
        | KeyModifiers::SUPER
        | KeyModifiers::HYPER
        | KeyModifiers::META;
    let has_reserved_modifier = modifiers.intersects(reserved_modifiers);
    let key = match code {
        KeyCode::Char(char) if !has_reserved_modifier => return Some(InputFieldInput::char(char)),
        KeyCode::Backspace if modifiers.is_empty() => InputFieldKey::Backspace,
        KeyCode::Delete if modifiers.is_empty() => InputFieldKey::Delete,
        KeyCode::Left if modifiers.is_empty() => InputFieldKey::Left,
        KeyCode::Right if modifiers.is_empty() => InputFieldKey::Right,
        KeyCode::Home if modifiers.is_empty() => InputFieldKey::Home,
        KeyCode::End if modifiers.is_empty() => InputFieldKey::End,
        _ => return None,
    };
    Some(InputFieldInput::new(key))
}

fn command_for_key(code: KeyCode, modifiers: KeyModifiers, focus: CommandFocus) -> AppCommand {
    if focus == CommandFocus::Output
        && let Some(command) = scroll_key(code, ScrollKeys::LineAndPage)
    {
        return command;
    }

    match code {
        KeyCode::Char('Q') => AppCommand::Quit,
        code if is_stop_key(code, modifiers) => AppCommand::StopRun,
        KeyCode::Char('u') if modifiers.is_empty() => AppCommand::RefreshTests,
        KeyCode::Char(',') => AppCommand::OpenSettings,
        KeyCode::Char('X') => AppCommand::OpenXtasks,
        KeyCode::Char('E') => AppCommand::OpenTestEvents,
        KeyCode::Char('D') => AppCommand::OpenDiskCleanup,
        KeyCode::Tab => AppCommand::ToggleFocus,
        KeyCode::Up => AppCommand::MoveUp,
        KeyCode::Down => AppCommand::MoveDown,
        KeyCode::Left if modifiers.contains(KeyModifiers::SHIFT) => AppCommand::NarrowTestsPane,
        KeyCode::Right if modifiers.contains(KeyModifiers::SHIFT) => AppCommand::WidenTestsPane,
        KeyCode::Home => AppCommand::MoveHome,
        KeyCode::End => AppCommand::MoveEnd,
        KeyCode::PageUp => AppCommand::PageUp,
        KeyCode::PageDown => AppCommand::PageDown,
        KeyCode::Char('[') => AppCommand::NarrowTestsPane,
        KeyCode::Char(']') => AppCommand::WidenTestsPane,
        _ => match focus {
            CommandFocus::Tests => command_for_tests_key(code),
            CommandFocus::Output => command_for_output_key(code, modifiers),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScrollKeys {
    Page,
    LineAndPage,
}

fn scroll_key(code: KeyCode, keys: ScrollKeys) -> Option<AppCommand> {
    let action = match code {
        KeyCode::Up if keys == ScrollKeys::LineAndPage => ScrollAction::LineUp,
        KeyCode::Down if keys == ScrollKeys::LineAndPage => ScrollAction::LineDown,
        KeyCode::PageUp => ScrollAction::PageUp,
        KeyCode::PageDown => ScrollAction::PageDown,
        KeyCode::Home => ScrollAction::Top,
        KeyCode::End => ScrollAction::Bottom,
        _ => return None,
    };
    Some(AppCommand::Scroll(action))
}

fn command_for_tests_key(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Left => AppCommand::MoveLeft,
        KeyCode::Right => AppCommand::MoveRight,
        KeyCode::Enter => AppCommand::ActivateSelected,
        KeyCode::Char(' ') => AppCommand::ToggleSelected,
        KeyCode::Char('p') => AppCommand::ToggleShowSuccess,
        KeyCode::Char('f') => AppCommand::ToggleShowFailed,
        KeyCode::Char('i') => AppCommand::ToggleShowIgnored,
        KeyCode::Char('s') => AppCommand::ToggleShowSkipped,
        KeyCode::Char('r') => AppCommand::RunSelected,
        KeyCode::Char('o') => AppCommand::OpenSource,
        KeyCode::Char('j') => AppCommand::SelectNextFailed,
        KeyCode::Char('J') => AppCommand::SelectPreviousFailed,
        _ => AppCommand::Noop,
    }
}

fn command_for_output_key(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Char('/') => AppCommand::StartOutputSearch,
        KeyCode::Char('n') => AppCommand::FindNextOutputMatch,
        KeyCode::Char('N') => AppCommand::FindPreviousOutputMatch,
        KeyCode::Char('f') => AppCommand::ToggleOutputFilter,
        KeyCode::Char('s') => AppCommand::ToggleOutputSnap,
        KeyCode::Char('r') => AppCommand::ToggleOutputRegex,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        KeyCode::Char('c') => AppCommand::ToggleOutputCaseSensitive,
        KeyCode::Char('o') => AppCommand::OpenOutput,
        _ => AppCommand::Noop,
    }
}

fn is_stop_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('c')) && modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests;
