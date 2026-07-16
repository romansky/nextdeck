use regex::{Regex, RegexBuilder};

use crate::{
    input_field::{InputField, InputFieldInput},
    output_layout::{OutputLayout, OutputPosition},
    scroll,
    symbols::bool_symbol,
};

#[derive(Clone, Debug, Default)]
pub struct OutputSearchState {
    pub input_active: bool,
    pub modal_open: bool,
    pub query: String,
    editor: InputField,
    pub filter: bool,
    pub draft_filter: bool,
    pub regex: bool,
    pub draft_regex: bool,
    pub case_sensitive: bool,
    pub draft_case_sensitive: bool,
    pub modal_focus: SearchModalFocus,
    pub current_line: Option<usize>,
    pub current_range: Option<(usize, usize)>,
}

const DEFAULT_CONTENT_WIDTH: usize = 4096;

#[derive(Clone, Debug)]
pub struct OutputPaneState {
    viewport: scroll::FollowViewportState,
    content_width: usize,
    layout: OutputLayout,
    layout_initialized: bool,
    #[cfg(test)]
    layout_build_count: usize,
    pub search: OutputSearchState,
}

impl Default for OutputPaneState {
    fn default() -> Self {
        Self {
            viewport: scroll::FollowViewportState::default(),
            content_width: DEFAULT_CONTENT_WIDTH,
            layout: OutputLayout::default(),
            layout_initialized: false,
            #[cfg(test)]
            layout_build_count: 0,
            search: OutputSearchState::default(),
        }
    }
}

impl OutputPaneState {
    pub fn output_view(&self, source_text: &str) -> OutputView {
        self.search.filtered_view(source_text)
    }

    pub fn status(&self, label: &str) -> String {
        let total = self.layout.row_count();
        let top = self.scroll();
        let visible = self.page_size();
        let bottom = top.saturating_add(visible).min(total);
        format!(
            "{label} <#{}-{bottom}/{total}> [s]nap-bottom:{}",
            top + 1,
            bool_symbol(self.follow())
        )
    }

    pub fn search_actions(&self, source_text: &str) -> String {
        self.search.view(source_text).actions_fragment()
    }

    pub fn scroll(&self) -> usize {
        self.viewport.viewport().scroll()
    }

    pub fn page_size(&self) -> usize {
        self.viewport.page_size()
    }

    pub fn content_width(&self) -> usize {
        self.content_width
    }

    pub fn follow(&self) -> bool {
        self.viewport.follow()
    }

    pub fn layout(&self) -> &OutputLayout {
        &self.layout
    }

    pub fn apply_viewport_page_size(&mut self, page_size: usize) {
        self.viewport.set_page_size(page_size);
    }

    pub fn set_content_width(&mut self, content_width: usize) {
        self.content_width = content_width.max(1);
    }

    #[cfg(test)]
    pub fn apply_content_len(&mut self, line_count: usize) {
        self.viewport.set_content_len(line_count);
    }

    #[cfg(test)]
    pub fn apply_viewport_metrics(&mut self, page_size: usize, line_count: usize) {
        self.viewport.set_metrics(page_size, line_count);
    }

    pub fn apply_viewport_geometry(
        &mut self,
        page_size: usize,
        content_width: usize,
        source_text: &str,
    ) {
        self.content_width = content_width.max(1);
        self.prepare_layout(source_text, page_size);
    }

    pub fn sync_layout(&mut self, source_text: &str) -> usize {
        self.prepare_layout(source_text, self.viewport.page_size());
        self.layout.row_count()
    }

    fn prepare_layout(&mut self, source_text: &str, page_size: usize) {
        let anchor = (self.layout_initialized && !self.follow())
            .then(|| self.layout.top_position(self.scroll()))
            .flatten();
        let view = self.output_view(source_text);
        if !self.layout.matches(&view, self.content_width) {
            self.layout = OutputLayout::new(view, self.content_width);
            self.layout_initialized = true;
            #[cfg(test)]
            {
                self.layout_build_count += 1;
            }
        }

        self.viewport
            .set_metrics(page_size, self.layout.row_count());
        if !self.follow()
            && let Some(anchor) = anchor
            && let Some(row) = self.layout.row_for_position(anchor)
        {
            self.viewport.set_scroll(row);
        }
    }

    pub fn apply_scroll(&mut self, action: scroll::ScrollAction, line_count: usize) {
        self.viewport.set_content_len(line_count);
        self.viewport.apply_scroll(action);
    }

    pub fn disable_snap(&mut self) {
        self.viewport.disable_follow();
    }

    #[cfg(test)]
    pub fn set_follow(&mut self, follow: bool) {
        self.viewport.set_follow(follow);
    }

    pub fn toggle_snap(&mut self, line_count: usize) -> bool {
        self.viewport.toggle_follow(line_count)
    }

    #[cfg(test)]
    pub fn set_scroll(&mut self, scroll: usize) {
        self.viewport.set_scroll(scroll);
    }

    pub fn top_position(&self) -> Option<OutputPosition> {
        self.layout.top_position(self.scroll())
    }

    pub fn restore_position(&mut self, position: OutputPosition) {
        if let Some(row) = self.layout.row_for_position(position) {
            self.viewport.set_scroll(row);
        }
    }

    pub fn ensure_source_range_visible(
        &mut self,
        source_line: usize,
        byte_range: std::ops::Range<usize>,
    ) {
        let Some(rows) = self
            .layout
            .row_range_for_source_bytes(source_line, byte_range)
        else {
            return;
        };
        let scroll = if rows.len() == 1 {
            scroll::ensure_visible(
                self.scroll(),
                rows.start,
                self.layout.row_count(),
                self.page_size(),
            )
        } else {
            scroll::ensure_range_visible(
                self.scroll(),
                rows.start,
                rows.len(),
                self.layout.row_count(),
                self.page_size(),
            )
        };
        self.viewport.set_scroll(scroll);
    }

    pub fn reset_for_source_change(&mut self) {
        self.viewport.reset_for_source_change();
        self.layout_initialized = false;
        self.search.clear_current_match();
    }

    pub fn reset_for_modal(&mut self) {
        self.viewport.reset_for_modal();
        self.layout_initialized = false;
        self.search.clear_current_match();
    }

    #[cfg(test)]
    pub fn layout_build_count(&self) -> usize {
        self.layout_build_count
    }
}

#[cfg(test)]
pub fn output_line_count(text: &str) -> usize {
    text.lines().count().max(1)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputView {
    pub text: String,
    pub source_lines: Vec<usize>,
}

impl OutputView {
    #[cfg(test)]
    pub fn line_count(&self) -> usize {
        output_line_count(&self.text)
    }

    #[cfg(test)]
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
    pub has_value: bool,
    pub match_summary: Option<(usize, usize)>,
    pub input_active: bool,
    pub filter: bool,
    pub regex: bool,
    pub case_sensitive: bool,
    pub invalid: bool,
}

impl SearchBoxView {
    pub fn actions_fragment(&self) -> String {
        if !self.has_value {
            return format!("[/]search<{}>", self.box_text);
        }

        let summary = self
            .match_summary
            .map(|(current, total)| format!(" {current}/{total}"))
            .unwrap_or_else(|| " 0/0".to_owned());
        let invalid = if self.invalid { " !regex" } else { "" };
        let input_actions = if self.input_active {
            " [enter]submit [shift+enter]advanced"
        } else {
            ""
        };
        let clear_action = if !self.input_active && self.has_value {
            " [C+u]clear"
        } else {
            ""
        };
        format!(
            "[/]search<{}{}{}{}{} [n/N]ext [f]ilter:{} [r]egex:{} [c]ase-sensitive:{}>",
            self.box_text,
            summary,
            invalid,
            input_actions,
            clear_action,
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

impl OutputSearchState {
    pub fn view(&self, text: &str) -> SearchBoxView {
        let field_value = if self.input_active || self.modal_open {
            self.draft_query()
        } else {
            &self.query
        };
        SearchBoxView {
            box_text: self.box_text(SEARCH_BOX_WIDTH),
            has_value: !field_value.is_empty(),
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
        let content = if self.input_active {
            self.editor.view(width, true)
        } else if self.modal_open {
            self.editor.view(width, false)
        } else {
            fit_search_content(&self.query, width)
        };
        format!("[{content:<width$}]")
    }

    pub fn prompt(&self) -> String {
        format!("Output search: {}", self.draft_query())
    }

    pub fn draft_query(&self) -> &str {
        self.editor.value()
    }

    pub fn draft_view(&self, width: usize, active: bool) -> String {
        self.editor.view(width, active)
    }

    pub fn sync_draft_from_applied(&mut self) {
        if self.editor.value() != self.query {
            self.editor.set_text(&self.query);
        }
        self.draft_filter = self.filter;
        self.draft_regex = self.regex;
        self.draft_case_sensitive = self.case_sensitive;
    }

    pub fn close_interaction(&mut self) {
        self.input_active = false;
        self.modal_open = false;
        self.modal_focus = SearchModalFocus::Query;
        self.sync_draft_from_applied();
    }

    pub fn apply_draft(&mut self) {
        self.query = self.editor.text();
        self.filter = self.draft_filter;
        self.regex = self.draft_regex;
        self.case_sensitive = self.draft_case_sensitive;
    }

    pub fn clear_draft(&mut self) {
        self.editor.clear();
    }

    pub fn clear_current_match(&mut self) {
        self.current_line = None;
        self.current_range = None;
    }

    pub fn set_current_match(&mut self, output_match: OutputMatch) {
        self.current_line = Some(output_match.line);
        self.current_range = Some((output_match.start, output_match.end));
    }

    pub fn edit_draft(&mut self, input: InputFieldInput) -> bool {
        self.editor.input(input)
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
        let matches = self.match_occurrences(text).ok()?;
        if matches.is_empty() {
            return Some((0, 0));
        }
        let current = self
            .current_line
            .and_then(|line| {
                matches.iter().position(|output_match| {
                    output_match.line == line
                        && self
                            .current_range
                            .is_none_or(|range| range == (output_match.start, output_match.end))
                })
            })
            .map(|index| index + 1)
            .unwrap_or(0);
        Some((current, matches.len()))
    }

    pub fn next_match(
        &self,
        text: &str,
        direction: SearchDirection,
    ) -> Result<Option<OutputMatch>, String> {
        let matches = self.match_occurrences(text)?;
        if matches.is_empty() {
            return Ok(None);
        }

        let index = match direction {
            SearchDirection::Next => {
                if let Some(current) = self.current_position() {
                    matches
                        .iter()
                        .position(|output_match| output_match.position() > current)
                        .unwrap_or(0)
                } else {
                    matches
                        .iter()
                        .position(|output_match| {
                            self.current_line
                                .is_none_or(|current| output_match.line > current)
                        })
                        .unwrap_or(0)
                }
            }
            SearchDirection::Previous => {
                if let Some(current) = self.current_position() {
                    matches
                        .iter()
                        .rposition(|output_match| output_match.position() < current)
                        .unwrap_or(matches.len() - 1)
                } else {
                    matches
                        .iter()
                        .rposition(|output_match| {
                            self.current_line
                                .is_none_or(|current| output_match.line < current)
                        })
                        .unwrap_or(matches.len() - 1)
                }
            }
        };

        Ok(Some(matches[index]))
    }

    pub fn match_occurrences(&self, text: &str) -> Result<Vec<OutputMatch>, String> {
        let Some(regex) = output_regex(self)? else {
            return Ok(Vec::new());
        };
        let positions = text
            .lines()
            .enumerate()
            .flat_map(|(line_index, line)| {
                regex.find_iter(line).filter_map(move |matched| {
                    (matched.start() < matched.end()).then_some(OutputMatchPosition {
                        line: line_index,
                        start: matched.start(),
                        end: matched.end(),
                    })
                })
            })
            .collect::<Vec<_>>();
        let total = positions.len();
        Ok(positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| OutputMatch {
                line: position.line,
                start: position.start,
                end: position.end,
                index,
                total,
            })
            .collect())
    }

    fn current_position(&self) -> Option<OutputMatchPosition> {
        let line = self.current_line?;
        let (start, end) = self.current_range?;
        Some(OutputMatchPosition { line, start, end })
    }
}

fn output_view_from_text(text: &str) -> OutputView {
    OutputView {
        text: text.to_owned(),
        source_lines: (0..text.lines().count()).collect(),
    }
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
    pub start: usize,
    pub end: usize,
    pub index: usize,
    pub total: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OutputMatchPosition {
    line: usize,
    start: usize,
    end: usize,
}

impl OutputMatch {
    fn position(self) -> OutputMatchPosition {
        OutputMatchPosition {
            line: self.line,
            start: self.start,
            end: self.end,
        }
    }
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
