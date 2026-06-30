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
    RunSelected,
    RunFailed,
    SelectNextFailed,
    SelectPreviousFailed,
    SearchNavigationPending,
    ReportStatus(String),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommandContext {
    pub help_visible: bool,
}

pub fn command_for_input(event: &InputEvent, context: CommandContext) -> AppCommand {
    match event {
        InputEvent::Terminal(Event::Resize(_, _)) => AppCommand::Resize,
        InputEvent::Terminal(Event::Key(key)) if key.kind == KeyEventKind::Release => {
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
        KeyCode::Char('r') => AppCommand::RunSelected,
        KeyCode::Char('R') => AppCommand::RunFailed,
        KeyCode::Char('f') => AppCommand::SelectNextFailed,
        KeyCode::Char('F') => AppCommand::SelectPreviousFailed,
        KeyCode::Char('n') | KeyCode::Char('N') => AppCommand::SearchNavigationPending,
        _ => AppCommand::Noop,
    }
}

fn is_help_key(code: KeyCode, _modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('?'))
        || matches!(
            code,
            KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::F(1)
        )
        || matches!(code, KeyCode::Char('/'))
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
    }

    #[test]
    fn maps_fallback_help_keys() {
        assert_eq!(
            command_for_key(KeyCode::Char('/'), KeyModifiers::NONE),
            AppCommand::ToggleHelp
        );
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
    fn resize_is_a_command() {
        let event = InputEvent::Terminal(Event::Resize(80, 24));
        assert_eq!(
            command_for_input(&event, CommandContext::default()),
            AppCommand::Resize
        );
    }
}
