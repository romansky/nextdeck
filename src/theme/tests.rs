    use super::*;

    #[test]
    fn forced_modes_select_expected_palettes() {
        assert_eq!(Theme::resolve(ThemeMode::Dark, false).text, Color::Gray);
        assert_eq!(Theme::resolve(ThemeMode::Light, false).text, Color::Black);
    }

    #[test]
    fn color_blind_mode_changes_status_colors() {
        let theme = Theme::resolve(ThemeMode::Dark, true);

        assert_eq!(theme.success, Color::Cyan);
        assert_eq!(theme.danger, Color::Magenta);
    }

    #[test]
    fn light_palette_uses_terminal_background_for_footer() {
        assert_eq!(Theme::light().footer_bg, Color::Reset);
    }

    #[test]
    fn panel_actions_follow_panel_focus_style() {
        let theme = Theme::dark();

        assert_eq!(theme.panel_actions(true), theme.title(true));
        assert_eq!(theme.panel_actions(false), theme.title(false));
    }

    #[test]
    fn panel_title_color_codes_boolean_symbols() {
        let theme = Theme::dark();
        let title = theme.panel_title("filters: ✓ ✗", true);

        let enabled = title
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "✓")
            .expect("enabled symbol");
        let disabled = title
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "✗")
            .expect("disabled symbol");

        assert_eq!(enabled.style.fg, Some(theme.success));
        assert_eq!(disabled.style.fg, Some(theme.danger));
    }
