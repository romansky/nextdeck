#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputField {
    text: String,
    cursor: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputFieldInput {
    pub key: InputFieldKey,
}

impl InputFieldInput {
    pub const fn new(key: InputFieldKey) -> Self {
        Self { key }
    }

    pub const fn char(char: char) -> Self {
        Self::new(InputFieldKey::Char(char))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFieldKey {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
}

impl InputField {
    pub fn set_text(&mut self, text: &str) {
        self.text = text.replace('\n', " ");
        self.cursor = self.text.chars().count();
    }

    pub fn text(&self) -> String {
        self.text.clone()
    }

    pub fn input(&mut self, input: InputFieldInput) -> bool {
        match input.key {
            InputFieldKey::Char(char) => self.insert_char(char),
            InputFieldKey::Backspace => self.backspace(),
            InputFieldKey::Delete => self.delete(),
            InputFieldKey::Left => self.move_left(),
            InputFieldKey::Right => self.move_right(),
            InputFieldKey::Home => self.move_home(),
            InputFieldKey::End => self.move_end(),
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
mod tests;
