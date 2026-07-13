use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    output_pane::{OutputSearchState, SearchModalFocus},
    theme::Theme,
};

use super::super::{
    geometry::centered_rect,
    primitives::{ModalChrome, draw_modal_shell},
};

pub(in crate::ui) struct OutputSearchModal<'a> {
    search: &'a OutputSearchState,
}

impl<'a> OutputSearchModal<'a> {
    pub(in crate::ui) fn new(search: &'a OutputSearchState) -> Self {
        Self { search }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
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
        let query_focused = self.search.modal_focus == SearchModalFocus::Query;

        frame.render_widget(
            Paragraph::new(Line::styled(
                "Query",
                Self::label_style(query_focused, theme),
            )),
            chunks[0],
        );

        let editor_width = chunks[1].width.saturating_sub(2) as usize;
        let empty = self.search.draft_query().is_empty();
        let editor_text = if empty && !query_focused {
            "Search output...".to_owned()
        } else {
            self.search.draft_view(editor_width, query_focused)
        };
        let editor_style = if query_focused {
            theme.selected()
        } else if empty {
            theme.muted()
        } else {
            theme.text()
        };
        frame.render_widget(
            Paragraph::new(editor_text).style(editor_style).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(query_focused)),
            ),
            chunks[1],
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Self::button(
                    "Clear",
                    self.search.modal_focus == SearchModalFocus::Clear,
                    theme,
                ),
                Span::raw("  "),
                Self::button(
                    "Apply",
                    self.search.modal_focus == SearchModalFocus::Apply,
                    theme,
                ),
            ])),
            chunks[2],
        );
        frame.render_widget(
            Paragraph::new(vec![
                Self::checkbox(
                    "filter matching lines",
                    self.search.draft_filter,
                    self.search.modal_focus == SearchModalFocus::Filter,
                    theme,
                ),
                Self::checkbox(
                    "regex",
                    self.search.draft_regex,
                    self.search.modal_focus == SearchModalFocus::Regex,
                    theme,
                ),
                Self::checkbox(
                    "case-sensitive",
                    self.search.draft_case_sensitive,
                    self.search.modal_focus == SearchModalFocus::CaseSensitive,
                    theme,
                ),
            ]),
            chunks[4],
        );
    }

    fn label_style(active: bool, theme: &Theme) -> ratatui::style::Style {
        if active {
            theme.title(true)
        } else {
            theme.muted()
        }
    }

    fn button(label: &'static str, active: bool, theme: &Theme) -> Span<'static> {
        Span::styled(
            format!("[ {label} ]"),
            if active {
                theme.selected()
            } else {
                theme.text()
            },
        )
    }

    fn checkbox(label: &'static str, checked: bool, active: bool, theme: &Theme) -> Line<'static> {
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
}
