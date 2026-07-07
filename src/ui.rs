use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane},
    command::{CommandGroup, CommandInfo, OverlayMode, command_infos},
    config,
    disk_usage::{StorageHealth, format_bytes, format_timestamp_utc},
    nextest::{manual_run_command, manual_test_command},
    output_pane::{
        OutputPaneState, OutputSearchState, OutputView, SearchModalFocus,
        output_render_scroll_for_count,
    },
    settings::SettingsField,
    symbols::bool_symbol,
    theme::Theme,
    tree::{NodeKind, TestNode, TestStatus},
    xtask::{XtaskArgValue, XtaskDetailFocus, XtaskState, XtaskValueSpec},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppLayout {
    pub tree: Rect,
    pub details: Rect,
    pub output: Rect,
    pub status: Rect,
}

struct PanelChrome<'a> {
    status: &'a str,
    actions: &'a str,
}

struct OutputPaneRender<'a> {
    state: &'a OutputPaneState,
    source_text: String,
    label: String,
    focused: bool,
}

struct OutputPaneContent {
    status: String,
    lines: Vec<Line<'static>>,
    scroll: u16,
}

struct ModalChrome<'a> {
    title: &'a str,
    actions: Option<&'a str>,
}

pub fn layout(area: Rect, tree_width_percent: u16) -> AppLayout {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let tree_width_percent = config::clamp_tree_width(tree_width_percent);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tree_width_percent),
            Constraint::Percentage(100 - tree_width_percent),
        ])
        .split(outer[0]);

    let details_height = if panes[1].height < 14 {
        panes[1].height.saturating_sub(3).max(1)
    } else {
        12
    };

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(details_height), Constraint::Min(1)])
        .split(panes[1]);

    AppLayout {
        tree: panes[0],
        details: right[0],
        output: right[1],
        status: outer[1],
    }
}

pub fn xtask_output_page_size(area: Rect) -> u16 {
    let modal_area = centered_rect(88, 82, area);
    let inner = modal_area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let output_area = xtask_detail_output_area(inner);
    output_area.height.saturating_sub(2).max(1)
}

pub fn draw(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let app_layout = layout(frame.area(), app.settings.tree_width_percent);
    draw_tree(frame, app, theme, app_layout.tree);
    draw_details(frame, app, theme, app_layout.details);
    draw_output(frame, app, theme, app_layout.output);
    draw_status(frame, app, theme, app_layout.status);

    match app.command_context().overlay {
        Some(OverlayMode::Discovery | OverlayMode::DiscoveryError) => {
            draw_discovery_modal(frame, app, theme);
        }
        Some(OverlayMode::OutputSearch) => {
            draw_output_search_modal(frame, app.active_output_search(), theme);
        }
        Some(OverlayMode::DiskCleanup) => draw_disk_cleanup_modal(frame, app, theme),
        Some(OverlayMode::Xtasks) => draw_xtasks_modal(frame, app, theme),
        Some(OverlayMode::TestDetails) => draw_test_details_modal(frame, app, theme),
        Some(OverlayMode::Settings) => draw_global_settings_modal(frame, app, theme),
        Some(OverlayMode::Help) => draw_help(frame, app, theme),
        None => {}
    }
    if app.xtasks.modal_open && app.xtasks.output.search.modal_open {
        draw_output_search_modal(frame, app.active_output_search(), theme);
    }
}

fn draw_tree(frame: &mut Frame<'_>, app: &App, theme: &Theme, area: Rect) {
    let selected = app.tree.selected_index();
    let rows = app.tree.visible_rows();
    let visible_height = area.height.saturating_sub(2).max(1) as usize;
    let items = rows
        .iter()
        .enumerate()
        .skip(app.tree_scroll)
        .take(visible_height)
        .map(|(index, row)| {
            tree_item(
                row.depth,
                row.node,
                index == selected,
                app.running_test_spinner(),
                app.settings.tree_duration_mode,
                theme,
            )
        })
        .collect::<Vec<_>>();

    let focused = pane_focused(app, FocusPane::Tree);
    let status = tests_status(app);
    let list = List::new(items)
        .block(theme.panel_block(&status, Some(tests_actions()), focused))
        .highlight_style(theme.selected());
    frame.render_widget(Clear, area);
    frame.render_widget(list, area);
}

fn pane_focused(app: &App, pane: FocusPane) -> bool {
    app.focus == pane && !app.command_context().pane_focus_suppressed()
}

fn tests_status(app: &App) -> String {
    format!(
        "Tests <filters: {} {} {} {}>",
        filter_hint("pass", "p", app.tree.view_filter.show_success),
        filter_hint("fail", "f", app.tree.view_filter.show_failed),
        filter_hint("ignore", "i", app.tree.view_filter.show_ignored),
        filter_hint("skip", "s", app.tree.view_filter.show_skipped)
    )
}

fn filter_hint(label: &str, key: &str, enabled: bool) -> String {
    let Some((head, tail)) = label.split_once(key) else {
        return format!("[{key}]{label}:{}", bool_symbol(enabled));
    };
    format!("{head}[{key}]{tail}:{}", bool_symbol(enabled))
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn fit_line_content(content: &str, width: usize) -> String {
    let char_count = content.chars().count();
    if char_count <= width {
        return format!("{content:<width$}");
    }
    content
        .chars()
        .skip(char_count.saturating_sub(width))
        .collect()
}

fn fit_line_prefix(content: &str, width: usize) -> String {
    let char_count = content.chars().count();
    if char_count <= width {
        return format!("{content:<width$}");
    }
    if width <= 3 {
        return content.chars().take(width).collect();
    }
    let prefix = content.chars().take(width - 3).collect::<String>();
    format!("{prefix}...")
}

fn draw_discovery_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(62, 58, frame.area());
    if app.discovery.error.is_some() {
        let output = output_pane_content(
            theme,
            OutputPaneRender {
                state: &app.main_output,
                source_text: app.output_source_text(),
                label: "Discovery".to_owned(),
                focused: false,
            },
        );
        draw_modal_output_lines(
            frame,
            theme,
            area,
            ModalChrome {
                title: &output.status,
                actions: Some(discovery_error_actions()),
            },
            output.lines,
            output.scroll,
        );
    } else {
        let lines = vec![
            Line::styled(
                format!("{} Discovering tests", app.discovery_spinner()),
                theme.accent(),
            ),
            Line::from(""),
            Line::styled(
                "Running cargo nextest list --message-format json",
                theme.text(),
            ),
            Line::styled(
                "Cold discovery may compile test binaries first.",
                theme.muted(),
            ),
            Line::styled(
                format!("Elapsed: {}s", app.discovery_elapsed_seconds()),
                theme.text(),
            ),
        ];
        draw_modal_lines(
            frame,
            theme,
            area,
            ModalChrome {
                title: "Discovery",
                actions: Some("[q]quit"),
            },
            lines,
        );
    }
}

fn draw_test_details_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(74, 72, frame.area());
    draw_modal_lines(
        frame,
        theme,
        area,
        ModalChrome {
            title: "Test Details",
            actions: Some("[esc]close"),
        },
        test_details_modal_lines(app, theme),
    );
}

fn draw_modal_lines(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: ModalChrome<'_>,
    lines: Vec<Line<'static>>,
) {
    let inner = draw_modal_shell(frame, theme, area, chrome);
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn draw_modal_shell(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: ModalChrome<'_>,
) -> Rect {
    let block = theme.panel_block(chrome.title, chrome.actions, true);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    inner
}

fn draw_modal_output_lines(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: ModalChrome<'_>,
    lines: Vec<Line<'static>>,
    scroll: u16,
) {
    let page_size = area.height.saturating_sub(2).max(1);
    let text_line_count = lines.len().max(1);
    let scroll = output_render_scroll_for_count(text_line_count, page_size, scroll);
    let inner = draw_modal_shell(frame, theme, area, chrome);
    let output = Paragraph::new(lines)
        .style(theme.text())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(output, inner);
}

fn draw_output_search_modal(frame: &mut Frame<'_>, search: &OutputSearchState, theme: &Theme) {
    let area = centered_rect(70, 70, frame.area());
    let inner = draw_modal_shell(
        frame,
        theme,
        area,
        ModalChrome {
            title: "Output Search",
            actions: Some("[tab]focus [enter]activate [C+enter]apply [esc]cancel"),
        },
    );
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(inner);
    let query_focused = search.modal_focus == SearchModalFocus::Query;

    frame.render_widget(
        Paragraph::new(Line::styled(
            "Query",
            modal_label_style(query_focused, theme),
        )),
        chunks[0],
    );

    let mut editor = search.editor.widget();
    editor.set_style(theme.text());
    editor.set_placeholder_text("Search output...");
    editor.set_placeholder_style(theme.muted());
    editor.set_cursor_line_style(if query_focused {
        theme.selected()
    } else {
        theme.text()
    });
    editor.set_cursor_style(if query_focused {
        theme.selected()
    } else {
        theme.text()
    });
    editor.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border(query_focused)),
    );
    frame.render_widget(&editor, chunks[1]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            modal_button(
                "Clear",
                search.modal_focus == SearchModalFocus::Clear,
                theme,
            ),
            Span::raw("  "),
            modal_button(
                "Apply",
                search.modal_focus == SearchModalFocus::Apply,
                theme,
            ),
        ])),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(vec![
            modal_checkbox(
                "filter matching lines",
                search.draft_filter,
                search.modal_focus == SearchModalFocus::Filter,
                theme,
            ),
            modal_checkbox(
                "regex",
                search.draft_regex,
                search.modal_focus == SearchModalFocus::Regex,
                theme,
            ),
            modal_checkbox(
                "case-sensitive",
                search.draft_case_sensitive,
                search.modal_focus == SearchModalFocus::CaseSensitive,
                theme,
            ),
        ]),
        chunks[4],
    );
}

fn draw_disk_cleanup_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(70, 62, frame.area());
    draw_modal_lines(
        frame,
        theme,
        area,
        ModalChrome {
            title: "Disk Cleanup",
            actions: Some(disk_cleanup_actions()),
        },
        disk_cleanup_lines(app, theme),
    );
}

fn disk_cleanup_lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled("Disk Usage", theme.title(false)),
        Line::from(""),
    ];

    if let Some(snapshot) = &app.disk_usage.snapshot {
        for entry in &snapshot.entries {
            lines.push(Line::from(vec![
                Span::styled(format!("{:<15}", entry.label), theme.muted()),
                Span::styled(format!("{:>10}", format_bytes(entry.bytes)), theme.text()),
                Span::raw("  "),
                Span::styled(entry.path.display().to_string(), theme.muted()),
            ]));
        }
    } else if app.disk_usage.loading {
        lines.push(Line::styled("Scanning disk usage...", theme.muted()));
    } else if let Some(error) = &app.disk_usage.error {
        lines.push(Line::styled(error.clone(), theme.danger()));
    } else {
        lines.push(Line::styled("No disk usage snapshot yet.", theme.muted()));
    }

    lines.extend([
        Line::from(""),
        Line::styled("Cleanup", theme.title(false)),
        Line::styled(
            "cargo clean removes this workspace's target directory.",
            theme.text(),
        ),
    ]);

    if let Some(result) = &app.disk_cleanup.last_result {
        lines.push(Line::from(""));
        match result {
            Ok(()) => lines.push(Line::styled("Last cleanup completed.", theme.success())),
            Err(error) => lines.push(Line::styled(
                format!("Last cleanup failed: {error}"),
                theme.danger(),
            )),
        }
    }

    lines
}

fn draw_xtasks_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(88, 82, frame.area());
    let title = xtask_modal_title(&app.xtasks);
    let inner = draw_modal_shell(
        frame,
        theme,
        area,
        ModalChrome {
            title: &title,
            actions: Some(xtask_actions(&app.xtasks)),
        },
    );
    if app.xtasks.detail_open {
        draw_xtask_detail_frame(frame, app, theme, inner);
    } else {
        let paragraph = Paragraph::new(xtask_list_lines(&app.xtasks, theme))
            .alignment(Alignment::Left)
            .style(theme.text())
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }
}

fn xtask_modal_title(xtasks: &XtaskState) -> String {
    if xtasks.detail_open {
        if let Some(command) = xtasks.selected_command() {
            return format!("Xtasks > {}", command.name);
        }
    }
    "Xtasks".to_owned()
}

fn xtask_list_lines(xtasks: &XtaskState, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if xtasks.loading {
        lines.push(Line::styled(
            "Loading cargo xtask metadata...",
            theme.muted(),
        ));
        if xtasks.manifest.is_none() {
            return lines;
        }
    }
    if let Some(error) = &xtasks.error {
        lines.push(Line::styled(format!("Error: {error}"), theme.danger()));
    }
    let Some(manifest) = &xtasks.manifest else {
        lines.extend([
            Line::from(""),
            Line::styled(
                "This project has not exposed nextdeck xtask metadata yet.",
                theme.text(),
            ),
            Line::styled(
                "Expected: cargo xtask nextdeck-info --format json",
                theme.muted(),
            ),
        ]);
        return lines;
    };
    if manifest.commands.is_empty() {
        lines.push(Line::styled("No xtask commands exposed.", theme.muted()));
        return lines;
    }

    for (index, command) in manifest.commands.iter().enumerate() {
        let selected = index == xtasks.selected_command;
        let marker = if selected { ">" } else { " " };
        let style = if selected {
            theme.selected()
        } else {
            theme.text()
        };
        let about = command.about.as_deref().unwrap_or("");
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} {:<18}", command.name), style),
            Span::styled(fit_line_content(about, 46), style),
        ]));
    }

    lines
}

fn draw_xtask_detail_frame(frame: &mut Frame<'_>, app: &App, theme: &Theme, area: Rect) {
    let output_area = xtask_detail_output_area(area);
    let params_focused = app.xtasks.detail_focus == XtaskDetailFocus::Parameters;
    let output_focused = app.xtasks.detail_focus == XtaskDetailFocus::Output;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(1),
            Constraint::Percentage(60),
        ])
        .split(area);

    draw_xtask_params_panel(frame, theme, chunks[0], &app.xtasks, params_focused);
    frame.render_widget(Clear, chunks[1]);
    draw_xtask_output_panel(frame, app, theme, output_area, output_focused);
}

fn xtask_detail_output_area(area: Rect) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(1),
            Constraint::Percentage(60),
        ])
        .split(area);
    chunks[2]
}

fn draw_xtask_params_panel(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    xtasks: &XtaskState,
    focused: bool,
) {
    let content_width = area.width.saturating_sub(2).max(1) as usize;
    let block = theme.panel_block(
        "Parameters",
        focused.then_some("[up/down]select [left/right]change"),
        focused,
    );
    let inner = block.inner(area);
    let paragraph = Paragraph::new(xtask_param_lines(xtasks, theme, content_width, focused))
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(paragraph, inner);
}

fn xtask_param_lines(
    xtasks: &XtaskState,
    theme: &Theme,
    content_width: usize,
    focused: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(command) = xtasks.selected_command() {
        let manual = xtasks
            .run_request()
            .map(|request| request.command_line())
            .unwrap_or_else(|error| error.to_string());
        lines.push(Line::styled(
            fit_line_prefix(&format!("Command: {}", command.name), content_width),
            theme.title(false),
        ));
        if let Some(about) = &command.about {
            lines.push(Line::styled(
                fit_line_prefix(about, content_width),
                theme.text(),
            ));
        }
        lines.push(Line::styled(
            fit_line_prefix(&manual, content_width),
            theme.muted(),
        ));
        lines.push(Line::from(""));
        if command.args.is_empty() {
            lines.push(Line::styled("No parameters.", theme.muted()));
        } else {
            let value_width = content_width.saturating_sub(10).min(13);
            let flag_width = content_width.saturating_sub(value_width + 2).max(1);
            let help_width = content_width.saturating_sub(10);
            for (index, arg) in command.args.iter().enumerate() {
                let selected = index == xtasks.selected_arg;
                let active = focused && selected;
                let marker = if selected { ">" } else { " " };
                let style = if active {
                    theme.selected()
                } else {
                    theme.text()
                };
                let value = xtask_arg_value_text(xtasks, &command.name, &arg.name, selected);
                let kind = xtask_arg_kind(&arg.value);
                let help = arg.help.as_deref().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{marker} {}", fit_line_prefix(&arg.flag(), flag_width)),
                        style,
                    ),
                    Span::styled(fit_line_prefix(&value, value_width), style),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{kind:<8}"), theme.muted()),
                    Span::styled(fit_line_prefix(help, help_width), theme.muted()),
                ]));
            }
        }
    }

    lines
}

fn draw_xtask_output_panel(
    frame: &mut Frame<'_>,
    app: &App,
    theme: &Theme,
    area: Rect,
    focused: bool,
) {
    draw_output_pane(
        frame,
        theme,
        area,
        OutputPaneRender {
            state: &app.xtasks.output,
            source_text: app.xtasks.output_text(),
            label: xtask_output_label(&app.xtasks, app.running_test_spinner()),
            focused,
        },
    );
}

fn xtask_output_label(xtasks: &XtaskState, spinner: &str) -> String {
    if xtasks.running {
        format!("Output: {spinner}")
    } else if let Some(output) = &xtasks.last_run {
        if output.success {
            "Output: ✓".to_owned()
        } else {
            "Output: ✗".to_owned()
        }
    } else {
        "Output".to_owned()
    }
}

fn xtask_arg_value_text(
    xtasks: &XtaskState,
    command_name: &str,
    arg_name: &str,
    selected: bool,
) -> String {
    if selected
        && let Some(editing) = &xtasks.editing
        && editing.command == command_name
        && editing.arg == arg_name
    {
        return format!("[{}]", editing.input.view(16, true));
    }
    xtasks
        .values
        .get(command_name)
        .and_then(|values| values.get(arg_name))
        .map(XtaskArgValue::display)
        .unwrap_or_default()
}

fn xtask_arg_kind(value: &XtaskValueSpec) -> &'static str {
    match value {
        XtaskValueSpec::Bool { .. } => "bool",
        XtaskValueSpec::Number { .. } => "number",
        XtaskValueSpec::String { .. } => "string",
        XtaskValueSpec::Enum { .. } => "enum",
    }
}

fn xtask_actions(xtasks: &XtaskState) -> &'static str {
    if xtasks.editing.is_some() {
        "[enter]save [esc]cancel"
    } else if xtasks.detail_open {
        match xtasks.detail_focus {
            XtaskDetailFocus::Parameters => {
                "[esc]back [tab]output [up/down]param [left/right]change [e]edit [r]run"
            }
            XtaskDetailFocus::Output => {
                "[esc]back [tab]params [up/down]scroll [/]search [n/N]match [r]run"
            }
        }
    } else {
        "[up/down]command [enter]open [u]refresh [esc]close"
    }
}

fn draw_global_settings_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(72, 62, frame.area());
    let settings = &app.global_settings;
    let lines = vec![
        settings_line(app, SettingsField::OpenWith, theme),
        settings_line(app, SettingsField::TreeWidth, theme),
        settings_line(app, SettingsField::TreeDuration, theme),
        settings_line(app, SettingsField::StorageThreshold, theme),
        settings_line(app, SettingsField::Theme, theme),
        settings_line(app, SettingsField::ColorBlindMode, theme),
    ];
    let actions = if settings.open_with_editing {
        "[enter]save [esc]cancel"
    } else {
        "[up/down]select [left/right]change [enter]edit/apply [esc]close"
    };
    let inner = draw_modal_shell(
        frame,
        theme,
        area,
        ModalChrome {
            title: "Settings",
            actions: Some(actions),
        },
    );
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn settings_line(app: &App, field: SettingsField, theme: &Theme) -> Line<'static> {
    let selected = app.global_settings.selected == field;
    let marker = if selected { ">" } else { " " };
    let value = settings_value(app, field);
    let style = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    Line::from(vec![
        Span::styled(format!("{marker} {:<13}", field.label()), style),
        Span::styled(value, style),
    ])
}

fn settings_value(app: &App, field: SettingsField) -> String {
    match field {
        SettingsField::OpenWith if app.global_settings.open_with_editing => {
            format!("[{}]", app.global_settings.open_with.view(42, true))
        }
        SettingsField::OpenWith => {
            format!("[{}]", fit_line_content(app.settings.open_with_label(), 42))
        }
        SettingsField::TreeWidth => format!("{}%", app.settings.tree_width_percent),
        SettingsField::TreeDuration => app.settings.tree_duration_mode.label().to_owned(),
        SettingsField::StorageThreshold => {
            format!("{} GiB", app.settings.storage_low_space_threshold_gb)
        }
        SettingsField::Theme => app.settings.theme_mode.label().to_owned(),
        SettingsField::ColorBlindMode => on_off(app.settings.color_blind_mode).to_owned(),
    }
}

fn modal_label_style(active: bool, theme: &Theme) -> ratatui::style::Style {
    if active {
        theme.title(true)
    } else {
        theme.muted()
    }
}

fn modal_button(label: &'static str, active: bool, theme: &Theme) -> Span<'static> {
    Span::styled(
        format!("[ {label} ]"),
        if active {
            theme.selected()
        } else {
            theme.text()
        },
    )
}

fn modal_checkbox(
    label: &'static str,
    checked: bool,
    active: bool,
    theme: &Theme,
) -> Line<'static> {
    let marker = if checked { "x" } else { " " };
    Line::styled(
        format!("[{marker}] {label}"),
        if active {
            theme.selected()
        } else {
            theme.text()
        },
    )
}

fn tree_item<'a>(
    depth: usize,
    node: &TestNode,
    selected: bool,
    running_spinner: &str,
    duration_mode: config::TreeDurationMode,
    theme: &Theme,
) -> ListItem<'a> {
    let row_style = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    let status_style = theme.status(node.status, selected);
    ListItem::new(Line::from(vec![
        Span::styled(tree_leading_fields(depth, node, duration_mode), row_style),
        Span::styled(
            node_label(node, running_spinner),
            if selected { row_style } else { status_style },
        ),
    ]))
}

fn tree_leading_fields(
    depth: usize,
    node: &TestNode,
    duration_mode: config::TreeDurationMode,
) -> String {
    format!(
        "{}{} {} ",
        "  ".repeat(depth),
        fold_marker(node),
        duration_field(node.display_duration(duration_mode))
    )
}

fn fold_marker(node: &TestNode) -> &'static str {
    if node.children.is_empty() {
        " "
    } else if node.expanded {
        "v"
    } else {
        ">"
    }
}

fn node_label(node: &TestNode, running_spinner: &str) -> String {
    let label = match &node.kind {
        NodeKind::Workspace => node.label.clone(),
        NodeKind::Package { name } => name.clone(),
        NodeKind::Binary { .. } | NodeKind::Module { .. } | NodeKind::Test(_) => node.label.clone(),
    };
    if node.status == TestStatus::Running {
        format!("{label} {running_spinner}")
    } else {
        label
    }
}

fn draw_details(frame: &mut Frame<'_>, app: &App, theme: &Theme, area: Rect) {
    let status = info_status(app);
    let block = theme.panel_block(&status, Some(info_actions()), false);
    let inner = block.inner(area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(inner);
    let run_details = Paragraph::new(selected_details(app, theme))
        .style(theme.text())
        .wrap(Wrap { trim: false });
    let storage_details = Paragraph::new(storage_details(app, theme))
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(run_details, columns[0]);
    frame.render_widget(storage_details, columns[1]);
}

fn selected_details(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let Some(node) = app.tree.selected_node() else {
        return vec![Line::styled("No selection", theme.muted())];
    };

    let mut lines = run_details(app, theme);
    lines.push(Line::from(""));
    lines.push(Line::styled("Selection", theme.title(false)));

    match &node.kind {
        NodeKind::Workspace => {
            lines.extend([
                detail_line("kind", "workspace", theme.text(), theme),
                detail_status_line(node.status, theme),
                detail_line("path", app.tree.selected_path(), theme.text(), theme),
            ]);
        }
        NodeKind::Package { name } => {
            lines.extend([
                detail_line("kind", "package", theme.text(), theme),
                detail_line("pkg", name.clone(), theme.accent(), theme),
                detail_status_line(node.status, theme),
                detail_line(
                    "duration",
                    duration_label(node, app.settings.tree_duration_mode),
                    theme.text(),
                    theme,
                ),
            ]);
        }
        NodeKind::Binary {
            package,
            name,
            kind,
        } => {
            let source = first_source_path(node);
            lines.extend([
                detail_line("kind", "target", theme.text(), theme),
                detail_line("pkg", package.clone(), theme.accent(), theme),
                detail_line("target", name.clone(), theme.accent(), theme),
                detail_line("type", kind.clone(), theme.text(), theme),
                detail_line("source", source, theme.text(), theme),
                detail_status_line(node.status, theme),
                detail_line(
                    "duration",
                    duration_label(node, app.settings.tree_duration_mode),
                    theme.text(),
                    theme,
                ),
            ]);
        }
        NodeKind::Module { path } => {
            lines.extend([
                detail_line("kind", "module", theme.text(), theme),
                detail_line("module", path.clone(), theme.accent(), theme),
                detail_status_line(node.status, theme),
                detail_line(
                    "duration",
                    duration_label(node, app.settings.tree_duration_mode),
                    theme.text(),
                    theme,
                ),
            ]);
        }
        NodeKind::Test(test) => {
            lines.extend([
                detail_line("kind", "test", theme.text(), theme),
                detail_status_line(node.status, theme),
                detail_line("pkg", test.package.clone(), theme.accent(), theme),
                detail_line("bin", test.binary.clone(), theme.text(), theme),
                detail_line(
                    "module",
                    test.module.clone().unwrap_or_else(|| "-".to_owned()),
                    theme.text(),
                    theme,
                ),
                detail_line("test", test.full_name.clone(), theme.accent(), theme),
                detail_line(
                    "source",
                    test.source_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_owned()),
                    theme.text(),
                    theme,
                ),
                detail_line(
                    "duration",
                    duration_label(node, app.settings.tree_duration_mode),
                    theme.text(),
                    theme,
                ),
            ]);
        }
    }

    lines
}

fn test_details_modal_lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let Some(node) = app.tree.selected_node() else {
        return vec![Line::styled("No selection", theme.muted())];
    };

    let scope = app.selected_scope();
    let counts = app.tree.status_counts_for_scope(&scope);
    let mut lines = vec![
        Line::styled(app.tree.selected_path(), theme.title(true)),
        Line::from(""),
        detail_line("kind", selected_kind_label(node), theme.text(), theme),
        detail_status_line(node.status, theme),
        detail_line(
            "duration",
            duration_label(node, app.settings.tree_duration_mode),
            theme.text(),
            theme,
        ),
        detail_line("tests", status_counts_label(counts), theme.text(), theme),
    ];

    match &node.kind {
        NodeKind::Workspace => {}
        NodeKind::Package { name } => {
            lines.push(detail_line("package", name.clone(), theme.accent(), theme));
        }
        NodeKind::Binary {
            package,
            name,
            kind,
        } => {
            lines.extend([
                detail_line("package", package.clone(), theme.accent(), theme),
                detail_line("binary", name.clone(), theme.text(), theme),
                detail_line("target", kind.clone(), theme.text(), theme),
                detail_line("source", first_source_path(node), theme.text(), theme),
            ]);
        }
        NodeKind::Module { path } => {
            lines.push(detail_line("module", path.clone(), theme.accent(), theme));
            lines.push(detail_line(
                "source",
                first_source_path(node),
                theme.text(),
                theme,
            ));
        }
        NodeKind::Test(test) => {
            lines.extend([
                detail_line("package", test.package.clone(), theme.accent(), theme),
                detail_line("binary", test.binary.clone(), theme.text(), theme),
                detail_line("target", test.binary_kind.clone(), theme.text(), theme),
                detail_line(
                    "module",
                    test.module.clone().unwrap_or_else(|| "-".to_owned()),
                    theme.text(),
                    theme,
                ),
                detail_line("ignored", bool_label(test.ignored), theme.text(), theme),
                detail_line(
                    "source",
                    test.source_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_owned()),
                    theme.text(),
                    theme,
                ),
                detail_line("output", output_summary(node), theme.text(), theme),
            ]);
        }
    }

    lines.extend([
        Line::from(""),
        Line::styled("Manual", theme.title(true)),
        detail_line(
            "cargo",
            selected_manual_command(app, node),
            theme.accent(),
            theme,
        ),
    ]);
    lines
}

fn selected_kind_label(node: &TestNode) -> &'static str {
    match &node.kind {
        NodeKind::Workspace => "workspace",
        NodeKind::Package { .. } => "package",
        NodeKind::Binary { .. } => "target",
        NodeKind::Module { .. } => "module",
        NodeKind::Test(_) => "test",
    }
}

fn status_counts_label(counts: crate::state::StatusCounts) -> String {
    format!(
        "{} pending, {} running, {} passed, {} failed, {} ignored, {} skipped",
        counts.pending,
        counts.running,
        counts.passed,
        counts.failed,
        counts.ignored,
        counts.skipped
    )
}

fn selected_manual_command(app: &App, node: &TestNode) -> String {
    if let NodeKind::Test(test) = &node.kind {
        manual_test_command(test)
    } else {
        manual_run_command(&app.selected_scope())
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn output_summary(node: &TestNode) -> String {
    let stdout_len = node.output.stdout.trim().len();
    let stderr_len = node.output.stderr.trim().len();
    match (stdout_len, stderr_len) {
        (0, 0) => "none captured".to_owned(),
        (_, 0) => format!("stdout {stdout_len} chars"),
        (0, _) => format!("stderr {stderr_len} chars"),
        (_, _) => format!("stdout {stdout_len} chars, stderr {stderr_len} chars"),
    }
}

fn first_source_path(node: &TestNode) -> String {
    if let NodeKind::Test(test) = &node.kind
        && let Some(path) = &test.source_path
    {
        return path.display().to_string();
    }
    node.children
        .iter()
        .find_map(|child| {
            let path = first_source_path(child);
            (path != "-").then_some(path)
        })
        .unwrap_or_else(|| "-".to_owned())
}

fn run_details(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let (finished, total) = app.run_progress();
    vec![
        Line::styled("Run", theme.title(false)),
        detail_line(
            "run id",
            app.run.run_id.clone().unwrap_or_else(|| "-".to_owned()),
            theme.text(),
            theme,
        ),
        detail_line("status", app.run_status_label(), theme.text(), theme),
        detail_line(
            "result",
            app.run_result_label(),
            run_result_style(app, theme),
            theme,
        ),
        detail_line("profile", app.run.profile.clone(), theme.accent(), theme),
        detail_line("scope", app.run.scope.label(), theme.text(), theme),
        detail_line("duration", run_duration_label(app), theme.text(), theme),
        detail_line("build", build_duration_label(app), theme.text(), theme),
        detail_line("tests", test_duration_label(app), theme.text(), theme),
        detail_line(
            "progress",
            format!("{finished}/{total}"),
            theme.text(),
            theme,
        ),
    ]
}

fn storage_details(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled("Storage", theme.title(false)),
        detail_line(
            "status",
            storage_status(app),
            storage_status_style(app, theme),
            theme,
        ),
    ];

    if let Some(snapshot) = &app.disk_usage.snapshot {
        lines.extend([
            detail_line(
                "available",
                snapshot
                    .available_bytes
                    .map(format_bytes)
                    .unwrap_or_else(|| "-".to_owned()),
                theme.text(),
                theme,
            ),
            detail_line(
                "updated",
                format_timestamp_utc(snapshot.updated_at),
                theme.text(),
                theme,
            ),
            detail_line(
                "total",
                format_bytes(snapshot.total_bytes()),
                theme.text(),
                theme,
            ),
        ]);
        for entry in &snapshot.entries {
            lines.push(detail_line(
                entry.label,
                format_bytes(entry.bytes),
                theme.text(),
                theme,
            ));
        }
    } else if app.disk_usage.loading {
        lines.push(Line::styled("Scanning disk usage...", theme.muted()));
    } else if let Some(error) = &app.disk_usage.error {
        lines.push(Line::styled(error.clone(), theme.danger()));
    } else {
        lines.push(Line::styled("No disk usage snapshot.", theme.muted()));
    }

    if let Some(result) = &app.disk_cleanup.last_result {
        lines.push(Line::from(""));
        match result {
            Ok(()) => lines.push(detail_line("cleanup", "completed", theme.success(), theme)),
            Err(_) => lines.push(detail_line("cleanup", "failed", theme.danger(), theme)),
        }
    }

    lines
}

fn storage_status(app: &App) -> &'static str {
    storage_health(app).label()
}

fn storage_health(app: &App) -> StorageHealth {
    app.disk_usage
        .health(app.settings.storage_low_space_threshold_bytes())
}

fn storage_status_style(app: &App, theme: &Theme) -> Style {
    match storage_health(app) {
        StorageHealth::Healthy => theme.success(),
        StorageHealth::Low | StorageHealth::Failed => theme.danger(),
        StorageHealth::Scanning => theme.accent(),
        StorageHealth::Unknown | StorageHealth::NotScanned => theme.muted(),
    }
}

fn run_result_style(app: &App, theme: &Theme) -> ratatui::style::Style {
    match app.run.outcome {
        crate::app::RunOutcome::Passed => theme.success(),
        crate::app::RunOutcome::Failed | crate::app::RunOutcome::CommandFailed => theme.danger(),
        crate::app::RunOutcome::Stopped => theme.warning(),
        crate::app::RunOutcome::Running => theme.accent(),
        crate::app::RunOutcome::NotStarted => theme.muted(),
    }
}

fn detail_line(
    label: &'static str,
    value: impl Into<String>,
    value_style: ratatui::style::Style,
    theme: &Theme,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<9}"), theme.muted()),
        Span::styled(value.into(), value_style),
    ])
}

fn detail_status_line(status: TestStatus, theme: &Theme) -> Line<'static> {
    detail_line(
        "status",
        status_label(status),
        theme.status(status, false),
        theme,
    )
}

fn duration_label(node: &TestNode, duration_mode: config::TreeDurationMode) -> String {
    node.display_duration(duration_mode)
        .map(format_duration)
        .unwrap_or_else(|| "-".to_owned())
}

fn run_duration_label(app: &App) -> String {
    app.run_duration()
        .map(format_duration)
        .unwrap_or_else(|| "-".to_owned())
}

fn build_duration_label(app: &App) -> String {
    app.build_duration()
        .map(format_duration)
        .unwrap_or_else(|| "-".to_owned())
}

fn test_duration_label(app: &App) -> String {
    app.test_duration()
        .map(format_duration)
        .unwrap_or_else(|| "-".to_owned())
}

fn duration_field(duration: Option<Duration>) -> String {
    duration
        .map(|duration| format!("[{:>8.3}s]", duration.as_secs_f64()))
        .unwrap_or_else(|| "[        ]".to_owned())
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}

fn status_label(status: TestStatus) -> &'static str {
    match status {
        TestStatus::Pending => "pending",
        TestStatus::Running => "running",
        TestStatus::Passed => "passed",
        TestStatus::Failed => "failed",
        TestStatus::Ignored => "ignored",
        TestStatus::Skipped => "skipped",
    }
}

fn draw_output(frame: &mut Frame<'_>, app: &App, theme: &Theme, area: Rect) {
    draw_output_pane(
        frame,
        theme,
        area,
        OutputPaneRender {
            state: &app.main_output,
            source_text: app.output_source_text(),
            label: "Output".to_owned(),
            focused: pane_focused(app, FocusPane::Output),
        },
    );
}

fn draw_output_pane(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    output: OutputPaneRender<'_>,
) {
    let focused = output.focused;
    let output = output_pane_content(theme, output);
    draw_output_panel(
        frame,
        theme,
        area,
        PanelChrome {
            status: &output.status,
            actions: output_actions(),
        },
        output.lines,
        focused,
        output.scroll,
    );
}

fn output_pane_content(theme: &Theme, output: OutputPaneRender<'_>) -> OutputPaneContent {
    let output_view = output.state.output_view(&output.source_text);
    OutputPaneContent {
        status: output
            .state
            .status(&output.label, &output_view.text, &output.source_text),
        lines: output_lines(&output.state.search, theme, &output_view),
        scroll: output.state.scroll,
    }
}

fn draw_output_panel(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: PanelChrome<'_>,
    lines: Vec<Line<'static>>,
    focused: bool,
    scroll: u16,
) {
    let page_size = area.height.saturating_sub(2).max(1);
    let text_line_count = lines.len().max(1);
    let scroll = output_render_scroll_for_count(text_line_count, page_size, scroll);
    let output = Paragraph::new(lines)
        .style(theme.text())
        .block(theme.panel_block(chrome.status, Some(chrome.actions), focused))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
}

fn tests_actions() -> &'static str {
    "[enter]details [r]un [R]failed [o]pen-editor [u]update"
}

fn info_actions() -> &'static str {
    "[d]disk-refresh [D]cleanup [x]tasks"
}

fn disk_cleanup_actions() -> &'static str {
    "[c]cargo-clean [r]refresh [esc]close"
}

fn output_actions() -> &'static str {
    "[/]search [n]ext [N]prev [o]pen-editor"
}

fn discovery_error_actions() -> &'static str {
    "[u]retry [/]search [n]ext [N]prev [o]pen-editor [q]quit"
}

fn info_status(_app: &App) -> String {
    "Info".to_owned()
}

fn output_lines(
    search: &OutputSearchState,
    theme: &Theme,
    output_view: &OutputView,
) -> Vec<Line<'static>> {
    let lines = output_view
        .text
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let source_line = output_view
                .source_lines
                .get(index)
                .copied()
                .unwrap_or(index);
            highlighted_output_line(search, theme, source_line, line)
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec![Line::from("")]
    } else {
        lines
    }
}

fn highlighted_output_line(
    search: &OutputSearchState,
    theme: &Theme,
    source_line: usize,
    line: &str,
) -> Line<'static> {
    let ranges = match search.match_ranges(line) {
        Ok(ranges) if !ranges.is_empty() => ranges,
        _ => return Line::styled(line.to_owned(), theme.text()),
    };
    let match_style = if search.current_line == Some(source_line) {
        theme.active_search_match()
    } else {
        theme.search_match()
    };
    let mut spans = Vec::new();
    let mut cursor = 0;
    for (start, end) in ranges {
        if start > cursor {
            spans.push(Span::styled(line[cursor..start].to_owned(), theme.text()));
        }
        spans.push(Span::styled(line[start..end].to_owned(), match_style));
        cursor = end;
    }
    if cursor < line.len() {
        spans.push(Span::styled(line[cursor..].to_owned(), theme.text()));
    }
    Line::from(spans)
}

fn draw_status(frame: &mut Frame<'_>, app: &App, theme: &Theme, area: ratatui::layout::Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(Line::from(status_spans(app, theme))).style(theme.footer()),
        area,
    );
}

fn status_spans<'a>(app: &'a App, theme: &'a Theme) -> Vec<Span<'a>> {
    let key = app
        .key_echo
        .as_ref()
        .map(|echo| echo.text.as_str())
        .unwrap_or("-");
    let storage = storage_status(app);
    vec![
        Span::styled(" branch ", theme.footer_label()),
        Span::styled(app.git_status.branch.as_str(), theme.footer_value()),
        Span::styled(" | unstaged ", theme.footer_label()),
        Span::styled(
            app.git_status.unstaged.added.to_string(),
            theme.footer_dirty(true),
        ),
        Span::styled(":", theme.footer_label()),
        Span::styled(
            app.git_status.unstaged.deleted.to_string(),
            theme.footer_dirty(false),
        ),
        Span::styled(" | staged ", theme.footer_label()),
        Span::styled(
            app.git_status.staged.added.to_string(),
            theme.footer_dirty(true),
        ),
        Span::styled(":", theme.footer_label()),
        Span::styled(
            app.git_status.staged.deleted.to_string(),
            theme.footer_dirty(false),
        ),
        Span::styled(" | run ", theme.footer_label()),
        Span::styled(footer_run_status(app), footer_run_status_style(app, theme)),
        Span::styled(" | storage ", theme.footer_label()),
        Span::styled(storage, footer_storage_status_style(app, theme)),
        Span::styled(" | key ", theme.footer_label()),
        Span::styled(key, theme.footer_value()),
        Span::styled(" | ", theme.footer_label()),
        Span::styled(app.status.as_str(), theme.footer()),
    ]
}

fn footer_run_status_style(app: &App, theme: &Theme) -> Style {
    match app.run.phase {
        crate::app::RunPhase::Building | crate::app::RunPhase::RunningTests => theme.accent(),
        crate::app::RunPhase::NotRunning => theme.muted(),
    }
    .bg(theme.footer_bg)
}

fn footer_run_status(app: &App) -> &'static str {
    app.run_status_label()
}

fn footer_storage_status_style(app: &App, theme: &Theme) -> Style {
    storage_status_style(app, theme).bg(theme.footer_bg)
}

fn draw_help(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(72, 96, frame.area());
    let text = help_text(theme, app.focus);
    let inner = draw_modal_shell(
        frame,
        theme,
        area,
        ModalChrome {
            title: "Help",
            actions: Some("[h/?/F1]close [esc]close [q]close"),
        },
    );
    let help = Paragraph::new(text)
        .alignment(Alignment::Left)
        .style(theme.text());
    frame.render_widget(help, inner);
}

fn help_text(theme: &Theme, focus: FocusPane) -> Vec<Line<'static>> {
    let mut text = Vec::new();
    text.push(Line::from(vec![
        Span::styled("NextDeck", theme.title(true)),
        Span::raw(" "),
        Span::styled(env!("CARGO_PKG_VERSION"), theme.muted()),
    ]));
    append_help_section(&mut text, "Global", true, theme);
    append_help_group(&mut text, CommandGroup::Navigation, true, theme);
    append_help_commands(&mut text, CommandGroup::Global, true, theme);

    let tests_active = focus == FocusPane::Tree;
    append_help_section(&mut text, "Tests", tests_active, theme);
    append_help_group(&mut text, CommandGroup::Runs, tests_active, theme);
    append_help_group(&mut text, CommandGroup::View, tests_active, theme);

    let output_active = focus == FocusPane::Output;
    append_help_section(&mut text, "Output", output_active, theme);
    append_help_commands(&mut text, CommandGroup::Output, output_active, theme);

    text
}

fn append_help_section(
    text: &mut Vec<Line<'static>>,
    title: &'static str,
    active: bool,
    theme: &Theme,
) {
    if !text.is_empty() {
        text.push(Line::from(""));
    }
    text.push(Line::styled(
        title,
        if active {
            theme.title(true)
        } else {
            theme.muted()
        },
    ));
}

fn append_help_group(
    text: &mut Vec<Line<'static>>,
    group: CommandGroup,
    active: bool,
    theme: &Theme,
) {
    text.push(Line::styled(
        format!("  {}", group.title()),
        if active {
            theme.accent()
        } else {
            theme.muted()
        },
    ));
    append_help_commands(text, group, active, theme);
}

fn append_help_commands(
    text: &mut Vec<Line<'static>>,
    group: CommandGroup,
    active: bool,
    theme: &Theme,
) {
    let mut infos = command_infos()
        .iter()
        .filter(|info| info.group == group)
        .collect::<Vec<_>>();
    infos.sort_by_key(|info| help_sort_text(info));
    for info in infos {
        text.push(help_line(info, active, theme));
    }
}

fn help_sort_text(info: &CommandInfo) -> String {
    format!("{} {}", info.keys.to_ascii_lowercase(), info.label)
}

fn help_line(info: &CommandInfo, active: bool, theme: &Theme) -> Line<'static> {
    let key_style = if active {
        theme.accent()
    } else {
        theme.muted()
    };
    let label_style = if active { theme.text() } else { theme.muted() };
    Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<15}", info.keys), key_style),
        Span::raw(" "),
        Span::styled(info.label, label_style),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests;
