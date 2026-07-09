use std::time::Duration;

use ratatui::style::Style;

use crate::{
    app::{App, FocusPane},
    config,
    disk_usage::StorageHealth,
    parameter_list::ParameterListStyles,
    text_fit,
    theme::Theme,
    tree::{NodeKind, TestNode, TestStatus},
};

pub(in crate::ui) const SELECTABLE_FIELD_PREFIX_WIDTH: usize = 2;

pub(in crate::ui) fn pane_focused(app: &App, pane: FocusPane) -> bool {
    app.focus == pane && !app.command_context().pane_focus_suppressed()
}

pub(in crate::ui) fn fit_line_content(content: &str, width: usize) -> String {
    text_fit::fit_line_content(content, width)
}

pub(in crate::ui) fn fit_line_prefix(content: &str, width: usize) -> String {
    text_fit::fit_line_prefix(content, width)
}

pub(in crate::ui) fn parameter_value_width(content_width: usize, label_width: usize) -> usize {
    content_width.saturating_sub(SELECTABLE_FIELD_PREFIX_WIDTH + label_width + 1)
}

pub(in crate::ui) fn parameter_list_styles(theme: &Theme) -> ParameterListStyles {
    ParameterListStyles {
        text: theme.text(),
        selected: theme.selected(),
        label: theme.muted(),
        hint: theme.muted(),
        details: theme.muted(),
        empty_value: theme.warning(),
    }
}

pub(in crate::ui) fn output_summary(node: &TestNode) -> String {
    node.output.summary_label()
}

pub(in crate::ui) fn first_source_path(node: &TestNode) -> String {
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

pub(in crate::ui) fn duration_label(
    node: &TestNode,
    duration_mode: config::TreeDurationMode,
) -> String {
    node.display_duration(duration_mode)
        .map(format_duration)
        .unwrap_or_else(|| "-".to_owned())
}

pub(in crate::ui) fn duration_field(duration: Option<Duration>) -> String {
    duration
        .map(|duration| format!("[{:>8.3}s]", duration.as_secs_f64()))
        .unwrap_or_else(|| "[        ]".to_owned())
}

pub(in crate::ui) fn format_duration(duration: Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}

pub(in crate::ui) fn status_label(status: TestStatus) -> &'static str {
    match status {
        TestStatus::Pending => "pending",
        TestStatus::Running => "running",
        TestStatus::Passed => "passed",
        TestStatus::Failed => "failed",
        TestStatus::Ignored => "ignored",
        TestStatus::Skipped => "skipped",
    }
}

pub(in crate::ui) fn storage_status(app: &App) -> &'static str {
    storage_health(app).label()
}

pub(in crate::ui) fn storage_status_style(app: &App, theme: &Theme) -> Style {
    match storage_health(app) {
        StorageHealth::Healthy => theme.success(),
        StorageHealth::Low | StorageHealth::Failed => theme.danger(),
        StorageHealth::Scanning => theme.accent(),
        StorageHealth::Unknown | StorageHealth::NotScanned => theme.muted(),
    }
}

fn storage_health(app: &App) -> StorageHealth {
    app.disk_usage
        .health(app.settings.storage_low_space_threshold_bytes())
}
