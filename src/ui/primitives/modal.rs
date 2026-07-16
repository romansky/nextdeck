use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::theme::Theme;

pub(in crate::ui) struct ModalChrome<'a> {
    pub(in crate::ui) title: &'a str,
    pub(in crate::ui) actions: Option<&'a str>,
}

pub(in crate::ui) fn draw_modal_lines(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: ModalChrome<'_>,
    lines: Vec<Line<'static>>,
) {
    let inner = draw_modal_shell(frame, theme, area, chrome);
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

pub(in crate::ui) fn draw_modal_shell(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: ModalChrome<'_>,
) -> Rect {
    let block = theme.panel_block(chrome.title, chrome.actions, true);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    inner
}

pub(in crate::ui) fn draw_modal_output_lines(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: ModalChrome<'_>,
    lines: Vec<Line<'static>>,
) {
    let inner = draw_modal_shell(frame, theme, area, chrome);
    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text());
    frame.render_widget(paragraph, inner);
}
