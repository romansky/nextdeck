use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthStr as _;

use crate::output_pane::OutputView;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputPosition {
    pub source_line: usize,
    pub byte_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualRow {
    projected_line: usize,
    source_line: Option<usize>,
    byte_range: Range<usize>,
}

impl VisualRow {
    pub fn source_line(&self) -> Option<usize> {
        self.source_line
    }

    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LogicalLine {
    text_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputLayout {
    view: OutputView,
    width_cells: usize,
    lines: Vec<LogicalLine>,
    rows: Vec<VisualRow>,
}

impl Default for OutputLayout {
    fn default() -> Self {
        Self::new(
            OutputView {
                text: String::new(),
                source_lines: Vec::new(),
            },
            1,
        )
    }
}

impl OutputLayout {
    pub fn new(view: OutputView, width_cells: usize) -> Self {
        let width_cells = width_cells.max(1);
        let lines = logical_lines(&view.text);
        let mut rows = Vec::new();
        for (projected_line, line) in lines.iter().enumerate() {
            let text = &view.text[line.text_range.clone()];
            let source_line = view.source_lines.get(projected_line).copied();
            for byte_range in wrap_line(text, width_cells) {
                rows.push(VisualRow {
                    projected_line,
                    source_line,
                    byte_range,
                });
            }
        }
        if rows.is_empty() {
            rows.push(VisualRow {
                projected_line: 0,
                source_line: None,
                byte_range: 0..0,
            });
        }
        Self {
            view,
            width_cells,
            lines,
            rows,
        }
    }

    pub fn matches(&self, view: &OutputView, width_cells: usize) -> bool {
        self.width_cells == width_cells.max(1) && self.view == *view
    }

    pub fn row_count(&self) -> usize {
        self.rows.len().max(1)
    }

    pub fn row(&self, index: usize) -> Option<&VisualRow> {
        self.rows.get(index)
    }

    #[cfg(test)]
    pub fn row_text(&self, row: &VisualRow) -> &str {
        let Some(line) = self.lines.get(row.projected_line) else {
            return "";
        };
        let text = &self.view.text[line.text_range.clone()];
        &text[row.byte_range.clone()]
    }

    pub fn logical_line_text(&self, row: &VisualRow) -> &str {
        self.lines
            .get(row.projected_line)
            .map(|line| &self.view.text[line.text_range.clone()])
            .unwrap_or("")
    }

    pub fn top_position(&self, row: usize) -> Option<OutputPosition> {
        let row = self.rows.get(row.min(self.rows.len().saturating_sub(1)))?;
        Some(OutputPosition {
            source_line: row.source_line?,
            byte_offset: row.byte_range.start,
        })
    }

    pub fn row_for_position(&self, position: OutputPosition) -> Option<usize> {
        let exact_rows = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.source_line == Some(position.source_line))
            .collect::<Vec<_>>();
        if !exact_rows.is_empty() {
            return exact_rows
                .iter()
                .find(|(_, row)| {
                    row.byte_range.start <= position.byte_offset
                        && (position.byte_offset < row.byte_range.end
                            || row.byte_range.start == row.byte_range.end)
                })
                .map(|(index, _)| *index)
                .or_else(|| exact_rows.last().map(|(index, _)| *index));
        }

        self.rows
            .iter()
            .position(|row| {
                row.source_line
                    .is_some_and(|line| line > position.source_line)
            })
            .or_else(|| {
                self.rows.iter().rposition(|row| {
                    row.source_line
                        .is_some_and(|line| line < position.source_line)
                })
            })
    }

    pub fn row_range_for_source_bytes(
        &self,
        source_line: usize,
        byte_range: Range<usize>,
    ) -> Option<Range<usize>> {
        let mut matching = self.rows.iter().enumerate().filter(|(_, row)| {
            row.source_line == Some(source_line) && ranges_intersect(&row.byte_range, &byte_range)
        });
        let (start, _) = matching.next()?;
        let end = matching
            .next_back()
            .map(|(index, _)| index + 1)
            .unwrap_or(start + 1);
        Some(start..end)
    }

    #[cfg(test)]
    pub fn rows(&self) -> &[VisualRow] {
        &self.rows
    }
}

fn ranges_intersect(left: &Range<usize>, right: &Range<usize>) -> bool {
    if right.start == right.end {
        left.start <= right.start && right.start <= left.end
    } else {
        left.start < right.end && right.start < left.end
    }
}

fn logical_lines(text: &str) -> Vec<LogicalLine> {
    let mut offset = 0;
    let mut lines = text
        .lines()
        .map(|line| {
            let start = offset;
            let end = start + line.len();
            offset = end.saturating_add(1);
            LogicalLine {
                text_range: start..end,
            }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(LogicalLine { text_range: 0..0 });
    }
    lines
}

// Output is a whitespace-sensitive log, so wrapping never removes bytes. Breaks prefer
// whitespace boundaries, while oversized words are split at extended grapheme boundaries.
fn wrap_line(line: &str, width_cells: usize) -> Vec<Range<usize>> {
    if line.is_empty() {
        return std::iter::once(0..0).collect();
    }

    let graphemes = line.grapheme_indices(true).collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut row_start = 0;
    let mut grapheme_index = 0;

    while grapheme_index < graphemes.len() {
        let mut width: usize = 0;
        let mut cursor = grapheme_index;
        let mut last_whitespace_end = None;
        let mut row_end = row_start;

        while cursor < graphemes.len() {
            let (byte_index, grapheme) = graphemes[cursor];
            let grapheme_end = byte_index + grapheme.len();
            let grapheme_width = grapheme.width();
            if width.saturating_add(grapheme_width) > width_cells && row_end > row_start {
                row_end = if grapheme.chars().all(char::is_whitespace) {
                    byte_index
                } else {
                    last_whitespace_end
                        .filter(|boundary| *boundary > row_start)
                        .unwrap_or(byte_index)
                };
                break;
            }

            width = width.saturating_add(grapheme_width);
            row_end = grapheme_end;
            cursor += 1;
            if grapheme.chars().all(char::is_whitespace) {
                last_whitespace_end = Some(grapheme_end);
            }
        }

        if row_end == row_start {
            let (byte_index, grapheme) = graphemes[grapheme_index];
            row_end = byte_index + grapheme.len();
        }
        rows.push(row_start..row_end);
        row_start = row_end;
        grapheme_index = graphemes.partition_point(|(byte_index, _)| *byte_index < row_start);
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(text: &str, width: usize) -> OutputLayout {
        OutputLayout::new(
            OutputView {
                text: text.to_owned(),
                source_lines: (0..text.lines().count()).collect(),
            },
            width,
        )
    }

    fn row_texts(layout: &OutputLayout) -> Vec<&str> {
        layout
            .rows()
            .iter()
            .map(|row| layout.row_text(row))
            .collect()
    }

    #[test]
    fn wraps_logs_without_discarding_whitespace_or_long_word_bytes() {
        let layout = layout("abc defghi", 5);

        assert_eq!(row_texts(&layout), vec!["abc ", "defgh", "i"]);
        assert_eq!(row_texts(&layout).concat(), "abc defghi");
    }

    #[test]
    fn wraps_only_on_extended_grapheme_boundaries_and_cell_widths() {
        let layout = layout("a\u{301}界b", 2);

        assert_eq!(row_texts(&layout), vec!["a\u{301}", "界", "b"]);
    }

    #[test]
    fn maps_positions_and_match_ranges_to_visual_rows() {
        let layout = layout("0123456789", 4);

        assert_eq!(
            layout.row_for_position(OutputPosition {
                source_line: 0,
                byte_offset: 6,
            }),
            Some(1)
        );
        assert_eq!(layout.row_range_for_source_bytes(0, 3..9), Some(0..3));
    }

    #[test]
    fn hidden_anchor_chooses_the_next_visible_source_line_then_previous() {
        let view = OutputView {
            text: "two\nfour".to_owned(),
            source_lines: vec![2, 4],
        };
        let layout = OutputLayout::new(view, 20);

        assert_eq!(
            layout.row_for_position(OutputPosition {
                source_line: 3,
                byte_offset: 0,
            }),
            Some(1)
        );
        assert_eq!(
            layout.row_for_position(OutputPosition {
                source_line: 9,
                byte_offset: 0,
            }),
            Some(1)
        );
    }
}
