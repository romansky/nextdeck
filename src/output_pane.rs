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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchDirection {
    Next,
    Previous,
}

impl OutputSearchState {
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
}
