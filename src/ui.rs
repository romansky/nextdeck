use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane},
    command::{CommandGroup, CommandInfo, command_infos},
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

    let focused = app.focus == FocusPane::Tree;
    let title = tests_title(app);
    let list = List::new(items)
        .block(theme.panel_block(&title, focused))
        .highlight_style(theme.selected());
    frame.render_widget(Clear, area);
    frame.render_widget(list, area);
}

fn tests_title(app: &App) -> String {
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

fn draw_discovery_modal(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let area = centered_rect(62, 58, frame.area());
    if app.discovery.error.is_some() {
        let text = app.output_text();
        let page_size = area.height.saturating_sub(2).max(1);
        let title = output_title_for("Discovery", app, &text, page_size, app.output_scroll);
        draw_output_panel(
            frame,
            theme,
            area,
            &title,
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
    let focused = app.focus == FocusPane::Output;
    let title = output_title(app, &text);
    draw_output_panel(
        frame,
        theme,
        area,
        &title,
        output_lines(app, theme, &text),
        focused,
        app.output_scroll,
    );
}

fn draw_output_panel(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    focused: bool,
    scroll: u16,
) {
    let page_size = area.height.saturating_sub(2).max(1);
    let text_line_count = lines.len().max(1);
    let scroll = output_render_scroll_for_count(text_line_count, page_size, scroll);
    let output = Paragraph::new(lines)
        .style(theme.text())
        .block(theme.panel_block(title, focused))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
}

fn output_title(app: &App, text: &str) -> String {
    output_title_for("Output", app, text, app.output_page_size, app.output_scroll)
}

fn output_title_for(label: &str, app: &App, text: &str, page_size: u16, scroll: u16) -> String {
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
        .map(|line| highlighted_output_line(app, theme, line))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec![Line::from("")]
    } else {
        lines
    }
}

fn highlighted_output_line(app: &App, theme: &Theme, line: &str) -> Line<'static> {
    let ranges = match app.output_search.match_ranges(line) {
        Ok(ranges) if !ranges.is_empty() => ranges,
        _ => return Line::styled(line.to_owned(), theme.text()),
    };
    let mut spans = Vec::new();
    let mut cursor = 0;
    for (start, end) in ranges {
        if start > cursor {
            spans.push(Span::styled(line[cursor..start].to_owned(), theme.text()));
        }
        spans.push(Span::styled(line[start..end].to_owned(), theme.search_match()));
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
    use crate::tree::{DiscoveredTest, TestKey, Tree};
    use std::path::PathBuf;

    #[test]
    fn output_title_shows_all_when_text_fits() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;

        assert_eq!(
            output_title(&app, "one\ntwo"),
            "Output <lines: 1-2/2> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );
    }

    #[test]
    fn output_title_shows_clamped_line_ranges() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 3;
        let text = "1\n2\n3\n4\n5\n6";

        app.output_scroll = 0;
        assert_eq!(
            output_title(&app, text),
            "Output <lines: 1-3/6> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );

        app.output_scroll = 2;
        assert_eq!(
            output_title(&app, text),
            "Output <lines: 3-5/6> <search: [            ] 0/0 [n]ext [f]ilter:off [r]egex:off [c]ase-sensitive:off>"
        );

        app.output_scroll = 3;
        assert_eq!(
            output_title(&app, text),
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
    fn tests_title_includes_focus_filter_hints() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.tree.view_filter.show_ignored = false;

        assert_eq!(
            tests_title(&app),
            "Tests <filters: [p]ass:on [f]ail:on [i]gnore:off [s]kip:on>"
        );
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
    fn output_title_includes_search_flags() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;
        app.output_search.query = "panic".to_owned();
        app.output_search.filter = true;

        assert_eq!(
            output_title(&app, "panic line"),
            "Output <lines: 1-1/1> <search: [panic       ] 0/1 [n]ext [f]ilter:on [r]egex:off [c]ase-sensitive:off>"
        );
    }

    #[test]
    fn output_search_box_marks_active_input() {
        let mut app = App::new(Tree::from_tests(Vec::new()));
        app.output_search.query = "panic".to_owned();
        app.output_search.input_active = true;

        assert_eq!(app.output_search.box_text(18), "[panic_            ]");
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
