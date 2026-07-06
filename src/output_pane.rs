use regex::{Regex, RegexBuilder};
use tui_textarea::{Input as TextAreaInput, Key as TextAreaKey, TextArea};

use crate::symbols::bool_symbol;

#[derive(Clone, Debug, Default)]
pub struct OutputSearchState {
    pub input_active: bool,
    pub modal_open: bool,
    pub query: String,
    pub draft_query: String,
    pub editor: SearchEditor,
    pub filter: bool,
    pub draft_filter: bool,
    pub regex: bool,
    pub draft_regex: bool,
    pub case_sensitive: bool,
    pub draft_case_sensitive: bool,
    pub modal_focus: SearchModalFocus,
    pub current_line: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputView {
    pub text: String,
    pub source_lines: Vec<usize>,
}

impl OutputView {
    pub fn line_index_for_source_line(&self, source_line: usize) -> Option<usize> {
        self.source_lines
            .iter()
            .position(|line| *line == source_line)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SearchModalFocus {
    #[default]
    Query,
    Clear,
    Apply,
    Filter,
    Regex,
    CaseSensitive,
}

impl SearchModalFocus {
    pub fn next(self) -> Self {
        match self {
            Self::Query => Self::Clear,
            Self::Clear => Self::Apply,
            Self::Apply => Self::Filter,
            Self::Filter => Self::Regex,
            Self::Regex => Self::CaseSensitive,
            Self::CaseSensitive => Self::Query,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Query => Self::CaseSensitive,
            Self::Clear => Self::Query,
            Self::Apply => Self::Clear,
            Self::Filter => Self::Apply,
            Self::Regex => Self::Filter,
            Self::CaseSensitive => Self::Regex,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchBoxView {
    pub box_text: String,
    pub match_summary: Option<(usize, usize)>,
    pub input_active: bool,
    pub filter: bool,
    pub regex: bool,
    pub case_sensitive: bool,
    pub invalid: bool,
}

impl SearchBoxView {
    pub fn title_fragment(&self) -> String {
        let summary = self
            .match_summary
            .map(|(current, total)| format!(" {current}/{total}"))
            .unwrap_or_else(|| " 0/0".to_owned());
        let invalid = if self.invalid { " !regex" } else { "" };
        let input_actions = if self.input_active {
            " [enter]submit [C+enter]advanced"
        } else {
            ""
        };
        format!(
            "<search: {}{}{}{} [n]ext [f]ilter:{} [r]egex:{} [c]ase-sensitive:{}>",
            self.box_text,
            summary,
            invalid,
            input_actions,
            bool_symbol(self.filter),
            bool_symbol(self.regex),
            bool_symbol(self.case_sensitive)
        )
    }
}

const SEARCH_BOX_WIDTH: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchDirection {
    Next,
    Previous,
}

#[derive(Clone, Debug)]
pub struct SearchEditor {
    textarea: TextArea<'static>,
}

impl Default for SearchEditor {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl SearchEditor {
    pub fn from_text(text: &str) -> Self {
        let mut textarea = TextArea::new(search_editor_lines(text));
        textarea.set_max_histories(100);
        textarea.set_tab_length(2);
        Self { textarea }
    }

    pub fn set_text(&mut self, text: &str) {
        *self = Self::from_text(text);
    }

    pub fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn clear(&mut self) {
        self.set_text("");
    }

    pub fn input(&mut self, input: SearchEditorInput) -> bool {
        self.textarea.input(TextAreaInput::from(input))
    }

    pub fn widget(&self) -> TextArea<'static> {
        self.textarea.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchEditorInput {
    pub key: SearchEditorKey,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl SearchEditorInput {
    pub const fn new(key: SearchEditorKey, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self {
            key,
            ctrl,
            alt,
            shift,
        }
    }

    pub const fn char(char: char) -> Self {
        Self::new(SearchEditorKey::Char(char), false, false, false)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchEditorKey {
    Char(char),
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Tab,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
}

impl OutputSearchState {
    pub fn view(&self, text: &str) -> SearchBoxView {
        SearchBoxView {
            box_text: self.box_text(SEARCH_BOX_WIDTH),
            match_summary: if self.query.is_empty() {
                None
            } else {
                self.match_summary(text)
            },
            input_active: self.input_active,
            filter: self.filter,
            regex: self.regex,
            case_sensitive: self.case_sensitive,
            invalid: self.error().is_some(),
        }
    }

    pub fn box_text(&self, width: usize) -> String {
        let mut content = if self.input_active || self.modal_open {
            self.draft_query.clone()
        } else {
            self.query.clone()
        };
        if self.input_active {
            content.push('_');
        }
        let content = fit_search_content(&content, width);
        format!("[{content:<width$}]")
    }

    pub fn prompt(&self) -> String {
        format!("Output search: {}", self.draft_query)
    }

    pub fn sync_draft_from_applied(&mut self) {
        self.draft_query = self.query.clone();
        self.editor.set_text(&self.draft_query);
        self.draft_filter = self.filter;
        self.draft_regex = self.regex;
        self.draft_case_sensitive = self.case_sensitive;
    }

    pub fn apply_draft(&mut self) {
        self.query.clone_from(&self.draft_query);
        self.filter = self.draft_filter;
        self.regex = self.draft_regex;
        self.case_sensitive = self.draft_case_sensitive;
    }

    pub fn clear_draft(&mut self) {
        self.draft_query.clear();
        self.editor.clear();
    }

    pub fn edit_draft(&mut self, input: SearchEditorInput) -> bool {
        let changed = self.editor.input(input);
        self.draft_query = self.editor.text();
        changed
    }

    pub fn error(&self) -> Option<String> {
        output_matcher(self).err()
    }

    pub fn filtered_view(&self, text: &str) -> OutputView {
        if !self.filter || self.query.is_empty() {
            return output_view_from_text(text);
        }

        let matcher = match output_matcher(self) {
            Ok(Some(matcher)) => matcher,
            Ok(None) => return output_view_from_text(text),
            Err(error) => {
                return OutputView {
                    text: format!("Invalid output search regex: {error}"),
                    source_lines: Vec::new(),
                };
            }
        };

        let matches = text
            .lines()
            .enumerate()
            .filter_map(|(index, line)| matcher.is_match(line).then_some((index, line)))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            OutputView {
                text: format!("No output lines match '{}'", self.query),
                source_lines: Vec::new(),
            }
        } else {
            OutputView {
                text: matches
                    .iter()
                    .map(|(_, line)| *line)
                    .collect::<Vec<_>>()
                    .join("\n"),
                source_lines: matches.iter().map(|(index, _)| *index).collect(),
            }
        }
    }

    pub fn match_lines(&self, text: &str) -> Result<Vec<usize>, String> {
        let Some(matcher) = output_matcher(self)? else {
            return Ok(Vec::new());
        };
        Ok(text
            .lines()
            .enumerate()
            .filter_map(|(index, line)| matcher.is_match(line).then_some(index))
            .collect())
    }

    pub fn match_ranges(&self, line: &str) -> Result<Vec<(usize, usize)>, String> {
        let Some(regex) = output_regex(self)? else {
            return Ok(Vec::new());
        };
        Ok(regex
            .find_iter(line)
            .filter_map(|matched| {
                (matched.start() < matched.end()).then_some((matched.start(), matched.end()))
            })
            .collect())
    }

    pub fn match_summary(&self, text: &str) -> Option<(usize, usize)> {
        let matches = self.match_lines(text).ok()?;
        if matches.is_empty() {
            return Some((0, 0));
        }
        let current = self
            .current_line
            .and_then(|line| matches.iter().position(|match_line| *match_line == line))
            .map(|index| index + 1)
            .unwrap_or(0);
        Some((current, matches.len()))
    }

    pub fn next_match(
        &self,
        text: &str,
        direction: SearchDirection,
    ) -> Result<Option<OutputMatch>, String> {
        let matches = self.match_lines(text)?;
        if matches.is_empty() {
            return Ok(None);
        }

        let current = self.current_line;
        let index = match direction {
            SearchDirection::Next => matches
                .iter()
                .position(|line| current.is_none_or(|current| *line > current))
                .unwrap_or(0),
            SearchDirection::Previous => matches
                .iter()
                .rposition(|line| current.is_none_or(|current| *line < current))
                .unwrap_or(matches.len() - 1),
        };

        Ok(Some(OutputMatch {
            line: matches[index],
            index,
            total: matches.len(),
        }))
    }
}

fn output_view_from_text(text: &str) -> OutputView {
    OutputView {
        text: text.to_owned(),
        source_lines: (0..text.lines().count()).collect(),
    }
}

impl From<SearchEditorInput> for TextAreaInput {
    fn from(input: SearchEditorInput) -> Self {
        Self {
            key: TextAreaKey::from(input.key),
            ctrl: input.ctrl,
            alt: input.alt,
            shift: input.shift,
        }
    }
}

impl From<SearchEditorKey> for TextAreaKey {
    fn from(key: SearchEditorKey) -> Self {
        match key {
            SearchEditorKey::Char(char) => Self::Char(char),
            SearchEditorKey::Backspace => Self::Backspace,
            SearchEditorKey::Enter => Self::Enter,
            SearchEditorKey::Left => Self::Left,
            SearchEditorKey::Right => Self::Right,
            SearchEditorKey::Up => Self::Up,
            SearchEditorKey::Down => Self::Down,
            SearchEditorKey::Tab => Self::Tab,
            SearchEditorKey::Delete => Self::Delete,
            SearchEditorKey::Home => Self::Home,
            SearchEditorKey::End => Self::End,
            SearchEditorKey::PageUp => Self::PageUp,
            SearchEditorKey::PageDown => Self::PageDown,
        }
    }
}

fn search_editor_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    text.split('\n').map(ToOwned::to_owned).collect()
}

fn fit_search_content(content: &str, width: usize) -> String {
    let char_count = content.chars().count();
    if char_count <= width {
        return content.to_owned();
    }
    content
        .chars()
        .skip(char_count.saturating_sub(width))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputMatch {
    pub line: usize,
    pub index: usize,
    pub total: usize,
}

enum OutputMatcher {
    Literal {
        needle: String,
        case_sensitive: bool,
    },
    Regex(Regex),
}

impl OutputMatcher {
    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Literal {
                needle,
                case_sensitive,
            } if *case_sensitive => line.contains(needle),
            Self::Literal { needle, .. } => line.to_lowercase().contains(needle),
            Self::Regex(regex) => regex.is_match(line),
        }
    }
}

fn output_matcher(search: &OutputSearchState) -> Result<Option<OutputMatcher>, String> {
    if search.query.is_empty() {
        return Ok(None);
    }

    if search.regex {
        RegexBuilder::new(&search.query)
            .case_insensitive(!search.case_sensitive)
            .build()
            .map(OutputMatcher::Regex)
            .map(Some)
            .map_err(|error| error.to_string())
    } else {
        let needle = if search.case_sensitive {
            search.query.clone()
        } else {
            search.query.to_lowercase()
        };
        Ok(Some(OutputMatcher::Literal {
            needle,
            case_sensitive: search.case_sensitive,
        }))
    }
}

fn output_regex(search: &OutputSearchState) -> Result<Option<Regex>, String> {
    if search.query.is_empty() {
        return Ok(None);
    }

    let pattern = if search.regex {
        search.query.clone()
    } else {
        regex::escape(&search.query)
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!search.case_sensitive)
        .build()
        .map(Some)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
