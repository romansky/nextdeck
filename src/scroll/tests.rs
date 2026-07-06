    use super::*;

    #[test]
    fn ensure_visible_scrolls_down_to_selection() {
        assert_eq!(ensure_visible(0, 9, 20, 5), 5);
    }

    #[test]
    fn ensure_visible_scrolls_up_to_selection() {
        assert_eq!(ensure_visible(8, 3, 20, 5), 3);
    }

    #[test]
    fn ensure_visible_clamps_to_bottom() {
        assert_eq!(ensure_visible(99, 19, 20, 5), 15);
    }

    #[test]
    fn scrolling_clamps_to_content() {
        assert_eq!(up(3, 10), 0);
        assert_eq!(down(0, 50, 20, 5), 15);
    }
