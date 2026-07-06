    use super::*;
    use crate::command::OverlayMode;
    use crossterm::event::KeyEvent;

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
