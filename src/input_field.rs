use crate::output_pane::{SearchEditorInput, SearchEditorKey};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputField {
    text: String,
    cursor: usize,
}

impl InputField {
    pub fn set_text(&mut self, text: &str) {
        self.text = text.replace('\n', " ");
        self.cursor = self.text.chars().count();
    }

    pub fn text(&self) -> String {
        self.text.clone()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn input(&mut self, input: SearchEditorInput) -> bool {
        match input.key {
            SearchEditorKey::Char(char) => self.insert_char(char),
            SearchEditorKey::Backspace => self.backspace(),
            SearchEditorKey::Delete => self.delete(),
            SearchEditorKey::Left => self.move_left(),
            SearchEditorKey::Right => self.move_right(),
            SearchEditorKey::Home => self.move_home(),
            SearchEditorKey::End => self.move_end(),
            SearchEditorKey::Enter
            | SearchEditorKey::Up
            | SearchEditorKey::Down
            | SearchEditorKey::Tab
            | SearchEditorKey::PageUp
            | SearchEditorKey::PageDown => false,
        }
    }

    pub fn view(&self, width: usize, active: bool) -> String {
        if width == 0 {
            return String::new();
        }
        let cursor = if active { Some(self.cursor) } else { None };
        let content = fit_content_around_cursor(&self.text, cursor, width);
        format!("{content:<width$}")
    }

    fn insert_char(&mut self, char: char) -> bool {
        if char == '\n' || char == '\r' {
            return false;
        }
        let index = byte_index_for_char(&self.text, self.cursor);
        self.text.insert(index, char);
        self.cursor += 1;
        true
    }

    fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let start = byte_index_for_char(&self.text, self.cursor - 1);
        let end = byte_index_for_char(&self.text, self.cursor);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
        true
    }

    fn delete(&mut self) -> bool {
        if self.cursor >= self.text.chars().count() {
            return false;
        }
        let start = byte_index_for_char(&self.text, self.cursor);
        let end = byte_index_for_char(&self.text, self.cursor + 1);
        self.text.replace_range(start..end, "");
        true
    }

    fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    fn move_right(&mut self) -> bool {
        if self.cursor >= self.text.chars().count() {
            return false;
        }
        self.cursor += 1;
        true
    }

    fn move_home(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor = 0;
        true
    }

    fn move_end(&mut self) -> bool {
        let end = self.text.chars().count();
        if self.cursor == end {
            return false;
        }
        self.cursor = end;
        true
    }
}

fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn fit_content_around_cursor(text: &str, cursor: Option<usize>, width: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let cursor = cursor.map(|cursor| cursor.min(chars.len()));
    let mut display = Vec::with_capacity(chars.len() + usize::from(cursor.is_some()));
    let mut cursor_display_index = None;
    for (index, char) in chars.into_iter().enumerate() {
        if cursor == Some(index) {
            cursor_display_index = Some(display.len());
            display.push('_');
        }
        display.push(char);
    }
    if cursor == Some(display.len()) {
        cursor_display_index = Some(display.len());
        display.push('_');
    }
    if display.len() <= width {
        return display.into_iter().collect();
    }

    let cursor_index = cursor_display_index.unwrap_or(display.len().saturating_sub(1));
    let start = cursor_index.saturating_add(1).saturating_sub(width);
    display.into_iter().skip(start).take(width).collect()
}

#[cfg(test)]
mod tests {
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
        input.input(SearchEditorInput::new(
            SearchEditorKey::Left,
            false,
            false,
            false,
        ));
        input.input(SearchEditorInput::char('X'));

        assert_eq!(input.text(), "ideXa");
        assert_eq!(input.view(8, true), "ideX_a  ");
    }

    #[test]
    fn deletes_around_cursor() {
        let mut input = InputField::default();
        input.set_text("abcd");
        input.input(SearchEditorInput::new(
            SearchEditorKey::Left,
            false,
            false,
            false,
        ));
        input.input(SearchEditorInput::new(
            SearchEditorKey::Backspace,
            false,
            false,
            false,
        ));
        input.input(SearchEditorInput::new(
            SearchEditorKey::Delete,
            false,
            false,
            false,
        ));

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
        input.input(SearchEditorInput::new(
            SearchEditorKey::Home,
            false,
            false,
            false,
        ));
        assert_eq!(input.view(6, true), "_idea ");

        input.input(SearchEditorInput::new(
            SearchEditorKey::End,
            false,
            false,
            false,
        ));
        assert_eq!(input.view(6, true), "idea_ ");
    }
}
