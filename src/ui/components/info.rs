use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::App,
    config,
    disk_usage::{format_bytes, format_timestamp_utc},
    parameter_list::{ParameterList, ParameterListRow, ParameterListRowKind},
    theme::Theme,
    tree::NodeKind,
};

use super::super::view_helpers::{
    duration_label, first_source_path, format_duration, parameter_list_styles, status_label,
    storage_status, storage_status_style,
};

const DETAIL_LIST_LABEL_WIDTH: usize = 9;
const RUN_DETAIL_LABEL_WIDTH: usize = 12;

macro_rules! detail_row {
    ($theme:expr, status: $status:expr) => {
        detail_row!($theme, "status" => status_label($status), $theme.status($status, false))
    };
    ($theme:expr, $label:expr => $value:expr) => {
        detail_row!($theme, $label => $value, $theme.text())
    };
    ($theme:expr, $label:expr => $value:expr, $value_style:expr) => {
        ParameterListRow {
            kind: ParameterListRowKind::Detail,
            label: ($label).into(),
            value: ($value).into(),
            value_style: Some($value_style),
            ..Default::default()
        }
    };
}

pub(in crate::ui) struct InfoPanel<'a> {
    app: &'a App,
}

impl<'a> InfoPanel<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme, area: Rect) {
        let status = Self::status(self.app);
        let block = theme.panel_block(&status, None, false);
        let inner = block.inner(area);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(inner);
        let run_details = Paragraph::new(Self::selection_lines(
            self.app,
            theme,
            columns[0].width as usize,
        ))
        .style(theme.text())
        .wrap(Wrap { trim: false });
        let storage_details = Paragraph::new(Self::storage_lines(
            self.app,
            theme,
            columns[1].width as usize,
        ))
        .style(theme.text())
        .wrap(Wrap { trim: false });
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(run_details, columns[0]);
        frame.render_widget(storage_details, columns[1]);
    }

    pub(in crate::ui) fn status(_app: &App) -> String {
        "Info".to_owned()
    }

    pub(in crate::ui) fn run_lines(
        app: &App,
        theme: &Theme,
        content_width: usize,
    ) -> Vec<Line<'static>> {
        let (finished, total) = app.run_progress();
        let rows = vec![
            detail_row!(theme, "run id" => app.run.run_id.clone().unwrap_or_else(|| "-".to_owned())),
            detail_row!(theme, "status" => app.run_status_label()),
            detail_row!(theme, "result" => app.run_result_label(), Self::run_result_style(app, theme)),
            detail_row!(theme, "profile" => app.run.profile.clone(), theme.accent()),
            detail_row!(theme, "scope" => app.run.scope.label()),
            detail_row!(theme, "duration" => Self::run_duration_summary_label(app)),
            detail_row!(theme, "latest event" => app.test_events.latest_event_label(), theme.accent()),
            detail_row!(theme, "progress" => format!("{finished}/{total}")),
        ];
        let mut lines = vec![Line::styled("Latest Nextest Run", theme.title(false))];
        lines.extend(
            ParameterList::new(
                &rows,
                0,
                RUN_DETAIL_LABEL_WIDTH,
                content_width,
                parameter_list_styles(theme),
            )
            .render(),
        );
        lines
    }

    pub(in crate::ui) fn storage_lines(
        app: &App,
        theme: &Theme,
        content_width: usize,
    ) -> Vec<Line<'static>> {
        let mut lines = vec![Line::styled("Storage", theme.title(false))];
        let status_row = [detail_row!(
            theme,
            "status" => storage_status(app),
            storage_status_style(app, theme)
        )];
        lines.extend(Self::detail_rows_lines(&status_row, theme, content_width));

        if let Some(snapshot) = &app.disk_usage.snapshot {
            let mut rows = vec![
                detail_row!(
                    theme,
                    "available" => snapshot
                        .available_bytes
                        .map(format_bytes)
                        .unwrap_or_else(|| "-".to_owned())
                ),
                detail_row!(theme, "updated" => format_timestamp_utc(snapshot.updated_at)),
            ];
            for entry in &snapshot.entries {
                rows.push(detail_row!(
                    theme,
                    Self::storage_entry_label(entry.label) => format_bytes(entry.bytes)
                ));
            }
            lines.extend(Self::detail_rows_lines(&rows, theme, content_width));
        } else if app.disk_usage.loading {
            lines.push(Line::styled("Scanning disk usage...", theme.muted()));
        } else if let Some(error) = &app.disk_usage.error {
            lines.push(Line::styled(error.clone(), theme.danger()));
        } else {
            lines.push(Line::styled("No disk usage snapshot.", theme.muted()));
        }

        if let Some(result) = &app.disk_cleanup.last_result {
            lines.push(Line::from(""));
            let row = [match result {
                Ok(()) => detail_row!(theme, "cleanup" => "completed", theme.success()),
                Err(_) => detail_row!(theme, "cleanup" => "failed", theme.danger()),
            }];
            lines.extend(Self::detail_rows_lines(&row, theme, content_width));
        }

        lines
    }

    fn selection_lines(app: &App, theme: &Theme, content_width: usize) -> Vec<Line<'static>> {
        let Some(node) = app.tree.selected_node() else {
            return vec![Line::styled("No selection", theme.muted())];
        };

        let mut lines = Self::run_lines(app, theme, content_width);
        lines.push(Line::from(""));
        lines.push(Line::styled("Selection", theme.title(false)));

        let rows = match &node.kind {
            NodeKind::Workspace => vec![
                detail_row!(theme, "kind" => "workspace"),
                detail_row!(theme, status: node.status),
                detail_row!(theme, "path" => app.tree.selected_path()),
            ],
            NodeKind::Package { name } => vec![
                detail_row!(theme, "kind" => "package"),
                detail_row!(theme, "pkg" => name.clone(), theme.accent()),
                detail_row!(theme, status: node.status),
                detail_row!(
                    theme,
                    "duration" => duration_label(node, app.settings.tree_duration_mode)
                ),
            ],
            NodeKind::Binary {
                package,
                name,
                kind,
            } => {
                let source = first_source_path(node);
                vec![
                    detail_row!(theme, "kind" => "target"),
                    detail_row!(theme, "pkg" => package.clone(), theme.accent()),
                    detail_row!(theme, "target" => name.clone(), theme.accent()),
                    detail_row!(theme, "type" => kind.clone()),
                    detail_row!(theme, "source" => source),
                    detail_row!(theme, status: node.status),
                    detail_row!(
                        theme,
                        "duration" => duration_label(node, app.settings.tree_duration_mode)
                    ),
                ]
            }
            NodeKind::Module { path } => vec![
                detail_row!(theme, "kind" => "module"),
                detail_row!(theme, "module" => path.clone(), theme.accent()),
                detail_row!(theme, status: node.status),
                detail_row!(
                    theme,
                    "duration" => duration_label(node, app.settings.tree_duration_mode)
                ),
            ],
            NodeKind::Test(test) => vec![
                detail_row!(theme, "kind" => "test"),
                detail_row!(theme, status: node.status),
                detail_row!(theme, "pkg" => test.package.clone(), theme.accent()),
                detail_row!(theme, "bin" => test.binary.clone()),
                detail_row!(
                    theme,
                    "module" => test.module.clone().unwrap_or_else(|| "-".to_owned())
                ),
                detail_row!(theme, "test" => test.full_name.clone(), theme.accent()),
                detail_row!(
                    theme,
                    "source" => test.source_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_owned())
                ),
                detail_row!(
                    theme,
                    "duration" => duration_label(node, app.settings.tree_duration_mode)
                ),
            ],
        };
        lines.extend(Self::detail_rows_lines(&rows, theme, content_width));

        lines
    }

    fn storage_entry_label(label: &str) -> String {
        if label.starts_with('/') {
            label.to_owned()
        } else {
            format!("/{label}")
        }
    }

    fn run_result_style(app: &App, theme: &Theme) -> Style {
        match app.run.outcome {
            crate::app::RunOutcome::Passed => theme.success(),
            crate::app::RunOutcome::Failed | crate::app::RunOutcome::CommandFailed => {
                theme.danger()
            }
            crate::app::RunOutcome::Stopped => theme.warning(),
            crate::app::RunOutcome::Running => theme.accent(),
            crate::app::RunOutcome::NotStarted => theme.muted(),
        }
    }

    fn detail_rows_lines(
        rows: &[ParameterListRow],
        theme: &Theme,
        content_width: usize,
    ) -> Vec<Line<'static>> {
        ParameterList::new(
            rows,
            0,
            DETAIL_LIST_LABEL_WIDTH,
            content_width,
            parameter_list_styles(theme),
        )
        .render()
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

    fn aggregate_duration_label(app: &App) -> String {
        app.tree
            .root
            .display_duration(config::TreeDurationMode::Aggregate)
            .map(format_duration)
            .unwrap_or_else(|| "-".to_owned())
    }

    fn run_duration_summary_label(app: &App) -> String {
        format!(
            "wall:{} aggregate:{} build:{} tests:{}",
            Self::run_duration_label(app),
            Self::aggregate_duration_label(app),
            Self::build_duration_label(app),
            Self::test_duration_label(app)
        )
    }
}
