use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane},
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

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(1)])
        .split(panes[1]);

    AppLayout {
        tree: panes[0],
        details: right[0],
        output: right[1],
        status: outer[1],
    }
}

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let app_layout = layout(frame.area());
    draw_tree(frame, app, app_layout.tree);
    draw_details(frame, app, app_layout.details);
    draw_output(frame, app, app_layout.output);
    draw_status(frame, app, app_layout.status);

    if app.is_discovering() || app.discovery.error.is_some() {
        draw_discovery_modal(frame, app);
    }

    if app.show_help {
        draw_help(frame);
    }
}

fn draw_tree(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let selected = app.tree.selected_index();
    let rows = app.tree.visible_rows();
    let visible_height = area.height.saturating_sub(2).max(1) as usize;
    let items = rows
        .iter()
        .enumerate()
        .skip(app.tree_scroll)
        .take(visible_height)
        .map(|(index, (depth, node))| tree_item(*depth, node, index == selected))
        .collect::<Vec<_>>();

    let title = if app.focus == FocusPane::Tree {
        "Tests *"
    } else {
        "Tests"
    };
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(Clear, area);
    frame.render_widget(list, area);
}

fn draw_discovery_modal(frame: &mut Frame<'_>, app: &App) {
    let area = centered_rect(62, 58, frame.area());
    let lines = if let Some(error) = &app.discovery.error {
        vec![
            Line::from("Discovery failed"),
            Line::from(""),
            Line::from(error.as_str()),
            Line::from(""),
            Line::from("Press q to quit."),
        ]
    } else {
        vec![
            Line::from(format!(
                "{} Discovering tests",
                app.discovery_spinner()
            )),
            Line::from(""),
            Line::from("Running cargo nextest list --message-format json"),
            Line::from("Cold discovery may compile test binaries first."),
            Line::from(format!("Elapsed: {}s", app.discovery_elapsed_seconds())),
            Line::from("Press q to quit."),
        ]
    };
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(Block::default().title("Discovery").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn tree_item<'a>(depth: usize, node: &TestNode, selected: bool) -> ListItem<'a> {
    let indent = "  ".repeat(depth);
    let fold = if node.children.is_empty() {
        " "
    } else if node.expanded {
        "v"
    } else {
        ">"
    };
    let style = if selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        status_style(node.status)
    };
    ListItem::new(Line::from(vec![
        Span::raw(indent),
        Span::raw(fold),
        Span::raw(" "),
        Span::styled(node.status.symbol(), status_style(node.status)),
        Span::raw(" "),
        Span::styled(node_label(node), style),
    ]))
}

fn node_label(node: &TestNode) -> String {
    match &node.kind {
        NodeKind::Workspace => node.label.clone(),
        NodeKind::Package { name } => name.clone(),
        NodeKind::Module { .. } | NodeKind::Test(_) => node.label.clone(),
    }
}

fn draw_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = selected_details(app);
    let details = Paragraph::new(lines)
        .block(Block::default().title("Info").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(details, area);
}

fn selected_details(app: &App) -> Vec<Line<'_>> {
    let Some(node) = app.tree.selected_node() else {
        return vec![Line::from("No selection")];
    };

    match &node.kind {
        NodeKind::Workspace => vec![
            Line::from("Kind: workspace"),
            Line::from(format!("Status: {}", status_label(node.status))),
            Line::from(format!("Path: {}", app.tree.selected_path())),
        ],
        NodeKind::Package { name } => vec![
            Line::from("Kind: package"),
            Line::from(format!("Package: {name}")),
            Line::from(format!("Status: {}", status_label(node.status))),
        ],
        NodeKind::Module { path } => vec![
            Line::from("Kind: module"),
            Line::from(format!("Module: {path}")),
            Line::from(format!("Status: {}", status_label(node.status))),
        ],
        NodeKind::Test(test) => vec![
            Line::from("Kind: test"),
            Line::from(format!("Status: {}", status_label(node.status))),
            Line::from(format!("Package: {}", test.package)),
            Line::from(format!("Binary: {}", test.binary)),
            Line::from(format!("Module: {}", test.module.as_deref().unwrap_or("-"))),
            Line::from(format!("Test: {}", test.full_name)),
            Line::from(format!("Duration: {}", duration_label(node))),
        ],
    }
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

fn draw_output(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = app.tree.selected_output();
    let title = if app.focus == FocusPane::Output {
        "Output *"
    } else {
        "Output"
    };
    let output = Paragraph::new(text)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((app.output_scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let text = app.status_line();
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text), area);
}

fn draw_help(frame: &mut Frame<'_>) {
    let area = centered_rect(68, 82, frame.area());
    let text = vec![
        Line::from("Navigation"),
        Line::from("  Up/Down         move selection"),
        Line::from("  PageUp/PageDown page active pane"),
        Line::from("  Home/End        first or last tree row"),
        Line::from("  Left/Right      collapse or expand"),
        Line::from("  Tab             switch tree/output focus"),
        Line::from(""),
        Line::from("Runs"),
        Line::from("  r               run selected scope"),
        Line::from("  R               rerun failures"),
        Line::from("  f/F             next or previous failure"),
        Line::from(""),
        Line::from("Output"),
        Line::from("  End             follow output bottom"),
        Line::from("  h/?/F1          close help"),
        Line::from("  q               quit"),
    ];
    let help = Paragraph::new(text)
        .alignment(Alignment::Left)
        .block(Block::default().title("Help").borders(Borders::ALL));
    frame.render_widget(Clear, area);
    frame.render_widget(help, area);
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

fn status_style(status: TestStatus) -> Style {
    match status {
        TestStatus::Pending => Style::default().fg(Color::Gray),
        TestStatus::Running => Style::default().fg(Color::Cyan),
        TestStatus::Passed => Style::default().fg(Color::Green),
        TestStatus::Failed => Style::default().fg(Color::Red),
        TestStatus::Ignored => Style::default().fg(Color::Yellow),
        TestStatus::Skipped => Style::default().fg(Color::Magenta),
    }
}
