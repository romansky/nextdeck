use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::super::view_helpers::fit_line_prefix;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui) struct AutoColumn {
    pub(in crate::ui) max_width: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::ui) struct AutoColumnLayout {
    #[cfg(test)]
    pub(in crate::ui) widths: Vec<usize>,
    #[cfg(not(test))]
    widths: Vec<usize>,
}

impl AutoColumnLayout {
    pub(in crate::ui) fn compute(
        columns: &[AutoColumn],
        rows: &[Vec<&str>],
        total_width: usize,
    ) -> Self {
        if columns.is_empty() || total_width == 0 {
            return Self {
                widths: vec![0; columns.len()],
            };
        }

        let separators = columns.len().saturating_sub(1).min(total_width);
        let available = total_width.saturating_sub(separators);
        let mut widths = vec![0; columns.len()];
        let flex_index = columns.len() - 1;

        for index in 0..flex_index {
            let content_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(|value| value.chars().count())
                .max()
                .unwrap_or_default();
            widths[index] = content_width.min(columns[index].max_width.unwrap_or(content_width));
        }

        let fixed_sum: usize = widths[..flex_index].iter().sum();
        if fixed_sum > available {
            let mut remaining = available;
            for width in &mut widths[..flex_index] {
                *width = (*width).min(remaining);
                remaining = remaining.saturating_sub(*width);
            }
        }

        let fixed_sum: usize = widths[..flex_index].iter().sum();
        widths[flex_index] = available.saturating_sub(fixed_sum);

        Self { widths }
    }

    pub(in crate::ui) fn row(&self, cells: &[(&str, Style)]) -> Line<'static> {
        let mut spans = Vec::new();
        for (index, width) in self.widths.iter().copied().enumerate() {
            if width > 0 {
                let (content, style) = cells.get(index).copied().unwrap_or(("", Style::default()));
                spans.push(Span::styled(fit_line_prefix(content, width), style));
            }
            if index + 1 < self.widths.len() && self.widths[index + 1..].iter().any(|w| *w > 0) {
                spans.push(Span::styled(
                    " ",
                    cells
                        .get(index)
                        .map(|(_, style)| *style)
                        .unwrap_or_default(),
                ));
            }
        }
        Line::from(spans)
    }
}
