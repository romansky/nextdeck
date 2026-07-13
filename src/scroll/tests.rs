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
fn viewport_keeps_selected_item_visible() {
    let mut viewport = ViewportState::default();
    viewport.set_metrics(5, 20);

    viewport.ensure_visible(9);

    assert_eq!(viewport.scroll(), 7);
    assert_eq!(viewport.page_size(), 5);
    assert_eq!(viewport.content_len(), 20);
}

#[test]
fn following_viewport_uses_complete_metrics_to_track_or_preserve_position() {
    let mut viewport = FollowViewportState::default();

    viewport.set_metrics(2, 5);

    assert!(viewport.follow());
    assert_eq!(viewport.viewport().scroll(), 3);

    viewport.apply_scroll(ScrollAction::PageUp);
    let manual_scroll = viewport.viewport().scroll();
    viewport.set_metrics(2, 8);

    assert!(!viewport.follow());
    assert_eq!(viewport.viewport().scroll(), manual_scroll);
}

#[test]
fn ensure_range_visible_keeps_multiline_item_in_view() {
    assert_eq!(ensure_range_visible(0, 7, 3, 20, 5), 5);
    assert_eq!(ensure_range_visible(5, 3, 3, 20, 5), 3);
}

#[test]
fn ensure_range_visible_anchors_oversized_item_to_start() {
    assert_eq!(ensure_range_visible(0, 7, 8, 20, 5), 7);
}
