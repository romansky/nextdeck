use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane},
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

pub fn layout(area: Rect) -> AppLayout {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(outer[0]);

    let details_height = if panes[1].height < 10 {
        panes[1].height.saturating_sub(3).max(1)
    } else {
        8
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
    let app_layout = layout(frame.area());
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
    let list = List::new(items)
        .block(theme.panel_block("Tests", focused))
        .highlight_style(theme.selected());
    frame.render_widget(Clear, area);
    frame.render_widget(list, area);
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
            Line::styled("Running cargo nextest list --message-format json", theme.text()),
            Line::styled("Cold discovery may compile test binaries first.", theme.muted()),
            Line::styled(format!("Elapsed: {}s", app.discovery_elapsed_seconds()), theme.text()),
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
    let indent = "  ".repeat(depth);
    let fold = if node.children.is_empty() {
        " "
    } else if node.expanded {
        "v"
    } else {
        ">"
    };
    let row_style = if selected { theme.selected() } else { theme.text() };
    let status_style = theme.status(node.status, selected);
    ListItem::new(Line::from(vec![
        Span::styled(indent, row_style),
        Span::styled(fold, row_style),
        Span::styled(" ", row_style),
        Span::styled(node.status.symbol(), status_style),
        Span::styled(" ", row_style),
        Span::styled(node_label(node), if selected { row_style } else { status_style }),
    ]))
}

fn node_label(node: &TestNode) -> String {
    match &node.kind {
        NodeKind::Workspace => node.label.clone(),
        NodeKind::Package { name } => name.clone(),
        NodeKind::Module { .. } | NodeKind::Test(_) => node.label.clone(),
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

    match &node.kind {
        NodeKind::Workspace => vec![
            detail_line("kind", "workspace", theme.text(), theme),
            detail_status_line(node.status, theme),
            detail_line("path", app.tree.selected_path(), theme.text(), theme),
        ],
        NodeKind::Package { name } => vec![
            detail_line("kind", "package", theme.text(), theme),
            detail_line("pkg", name.clone(), theme.accent(), theme),
            detail_status_line(node.status, theme),
        ],
        NodeKind::Module { path } => vec![
            detail_line("kind", "module", theme.text(), theme),
            detail_line("module", path.clone(), theme.accent(), theme),
            detail_status_line(node.status, theme),
        ],
        NodeKind::Test(test) => vec![
            detail_line("kind", "test", theme.text(), theme),
            detail_status_line(node.status, theme),
            detail_line("pkg", test.package.clone(), theme.accent(), theme),
            detail_line("bin", test.binary.clone(), theme.text(), theme),
            detail_line("module", test.module.clone().unwrap_or_else(|| "-".to_owned()), theme.text(), theme),
            detail_line("test", test.full_name.clone(), theme.accent(), theme),
            detail_line("duration", duration_label(node), theme.text(), theme),
        ],
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
    detail_line("status", status_label(status), theme.status(status, false), theme)
}

fn duration_label(node: &TestNode) -> String {
    node.output
        .duration
        .map(|duration| format!("{:.3}s", duration.as_secs_f64()))
        .unwrap_or_else(|| "-".to_owned())
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
    let text = app.tree.selected_output();
    let focused = app.focus == FocusPane::Output;
    let output = Paragraph::new(text)
        .style(theme.text())
        .block(theme.panel_block("Output", focused))
        .wrap(Wrap { trim: false })
        .scroll((app.output_scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
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
        Span::styled(app.git_status.unstaged.added.to_string(), theme.footer_dirty(true)),
        Span::styled(":", theme.footer_label()),
        Span::styled(app.git_status.unstaged.deleted.to_string(), theme.footer_dirty(false)),
        Span::styled(" | staged ", theme.footer_label()),
        Span::styled(app.git_status.staged.added.to_string(), theme.footer_dirty(true)),
        Span::styled(":", theme.footer_label()),
        Span::styled(app.git_status.staged.deleted.to_string(), theme.footer_dirty(false)),
        Span::styled(" | key ", theme.footer_label()),
        Span::styled(key, theme.footer_value()),
        Span::styled(" | ", theme.footer_label()),
        Span::styled(app.status.as_str(), theme.footer()),
    ]
}

fn draw_help(frame: &mut Frame<'_>, theme: &Theme) {
    let area = centered_rect(68, 82, frame.area());
    let text = vec![
        Line::styled("Navigation", theme.title(true)),
        help_line("Up/Down", "move selection", theme),
        help_line("PageUp/PageDown", "page active pane", theme),
        help_line("Home/End", "first or last tree row", theme),
        help_line("Left/Right", "collapse or expand", theme),
        help_line("Tab", "switch tree/output focus", theme),
        Line::from(""),
        Line::styled("Runs", theme.title(true)),
        help_line("r", "run selected scope", theme),
        help_line("R", "rerun failures", theme),
        help_line("f/F", "next or previous failure", theme),
        Line::from(""),
        Line::styled("Output", theme.title(true)),
        help_line("End", "follow output bottom", theme),
        help_line("h/?/F1", "close help", theme),
        help_line("q", "quit", theme),
    ];
    let help = Paragraph::new(text)
        .alignment(Alignment::Left)
        .style(theme.text())
        .block(theme.modal_block("Help"));
    frame.render_widget(Clear, area);
    frame.render_widget(help, area);
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
