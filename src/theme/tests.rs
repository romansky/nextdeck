    use super::*;

    #[test]
    fn forced_modes_select_expected_palettes() {
        assert_eq!(
            Theme::resolve(ThemeMode::Dark, false).text,
            Color::Rgb(205, 214, 244)
        );
        assert_eq!(
            Theme::resolve(ThemeMode::Light, false).text,
            Color::Rgb(76, 79, 105)
        );
    }

    #[test]
    fn color_blind_mode_changes_status_colors() {
        let theme = Theme::resolve(ThemeMode::Dark, true);

        assert_eq!(theme.success, Color::Cyan);
        assert_eq!(theme.danger, Color::Magenta);
    }

    #[test]
    fn light_palette_uses_soft_footer_background() {
        assert_eq!(Theme::light().footer_bg, Color::Rgb(230, 233, 239));
    }

    #[test]
    fn panel_actions_follow_panel_focus_style() {
        let theme = Theme::dark();

        assert_eq!(theme.panel_actions(true), theme.title(true));
        assert_eq!(theme.panel_actions(false), theme.title(false));
    }

    #[test]
    fn panel_title_uses_uniform_title_style_for_boolean_symbols() {
        let theme = Theme::dark();
        let title = theme.panel_title("filters: ✓ ✗", true);
        let title_style = theme.title(true);

        assert_eq!(title.spans.len(), 3);
        assert_eq!(title.spans[1].content.as_ref(), "filters: ✓ ✗");
        assert!(title.spans.iter().all(|span| span.style == title_style));
    }
