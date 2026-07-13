use super::*;
use crate::command::OverlayMode;
use crossterm::event::KeyEvent;
use ratatui::style::Color;
use tokio::sync::oneshot;

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
        xtask_persistence: XtaskPersistence::default(),
        disk_usage: None,
    }
}

#[tokio::test]
async fn run_control_shutdown_requests_stop_and_awaits_owned_tasks() {
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = oneshot::channel();
    let producer = tokio::spawn(async move {
        let _ = stop_rx.recv().await;
        let _ = done_tx.send(());
    });
    let forwarder = tokio::spawn(async {});
    let control = RunControl {
        stop_tx: Some(stop_tx),
        process_tracker: ProcessTracker::default(),
        producer: Some(producer),
        forwarder: Some(forwarder),
    };

    control.shutdown().await;

    assert!(done_rx.await.is_ok());
}

#[tokio::test]
async fn dropping_disk_usage_control_cancels_its_scan() {
    let cancellation = disk_usage::DiskScanCancellation::default();
    let observed = cancellation.clone();
    let control = DiskUsageControl {
        request_id: RequestId(1),
        cancellation,
        task: tokio::spawn(std::future::pending()),
    };

    drop(control);

    assert!(observed.is_cancelled());
}

#[tokio::test]
async fn stale_disk_result_does_not_cancel_newer_scan() {
    let client = NextestClient::default();
    let settings = AppSettings::default();
    let mut context = runtime_context(&client, &settings, Theme::dark());
    let cancellation = disk_usage::DiskScanCancellation::default();
    let observed = cancellation.clone();
    context.disk_usage = Some(DiskUsageControl {
        request_id: RequestId(2),
        cancellation,
        task: tokio::spawn(std::future::pending()),
    });

    context.finish_disk_usage(RequestId(1));
    assert!(context.disk_usage.is_some());
    assert!(!observed.is_cancelled());

    context.finish_disk_usage(RequestId(2));
    assert!(context.disk_usage.is_none());
    assert!(observed.is_cancelled());
}

#[tokio::test]
async fn terminal_run_event_is_forwarded_after_late_stream_events() {
    let (run_tx, run_rx) = mpsc::channel(4);
    let (queue_tx, mut queue_rx) = queue::channel();
    let forwarder = tokio::spawn(forward_run_events(run_rx, queue_tx));

    run_tx
        .send(RunEvent::RunnerFinished { exit_code: Some(0) })
        .await
        .expect("run channel open");
    run_tx
        .send(RunEvent::RunnerOutput("late event".to_owned()))
        .await
        .expect("run channel open");

    assert!(matches!(
        queue_rx.recv().await,
        Some(QueueEvent::Run(RunEvent::RunnerOutput(output))) if output == "late event"
    ));
    assert!(queue_rx.try_recv().is_err());

    drop(run_tx);
    assert!(matches!(
        queue_rx.recv().await,
        Some(QueueEvent::Run(RunEvent::RunnerFinished {
            exit_code: Some(0)
        }))
    ));
    forwarder.await.expect("forwarder completes");
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
        key(KeyCode::Char('Q'), KeyModifiers::SHIFT),
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
fn idle_tick_does_not_request_redraw() {
    let client = NextestClient::default();
    let settings = AppSettings::default();
    let mut context = runtime_context(&client, &settings, Theme::dark());
    let mut app = App::with_settings(crate::tree::Tree::from_tests(Vec::new()), settings);
    let mut run_control = None;
    let mut events = [QueueEvent::Tick];

    assert_eq!(
        handle_queue_events(&mut app, &mut context, &mut events, &mut run_control,),
        UiDirty::NONE
    );

    app.record_key("x");
    let mut events = [QueueEvent::Tick];
    let dirty = handle_queue_events(&mut app, &mut context, &mut events, &mut run_control);

    assert!(dirty.contains(UiDirty::STATUS));
    assert!(!dirty.contains(UiDirty::TREE));
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
