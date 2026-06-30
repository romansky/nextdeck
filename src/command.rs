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
    RefreshTests,
    RunSelected,
    RunFailed,
    ToggleShowSuccess,
    ToggleShowFailed,
    SelectNextFailed,
    SelectPreviousFailed,
    SearchNavigationPending,
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
            Self::RefreshTests => Some("refresh tests"),
            Self::RunSelected => Some("run"),
            Self::RunFailed => Some("rerun failed"),
            Self::ToggleShowSuccess => Some("toggle success"),
            Self::ToggleShowFailed => Some("toggle failed"),
            Self::SelectNextFailed => Some("next failed"),
            Self::SelectPreviousFailed => Some("previous failed"),
            Self::SearchNavigationPending => Some("search"),
            Self::ReportStatus(_) => Some("status"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommandContext {
    pub help_visible: bool,
}

pub fn command_for_input(event: &InputEvent, context: CommandContext) -> AppCommand {
    match event {
        InputEvent::Terminal(Event::Resize(_, _)) => AppCommand::Resize,
        InputEvent::Terminal(Event::Key(key)) if key.kind != KeyEventKind::Press => {
            AppCommand::Noop
        }
        InputEvent::Terminal(Event::Key(key)) if context.help_visible => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => AppCommand::CloseHelp,
            code if is_help_key(code, key.modifiers) => AppCommand::CloseHelp,
            _ => AppCommand::Noop,
        },
        InputEvent::Terminal(Event::Key(key)) => command_for_key(key.code, key.modifiers),
        InputEvent::Terminal(_) => AppCommand::Noop,
        InputEvent::Error(error) => AppCommand::ReportStatus(format!("Input error: {error}")),
    }
}

fn command_for_key(code: KeyCode, modifiers: KeyModifiers) -> AppCommand {
    match code {
        KeyCode::Char('q') => AppCommand::Quit,
        code if is_help_key(code, modifiers) => AppCommand::ToggleHelp,
        KeyCode::Char('/') => AppCommand::SearchNavigationPending,
        KeyCode::Tab => AppCommand::ToggleFocus,
        KeyCode::Up => AppCommand::MoveUp,
        KeyCode::Down => AppCommand::MoveDown,
        KeyCode::Left => AppCommand::MoveLeft,
        KeyCode::Right => AppCommand::MoveRight,
        KeyCode::Enter | KeyCode::Char(' ') => AppCommand::ToggleSelected,
        KeyCode::Home => AppCommand::MoveHome,
        KeyCode::End => AppCommand::MoveEnd,
        KeyCode::PageUp => AppCommand::PageUp,
        KeyCode::PageDown => AppCommand::PageDown,
        KeyCode::Char('u') => AppCommand::RefreshTests,
        KeyCode::Char('s') => AppCommand::ToggleShowSuccess,
        KeyCode::Char('x') => AppCommand::ToggleShowFailed,
        KeyCode::Char('r') => AppCommand::RunSelected,
        KeyCode::Char('R') => AppCommand::RunFailed,
        KeyCode::Char('f') => AppCommand::SelectNextFailed,
        KeyCode::Char('F') => AppCommand::SelectPreviousFailed,
        KeyCode::Char('n') | KeyCode::Char('N') => AppCommand::SearchNavigationPending,
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
            command_for_key(KeyCode::Char('?'), KeyModifiers::NONE),
            AppCommand::ToggleHelp
        );
        assert_eq!(
            command_for_key(KeyCode::Char('?'), KeyModifiers::SHIFT),
            AppCommand::ToggleHelp
        );
        assert_eq!(
            command_for_key(KeyCode::Char('/'), KeyModifiers::SHIFT),
            AppCommand::ToggleHelp
        );
    }

    #[test]
    fn maps_fallback_help_keys() {
        assert_eq!(
            command_for_key(KeyCode::Char('h'), KeyModifiers::NONE),
            AppCommand::ToggleHelp
        );
        assert_eq!(
            command_for_key(KeyCode::F(1), KeyModifiers::NONE),
            AppCommand::ToggleHelp
        );
    }

    #[test]
    fn plain_slash_starts_search_instead_of_help() {
        assert_eq!(
            command_for_key(KeyCode::Char('/'), KeyModifiers::NONE),
            AppCommand::SearchNavigationPending
        );
    }

    #[test]
    fn maps_refresh_and_view_filter_keys() {
        assert_eq!(
            command_for_key(KeyCode::Char('u'), KeyModifiers::NONE),
            AppCommand::RefreshTests
        );
        assert_eq!(
            command_for_key(KeyCode::Char('s'), KeyModifiers::NONE),
            AppCommand::ToggleShowSuccess
        );
        assert_eq!(
            command_for_key(KeyCode::Char('x'), KeyModifiers::NONE),
            AppCommand::ToggleShowFailed
        );
    }

    #[test]
    fn help_context_only_closes_on_close_keys() {
        let context = CommandContext { help_visible: true };
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
