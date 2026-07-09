use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

pub(in crate::ui) fn modal_inner_area(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    centered_rect(percent_x, percent_y, area).inner(Margin {
        horizontal: 1,
        vertical: 1,
    })
}

pub(in crate::ui) fn panel_body_page_size(area: Rect) -> usize {
    area.height.saturating_sub(2).max(1) as usize
}

pub(in crate::ui) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
