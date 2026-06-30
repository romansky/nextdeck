use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    app::{App, FocusPane},
    nextest::{NextestClient, RunRequest},
};

pub fn handle_input(app: &mut App, client: &NextestClient) -> Result<()> {
    if !event::poll(Duration::from_millis(25))? {
        return Ok(());
    }

    let event = event::read()?;
    let Event::Key(key) = event else {
        if matches!(event, Event::Resize(_, _)) {
            app.on_resize();
        }
        return Ok(());
    };
    if key.kind == KeyEventKind::Release {
        return Ok(());
    }

    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.show_help = false,
            code if is_help_key(code, key.modifiers) => app.show_help = false,
            _ => {}
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        code if is_help_key(code, key.modifiers) => app.toggle_help(),
        KeyCode::Tab => app.toggle_focus(),
        KeyCode::Up => match app.focus {
            FocusPane::Tree => app.select_previous(),
            FocusPane::Output => app.scroll_output_up(1),
        },
        KeyCode::Down => match app.focus {
            FocusPane::Tree => app.select_next(),
            FocusPane::Output => app.scroll_output_down(1),
        },
        KeyCode::Left => app.collapse_selected(),
        KeyCode::Right => app.expand_selected(),
        KeyCode::Enter | KeyCode::Char(' ') => app.toggle_selected(),
        KeyCode::Home => match app.focus {
            FocusPane::Tree => app.select_first(),
            FocusPane::Output => app.scroll_output_up(u16::MAX),
        },
        KeyCode::End => match app.focus {
            FocusPane::Tree => app.select_last(),
            FocusPane::Output => app.scroll_output_bottom(),
        },
        KeyCode::Char('r') => {
            app.start_run(
                client.clone(),
                RunRequest {
                    scope: app.selected_scope(),
                },
            );
        }
        KeyCode::Char('R') => {
            if let Some(scope) = app.failed_scope() {
                app.start_run(client.clone(), RunRequest { scope });
            } else {
                app.status = "No failed tests to rerun".to_owned();
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            app.status = "Search navigation is planned for phase 3".to_owned();
        }
        KeyCode::Char('f') => app.select_next_failed(),
        KeyCode::Char('F') => app.select_previous_failed(),
        KeyCode::PageUp => match app.focus {
            FocusPane::Tree => app.select_previous_page(),
            FocusPane::Output => app.scroll_output_up(app.output_page_size),
        },
        KeyCode::PageDown => match app.focus {
            FocusPane::Tree => app.select_next_page(),
            FocusPane::Output => app.scroll_output_down(app.output_page_size),
        },
        _ => {}
    }

    Ok(())
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

    #[test]
    fn recognizes_normalized_question_mark_as_help_key() {
        assert!(is_help_key(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(is_help_key(KeyCode::Char('?'), KeyModifiers::SHIFT));
    }

    #[test]
    fn recognizes_shift_slash_as_help_key() {
        assert!(is_help_key(KeyCode::Char('/'), KeyModifiers::SHIFT));
    }

    #[test]
    fn recognizes_fallback_help_keys() {
        assert!(is_help_key(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(is_help_key(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(is_help_key(KeyCode::Char('H'), KeyModifiers::SHIFT));
        assert!(is_help_key(KeyCode::F(1), KeyModifiers::NONE));
    }
}
