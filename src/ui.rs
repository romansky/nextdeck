use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane},
    command::{CommandGroup, CommandInfo, command_infos},
    config,
    disk_usage::format_bytes,
    output_pane::SearchModalFocus,
    settings::SettingsField,
    theme::Theme,
    tree::{NodeKind, TestNode, TestStatus},
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

pub fn draw(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let app_layout = layout(frame.area(), app.settings.tree_width_percent);
    draw_tree(frame, app, theme, app_layout.tree);
    draw_details(frame, app, theme, app_layout.details);
    draw_output(frame, app, theme, app_layout.output);
    draw_status(frame, app, theme, app_layout.status);

    if app.is_discovering() || app.discovery.error.is_some() {
        draw_discovery_modal(frame, app, theme);
    }

    if app.output_search.modal_open {
        draw_output_search_modal(frame, app, theme);
    }

    if app.disk_cleanup.modal_open {
        draw_disk_cleanup_modal(frame, app, theme);
    }

    if app.global_settings.modal_open {
        draw_global_settings_modal(frame, app, theme);
    }

    if app.show_help {
        draw_help(frame, app, theme);
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
        .map(|(index, row)| tree_item(row.depth, row.node, index == selected, theme))
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
    app.focus == pane && !modal_visible(app)
}

fn modal_visible(app: &App) -> bool {
    app.show_help
        || app.output_search.modal_open
        || app.disk_cleanup.modal_open
        || app.global_settings.modal_open
        || app.is_discovering()
        || app.discovery.error.is_some()
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
        return format!("[{key}]{label}:{}", on_off(enabled));
    };
    format!("{head}[{key}]{tail}:{}", on_off(enabled))
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

fn draw_discovery_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(62, 58, frame.area());
    if app.discovery.error.is_some() {
        let text = app.output_text();
        let page_size = area.height.saturating_sub(2).max(1);
        let status = output_status_for("Discovery", app, &text, page_size, app.output_scroll);
        draw_output_panel(
            frame,
            theme,
            area,
            PanelChrome {
                status: &status,
                actions: discovery_error_actions(),
            },
            output_lines(app, theme, &text),
            true,
            app.output_scroll,
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
            Line::styled("Press q to quit.", theme.muted()),
        ];
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .block(theme.modal_block("Discovery"))
            .wrap(Wrap { trim: false });
        frame.render_widget(Clear, area);
        frame.render_widget(paragraph, area);
    }
}

fn draw_output_search_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(70, 70, frame.area());
    let search = &app.output_search;
    frame.render_widget(Clear, area);
    frame.render_widget(theme.modal_block("Output Search"), area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
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
            modal_button("Clear", search.modal_focus == SearchModalFocus::Clear, theme),
            Span::raw("  "),
            modal_button("Apply", search.modal_focus == SearchModalFocus::Apply, theme),
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
    let mut lines = vec![
        Line::styled("Disk Usage", theme.title(false)),
        Line::styled(app.disk_usage.summary_label(), theme.text()),
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
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if app.disk_cleanup.running {
                    "[c] cargo clean..."
                } else {
                    "[c] cargo clean"
                },
                theme.text(),
            ),
            Span::raw("  "),
            Span::styled("[r] refresh", theme.text()),
            Span::raw("  "),
            Span::styled("[q] close", theme.text()),
        ]),
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

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(theme.modal_block("Disk Cleanup"))
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn draw_global_settings_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(72, 58, frame.area());
    let settings = &app.global_settings;
    let lines = vec![
        settings_line(app, SettingsField::Editor, theme),
        settings_line(app, SettingsField::TreeWidth, theme),
        settings_line(app, SettingsField::Theme, theme),
        settings_line(app, SettingsField::ColorBlindMode, theme),
        Line::from(""),
        Line::styled(
            "Editor examples: idea, code, cursor, zed, open -a \"IntelliJ IDEA\"",
            theme.muted(),
        ),
    ];
    let actions = if settings.editor_editing {
        "actions: [enter]save [esc]cancel [C-u]clear"
    } else {
        "actions: [up/down]select [left/right]change [enter]edit/apply [x]clear-editor [q]close"
    };
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text())
        .block(theme.panel_block("Settings", Some(actions), true))
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
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
        SettingsField::Editor if app.global_settings.editor_editing => {
            let text = format!("{}_", app.global_settings.editor_draft);
            format!("[{}]", fit_line_content(&text, 42))
        }
        SettingsField::Editor => format!("[{}]", fit_line_content(app.settings.editor_label(), 42)),
        SettingsField::TreeWidth => format!("{}%", app.settings.tree_width_percent),
        SettingsField::Theme => app.settings.theme_mode.label().to_owned(),
        SettingsField::ColorBlindMode => on_off(app.settings.color_blind_mode).to_owned(),
    }
}

fn modal_label_style(active: bool, theme: &Theme) -> ratatui::style::Style {
    if active { theme.title(true) } else { theme.muted() }
}

fn modal_button(label: &'static str, active: bool, theme: &Theme) -> Span<'static> {
    Span::styled(
        format!("[ {label} ]"),
        if active { theme.selected() } else { theme.text() },
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
        if active { theme.selected() } else { theme.text() },
    )
}

fn tree_item<'a>(depth: usize, node: &TestNode, selected: bool, theme: &Theme) -> ListItem<'a> {
    let row_style = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    let status_style = theme.status(node.status, selected);
    ListItem::new(Line::from(vec![
        Span::styled(tree_leading_fields(depth, node), row_style),
        Span::styled(
            node_label(node),
            if selected { row_style } else { status_style },
        ),
    ]))
}

fn tree_leading_fields(depth: usize, node: &TestNode) -> String {
    format!(
        "{}{} {} ",
        "  ".repeat(depth),
        fold_marker(node),
        duration_field(node.display_duration())
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

fn node_label(node: &TestNode) -> String {
    match &node.kind {
        NodeKind::Workspace => node.label.clone(),
        NodeKind::Package { name } => name.clone(),
        NodeKind::Binary { .. } | NodeKind::Module { .. } | NodeKind::Test(_) => {
            node.label.clone()
        }
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
                detail_line("duration", duration_label(node), theme.text(), theme),
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
                detail_line("duration", duration_label(node), theme.text(), theme),
            ]);
        }
        NodeKind::Module { path } => {
            lines.extend([
                detail_line("kind", "module", theme.text(), theme),
                detail_line("module", path.clone(), theme.accent(), theme),
                detail_status_line(node.status, theme),
                detail_line("duration", duration_label(node), theme.text(), theme),
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
                detail_line("duration", duration_label(node), theme.text(), theme),
            ]);
        }
    }

    lines
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
        detail_line("status", app.disk_usage.summary_label(), theme.text(), theme),
    ];

    if let Some(snapshot) = &app.disk_usage.snapshot {
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

fn duration_label(node: &TestNode) -> String {
    node.display_duration()
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
    let text = app.output_text();
    let focused = pane_focused(app, FocusPane::Output);
    let status = output_status(app, &text);
    draw_output_panel(
        frame,
        theme,
        area,
        PanelChrome {
            status: &status,
            actions: output_actions(),
        },
        output_lines(app, theme, &text),
        focused,
        app.output_scroll,
    );
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
        .block(theme.panel_block(
            chrome.status,
            Some(chrome.actions),
            focused,
        ))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
}

fn tests_actions() -> &'static str {
    "actions: [r]un [R]failed [o]pen-editor [u]update"
}

fn info_actions() -> &'static str {
    "actions: [d]disk-refresh [D]cleanup"
}

fn output_actions() -> &'static str {
    "actions: [/]search [n]ext [N]prev [o]pen-editor"
}

fn discovery_error_actions() -> &'static str {
    "actions: [u]retry [/]search [n]ext [N]prev [o]pen-editor [q]quit"
}

fn info_status(app: &App) -> String {
    format!("Info <disk: {}>", app.disk_usage.summary_label())
}

fn output_status(app: &App, text: &str) -> String {
    output_status_for("Output", app, text, app.output_page_size, app.output_scroll)
}

fn output_status_for(label: &str, app: &App, text: &str, page_size: u16, scroll: u16) -> String {
    let total = output_line_count(text);
    let search = output_search_title(app, text);
    let top = output_render_scroll(text, page_size, scroll) as usize;
    let visible = page_size.max(1) as usize;
    let bottom = top.saturating_add(visible).min(total);
    format!("{label} <lines: {}-{bottom}/{total}> {search}", top + 1)
}

fn output_search_title(app: &App, text: &str) -> String {
    app.output_search.view(text).title_fragment()
}

fn output_render_scroll(text: &str, page_size: u16, scroll: u16) -> u16 {
    output_render_scroll_for_count(output_line_count(text), page_size, scroll)
}

fn output_render_scroll_for_count(total: usize, page_size: u16, scroll: u16) -> u16 {
    let visible = page_size.max(1) as usize;
    let max_scroll = total.saturating_sub(visible).min(u16::MAX as usize) as u16;
    scroll.min(max_scroll)
}

fn output_line_count(text: &str) -> usize {
    text.lines().count().max(1)
}

fn output_lines(app: &App, theme: &Theme, text: &str) -> Vec<Line<'static>> {
    let lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| highlighted_output_line(app, theme, index, line))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec![Line::from("")]
    } else {
        lines
    }
}

fn highlighted_output_line(
    app: &App,
    theme: &Theme,
    line_index: usize,
    line: &str,
) -> Line<'static> {
    let ranges = match app.output_search.match_ranges(line) {
        Ok(ranges) if !ranges.is_empty() => ranges,
        _ => return Line::styled(line.to_owned(), theme.text()),
    };
    let match_style = if app.output_search.current_line == Some(line_index) {
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
        Span::styled(" | key ", theme.footer_label()),
        Span::styled(key, theme.footer_value()),
        Span::styled(" | ", theme.footer_label()),
        Span::styled(app.status.as_str(), theme.footer()),
    ]
}

fn draw_help(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(72, 96, frame.area());
    let text = help_text(theme, app.focus);
    let help = Paragraph::new(text)
        .alignment(Alignment::Left)
        .style(theme.text())
        .block(theme.modal_block("Help"));
    frame.render_widget(Clear, area);
    frame.render_widget(help, area);
}

fn help_text(theme: &Theme, focus: FocusPane) -> Vec<Line<'static>> {
    let mut text = Vec::new();
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

fn append_help_section(text: &mut Vec<Line<'static>>, title: &'static str, active: bool, theme: &Theme) {
    if !text.is_empty() {
        text.push(Line::from(""));
    }
    text.push(Line::styled(
        title,
        if active { theme.title(true) } else { theme.muted() },
    ));
}

fn append_help_group(text: &mut Vec<Line<'static>>, group: CommandGroup, active: bool, theme: &Theme) {
    text.push(Line::styled(
        format!("  {}", group.title()),
        if active { theme.accent() } else { theme.muted() },
    ));
    append_help_commands(text, group, active, theme);
}

fn append_help_commands(
    text: &mut Vec<Line<'static>>,
    group: CommandGroup,
    active: bool,
    theme: &Theme,
) {
    for info in command_infos().iter().filter(|info| info.group == group) {
        text.push(help_line(info, active, theme));
    }
}

fn help_line(info: &CommandInfo, active: bool, theme: &Theme) -> Line<'static> {
    let key_style = if active { theme.accent() } else { theme.muted() };
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
mod tests {
    use super::*;
    use crate::disk_usage::{DiskUsageEntry, DiskUsageSnapshot};
    use crate::tree::{DiscoveredTest, TestKey, Tree};
    use std::path::PathBuf;

    #[test]
    fn output_status_shows_all_when_text_fits() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;

        assert_eq!(
            output_status(&app, "one\ntwo"),
            "Output <lines: 1-2/2> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );
    }

    #[test]
    fn output_status_shows_clamped_line_ranges() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 3;
        let text = "1\n2\n3\n4\n5\n6";

        app.output_scroll = 0;
        assert_eq!(
            output_status(&app, text),
            "Output <lines: 1-3/6> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );

        app.output_scroll = 2;
        assert_eq!(
            output_status(&app, text),
            "Output <lines: 3-5/6> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );

        app.output_scroll = 3;
        assert_eq!(
            output_status(&app, text),
            "Output <lines: 4-6/6> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );
    }

    #[test]
    fn filter_hint_includes_toggle_key() {
        assert_eq!(filter_hint("pass", "p", true), "[p]ass:on");
        assert_eq!(filter_hint("fail", "f", false), "[f]ail:off");
        assert_eq!(filter_hint("ignore", "i", false), "[i]gnore:off");
    }

    #[test]
    fn tests_status_includes_filter_hints() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.tree.view_filter.show_ignored = false;

        assert_eq!(
            tests_status(&app),
            "Tests <filters: [p]ass:on [f]ail:on [i]gnore:off [s]kip:on>"
        );
    }

    #[test]
    fn info_status_includes_disk_state() {
        let app = App::new(Tree::from_tests(Vec::new()));

        assert_eq!(info_status(&app), "Info <disk: not scanned>");
    }

    #[test]
    fn info_columns_keep_run_and_storage_details_separate() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.disk_usage.snapshot = Some(DiskUsageSnapshot {
            entries: vec![DiskUsageEntry {
                label: "target",
                path: PathBuf::from("target"),
                bytes: 1024,
            }],
        });

        let run_text = run_details(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");
        let storage_text = storage_details(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(run_text.contains("run id"));
        assert!(!run_text.contains("target"));
        assert!(storage_text.contains("Storage"));
        assert!(storage_text.contains("target"));
        assert!(storage_text.contains("1.0 KiB"));
    }

    #[test]
    fn panel_actions_describe_local_commands() {
        assert_eq!(
            tests_actions(),
            "actions: [r]un [R]failed [o]pen-editor [u]update"
        );
        assert_eq!(info_actions(), "actions: [d]disk-refresh [D]cleanup");
        assert_eq!(
            output_actions(),
            "actions: [/]search [n]ext [N]prev [o]pen-editor"
        );
        assert_eq!(
            discovery_error_actions(),
            "actions: [u]retry [/]search [n]ext [N]prev [o]pen-editor [q]quit"
        );
    }

    #[test]
    fn pane_focus_is_suppressed_while_modal_is_visible() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.focus = FocusPane::Tree;
        assert!(pane_focused(&app, FocusPane::Tree));

        app.discovery.running = true;
        assert!(!pane_focused(&app, FocusPane::Tree));

        app.discovery.running = false;
        app.discovery.error = Some("boom".to_owned());
        assert!(!pane_focused(&app, FocusPane::Tree));

        app.discovery.error = None;
        app.show_help = true;
        app.focus = FocusPane::Output;
        assert!(!pane_focused(&app, FocusPane::Output));
    }

    #[test]
    fn help_text_uses_contextual_sections() {
        let theme = Theme::dark();
        let text = help_text(&theme, FocusPane::Tree);
        let lines = text.iter().map(line_text).collect::<Vec<_>>();

        assert!(lines.contains(&"Global".to_owned()));
        assert!(lines.contains(&"  Navigation".to_owned()));
        assert!(lines.contains(&"Tests".to_owned()));
        assert!(lines.contains(&"  Runs".to_owned()));
        assert!(lines.contains(&"  View".to_owned()));
        assert!(lines.contains(&"Output".to_owned()));
        assert!(lines.iter().any(|line| line.contains("h/?/F1")));
        assert!(lines.iter().any(|line| line.contains("q")));
    }

    #[test]
    fn help_text_dims_inactive_pane_commands() {
        let theme = Theme::dark();
        let tests_help = help_text(&theme, FocusPane::Tree);
        let output_help = help_text(&theme, FocusPane::Output);

        assert_eq!(
            help_line_with_label(&tests_help, "search output").spans[1].style,
            theme.muted()
        );
        assert_eq!(
            help_line_with_label(&tests_help, "run selected scope").spans[1].style,
            theme.accent()
        );
        assert_eq!(
            help_line_with_label(&output_help, "run selected scope").spans[1].style,
            theme.muted()
        );
        assert_eq!(
            help_line_with_label(&output_help, "search output").spans[1].style,
            theme.accent()
        );
    }

    #[test]
    fn tree_leading_fields_have_no_status_gap() {
        let tree = Tree::from_tests(vec![DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: "case".to_owned(),
            full_name: "tests::case".to_owned(),
            status: TestStatus::Pending,
            ignored: false,
        }]);

        assert_eq!(tree_leading_fields(0, &tree.root), "v [        ] ");
    }

    #[test]
    fn running_duration_field_is_only_populated_for_test_leaf() {
        let mut tree = Tree::from_tests(vec![DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: "case".to_owned(),
            full_name: "tests::case".to_owned(),
            status: TestStatus::Pending,
            ignored: false,
        }]);
        tree.update_status(
            &TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            TestStatus::Running,
        );

        let package = &tree.root.children[0];
        let module = &package.children[0];
        let test = &module.children[0];
        assert_eq!(tree_leading_fields(1, package), "  > [        ] ");
        assert_eq!(tree_leading_fields(2, module), "    > [        ] ");
        assert_ne!(tree_leading_fields(3, test), "        [        ] ");
    }

    #[test]
    fn output_status_includes_search_flags() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;
        app.output_search.query = "panic".to_owned();
        app.output_search.filter = true;

        assert_eq!(
            output_status(&app, "panic line"),
            "Output <lines: 1-1/1> <search: [panic       ] 0/1 [n]ext [f]ilter:on [r]egex:off [c]ase-sensitive:off>"
        );
    }

    #[test]
    fn output_lines_marks_current_search_result_differently() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        let theme = Theme::dark();
        app.output_search.query = "panic".to_owned();
        app.output_search.current_line = Some(1);

        let lines = output_lines(&app, &theme, "panic one\npanic two");

        assert_eq!(lines[0].spans[0].style, theme.search_match());
        assert_eq!(lines[1].spans[0].style, theme.active_search_match());
    }

    #[test]
    fn output_search_box_marks_active_input() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_search.draft_query = "panic".to_owned();
        app.output_search.input_active = true;

        assert_eq!(app.output_search.box_text(18), "[panic_            ]");
    }

    #[test]
    fn output_status_shows_submit_and_advanced_hints_while_searching() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_search.draft_query = "panic".to_owned();
        app.output_search.input_active = true;

        assert_eq!(
            output_status(&app, "panic line"),
            "Output <lines: 1-1/1> <search: [panic_      ] 0/0 [enter]submit [C+enter]advanced [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );
    }

    #[test]
    fn output_search_box_keeps_fixed_width_for_long_query() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_search.query = "abcdefghijklmnopqrstuvwxyz".to_owned();

        assert_eq!(app.output_search.box_text(18).len(), 20);
        assert_eq!(app.output_search.box_text(18), "[ijklmnopqrstuvwxyz]");
    }

    fn help_line_with_label<'a>(lines: &'a [Line<'a>], label: &str) -> &'a Line<'a> {
        lines
            .iter()
            .find(|line| line_text(line).contains(label))
            .expect("help line")
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }
}
