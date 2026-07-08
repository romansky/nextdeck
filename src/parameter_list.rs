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
    pub details: Option<ParameterDetails>,
    pub style: Style,
    pub value_style: Option<Style>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParameterDetails {
    kind: ParameterKind,
    choices: Vec<String>,
    default: Option<String>,
    custom_value: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParameterKind {
    Bool,
    Enum,
    Number,
    String,
}

impl ParameterDetails {
    pub(crate) fn bool(default: bool) -> Self {
        Self {
            kind: ParameterKind::Bool,
            choices: vec!["off".to_owned(), "on".to_owned()],
            default: Some(on_off(default).to_owned()),
            custom_value: false,
        }
    }

    pub(crate) fn enum_values(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::new(ParameterKind::Enum).with_choices(values)
    }

    pub(crate) fn number() -> Self {
        Self::new(ParameterKind::Number)
    }

    pub(crate) fn string() -> Self {
        Self::new(ParameterKind::String)
    }

    pub(crate) fn with_choices(
        mut self,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.choices = values
            .into_iter()
            .map(Into::into)
            .filter(|value: &String| !value.trim().is_empty())
            .collect();
        self
    }

    pub(crate) fn with_default(mut self, default: impl Into<String>) -> Self {
        let default = default.into();
        if !default.trim().is_empty() {
            self.default = Some(default);
        }
        self
    }

    pub(crate) fn custom_value(mut self) -> Self {
        self.custom_value = true;
        self
    }

    fn new(kind: ParameterKind) -> Self {
        Self {
            kind,
            choices: Vec::new(),
            default: None,
            custom_value: false,
        }
    }

    fn render(&self) -> String {
        let mut details = format!("# {}", self.kind.label());
        if !self.choices.is_empty() {
            details.push_str(": ");
            details.push_str(&self.choices.join(", "));
        }

        let mut notes = Vec::new();
        if let Some(default) = &self.default {
            notes.push(format!("default: {default}"));
        }
        if self.custom_value {
            notes.push("[e] custom".to_owned());
        }
        if !notes.is_empty() {
            details.push_str(" (");
            details.push_str(&notes.join("; "));
            details.push(')');
        }

        details
    }
}

impl ParameterKind {
    fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Enum => "enum",
            Self::Number => "number",
            Self::String => "string",
        }
    }
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
    pub marker_width: usize,
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
        if let Some(details) = row.details.as_ref().map(ParameterDetails::render) {
            lines.push(Line::styled(
                fit_line_prefix(&details, self.content_width),
                self.styles.details,
            ));
        }
        lines
    }

    fn value_line(&self, row: &ParameterListRow) -> Line<'static> {
        let marker = fit_line_prefix(row.marker.trim_end(), self.marker_width);
        let label = fit_line_prefix(row.label.trim_end(), self.label_width);
        let prefix = format!("{marker}{label}{}", " ".repeat(LABEL_VALUE_GAP));
        let prefix_width = self.marker_width + self.label_width + LABEL_VALUE_GAP;
        let value_width = self.content_width.saturating_sub(prefix_width);
        let mut spans = vec![Span::styled(prefix, row.style)];
        let value_style = row.value_style.unwrap_or(row.style);

        if row.value.trim().is_empty() {
            let placeholder = fit_line_prefix(EMPTY_VALUE_PLACEHOLDER, value_width);
            spans.push(Span::styled(placeholder, self.styles.empty_value));
        } else {
            spans.push(Span::styled(
                fit_line_prefix(row.value.trim_end(), value_width),
                value_style,
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

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
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
            details: Some(ParameterDetails::enum_values(["debug", "release"])),
            style: Style::default(),
            value_style: None,
        }];
        let lines = ParameterList {
            rows: &rows,
            marker_width: 2,
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
            details: Some(ParameterDetails::string()),
            style: Style::default(),
            value_style: None,
        }];
        let lines = ParameterList {
            rows: &rows,
            marker_width: 2,
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

    #[test]
    fn renders_rows_without_marker_column() {
        let rows = vec![ParameterListRow {
            marker: String::new(),
            label: "status".to_owned(),
            value: "passed".to_owned(),
            hint: None,
            details: None,
            style: Style::default(),
            value_style: None,
        }];
        let lines = ParameterList {
            rows: &rows,
            marker_width: 0,
            label_width: 8,
            content_width: 20,
            styles: ParameterListStyles {
                hint: Style::default(),
                details: Style::default(),
                empty_value: Style::default(),
            },
        }
        .render();

        assert_eq!(line_text(&lines[0]), "status   passed     ");
    }

    #[test]
    fn formats_parameter_details_with_one_convention() {
        assert_eq!(
            ParameterDetails::bool(false).render(),
            "# bool: off, on (default: off)"
        );
        assert_eq!(
            ParameterDetails::number()
                .with_choices(["profile", "0..20"])
                .with_default("profile")
                .custom_value()
                .render(),
            "# number: profile, 0..20 (default: profile; [e] custom)"
        );
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }
}
