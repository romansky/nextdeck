use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{App, AppEffect},
    config::AppSettings,
    command::{AppCommand, CommandContext, InputMode, command_for_input},
    config,
    disk_usage,
    editor::EditorConfig,
    git_status,
    input::{InputEvent, InputSource},
    nextest::{DiscoveryEvent, NextestClient, RunEvent, RunRequest},
    queue::{self, QueueEvent, QueueSender},
    terminal::AppTerminal,
    theme::Theme,
    ui,
};

pub async fn run(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    run_on_start: bool,
    theme: Theme,
    editor: EditorConfig,
    cli_open_with: Option<String>,
) -> Result<()> {
    let (queue_tx, queue_rx) = queue::channel();

    let discovery = start_discovery(client.clone(), queue_tx.clone());
    let git_status = start_git_status(
        client.project_dir(),
        queue_tx.clone(),
    );
    app.begin_disk_usage_scan();
    let disk_usage = start_disk_usage(
        client.project_dir(),
        queue_tx.clone(),
    );
    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), Duration::from_millis(250));
    let result = run_loop(
        terminal,
        app,
        run_on_start,
        RunLoopContext {
            client,
            theme,
            editor,
            cli_open_with,
            queue_tx,
        },
        queue_rx,
    )
    .await;
    discovery.abort();
    git_status.abort();
    disk_usage.abort();
    ticker.abort();
    drop(input);
    result
}

async fn run_loop(
    terminal: &mut AppTerminal,
    app: &mut App,
    mut run_on_start: bool,
    mut context: RunLoopContext<'_>,
    mut queue_rx: queue::QueueReceiver,
) -> Result<()> {
    let mut run_control = None;
    let mut pending_events = Vec::new();
    while !app.should_quit {
        let size = terminal.size()?;
        let layout = ui::layout(
            Rect::new(0, 0, size.width, size.height),
            app.settings.tree_width_percent,
        );
        app.prepare_frame(layout.tree.height, layout.output.height);
        terminal.draw(|frame| ui::draw(frame, app, &context.theme))?;
        let Some(event) = queue_rx.recv().await else {
            break;
        };
        pending_events.push(event);
        drain_pending_events(&mut queue_rx, &mut pending_events);
        handle_queue_events(
            app,
            &mut context,
            &mut run_on_start,
            &mut pending_events,
            &mut run_control,
        );
        pending_events.clear();
    }
    Ok(())
}

const MAX_EVENTS_PER_FRAME: usize = 256;

fn drain_pending_events(queue_rx: &mut queue::QueueReceiver, events: &mut Vec<QueueEvent>) {
    while events.len() < MAX_EVENTS_PER_FRAME {
        let Ok(event) = queue_rx.try_recv() else {
            break;
        };
        events.push(event);
    }
}

struct RunLoopContext<'a> {
    client: &'a NextestClient,
    theme: Theme,
    editor: EditorConfig,
    cli_open_with: Option<String>,
    queue_tx: QueueSender,
}

fn handle_queue_events(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    run_on_start: &mut bool,
    events: &mut [QueueEvent],
    run_control: &mut Option<RunControl>,
) {
    for index in 0..events.len() {
        if should_skip_stale_event(events, index, app.command_context()) {
            continue;
        }
        handle_queue_event(
            app,
            context,
            run_on_start,
            events[index].clone(),
            run_control,
        );
        if app.should_quit {
            break;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LatestOnlyInput {
    TerminalResize,
    TestsPaneWidth,
}

fn should_skip_stale_event(
    events: &[QueueEvent],
    index: usize,
    context: CommandContext,
) -> bool {
    let Some(class) = latest_only_event(&events[index], context) else {
        return false;
    };
    match class {
        LatestOnlyInput::TerminalResize => events[index + 1..]
            .iter()
            .any(|event| latest_only_event(event, context) == Some(class)),
        LatestOnlyInput::TestsPaneWidth => events[index + 1..]
            .iter()
            .map(|event| latest_only_event(event, context))
            .take_while(Option::is_some)
            .any(|later| later == Some(class)),
    }
}

fn latest_only_event(event: &QueueEvent, context: CommandContext) -> Option<LatestOnlyInput> {
    match event {
        QueueEvent::Input(input) => latest_only_input(input, context),
        QueueEvent::Discovery(_)
        | QueueEvent::CargoClean(_)
        | QueueEvent::DiskUsage(_)
        | QueueEvent::GitStatus(_)
        | QueueEvent::Run(_)
        | QueueEvent::Tick => None,
    }
}

fn latest_only_input(input: &InputEvent, context: CommandContext) -> Option<LatestOnlyInput> {
    match input {
        InputEvent::Terminal(Event::Resize(_, _)) => Some(LatestOnlyInput::TerminalResize),
        InputEvent::Terminal(Event::Key(key))
            if key.kind == KeyEventKind::Press
                && context_accepts_tests_pane_width(context)
                && matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char('[') | KeyCode::Char(']')
                )
                && (key.modifiers.contains(KeyModifiers::SHIFT)
                    || matches!(key.code, KeyCode::Char('[') | KeyCode::Char(']'))) =>
        {
            Some(LatestOnlyInput::TestsPaneWidth)
        }
        InputEvent::Terminal(_) | InputEvent::Error(_) => None,
    }
}

fn context_accepts_tests_pane_width(context: CommandContext) -> bool {
    matches!(context.input, InputMode::Normal(_))
}

fn handle_queue_event(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    run_on_start: &mut bool,
    event: QueueEvent,
    run_control: &mut Option<RunControl>,
) {
    match event {
        QueueEvent::Input(input) => {
            let command = command_for_input(&input, app.command_context());
            if let Some(key) = input.key_display() {
                app.record_key(key_echo_text(key, &command));
            }
            let effect = app.apply_command(command);
            handle_effect(app, context, effect, run_control);
        }
        QueueEvent::Discovery(event) => {
            if app.apply_discovery_event(event) && *run_on_start {
                *run_on_start = false;
                *run_control = start_run(
                    app,
                    context.client.clone(),
                    RunRequest::default(),
                    context.queue_tx.clone(),
                );
            }
        }
        QueueEvent::DiskUsage(result) => app.apply_disk_usage(result),
        QueueEvent::CargoClean(result) => {
            if app.apply_cargo_clean(result) {
                app.begin_disk_usage_scan();
                start_disk_usage(
                    context.client.project_dir(),
                    context.queue_tx.clone(),
                );
            }
        }
        QueueEvent::GitStatus(git_status) => app.apply_git_status(git_status),
        QueueEvent::Run(event) => {
            let finished = matches!(
                event,
                RunEvent::RunnerFinished { .. } | RunEvent::RunnerStopped
            );
            app.apply_run_event(event);
            if finished {
                *run_control = None;
            }
        }
        QueueEvent::Tick => app.tick(),
    }
}

fn key_echo_text(key: String, command: &AppCommand) -> String {
    match command.ticker_label() {
        Some(label) => format!("{key} {label}"),
        None => key,
    }
}

fn handle_effect(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    effect: AppEffect,
    run_control: &mut Option<RunControl>,
) {
    match effect {
        AppEffect::None => {}
        AppEffect::SaveSettings(settings) => {
            if let Err(error) = config::save(settings) {
                app.status = format!("Failed to save settings: {error}");
            } else {
                apply_runtime_settings(context, &app.settings);
            }
        }
        AppEffect::StartDiscovery => {
            start_discovery(context.client.clone(), context.queue_tx.clone());
        }
        AppEffect::StartRun(request) => {
            *run_control = start_run(
                app,
                context.client.clone(),
                request,
                context.queue_tx.clone(),
            );
        }
        AppEffect::StopRun => {
            if let Some(control) = run_control {
                if control.stop_tx.send(()).is_err() {
                    app.status = "Run already stopped".to_owned();
                }
            } else {
                app.status = "No run in progress".to_owned();
            }
        }
        AppEffect::OpenSource(location) => match context.editor.open_source(&location) {
            Ok(()) => {
                app.status = format!("Opened source with {}", context.editor.command());
            }
            Err(error) => {
                app.status = format!("Failed to open source: {error}");
            }
        },
        AppEffect::OpenOutput(request) => match context
            .editor
            .open_text(&request.title, &request.text)
        {
            Ok(path) => {
                app.status = format!("Opened output {}", path.display());
            }
            Err(error) => {
                app.status = format!("Failed to open output: {error}");
            }
        },
        AppEffect::RefreshDiskUsage => {
            start_disk_usage(
                context.client.project_dir(),
                context.queue_tx.clone(),
            );
        }
        AppEffect::RunCargoClean => {
            start_cargo_clean(
                context.client.project_dir(),
                context.queue_tx.clone(),
            );
        }
    }
}

fn apply_runtime_settings(context: &mut RunLoopContext<'_>, settings: &AppSettings) {
    context.editor = EditorConfig::resolve(
        context.cli_open_with.clone(),
        settings.open_with_command.clone(),
    );
    context.theme = Theme::resolve(settings.theme_mode.into(), settings.color_blind_mode);
}

struct RunControl {
    stop_tx: mpsc::UnboundedSender<()>,
}

fn start_discovery(client: NextestClient, tx: QueueSender) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = client
            .discover()
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(QueueEvent::Discovery(DiscoveryEvent::Finished(result)));
    })
}

fn start_git_status(
    cwd: Option<std::path::PathBuf>,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        loop {
            ticker.tick().await;
            let status = git_status::load(cwd.clone()).await;
            if tx.send(QueueEvent::GitStatus(status)).is_err() {
                break;
            }
        }
    })
}

fn start_disk_usage(
    cwd: Option<std::path::PathBuf>,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = disk_usage::load(cwd).await;
        let _ = tx.send(QueueEvent::DiskUsage(result));
    })
}

fn start_cargo_clean(
    cwd: Option<std::path::PathBuf>,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut command = tokio::process::Command::new("cargo");
        command.arg("clean");
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let result = match command.output().await {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                if stderr.is_empty() {
                    Err(format!("cargo clean exited with {}", output.status))
                } else {
                    Err(stderr)
                }
            }
            Err(error) => Err(format!("failed to run cargo clean: {error}")),
        };
        let _ = tx.send(QueueEvent::CargoClean(result));
    })
}

fn start_run(
    app: &mut App,
    client: NextestClient,
    request: RunRequest,
    tx: QueueSender,
) -> Option<RunControl> {
    if !app.begin_run(&request) {
        return None;
    }

    start_disk_usage(client.project_dir(), tx.clone());

    let (run_tx, mut run_rx) = mpsc::unbounded_channel::<RunEvent>();
    let (stop_tx, stop_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        if let Err(error) = client.run(request, run_tx.clone(), stop_rx).await {
            let _ = run_tx.send(RunEvent::RunnerOutput(format!(
                "nextest failed to start: {error}"
            )));
            let _ = run_tx.send(RunEvent::RunnerFinished { exit_code: None });
        }
    });

    tokio::spawn(async move {
        while let Some(event) = run_rx.recv().await {
            if tx.send(QueueEvent::Run(event)).is_err() {
                break;
            }
        }
    });

    Some(RunControl { stop_tx })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> QueueEvent {
        QueueEvent::Input(InputEvent::Terminal(Event::Key(KeyEvent::new(
            code, modifiers,
        ))))
    }

    fn resize(width: u16, height: u16) -> QueueEvent {
        QueueEvent::Input(InputEvent::Terminal(Event::Resize(width, height)))
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
        let context = CommandContext::output_search_inline();

        assert!(!should_skip_stale_event(&events, 0, context));
        assert!(!should_skip_stale_event(&events, 1, context));
    }

    #[test]
    fn modal_contexts_do_not_treat_shift_arrows_as_pane_resize() {
        let events = vec![
            key(KeyCode::Right, KeyModifiers::SHIFT),
            key(KeyCode::Right, KeyModifiers::SHIFT),
        ];
        let context = CommandContext::settings_modal();

        assert!(!should_skip_stale_event(&events, 0, context));
        assert!(!should_skip_stale_event(&events, 1, context));
    }
}
