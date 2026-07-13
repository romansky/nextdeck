use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, List, ListItem},
};

use crate::{
    app::App,
    config,
    symbols::{EVENT_BUBBLE, bool_symbol},
    theme::Theme,
    tree::{NodeKind, TestNode, TestStatus},
};

use super::super::view_helpers::{duration_field, pane_focused};
use crate::app::FocusPane;

pub(in crate::ui) struct TestsPanel<'a> {
    app: &'a App,
}

impl<'a> TestsPanel<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme, area: Rect) {
        let visible = self.app.tree.visible_rows_with_selection();
        let visible_height = area.height.saturating_sub(2).max(1) as usize;
        let items = visible
            .rows
            .iter()
            .enumerate()
            .skip(self.app.tree_viewport.scroll())
            .take(visible_height)
            .map(|(index, row)| {
                Self::item(
                    row.depth,
                    row.node,
                    index == visible.selected_index,
                    self.app.running_test_spinner(),
                    self.app.settings.tree_duration_mode,
                    theme,
                )
            })
            .collect::<Vec<_>>();

        let focused = pane_focused(self.app, FocusPane::Tree);
        let status = Self::status(self.app);
        let list = List::new(items)
            .block(theme.panel_block(&status, Some(Self::actions()), focused))
            .highlight_style(theme.selected());
        frame.render_widget(Clear, area);
        frame.render_widget(list, area);
    }

    pub(in crate::ui) fn status(app: &App) -> String {
        format!(
            "Tests <filters: {} {} {} {}>",
            Self::filter_hint("pass", "p", app.tree.view_filter.show_success),
            Self::filter_hint("fail", "f", app.tree.view_filter.show_failed),
            Self::filter_hint("ignore", "i", app.tree.view_filter.show_ignored),
            Self::filter_hint("skip", "s", app.tree.view_filter.show_skipped)
        )
    }

    pub(in crate::ui) fn filter_hint(label: &str, key: &str, enabled: bool) -> String {
        let Some((head, tail)) = label.split_once(key) else {
            return format!("[{key}]{label}:{}", bool_symbol(enabled));
        };
        format!("{head}[{key}]{tail}:{}", bool_symbol(enabled))
    }

    pub(in crate::ui) fn actions() -> &'static str {
        "[r]un [j/J]failure [o]pen-editor [u]pdate"
    }

    pub(in crate::ui) fn leading_fields(
        depth: usize,
        node: &TestNode,
        duration_mode: config::TreeDurationMode,
    ) -> String {
        format!(
            "{}{} {} ",
            "  ".repeat(depth),
            Self::fold_marker(node),
            duration_field(node.display_duration(duration_mode))
        )
    }

    pub(in crate::ui) fn label(node: &TestNode, running_spinner: &str) -> String {
        let label = match &node.kind {
            NodeKind::Workspace => node.label.clone(),
            NodeKind::Package { name } => name.clone(),
            NodeKind::Binary { .. } | NodeKind::Module { .. } | NodeKind::Test(_) => {
                node.label.clone()
            }
        };
        let mut label = if node.status == TestStatus::Running {
            format!("{label} {running_spinner}")
        } else {
            label
        };
        if node.has_events {
            label.push(' ');
            label.push(EVENT_BUBBLE);
        }
        label
    }

    fn item<'b>(
        depth: usize,
        node: &TestNode,
        selected: bool,
        running_spinner: &str,
        duration_mode: config::TreeDurationMode,
        theme: &Theme,
    ) -> ListItem<'b> {
        let row_style = if selected {
            theme.selected()
        } else {
            theme.text()
        };
        let status_style = theme.status(node.status, selected);
        ListItem::new(Line::from(vec![
            Span::styled(Self::leading_fields(depth, node, duration_mode), row_style),
            Span::styled(
                Self::label(node, running_spinner),
                if selected { row_style } else { status_style },
            ),
        ]))
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
}
