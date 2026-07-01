use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane},
    command::{CommandGroup, command_infos, help_groups},
    config,
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

    if app.show_help {
        draw_help(frame, theme);
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
        .map(|(index, (depth, node))| tree_item(*depth, node, index == selected, theme))
        .collect::<Vec<_>>();

    let focused = app.focus == FocusPane::Tree;
    let title = format!(
        "Tests {} {} {} {}",
        filter_hint("pass", "s", app.tree.view_filter.show_success),
        filter_hint("fail", "x", app.tree.view_filter.show_failed),
        filter_hint("ign", "i", app.tree.view_filter.show_ignored),
        filter_hint("skip", "k", app.tree.view_filter.show_skipped)
    );
    let list = List::new(items)
        .block(theme.panel_block(&title, focused))
        .highlight_style(theme.selected());
    frame.render_widget(Clear, area);
    frame.render_widget(list, area);
}

fn filter_hint(label: &str, key: &str, enabled: bool) -> String {
    format!("{label}[{key}]:{}", on_off(enabled))
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn draw_discovery_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(62, 58, frame.area());
    let lines = if let Some(error) = &app.discovery.error {
        vec![
            Line::styled("Discovery failed", theme.danger()),
            Line::from(""),
            Line::styled(error.clone(), theme.text()),
            Line::from(""),
            Line::styled("Press q to quit.", theme.muted()),
        ]
    } else {
        vec![
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
        ]
    };
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(theme.modal_block("Discovery"))
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
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
        duration_field(node.duration())
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
    let lines = selected_details(app, theme);
    let details = Paragraph::new(lines)
        .style(theme.text())
        .block(theme.panel_block("Info", false))
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(details, area);
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

fn run_result_style(app: &App, theme: &Theme) -> ratatui::style::Style {
    match app.run.outcome {
        crate::app::RunOutcome::Passed => theme.success(),
        crate::app::RunOutcome::Failed | crate::app::RunOutcome::CommandFailed => theme.danger(),
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
    node.duration()
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
    let focused = app.focus == FocusPane::Output;
    let title = output_title(app, &text);
    let output = Paragraph::new(text)
        .style(theme.text())
        .block(theme.panel_block(&title, focused))
        .wrap(Wrap { trim: false })
        .scroll((app.output_scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
}

fn output_title(app: &App, text: &str) -> String {
    let total = text.lines().count().max(1);
    let visible = app.output_page_size.max(1) as usize;
    let search = output_search_title(app);
    if total <= visible {
        return format!("Output All {total}/{total}{search}");
    }

    let top = (app.output_scroll as usize).min(total.saturating_sub(1));
    let bottom = top.saturating_add(visible).min(total);
    let position = if top == 0 {
        "Top"
    } else if bottom == total {
        "Bot"
    } else {
        ""
    };

    if position.is_empty() {
        format!("Output {}-{bottom}/{total}{search}", top + 1)
    } else {
        format!("Output {position} {}-{bottom}/{total}{search}", top + 1)
    }
}

fn output_search_title(app: &App) -> String {
    if app.output_search.query.is_empty()
        && !app.output_search.filter
        && !app.output_search.regex
        && !app.output_search.case_sensitive
    {
        return String::new();
    }

    let query = if app.output_search.input_active {
        format!("/{query}_", query = app.output_search.query)
    } else if app.output_search.query.is_empty() {
        "/-".to_owned()
    } else {
        format!("/{query}", query = app.output_search.query)
    };
    let summary = app
        .output_search_match_summary()
        .map(|(current, total)| format!(" {current}/{total}"))
        .unwrap_or_default();
    format!(
        " {query}{summary} f:{} r:{} c:{}",
        on_off(app.output_search.filter),
        on_off(app.output_search.regex),
        on_off(app.output_search.case_sensitive)
    )
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

fn draw_help(frame: &mut Frame<'_>, theme: &Theme) {
    let area = centered_rect(72, 96, frame.area());
    let text = help_text(theme);
    let help = Paragraph::new(text)
        .alignment(Alignment::Left)
        .style(theme.text())
        .block(theme.modal_block("Help"));
    frame.render_widget(Clear, area);
    frame.render_widget(help, area);
}

fn help_text(theme: &Theme) -> Vec<Line<'static>> {
    let mut text = Vec::new();
    for group in help_groups() {
        append_help_group(&mut text, *group, theme);
    }
    text
}

fn append_help_group(text: &mut Vec<Line<'static>>, group: CommandGroup, theme: &Theme) {
    if !text.is_empty() {
        text.push(Line::from(""));
    }
    text.push(Line::styled(group.title(), theme.title(true)));
    for info in command_infos().iter().filter(|info| info.group == group) {
        text.push(help_line(info.keys, info.label, theme));
    }
}

fn help_line(key: &'static str, label: &'static str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{key:<15}"), theme.accent()),
        Span::raw(" "),
        Span::styled(label, theme.text()),
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
    use crate::tree::{DiscoveredTest, TestKey, Tree};
    use std::path::PathBuf;

    #[test]
    fn output_title_shows_all_when_text_fits() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;

        assert_eq!(output_title(&app, "one\ntwo"), "Output All 2/2");
    }

    #[test]
    fn output_title_shows_top_middle_and_bottom_ranges() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 3;
        let text = "1\n2\n3\n4\n5\n6";

        app.output_scroll = 0;
        assert_eq!(output_title(&app, text), "Output Top 1-3/6");

        app.output_scroll = 2;
        assert_eq!(output_title(&app, text), "Output 3-5/6");

        app.output_scroll = 3;
        assert_eq!(output_title(&app, text), "Output Bot 4-6/6");
    }

    #[test]
    fn filter_hint_includes_toggle_key() {
        assert_eq!(filter_hint("pass", "s", true), "pass[s]:on");
        assert_eq!(filter_hint("fail", "x", false), "fail[x]:off");
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
    fn output_title_includes_search_flags() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;
        app.output_search.query = "panic".to_owned();
        app.output_search.filter = true;

        assert_eq!(
            output_title(&app, "panic line"),
            "Output All 1/1 /panic 0/1 f:on r:off c:off"
        );
    }
}
