use regex::{Regex, RegexBuilder};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutputSearchState {
    pub input_active: bool,
    pub query: String,
    pub filter: bool,
    pub regex: bool,
    pub case_sensitive: bool,
    pub current_line: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchBoxView {
    pub box_text: String,
    pub match_summary: Option<(usize, usize)>,
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
        format!(
            "<search: {}{}{} [n]ext [f]ilter:{} [r]egex:{} [c]ase-sensitive:{}>",
            self.box_text,
            summary,
            invalid,
            on_off(self.filter),
            on_off(self.regex),
            on_off(self.case_sensitive)
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
        SearchBoxView {
            box_text: self.box_text(SEARCH_BOX_WIDTH),
            match_summary: if self.query.is_empty() {
                None
            } else {
                self.match_summary(text)
            },
            filter: self.filter,
            regex: self.regex,
            case_sensitive: self.case_sensitive,
            invalid: self.error().is_some(),
        }
    }

    pub fn box_text(&self, width: usize) -> String {
        let mut content = self.query.clone();
        if self.input_active {
            content.push('_');
        }
        let content = fit_search_content(&content, width);
        format!("[{content:<width$}]")
    }

    pub fn prompt(&self) -> String {
        format!("Output search: {}", self.query)
    }

    pub fn error(&self) -> Option<String> {
        output_matcher(self).err()
    }

    pub fn filtered_text(&self, text: &str) -> String {
        if !self.filter || self.query.is_empty() {
            return text.to_owned();
        }

        let matcher = match output_matcher(self) {
            Ok(Some(matcher)) => matcher,
            Ok(None) => return text.to_owned(),
            Err(error) => return format!("Invalid output search regex: {error}"),
        };

        let filtered = text
            .lines()
            .filter(|line| matcher.is_match(line))
            .collect::<Vec<_>>()
            .join("\n");
        if filtered.is_empty() {
            format!("No output lines match '{}'", self.query)
        } else {
            filtered
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

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
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
mod tests {
    use super::*;

    #[test]
    fn filters_literal_matches_case_insensitively_by_default() {
        let search = OutputSearchState {
            query: "panic".to_owned(),
            filter: true,
            ..OutputSearchState::default()
        };

        assert_eq!(search.filtered_text("ok\nPANIC\nfine"), "PANIC");
    }

    #[test]
    fn finds_next_and_previous_matches() {
        let mut search = OutputSearchState {
            query: "case".to_owned(),
            ..OutputSearchState::default()
        };

        let next = search
            .next_match("case_1\nother\ncase_2", SearchDirection::Next)
            .expect("valid search")
            .expect("match");
        assert_eq!(next.line, 0);
        assert_eq!(next.index, 0);
        assert_eq!(next.total, 2);

        search.current_line = Some(next.line);
        let previous = search
            .next_match("case_1\nother\ncase_2", SearchDirection::Previous)
            .expect("valid search")
            .expect("match");
        assert_eq!(previous.line, 2);
    }

    #[test]
    fn search_box_view_is_fixed_width_and_marks_active_input() {
        let search = OutputSearchState {
            query: "panic".to_owned(),
            input_active: true,
            ..OutputSearchState::default()
        };

        assert_eq!(search.box_text(18), "[panic_            ]");
        assert_eq!(search.box_text(18).len(), 20);
    }

    #[test]
    fn search_box_view_truncates_long_query_from_left() {
        let search = OutputSearchState {
            query: "abcdefghijklmnopqrstuvwxyz".to_owned(),
            ..OutputSearchState::default()
        };

        assert_eq!(search.box_text(18), "[ijklmnopqrstuvwxyz]");
    }

    #[test]
    fn search_box_view_marks_invalid_regex() {
        let search = OutputSearchState {
            query: "(".to_owned(),
            regex: true,
            ..OutputSearchState::default()
        };

        assert!(search.view("anything").invalid);
        assert!(search.view("anything").title_fragment().contains("!regex"));
    }

    #[test]
    fn match_ranges_find_literal_ranges_case_insensitively() {
        let search = OutputSearchState {
            query: "panic".to_owned(),
            ..OutputSearchState::default()
        };

        assert_eq!(
            search.match_ranges("PANIC then panic").expect("ranges"),
            vec![(0, 5), (11, 16)]
        );
    }

    #[test]
    fn match_ranges_find_regex_ranges() {
        let search = OutputSearchState {
            query: r"case_\d+".to_owned(),
            regex: true,
            ..OutputSearchState::default()
        };

        assert_eq!(
            search.match_ranges("case_01 case_aa case_22").expect("ranges"),
            vec![(0, 7), (16, 23)]
        );
    }
}
