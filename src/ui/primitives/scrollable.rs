use ratatui::{
    layout::Alignment,
    text::Line,
    widgets::{Paragraph, Wrap},
};

use crate::{scroll::ViewportState, theme::Theme};

pub(in crate::ui) fn scrollable_paragraph(
    lines: Vec<Line<'static>>,
    theme: &Theme,
    viewport: &ViewportState,
) -> Paragraph<'static> {
    let scroll = viewport.render_scroll_for(lines.len().max(1));
    Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}
