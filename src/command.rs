use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    input::InputEvent,
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
        label: "refresh test list",
        ticker: "refresh tests",
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
            Self::ToggleShowSuccess => Some(CommandKind::ToggleShowSuccess),
            Self::ToggleShowFailed => Some(CommandKind::ToggleShowFailed),
            Self::ToggleShowIgnored => Some(CommandKind::ToggleShowIgnored),
            Self::ToggleShowSkipped => Some(CommandKind::ToggleShowSkipped),
            Self::SelectNextFailed | Self::SelectPreviousFailed => Some(CommandKind::SelectFailed),
            Self::StartOutputSearch => Some(CommandKind::StartOutputSearch),
            Self::OpenOutputSearchModal
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommandContext {
    pub help_visible: bool,
    pub focus: CommandFocus,
    pub output_search_input: bool,
    pub output_search_modal: bool,
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
        InputEvent::Terminal(Event::Key(key)) if context.output_search_modal => {
            command_for_output_search_modal(key.code, key.modifiers)
        }
        InputEvent::Terminal(Event::Key(key)) if context.output_search_input => {
            command_for_output_search_input(key.code, key.modifiers)
        }
        InputEvent::Terminal(Event::Key(key)) if context.help_visible => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => AppCommand::CloseHelp,
                code if is_help_key(code, key.modifiers) => AppCommand::CloseHelp,
                _ => AppCommand::Noop,
            }
        }
        InputEvent::Terminal(Event::Key(key)) => {
            command_for_key(key.code, key.modifiers, context.focus)
        }
        InputEvent::Terminal(_) => AppCommand::Noop,
        InputEvent::Error(error) => AppCommand::ReportStatus(format!("Input error: {error}")),
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

fn command_for_key(code: KeyCode, modifiers: KeyModifiers, focus: CommandFocus) -> AppCommand {
    match code {
        KeyCode::Char('q') => AppCommand::Quit,
        code if is_stop_key(code, modifiers) => AppCommand::StopRun,
        code if is_help_key(code, modifiers) => AppCommand::ToggleHelp,
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
        KeyCode::Char('u') => AppCommand::RefreshTests,
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
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn maps_normalized_question_mark_to_help() {
        assert_eq!(
            command_for_key(
                KeyCode::Char('?'),
                KeyModifiers::NONE,
                CommandFocus::Tests
            ),
            AppCommand::ToggleHelp
        );
        assert_eq!(
            command_for_key(
                KeyCode::Char('?'),
                KeyModifiers::SHIFT,
                CommandFocus::Tests
            ),
            AppCommand::ToggleHelp
        );
        assert_eq!(
            command_for_key(
                KeyCode::Char('/'),
                KeyModifiers::SHIFT,
                CommandFocus::Tests
            ),
            AppCommand::ToggleHelp
        );
    }

    #[test]
    fn maps_fallback_help_keys() {
        assert_eq!(
            command_for_key(KeyCode::Char('h'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleHelp
        );
        assert_eq!(
            command_for_key(KeyCode::F(1), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleHelp
        );
    }

    #[test]
    fn plain_slash_searches_output_only_when_output_is_focused() {
        assert_eq!(
            command_for_key(KeyCode::Char('/'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::Noop
        );
        assert_eq!(
            command_for_key(KeyCode::Char('/'), KeyModifiers::NONE, CommandFocus::Output),
            AppCommand::StartOutputSearch
        );
    }

    #[test]
    fn maps_ctrl_c_to_stop_run() {
        assert_eq!(
            command_for_key(KeyCode::Char('c'), KeyModifiers::CONTROL, CommandFocus::Tests),
            AppCommand::StopRun
        );
        assert_eq!(
            command_for_key(KeyCode::Char('c'), KeyModifiers::CONTROL, CommandFocus::Output),
            AppCommand::StopRun
        );
    }

    #[test]
    fn ctrl_c_stops_run_in_search_and_help_contexts() {
        let event = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));

        assert_eq!(
            command_for_input(
                &event,
                CommandContext {
                    output_search_input: true,
                    ..CommandContext::default()
                }
            ),
            AppCommand::StopRun
        );
        assert_eq!(
            command_for_input(
                &event,
                CommandContext {
                    help_visible: true,
                    ..CommandContext::default()
                }
            ),
            AppCommand::StopRun
        );
    }

    #[test]
    fn maps_refresh_and_view_filter_keys() {
        assert_eq!(
            command_for_key(KeyCode::Char('u'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::RefreshTests
        );
        assert_eq!(
            command_for_key(KeyCode::Char('p'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowSuccess
        );
        assert_eq!(
            command_for_key(KeyCode::Char('f'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowFailed
        );
        assert_eq!(
            command_for_key(KeyCode::Char('i'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowIgnored
        );
        assert_eq!(
            command_for_key(KeyCode::Char('s'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowSkipped
        );
        assert_eq!(
            command_for_key(KeyCode::Char('j'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::SelectNextFailed
        );
        assert_eq!(
            command_for_key(KeyCode::Char('J'), KeyModifiers::SHIFT, CommandFocus::Tests),
            AppCommand::SelectPreviousFailed
        );
        assert_eq!(
            command_for_key(KeyCode::Char('o'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::OpenSource
        );
    }

    #[test]
    fn maps_tests_pane_resize_keys() {
        assert_eq!(
            command_for_key(KeyCode::Left, KeyModifiers::SHIFT, CommandFocus::Tests),
            AppCommand::NarrowTestsPane
        );
        assert_eq!(
            command_for_key(KeyCode::Right, KeyModifiers::SHIFT, CommandFocus::Tests),
            AppCommand::WidenTestsPane
        );
        assert_eq!(
            command_for_key(KeyCode::Char('['), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::NarrowTestsPane
        );
        assert_eq!(
            command_for_key(KeyCode::Char(']'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::WidenTestsPane
        );
    }

    #[test]
    fn output_focus_uses_output_search_commands() {
        assert_eq!(
            command_for_key(KeyCode::Char('f'), KeyModifiers::NONE, CommandFocus::Output),
            AppCommand::ToggleOutputFilter
        );
        assert_eq!(
            command_for_key(KeyCode::Char('r'), KeyModifiers::NONE, CommandFocus::Output),
            AppCommand::ToggleOutputRegex
        );
        assert_eq!(
            command_for_key(KeyCode::Char('c'), KeyModifiers::NONE, CommandFocus::Output),
            AppCommand::ToggleOutputCaseSensitive
        );
        assert_eq!(
            command_for_key(KeyCode::Char('n'), KeyModifiers::NONE, CommandFocus::Output),
            AppCommand::FindNextOutputMatch
        );
        assert_eq!(
            command_for_key(KeyCode::Char('N'), KeyModifiers::SHIFT, CommandFocus::Output),
            AppCommand::FindPreviousOutputMatch
        );
        assert_eq!(
            command_for_key(KeyCode::Char('o'), KeyModifiers::NONE, CommandFocus::Output),
            AppCommand::OpenOutput
        );
    }

    #[test]
    fn command_metadata_drives_ticker_labels() {
        assert_eq!(AppCommand::RunSelected.ticker_label(), Some("run"));
        assert_eq!(
            AppCommand::ToggleOutputRegex.ticker_label(),
            Some("regex")
        );
        assert_eq!(AppCommand::CloseHelp.ticker_label(), Some("close help"));
    }

    #[test]
    fn command_metadata_contains_help_entries() {
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Navigation
                && info.keys == "Tab"
                && info.label == "switch tree/output focus"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Global
                && info.keys == "h/?/F1"
                && info.label == "open or close help"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Runs
                && info.keys == "r"
                && info.label == "run selected scope"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Global
                && info.keys == "Ctrl+C"
                && info.label == "stop running tests"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Output && info.keys == "/" && info.label == "search output"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::View
                && info.keys == "f"
                && info.label == "toggle failed tests"
        }));
    }

    #[test]
    fn output_search_input_accepts_text_and_controls() {
        let context = CommandContext {
            output_search_input: true,
            ..CommandContext::default()
        };
        let text = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Char('p'),
            KeyModifiers::NONE,
        )));
        assert_eq!(
            command_for_input(&text, context),
            AppCommand::OutputSearchEdit(SearchEditorInput::char('p'))
        );

        let backspace =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)));
        assert_eq!(
            command_for_input(&backspace, context),
            AppCommand::OutputSearchEdit(SearchEditorInput::new(
                SearchEditorKey::Backspace,
                false,
                false,
                false,
            ))
        );

        let left =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)));
        assert_eq!(
            command_for_input(&left, context),
            AppCommand::OutputSearchEdit(SearchEditorInput::new(
                SearchEditorKey::Left,
                false,
                false,
                false,
            ))
        );

        let clear = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
        )));
        assert_eq!(
            command_for_input(&clear, context),
            AppCommand::ClearOutputSearch
        );

        let enter =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert_eq!(
            command_for_input(&enter, context),
            AppCommand::ApplyOutputSearch
        );

        let advanced = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::CONTROL,
        )));
        assert_eq!(
            command_for_input(&advanced, context),
            AppCommand::OpenOutputSearchModal
        );

        let mac_advanced = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::SUPER,
        )));
        assert_eq!(
            command_for_input(&mac_advanced, context),
            AppCommand::OpenOutputSearchModal
        );
    }

    #[test]
    fn output_search_modal_accepts_navigation_and_apply_keys() {
        let context = CommandContext {
            output_search_modal: true,
            ..CommandContext::default()
        };

        let tab = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        )));
        assert_eq!(
            command_for_input(&tab, context),
            AppCommand::SearchModalNextControl
        );

        let enter =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert_eq!(
            command_for_input(&enter, context),
            AppCommand::SearchModalActivate
        );

        let apply = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::CONTROL,
        )));
        assert_eq!(
            command_for_input(&apply, context),
            AppCommand::ApplyOutputSearch
        );
    }

    #[test]
    fn help_context_only_closes_on_close_keys() {
        let context = CommandContext {
            help_visible: true,
            ..CommandContext::default()
        };
        let event =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
        assert_eq!(command_for_input(&event, context), AppCommand::Noop);

        let event =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert_eq!(command_for_input(&event, context), AppCommand::CloseHelp);
    }

    #[test]
    fn ignores_non_press_key_events() {
        let repeat = InputEvent::Terminal(Event::Key(KeyEvent::new(
            KeyCode::Char('h'),
            KeyModifiers::NONE,
        )));
        let mut repeat = match repeat {
            InputEvent::Terminal(Event::Key(key)) => key,
            _ => unreachable!(),
        };
        repeat.kind = KeyEventKind::Repeat;

        assert_eq!(
            command_for_input(
                &InputEvent::Terminal(Event::Key(repeat)),
                CommandContext::default()
            ),
            AppCommand::Noop
        );
    }

    #[test]
    fn resize_is_a_command() {
        let event = InputEvent::Terminal(Event::Resize(80, 24));
        assert_eq!(
            command_for_input(&event, CommandContext::default()),
            AppCommand::Resize
        );
    }
}
