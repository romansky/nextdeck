use ratatui::{
    style::Style,
    text::{Line, Span},
};

const EMPTY_VALUE_PLACEHOLDER: &str = "[empty]";
const LABEL_VALUE_GAP: usize = 1;

#[derive(Clone, Debug)]
pub(crate) struct ParameterListRow {
    pub marker: String,
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
    pub details: Option<String>,
    pub style: Style,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParameterListStyles {
    pub hint: Style,
    pub details: Style,
    pub empty_value: Style,
}

#[derive(Clone, Debug)]
pub(crate) struct ParameterList<'a> {
    pub rows: &'a [ParameterListRow],
    pub label_width: usize,
    pub content_width: usize,
    pub styles: ParameterListStyles,
}

impl ParameterList<'_> {
    pub(crate) fn render(&self) -> Vec<Line<'static>> {
        self.rows
            .iter()
            .flat_map(|row| self.render_row(row))
            .collect()
    }

    fn render_row(&self, row: &ParameterListRow) -> Vec<Line<'static>> {
        let mut lines = vec![self.value_line(row)];
        if let Some(hint) = non_empty(row.hint.as_deref()) {
            lines.push(Line::styled(
                fit_line_prefix(hint, self.content_width),
                self.styles.hint,
            ));
        }
        if let Some(details) = non_empty(row.details.as_deref()) {
            lines.push(Line::styled(
                fit_line_prefix(details, self.content_width),
                self.styles.details,
            ));
        }
        lines
    }

    fn value_line(&self, row: &ParameterListRow) -> Line<'static> {
        let label = fit_line_prefix(row.label.trim_end(), self.label_width);
        let prefix = format!(
            "{} {:<width$}{}",
            row.marker,
            label.trim_end(),
            " ".repeat(LABEL_VALUE_GAP),
            width = self.label_width
        );
        let prefix_width = 2 + self.label_width + LABEL_VALUE_GAP;
        let value_width = self.content_width.saturating_sub(prefix_width);
        let mut spans = vec![Span::styled(prefix, row.style)];

        if row.value.trim().is_empty() {
            let placeholder = fit_line_prefix(EMPTY_VALUE_PLACEHOLDER, value_width);
            spans.push(Span::styled(placeholder, self.styles.empty_value));
        } else {
            spans.push(Span::styled(
                fit_line_prefix(row.value.trim_end(), value_width),
                row.style,
            ));
        }

        Line::from(spans)
    }
}

fn non_empty(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

fn fit_line_prefix(content: &str, width: usize) -> String {
    let char_count = content.chars().count();
    if char_count <= width {
        return format!("{content:<width$}");
    }
    if width <= 3 {
        return content.chars().take(width).collect();
    }
    let prefix = content.chars().take(width - 3).collect::<String>();
    format!("{prefix}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_multiline_parameter_rows() {
        let rows = vec![ParameterListRow {
            marker: ">".to_owned(),
            label: "--profile".to_owned(),
            value: "debug".to_owned(),
            hint: Some("# Build profile".to_owned()),
            details: Some("# enum: debug, release".to_owned()),
            style: Style::default(),
        }];
        let lines = ParameterList {
            rows: &rows,
            label_width: 10,
            content_width: 42,
            styles: ParameterListStyles {
                hint: Style::default(),
                details: Style::default(),
                empty_value: Style::default(),
            },
        }
        .render();

        assert_eq!(
            line_text(&lines[0]),
            "> --profile  debug                        "
        );
        assert_eq!(
            line_text(&lines[1]),
            "# Build profile                           "
        );
        assert_eq!(
            line_text(&lines[2]),
            "# enum: debug, release                    "
        );
    }

    #[test]
    fn marks_empty_values() {
        let rows = vec![ParameterListRow {
            marker: " ".to_owned(),
            label: "--version".to_owned(),
            value: String::new(),
            hint: None,
            details: Some("# string".to_owned()),
            style: Style::default(),
        }];
        let lines = ParameterList {
            rows: &rows,
            label_width: 10,
            content_width: 24,
            styles: ParameterListStyles {
                hint: Style::default(),
                details: Style::default(),
                empty_value: Style::default(),
            },
        }
        .render();

        assert_eq!(line_text(&lines[0]), "  --version  [empty]    ");
        assert_eq!(line_text(&lines[1]), "# string                ");
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }
}
