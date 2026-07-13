use ratatui::{
    Frame,
    text::{Line, Span},
    widgets::Clear,
};

use crate::{
    app::App,
    custom_run::{CustomRunField, CustomRunState},
    parameter_list::{ParameterList, ParameterListRow, ParameterListRowKind},
    state::StatusCounts,
    theme::Theme,
    tree::{NodeKind, TestNode},
};

use super::super::{
    geometry::centered_rect,
    primitives::scrollable_paragraph,
    view_helpers::{
        SELECTABLE_FIELD_PREFIX_WIDTH, duration_label, first_source_path, output_summary,
        parameter_list_styles, parameter_value_width, status_label,
    },
};

const CUSTOM_RUN_FIELD_LABEL_WIDTH: usize = 15;
const DETAIL_MODAL_LABEL_WIDTH: usize = 8;
#[cfg(test)]
const CONTENT_WIDTH: usize = 82;

pub(in crate::ui) struct TestDetailsModal<'a> {
    app: &'a App,
}

impl<'a> TestDetailsModal<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(86, 88, frame.area());
        let block = theme.panel_block_with_actions(
            Self::title(self.app),
            Some(Self::action_line(self.app, theme)),
            true,
        );
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let lines = Self::lines_with_width(self.app, theme, inner.width as usize);
        let paragraph = scrollable_paragraph(lines, theme, &self.app.custom_run.viewport);
        frame.render_widget(paragraph, inner);
    }

    pub(in crate::ui) fn actions(app: &App) -> &'static str {
        if app.custom_run.editing.is_some() {
            return "[esc]cancel";
        }
        if app.custom_run.open {
            return "[r]run [esc]back";
        }
        if app
            .tree
            .selected_node()
            .is_some_and(|node| matches!(node.kind, NodeKind::Test(_)))
        {
            "[R]un-custom [s]sample-stacks [esc]close"
        } else {
            "[R]un-custom [esc]close"
        }
    }

    pub(in crate::ui) fn title(app: &App) -> &'static str {
        if app.custom_run.open {
            "Test Details > Custom Run"
        } else {
            "Test Details"
        }
    }

    pub(in crate::ui) fn action_line(app: &App, theme: &Theme) -> Line<'static> {
        let active = theme.title(true);
        if app.custom_run.open || app.custom_run.editing.is_some() {
            return Line::styled(format!(" {} ", Self::actions(app)), active);
        }

        let mut spans = vec![Span::styled(" [R]un-custom", active)];
        if app
            .tree
            .selected_node()
            .is_some_and(|node| matches!(node.kind, NodeKind::Test(_)))
        {
            spans.push(Span::styled(
                " [s]sample-stacks",
                if Self::stack_sample_available(app) {
                    active
                } else {
                    theme.muted()
                },
            ));
        }
        spans.push(Span::styled(" [esc]close ", active));
        Line::from(spans)
    }

    pub(in crate::ui) fn stack_sample_available(app: &App) -> bool {
        app.running
            && app.tree.selected_node().is_some_and(|node| {
                matches!(node.kind, NodeKind::Test(_))
                    && node.status == crate::tree::TestStatus::Running
            })
    }

    #[cfg(test)]
    pub(in crate::ui) fn lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
        Self::lines_with_width(app, theme, CONTENT_WIDTH)
    }

    pub(in crate::ui) fn custom_run_lines(
        custom_run: &CustomRunState,
        theme: &Theme,
        content_width: usize,
    ) -> Vec<Line<'static>> {
        let label_width = CUSTOM_RUN_FIELD_LABEL_WIDTH;
        let value_width = parameter_value_width(content_width, label_width);
        let rows = Self::custom_run_parameter_rows(custom_run, value_width);
        ParameterList::new(
            &rows,
            SELECTABLE_FIELD_PREFIX_WIDTH,
            label_width,
            content_width,
            parameter_list_styles(theme),
        )
        .render()
    }

    fn lines_with_width(app: &App, theme: &Theme, content_width: usize) -> Vec<Line<'static>> {
        let Some(node) = app.tree.selected_node() else {
            return vec![Line::styled("No selection", theme.muted())];
        };

        if app.custom_run.open {
            return Self::custom_run_view_lines(app, theme, content_width);
        }

        let scope = app.selected_scope();
        let counts = app.tree.status_counts_for_scope(&scope);
        let mut lines = vec![
            Line::styled(app.tree.selected_path(), theme.title(true)),
            Line::from(""),
        ];
        let mut detail_rows = vec![
            detail_row(theme, "kind", Self::selected_kind_label(node), None),
            detail_row(
                theme,
                "status",
                status_label(node.status),
                Some(theme.status(node.status, false)),
            ),
            detail_row(
                theme,
                "duration",
                duration_label(node, app.settings.tree_duration_mode),
                None,
            ),
            detail_row(theme, "tests", Self::status_counts_label(counts), None),
        ];

        match &node.kind {
            NodeKind::Workspace => {}
            NodeKind::Package { name } => {
                detail_rows.push(detail_row(
                    theme,
                    "package",
                    name.clone(),
                    Some(theme.accent()),
                ));
            }
            NodeKind::Binary {
                package,
                name,
                kind,
            } => {
                detail_rows.extend([
                    detail_row(theme, "package", package.clone(), Some(theme.accent())),
                    detail_row(theme, "binary", name.clone(), None),
                    detail_row(theme, "target", kind.clone(), None),
                    detail_row(theme, "source", first_source_path(node), None),
                ]);
            }
            NodeKind::Module { path } => {
                detail_rows.push(detail_row(
                    theme,
                    "module",
                    path.clone(),
                    Some(theme.accent()),
                ));
                detail_rows.push(detail_row(theme, "source", first_source_path(node), None));
            }
            NodeKind::Test(test) => {
                detail_rows.extend([
                    detail_row(theme, "package", test.package.clone(), Some(theme.accent())),
                    detail_row(theme, "binary", test.binary.clone(), None),
                    detail_row(theme, "target", test.binary_kind.clone(), None),
                    detail_row(
                        theme,
                        "module",
                        test.module.clone().unwrap_or_else(|| "-".to_owned()),
                        None,
                    ),
                    detail_row(theme, "ignored", Self::bool_label(test.ignored), None),
                    detail_row(
                        theme,
                        "ignore",
                        test.ignore_reason.clone().unwrap_or_else(|| "-".to_owned()),
                        None,
                    ),
                    detail_row(
                        theme,
                        "source",
                        test.source_path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "-".to_owned()),
                        None,
                    ),
                    detail_row(theme, "output", output_summary(node), None),
                ]);
            }
        }

        lines.extend(
            ParameterList::new(
                &detail_rows,
                0,
                DETAIL_MODAL_LABEL_WIDTH,
                content_width,
                parameter_list_styles(theme),
            )
            .render(),
        );
        lines
    }

    fn custom_run_view_lines(app: &App, theme: &Theme, content_width: usize) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::styled(app.tree.selected_path(), theme.title(true)),
            Line::from(""),
        ];
        lines.extend(Self::custom_run_lines(
            &app.custom_run,
            theme,
            content_width,
        ));
        lines.push(Line::from(""));
        match app.custom_run_command_preview() {
            Ok(command) => lines.push(Line::styled(command, theme.accent())),
            Err(error) => lines.push(Line::styled(error, theme.danger())),
        }
        lines
    }

    fn custom_run_parameter_rows(
        custom_run: &CustomRunState,
        value_width: usize,
    ) -> Vec<ParameterListRow> {
        CustomRunField::ALL
            .into_iter()
            .map(|field| {
                let selected = custom_run.selected == field;
                ParameterListRow {
                    selected,
                    active: selected,
                    label: field.label().to_owned(),
                    value: custom_run.field_value(field, value_width),
                    details: Some(custom_run.field_details(field)),
                    ..Default::default()
                }
            })
            .collect()
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

    fn status_counts_label(counts: StatusCounts) -> String {
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

    fn bool_label(value: bool) -> &'static str {
        if value { "yes" } else { "no" }
    }
}

fn detail_row(
    theme: &Theme,
    label: impl Into<String>,
    value: impl Into<String>,
    value_style: Option<ratatui::style::Style>,
) -> ParameterListRow {
    ParameterListRow {
        kind: ParameterListRowKind::Detail,
        label: label.into(),
        value: value.into(),
        value_style: value_style.or_else(|| Some(theme.text())),
        ..Default::default()
    }
}
