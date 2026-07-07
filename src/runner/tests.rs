use super::*;
use crate::command::OverlayMode;
use crossterm::event::KeyEvent;
use ratatui::style::Color;

fn key(code: KeyCode, modifiers: KeyModifiers) -> QueueEvent {
    QueueEvent::Input(InputEvent::Terminal(Event::Key(KeyEvent::new(
        code, modifiers,
    ))))
}

fn resize(width: u16, height: u16) -> QueueEvent {
    QueueEvent::Input(InputEvent::Terminal(Event::Resize(width, height)))
}

const fn settings_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::SettingsModal,
        overlay: Some(OverlayMode::Settings),
    }
}

const fn output_search_inline_context() -> CommandContext {
    CommandContext {
        input: InputMode::OutputSearchInline,
        overlay: None,
    }
}

fn runtime_context<'a>(
    client: &'a NextestClient,
    settings: &AppSettings,
    theme: Theme,
) -> RunLoopContext<'a> {
    let (queue_tx, _queue_rx) = queue::channel();
    RunLoopContext {
        client,
        theme,
        editor: EditorConfig::resolve(None, settings.open_with_command.clone()),
        cli_open_with: None,
        queue_tx,
        runtime_settings: RuntimeSettings::from_settings(settings),
    }
}

#[test]
fn terminal_resize_events_keep_only_the_latest_pending_resize() {
    let events = vec![resize(80, 24), QueueEvent::Tick, resize(120, 40)];

    assert!(should_skip_stale_event(
        &events,
        0,
        CommandContext::default()
    ));
    assert!(!should_skip_stale_event(
        &events,
        2,
        CommandContext::default()
    ));
}

#[test]
fn tests_pane_width_repeats_keep_only_the_latest_contiguous_intent() {
    let events = vec![
        key(KeyCode::Char(']'), KeyModifiers::NONE),
        key(KeyCode::Right, KeyModifiers::SHIFT),
        key(KeyCode::Char('q'), KeyModifiers::NONE),
    ];

    assert!(should_skip_stale_event(
        &events,
        0,
        CommandContext::default()
    ));
    assert!(!should_skip_stale_event(
        &events,
        1,
        CommandContext::default()
    ));
    assert!(!should_skip_stale_event(
        &events,
        2,
        CommandContext::default()
    ));
}

#[test]
fn repeated_shift_arrow_burst_keeps_only_latest_width_event() {
    let events = (0..80)
        .map(|index| {
            if index % 2 == 0 {
                key(KeyCode::Right, KeyModifiers::SHIFT)
            } else {
                key(KeyCode::Left, KeyModifiers::SHIFT)
            }
        })
        .collect::<Vec<_>>();
    let context = CommandContext::default();

    for index in 0..events.len() - 1 {
        assert!(
            should_skip_stale_event(&events, index, context),
            "width event {index} was not coalesced"
        );
    }
    assert!(!should_skip_stale_event(&events, events.len() - 1, context));
}

#[test]
fn tests_pane_width_coalescing_stops_at_semantic_input_boundaries() {
    let events = vec![
        key(KeyCode::Char(']'), KeyModifiers::NONE),
        key(KeyCode::Tab, KeyModifiers::NONE),
        key(KeyCode::Char(']'), KeyModifiers::NONE),
    ];

    assert!(!should_skip_stale_event(
        &events,
        0,
        CommandContext::default()
    ));
}

#[test]
fn text_input_contexts_do_not_treat_brackets_as_pane_resize() {
    let events = vec![
        key(KeyCode::Char(']'), KeyModifiers::NONE),
        key(KeyCode::Char(']'), KeyModifiers::NONE),
    ];
    let context = output_search_inline_context();

    assert!(!should_skip_stale_event(&events, 0, context));
    assert!(!should_skip_stale_event(&events, 1, context));
}

#[test]
fn modal_contexts_do_not_treat_shift_arrows_as_pane_resize() {
    let events = vec![
        key(KeyCode::Right, KeyModifiers::SHIFT),
        key(KeyCode::Right, KeyModifiers::SHIFT),
    ];
    let context = settings_modal_context();

    assert!(!should_skip_stale_event(&events, 0, context));
    assert!(!should_skip_stale_event(&events, 1, context));
}

#[test]
fn pane_width_settings_save_does_not_rebuild_theme() {
    let client = NextestClient::default();
    let initial_settings = AppSettings {
        theme_mode: config::ThemePreference::Dark,
        ..AppSettings::default()
    };
    let mut context = runtime_context(&client, &initial_settings, Theme::light());
    let mut next_settings = initial_settings.clone();
    next_settings.tree_width_percent += config::TREE_WIDTH_STEP_PERCENT;

    apply_runtime_settings(&mut context, &next_settings);

    assert_eq!(context.theme.text, Theme::light().text);
    assert_eq!(
        context.runtime_settings,
        RuntimeSettings::from_settings(&next_settings)
    );
}

#[test]
fn theme_settings_save_rebuilds_theme() {
    let client = NextestClient::default();
    let initial_settings = AppSettings {
        theme_mode: config::ThemePreference::Dark,
        ..AppSettings::default()
    };
    let mut context = runtime_context(&client, &initial_settings, Theme::light());
    let next_settings = AppSettings {
        color_blind_mode: true,
        ..initial_settings
    };

    apply_runtime_settings(&mut context, &next_settings);

    assert_eq!(context.theme.success, Color::Cyan);
    assert_eq!(
        context.runtime_settings,
        RuntimeSettings::from_settings(&next_settings)
    );
}
