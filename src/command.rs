use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::input::InputEvent;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppCommand {
    Noop,
    Quit,
    Resize,
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
    OutputSearchInput(char),
    OutputSearchBackspace,
    ClearOutputSearch,
    AcceptOutputSearch,
    CancelOutputSearch,
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
        label: "toggle passed tests (tests focus)",
        ticker: "toggle success",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowFailed,
        group: CommandGroup::View,
        keys: "f",
        label: "toggle failed tests (tests focus)",
        ticker: "toggle failed",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowIgnored,
        group: CommandGroup::View,
        keys: "i",
        label: "toggle ignored tests (tests focus)",
        ticker: "toggle ignored",
    },
    CommandInfo {
        kind: CommandKind::ToggleShowSkipped,
        group: CommandGroup::View,
        keys: "s",
        label: "toggle skipped tests (tests focus)",
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
        label: "toggle output match filter (output focus)",
        ticker: "output filter",
    },
    CommandInfo {
        kind: CommandKind::ToggleOutputRegex,
        group: CommandGroup::Output,
        keys: "r",
        label: "toggle output regex (output focus)",
        ticker: "regex",
    },
    CommandInfo {
        kind: CommandKind::ToggleOutputCaseSensitive,
        group: CommandGroup::Output,
        keys: "c",
        label: "toggle output case sensitivity (output focus)",
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
        kind: CommandKind::Quit,
        group: CommandGroup::Global,
        keys: "q",
        label: "quit",
        ticker: "quit",
    },
];

const HELP_GROUPS: &[CommandGroup] = &[
    CommandGroup::Navigation,
    CommandGroup::Runs,
    CommandGroup::View,
    CommandGroup::Output,
    CommandGroup::Global,
];

const CLOSE_HELP_INFO: CommandInfo = CommandInfo {
    kind: CommandKind::CloseHelp,
    group: CommandGroup::Global,
    keys: "h/?/F1",
    label: "close help",
    ticker: "close help",
};

pub const fn help_groups() -> &'static [CommandGroup] {
    HELP_GROUPS
}

pub const fn command_infos() -> &'static [CommandInfo] {
    COMMANDS
}

impl AppCommand {
    pub fn kind(&self) -> Option<CommandKind> {
        match self {
            Self::Noop | Self::Resize | Self::ReportStatus(_) => None,
            Self::Quit => Some(CommandKind::Quit),
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
            Self::OutputSearchInput(_)
            | Self::OutputSearchBackspace
            | Self::ClearOutputSearch
            | Self::AcceptOutputSearch
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
            Self::OutputSearchInput(_) => Some("search text"),
            Self::OutputSearchBackspace => Some("search erase"),
            Self::ClearOutputSearch => Some("search clear"),
            Self::AcceptOutputSearch => Some("search accept"),
            Self::CancelOutputSearch => Some("search cancel"),
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
}

pub fn command_for_input(event: &InputEvent, context: CommandContext) -> AppCommand {
    match event {
        InputEvent::Terminal(Event::Resize(_, _)) => AppCommand::Resize,
        InputEvent::Terminal(Event::Key(key)) if key.kind != KeyEventKind::Press => {
            AppCommand::Noop
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
        KeyCode::Enter => AppCommand::AcceptOutputSearch,
        KeyCode::Backspace => AppCommand::OutputSearchBackspace,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            AppCommand::ClearOutputSearch
        }
        KeyCode::Char(char) if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
            AppCommand::OutputSearchInput(char)
        }
        _ => AppCommand::Noop,
    }
}

fn command_for_key(code: KeyCode, modifiers: KeyModifiers, focus: CommandFocus) -> AppCommand {
    match code {
        KeyCode::Char('q') => AppCommand::Quit,
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
    fn command_metadata_contains_help_groups() {
        assert!(help_groups().contains(&CommandGroup::Navigation));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Runs
                && info.keys == "r"
                && info.label == "run selected scope"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::Output && info.keys == "/" && info.label == "search output"
        }));
        assert!(command_infos().iter().any(|info| {
            info.group == CommandGroup::View
                && info.keys == "f"
                && info.label == "toggle failed tests (tests focus)"
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
            AppCommand::OutputSearchInput('p')
        );

        let backspace =
            InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)));
        assert_eq!(
            command_for_input(&backspace, context),
            AppCommand::OutputSearchBackspace
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
            AppCommand::AcceptOutputSearch
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
