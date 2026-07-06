use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    input::InputEvent,
    input_field::{InputFieldInput, InputFieldKey},
    output_pane::{SearchEditorInput, SearchEditorKey},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppCommand {
    Noop,
    Quit,
    Resize,
    StopRun,
    ToggleHelp,
    CloseHelp,
    ToggleFocus,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ToggleSelected,
    MoveHome,
    MoveEnd,
    PageUp,
    PageDown,
    NarrowTestsPane,
    WidenTestsPane,
    RefreshTests,
    RunSelected,
    RunFailed,
    OpenSource,
    OpenOutput,
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
    ToggleShowSuccess,
    ToggleShowFailed,
    ToggleShowIgnored,
    ToggleShowSkipped,
    SelectNextFailed,
    SelectPreviousFailed,
    StartOutputSearch,
    OpenOutputSearchModal,
    OutputSearchEdit(SearchEditorInput),
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

impl CommandGroup {
    pub const fn title(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Runs => "Runs",
            Self::View => "View",
            Self::Output => "Output",
            Self::Global => "Global",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKind {
    Quit,
    StopRun,
    ToggleHelp,
    CloseHelp,
    ToggleFocus,
    MoveUpDown,
    MoveLeftRight,
    ToggleSelected,
    MoveHomeEnd,
    PageUpDown,
    NarrowTestsPane,
    WidenTestsPane,
    RefreshTests,
    RunSelected,
    RunFailed,
    OpenSource,
    OpenOutput,
    OpenSettings,
    RefreshDiskUsage,
    OpenDiskCleanup,
    ToggleShowSuccess,
    ToggleShowFailed,
    ToggleShowIgnored,
    ToggleShowSkipped,
    SelectFailed,
    FollowOutputBottom,
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
        kind: CommandKind::ToggleSelected,
        group: CommandGroup::Navigation,
        keys: "Enter/Space",
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
        kind: CommandKind::RunFailed,
        group: CommandGroup::Runs,
        keys: "R",
        label: "rerun failures",
        ticker: "rerun failed",
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
        keys: "d",
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
        kind: CommandKind::FollowOutputBottom,
        group: CommandGroup::Output,
        keys: "End",
        label: "follow output bottom",
        ticker: "bottom",
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
        kind: CommandKind::ToggleHelp,
        group: CommandGroup::Global,
        keys: "h/?/F1",
        label: "open or close help",
        ticker: "help",
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
        keys: "q",
        label: "quit",
        ticker: "quit",
    },
];

const CLOSE_HELP_INFO: CommandInfo = CommandInfo {
    kind: CommandKind::CloseHelp,
    group: CommandGroup::Global,
    keys: "h/?/F1",
    label: "close help",
    ticker: "close help",
};

pub const fn command_infos() -> &'static [CommandInfo] {
    COMMANDS
}

impl AppCommand {
    pub fn kind(&self) -> Option<CommandKind> {
        match self {
            Self::Noop | Self::Resize | Self::ReportStatus(_) => None,
            Self::Quit => Some(CommandKind::Quit),
            Self::StopRun => Some(CommandKind::StopRun),
            Self::ToggleHelp => Some(CommandKind::ToggleHelp),
            Self::CloseHelp => Some(CommandKind::CloseHelp),
            Self::ToggleFocus => Some(CommandKind::ToggleFocus),
            Self::MoveUp | Self::MoveDown => Some(CommandKind::MoveUpDown),
            Self::MoveLeft | Self::MoveRight => Some(CommandKind::MoveLeftRight),
            Self::ToggleSelected => Some(CommandKind::ToggleSelected),
            Self::MoveHome | Self::MoveEnd => Some(CommandKind::MoveHomeEnd),
            Self::PageUp | Self::PageDown => Some(CommandKind::PageUpDown),
            Self::NarrowTestsPane => Some(CommandKind::NarrowTestsPane),
            Self::WidenTestsPane => Some(CommandKind::WidenTestsPane),
            Self::RefreshTests => Some(CommandKind::RefreshTests),
            Self::RunSelected => Some(CommandKind::RunSelected),
            Self::RunFailed => Some(CommandKind::RunFailed),
            Self::OpenSource => Some(CommandKind::OpenSource),
            Self::OpenOutput => Some(CommandKind::OpenOutput),
            Self::OpenSettings => Some(CommandKind::OpenSettings),
            Self::RefreshDiskUsage => Some(CommandKind::RefreshDiskUsage),
            Self::OpenDiskCleanup => Some(CommandKind::OpenDiskCleanup),
            Self::ToggleShowSuccess => Some(CommandKind::ToggleShowSuccess),
            Self::ToggleShowFailed => Some(CommandKind::ToggleShowFailed),
            Self::ToggleShowIgnored => Some(CommandKind::ToggleShowIgnored),
            Self::ToggleShowSkipped => Some(CommandKind::ToggleShowSkipped),
            Self::SelectNextFailed | Self::SelectPreviousFailed => Some(CommandKind::SelectFailed),
            Self::StartOutputSearch => Some(CommandKind::StartOutputSearch),
            Self::OpenOutputSearchModal
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
            | Self::ApplyOutputSearch
            | Self::SearchModalNextControl
            | Self::SearchModalPreviousControl
            | Self::SearchModalActivate => None,
            Self::OutputSearchEdit(_)
            | Self::ClearOutputSearch
            | Self::CancelOutputSearch => None,
            Self::FindNextOutputMatch | Self::FindPreviousOutputMatch => {
                Some(CommandKind::FindOutputMatch)
            }
            Self::ToggleOutputFilter => Some(CommandKind::ToggleOutputFilter),
            Self::ToggleOutputRegex => Some(CommandKind::ToggleOutputRegex),
            Self::ToggleOutputCaseSensitive => Some(CommandKind::ToggleOutputCaseSensitive),
        }
    }

    pub fn info(&self) -> Option<&'static CommandInfo> {
        let kind = match self {
            Self::CloseHelp => return Some(&CLOSE_HELP_INFO),
            _ => self.kind()?,
        };
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

    pub const fn pane_focus_suppressed(self) -> bool {
        self.overlay.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputMode {
    Normal(CommandFocus),
    Help,
    DiscoveryRunning,
    SettingsOpenWith,
    SettingsModal,
    DiskCleanupModal,
    OutputSearchModal,
    OutputSearchInline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayMode {
    Help,
    Discovery,
    DiscoveryError,
    Settings,
    DiskCleanup,
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

fn command_for_input_mode(
    code: KeyCode,
    modifiers: KeyModifiers,
    input: InputMode,
) -> AppCommand {
    match input {
        InputMode::DiscoveryRunning => command_for_discovery_running(code),
        InputMode::SettingsOpenWith => command_for_settings_open_with_input(code, modifiers),
        InputMode::SettingsModal => command_for_settings_modal(code),
        InputMode::DiskCleanupModal => command_for_disk_cleanup_modal(code),
        InputMode::OutputSearchModal => command_for_output_search_modal(code, modifiers),
        InputMode::OutputSearchInline => command_for_output_search_input(code, modifiers),
        InputMode::Help => command_for_help(code, modifiers),
        InputMode::Normal(focus) => command_for_key(code, modifiers, focus),
    }
}

fn command_for_help(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => AppCommand::CloseHelp,
        code if is_help_key(code, modifiers) => AppCommand::CloseHelp,
        _ => AppCommand::Noop,
    }
}

fn command_for_output_search_input(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelOutputSearch,
        KeyCode::Enter if is_advanced_search_modifier(modifiers) => {
            AppCommand::OpenOutputSearchModal
        }
        KeyCode::Enter => AppCommand::ApplyOutputSearch,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        _ => search_editor_input_for_key(code, modifiers)
            .map(AppCommand::OutputSearchEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn is_advanced_search_modifier(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER)
}

fn command_for_discovery_running(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Char('q') => AppCommand::Quit,
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

fn command_for_settings_modal(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CloseSettings,
        KeyCode::Up | KeyCode::BackTab => AppCommand::SettingsPrevious,
        KeyCode::Down | KeyCode::Tab => AppCommand::SettingsNext,
        KeyCode::Left => AppCommand::SettingsAdjustLeft,
        KeyCode::Right => AppCommand::SettingsAdjustRight,
        KeyCode::Enter => AppCommand::SettingsActivate,
        KeyCode::Char('e') => AppCommand::SettingsActivate,
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

fn command_for_output_search_modal(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Esc => AppCommand::CancelOutputSearch,
        KeyCode::Tab => AppCommand::SearchModalNextControl,
        KeyCode::BackTab => AppCommand::SearchModalPreviousControl,
        KeyCode::Enter if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ApplyOutputSearch
        }
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
        _ => search_editor_input_for_key(code, modifiers)
            .map(AppCommand::OutputSearchEdit)
            .unwrap_or(AppCommand::Noop),
    }
}

fn search_editor_input_for_key(
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Option<SearchEditorInput> {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let key = match code {
        KeyCode::Char(char) => SearchEditorKey::Char(char),
        KeyCode::Backspace => SearchEditorKey::Backspace,
        KeyCode::Enter => SearchEditorKey::Enter,
        KeyCode::Left => SearchEditorKey::Left,
        KeyCode::Right => SearchEditorKey::Right,
        KeyCode::Up => SearchEditorKey::Up,
        KeyCode::Down => SearchEditorKey::Down,
        KeyCode::Tab => SearchEditorKey::Tab,
        KeyCode::Delete => SearchEditorKey::Delete,
        KeyCode::Home => SearchEditorKey::Home,
        KeyCode::End => SearchEditorKey::End,
        KeyCode::PageUp => SearchEditorKey::PageUp,
        KeyCode::PageDown => SearchEditorKey::PageDown,
        _ => return None,
    };
    Some(SearchEditorInput::new(key, ctrl, alt, shift))
}

fn input_field_input_for_key(
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Option<InputFieldInput> {
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
    match code {
        KeyCode::Char('q') => AppCommand::Quit,
        code if is_stop_key(code, modifiers) => AppCommand::StopRun,
        code if is_help_key(code, modifiers) => AppCommand::ToggleHelp,
        KeyCode::Char('u') => AppCommand::RefreshTests,
        KeyCode::Char(',') => AppCommand::OpenSettings,
        KeyCode::Char('d') => AppCommand::RefreshDiskUsage,
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
            CommandFocus::Output => command_for_output_key(code),
        },
    }
}

fn command_for_tests_key(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Left => AppCommand::MoveLeft,
        KeyCode::Right => AppCommand::MoveRight,
        KeyCode::Enter | KeyCode::Char(' ') => AppCommand::ToggleSelected,
        KeyCode::Char('p') => AppCommand::ToggleShowSuccess,
        KeyCode::Char('f') => AppCommand::ToggleShowFailed,
        KeyCode::Char('i') => AppCommand::ToggleShowIgnored,
        KeyCode::Char('s') => AppCommand::ToggleShowSkipped,
        KeyCode::Char('r') => AppCommand::RunSelected,
        KeyCode::Char('R') => AppCommand::RunFailed,
        KeyCode::Char('o') => AppCommand::OpenSource,
        KeyCode::Char('j') => AppCommand::SelectNextFailed,
        KeyCode::Char('J') => AppCommand::SelectPreviousFailed,
        _ => AppCommand::Noop,
    }
}

fn command_for_output_key(code: KeyCode) -> AppCommand {
    match code {
        KeyCode::Char('/') => AppCommand::StartOutputSearch,
        KeyCode::Char('n') => AppCommand::FindNextOutputMatch,
        KeyCode::Char('N') => AppCommand::FindPreviousOutputMatch,
        KeyCode::Char('f') => AppCommand::ToggleOutputFilter,
        KeyCode::Char('r') => AppCommand::ToggleOutputRegex,
        KeyCode::Char('c') => AppCommand::ToggleOutputCaseSensitive,
        KeyCode::Char('o') => AppCommand::OpenOutput,
        _ => AppCommand::Noop,
    }
}

fn is_stop_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('c')) && modifiers.contains(KeyModifiers::CONTROL)
}

fn is_help_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    is_question_mark(code, modifiers)
        || matches!(
            code,
            KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::F(1)
        )
}

fn is_question_mark(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('?'))
        || (matches!(code, KeyCode::Char('/')) && modifiers.contains(KeyModifiers::SHIFT))
}

#[cfg(test)]
mod tests;
