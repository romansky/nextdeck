use super::*;

#[test]
fn ensure_visible_scrolls_down_to_selection() {
    assert_eq!(ensure_visible(0, 9, 20, 5), 7);
}

#[test]
fn ensure_visible_scrolls_up_to_selection() {
    assert_eq!(ensure_visible(8, 3, 20, 5), 1);
}

#[test]
fn ensure_visible_clamps_to_bottom() {
    assert_eq!(ensure_visible(99, 19, 20, 5), 15);
}

#[test]
fn ensure_visible_preserves_context_until_scrolloff_boundary() {
    assert_eq!(ensure_visible(5, 7, 20, 5), 5);
    assert_eq!(ensure_visible(5, 8, 20, 5), 6);
}

#[test]
fn ensure_visible_uses_zero_scrolloff_for_tiny_viewports() {
    assert_eq!(ensure_visible(0, 9, 20, 2), 8);
    assert_eq!(ensure_visible(8, 3, 20, 2), 3);
}

#[test]
fn scrolling_clamps_to_content() {
    assert_eq!(up(3, 10), 0);
    assert_eq!(down(0, 50, 20, 5), 15);
}

#[test]
fn selection_viewport_keeps_selected_item_visible() {
    let mut viewport = SelectionViewport::default();
    viewport.set_page_size(5);

    viewport.ensure_visible(9, 20);

    assert_eq!(viewport.scroll(), 7);
    assert_eq!(viewport.page_size(), 5);
}
