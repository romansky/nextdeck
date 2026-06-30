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
    ToggleShowSuccess,
    ToggleShowFailed,
    ToggleShowIgnored,
    ToggleShowSkipped,
    SelectNextFailed,
    SelectPreviousFailed,
    StartOutputSearch,
    OutputSearchInput(char),
    OutputSearchBackspace,
    AcceptOutputSearch,
    CancelOutputSearch,
    FindNextOutputMatch,
    FindPreviousOutputMatch,
    ToggleOutputFilter,
    ToggleOutputRegex,
    ToggleOutputCaseSensitive,
    ReportStatus(String),
}

impl AppCommand {
    pub fn ticker_label(&self) -> Option<&'static str> {
        match self {
            Self::Noop => None,
            Self::Quit => Some("quit"),
            Self::Resize => Some("resize"),
            Self::ToggleHelp => Some("help"),
            Self::CloseHelp => Some("close help"),
            Self::ToggleFocus => Some("focus"),
            Self::MoveUp => Some("up"),
            Self::MoveDown => Some("down"),
            Self::MoveLeft => Some("collapse"),
            Self::MoveRight => Some("expand"),
            Self::ToggleSelected => Some("toggle"),
            Self::MoveHome => Some("home"),
            Self::MoveEnd => Some("end"),
            Self::PageUp => Some("page up"),
            Self::PageDown => Some("page down"),
            Self::NarrowTestsPane => Some("narrow tests"),
            Self::WidenTestsPane => Some("widen tests"),
            Self::RefreshTests => Some("refresh tests"),
            Self::RunSelected => Some("run"),
            Self::RunFailed => Some("rerun failed"),
            Self::ToggleShowSuccess => Some("toggle success"),
            Self::ToggleShowFailed => Some("toggle failed"),
            Self::ToggleShowIgnored => Some("toggle ignored"),
            Self::ToggleShowSkipped => Some("toggle skipped"),
            Self::SelectNextFailed => Some("next failed"),
            Self::SelectPreviousFailed => Some("previous failed"),
            Self::StartOutputSearch => Some("search output"),
            Self::OutputSearchInput(_) => Some("search text"),
            Self::OutputSearchBackspace => Some("search erase"),
            Self::AcceptOutputSearch => Some("search accept"),
            Self::CancelOutputSearch => Some("search cancel"),
            Self::FindNextOutputMatch => Some("next match"),
            Self::FindPreviousOutputMatch => Some("previous match"),
            Self::ToggleOutputFilter => Some("output filter"),
            Self::ToggleOutputRegex => Some("regex"),
            Self::ToggleOutputCaseSensitive => Some("case"),
            Self::ReportStatus(_) => Some("status"),
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
        KeyCode::Char('s') => AppCommand::ToggleShowSuccess,
        KeyCode::Char('x') => AppCommand::ToggleShowFailed,
        KeyCode::Char('i') => AppCommand::ToggleShowIgnored,
        KeyCode::Char('k') => AppCommand::ToggleShowSkipped,
        KeyCode::Char('r') => AppCommand::RunSelected,
        KeyCode::Char('R') => AppCommand::RunFailed,
        KeyCode::Char('f') => AppCommand::SelectNextFailed,
        KeyCode::Char('F') => AppCommand::SelectPreviousFailed,
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
            command_for_key(KeyCode::Char('s'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowSuccess
        );
        assert_eq!(
            command_for_key(KeyCode::Char('x'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowFailed
        );
        assert_eq!(
            command_for_key(KeyCode::Char('i'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowIgnored
        );
        assert_eq!(
            command_for_key(KeyCode::Char('k'), KeyModifiers::NONE, CommandFocus::Tests),
            AppCommand::ToggleShowSkipped
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
