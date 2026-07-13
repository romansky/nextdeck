use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::field_schema::ParameterDetails;
use crate::text_fit::fit_line_prefix;

const EMPTY_VALUE_PLACEHOLDER: &str = "[empty]";
const LABEL_VALUE_GAP: usize = 1;
const VALUE_HINT_GAP: usize = 1;
const INLINE_HINT_MIN_WIDTH: usize = 3;

#[derive(Clone, Debug, Default)]
pub(crate) struct ParameterListRow {
    pub kind: ParameterListRowKind,
    pub selected: bool,
    pub active: bool,
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
    pub details: Option<ParameterDetails>,
    pub value_style: Option<Style>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ParameterListRowKind {
    #[default]
    Control,
    Detail,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParameterListStyles {
    pub text: Style,
    pub selected: Style,
    pub label: Style,
    pub hint: Style,
    pub details: Style,
    pub empty_value: Style,
}

#[derive(Clone, Debug)]
pub(crate) struct ParameterList<'a> {
    pub rows: &'a [ParameterListRow],
    pub marker_width: usize,
    pub label_width: usize,
    pub content_width: usize,
    pub styles: ParameterListStyles,
}

impl ParameterList<'_> {
    pub(crate) fn new(
        rows: &[ParameterListRow],
        marker_width: usize,
        label_width: usize,
        content_width: usize,
        styles: ParameterListStyles,
    ) -> ParameterList<'_> {
        ParameterList {
            rows,
            marker_width,
            label_width,
            content_width,
            styles,
        }
    }

    pub(crate) fn render(&self) -> Vec<Line<'static>> {
        self.rows
            .iter()
            .flat_map(|row| self.render_row(row))
            .collect()
    }

    fn render_row(&self, row: &ParameterListRow) -> Vec<Line<'static>> {
        let mut lines = vec![self.value_line(row)];
        if let Some(details) = row.details.as_ref().map(ParameterDetails::render) {
            lines.push(Line::styled(
                fit_line_prefix(&details, self.content_width),
                self.styles.details,
            ));
        }
        lines
    }

    fn value_line(&self, row: &ParameterListRow) -> Line<'static> {
        let marker = fit_line_prefix(self.row_marker(row), self.marker_width);
        let label = fit_line_prefix(row.label.trim_end(), self.label_width);
        let prefix = format!("{marker}{label}{}", " ".repeat(LABEL_VALUE_GAP));
        let prefix_width = self.marker_width + self.label_width + LABEL_VALUE_GAP;
        let available_width = self.content_width.saturating_sub(prefix_width);
        let label_style = self.label_style(row);
        let mut spans = vec![Span::styled(prefix, label_style)];
        let value_style = row.value_style.unwrap_or(label_style);
        let value = display_value(row);

        if let Some(hint) = row.hint.as_deref().and_then(non_empty)
            && available_width >= self.inline_value_width() + VALUE_HINT_GAP + INLINE_HINT_MIN_WIDTH
        {
            let value_width = self.inline_value_width();
            spans.push(Span::styled(
                fit_line_prefix(value, value_width),
                if row.value.trim().is_empty() {
                    self.styles.empty_value
                } else {
                    value_style
                },
            ));
            spans.push(Span::raw(" ".repeat(VALUE_HINT_GAP)));
            let hint_width = available_width.saturating_sub(value_width + VALUE_HINT_GAP);
            spans.push(Span::styled(
                fit_line_prefix(&format!("# {hint}"), hint_width),
                self.styles.hint,
            ));
        } else {
            spans.push(Span::styled(
                fit_line_prefix(value, available_width),
                if row.value.trim().is_empty() {
                    self.styles.empty_value
                } else {
                    value_style
                },
            ));
        }

        Line::from(spans)
    }

    fn row_marker(&self, row: &ParameterListRow) -> &'static str {
        if row.kind == ParameterListRowKind::Control && row.selected {
            "@"
        } else {
            ""
        }
    }

    fn inline_value_width(&self) -> usize {
        self.rows
            .iter()
            .map(display_value)
            .map(|value| value.chars().count())
            .max()
            .unwrap_or_default()
    }

    fn label_style(&self, row: &ParameterListRow) -> Style {
        match row.kind {
            ParameterListRowKind::Control if row.active => self.styles.selected,
            ParameterListRowKind::Control => self.styles.text,
            ParameterListRowKind::Detail => self.styles.label,
        }
    }
}

fn display_value(row: &ParameterListRow) -> &str {
    non_empty(&row.value).unwrap_or(EMPTY_VALUE_PLACEHOLDER)
}

fn non_empty(text: &str) -> Option<&str> {
    let text = text.trim();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_multiline_parameter_rows() {
        let rows = vec![ParameterListRow {
            selected: true,
            active: true,
            label: "--profile".to_owned(),
            value: "debug".to_owned(),
            hint: Some("Build profile".to_owned()),
            details: Some(ParameterDetails::enum_values(["debug", "release"])),
            ..Default::default()
        }];
        let lines = render(&rows, 2, 10, 42);

        assert_eq!(
            line_text(&lines[0]),
            "@ --profile  debug # Build profile        "
        );
        assert_eq!(
            line_text(&lines[1]),
            "# enum: debug, release                    "
        );
    }

    #[test]
    fn marks_empty_values() {
        let rows = vec![ParameterListRow {
            label: "--version".to_owned(),
            value: String::new(),
            details: Some(ParameterDetails::string()),
            ..Default::default()
        }];
        let lines = render(&rows, 2, 10, 24);

        assert_eq!(line_text(&lines[0]), "  --version  [empty]    ");
        assert_eq!(line_text(&lines[1]), "# string                ");
    }

    #[test]
    fn renders_rows_without_marker_column() {
        let rows = vec![ParameterListRow {
            kind: ParameterListRowKind::Detail,
            label: "status".to_owned(),
            value: "passed".to_owned(),
            value_style: Some(Style::default()),
            ..Default::default()
        }];
        let lines = render(&rows, 0, 8, 20);

        assert_eq!(line_text(&lines[0]), "status   passed     ");
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn render(
        rows: &[ParameterListRow],
        marker_width: usize,
        label_width: usize,
        content_width: usize,
    ) -> Vec<Line<'static>> {
        ParameterList::new(
            rows,
            marker_width,
            label_width,
            content_width,
            ParameterListStyles {
                text: Style::default(),
                selected: Style::default(),
                label: Style::default(),
                hint: Style::default(),
                details: Style::default(),
                empty_value: Style::default(),
            },
        )
        .render()
    }
}
