    use super::*;

    #[test]
    fn set_text_places_cursor_at_end() {
        let mut input = InputField::default();

        input.set_text("idea");

        assert_eq!(input.view(8, true), "idea_   ");
    }

    #[test]
    fn edits_at_cursor_position() {
        let mut input = InputField::default();
        input.set_text("idea");
        input.input(InputFieldInput::new(InputFieldKey::Left));
        input.input(InputFieldInput::char('X'));

        assert_eq!(input.text(), "ideXa");
        assert_eq!(input.view(8, true), "ideX_a  ");
    }

    #[test]
    fn deletes_around_cursor() {
        let mut input = InputField::default();
        input.set_text("abcd");
        input.input(InputFieldInput::new(InputFieldKey::Left));
        input.input(InputFieldInput::new(InputFieldKey::Backspace));
        input.input(InputFieldInput::new(InputFieldKey::Delete));

        assert_eq!(input.text(), "ab");
        assert_eq!(input.view(6, true), "ab_   ");
    }

    #[test]
    fn view_keeps_cursor_visible_for_long_text() {
        let mut input = InputField::default();
        input.set_text("abcdefghijklmnopqrstuvwxyz");

        assert_eq!(input.view(8, true), "tuvwxyz_");
    }

    #[test]
    fn home_and_end_move_cursor() {
        let mut input = InputField::default();
        input.set_text("idea");
        input.input(InputFieldInput::new(InputFieldKey::Home));
        assert_eq!(input.view(6, true), "_idea ");

        input.input(InputFieldInput::new(InputFieldKey::End));
        assert_eq!(input.view(6, true), "idea_ ");
    }
