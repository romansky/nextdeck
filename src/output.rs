use std::{collections::VecDeque, time::Duration};

use nextdeck_test_events::Level;

pub(crate) const OUTPUT_TEXT_LIMIT_BYTES: usize = 512 * 1024;
const OUTPUT_TRUNCATED_MARKER: &str = "[... output truncated; showing tail ...]\n";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestOutput {
    entries: VecDeque<TestOutputEntry>,
    retained_bytes: usize,
    truncated: bool,
    pub duration: Option<Duration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TestOutputEntry {
    Text(String),
    Event { level: Level, text: String },
}

impl TestOutput {
    pub fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let added = if let Some(TestOutputEntry::Text(existing)) = self.entries.back_mut() {
            let before = existing.len();
            append_stream_chunk(existing, text);
            existing.len() - before
        } else {
            let text = stream_chunk(text);
            let added = text.len();
            self.entries.push_back(TestOutputEntry::Text(text));
            added
        };
        self.retained_bytes += added;
        self.enforce_retention();
    }

    pub fn append_event(&mut self, level: Level, text: &str) {
        if text.is_empty() {
            return;
        }
        let text = stream_chunk(text);
        self.retained_bytes += text.len();
        self.entries
            .push_back(TestOutputEntry::Event { level, text });
        self.enforce_retention();
    }

    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn has_events(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| matches!(entry, TestOutputEntry::Event { .. }))
    }

    pub fn summary_label(&self) -> String {
        let text_len = self
            .entries
            .iter()
            .filter_map(TestOutputEntry::text)
            .map(|text| text.trim().len())
            .sum::<usize>();
        let event_count = self
            .entries
            .iter()
            .filter(|entry| matches!(entry, TestOutputEntry::Event { .. }))
            .count();
        match (text_len, event_count) {
            (0, 0) => "none captured".to_owned(),
            (_, 0) => format!("text {text_len} chars"),
            (0, 1) => "1 event".to_owned(),
            (0, _) => format!("{event_count} events"),
            (_, 1) => format!("text {text_len} chars, 1 event"),
            (_, _) => format!("text {text_len} chars, {event_count} events"),
        }
    }

    pub fn captured_text(&self) -> String {
        let text = self.stream_text();
        if text.trim().is_empty() {
            "No output captured".to_owned()
        } else {
            text
        }
    }

    pub fn display_text(&self) -> String {
        let mut text = String::new();
        if let Some(duration) = self.duration {
            text.push_str(&format!("duration: {:.2?}\n\n", duration));
        }
        append_output_text(&mut text, &self.stream_text());
        if text.trim().is_empty() {
            "No output captured".to_owned()
        } else {
            text
        }
    }

    fn stream_text(&self) -> String {
        let marker_len = if self.truncated {
            OUTPUT_TRUNCATED_MARKER.len()
        } else {
            0
        };
        let mut text = String::with_capacity(marker_len + self.retained_bytes);
        if self.truncated {
            text.push_str(OUTPUT_TRUNCATED_MARKER);
        }
        for entry in &self.entries {
            text.push_str(entry.rendered_text());
        }
        text
    }

    fn enforce_retention(&mut self) {
        if !self.truncated && self.retained_bytes <= OUTPUT_TEXT_LIMIT_BYTES {
            return;
        }
        self.truncated = true;
        let budget = OUTPUT_TEXT_LIMIT_BYTES.saturating_sub(OUTPUT_TRUNCATED_MARKER.len());
        while self.retained_bytes > budget {
            let excess = self.retained_bytes - budget;
            let Some(entry) = self.entries.front_mut() else {
                self.retained_bytes = 0;
                break;
            };
            let removed = entry.trim_front(excess);
            self.retained_bytes -= removed;
            if entry.rendered_text().is_empty() {
                self.entries.pop_front();
            }
        }
    }
}

impl TestOutputEntry {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Event { .. } => None,
        }
    }

    fn rendered_text(&self) -> &str {
        match self {
            Self::Text(text) | Self::Event { text, .. } => text,
        }
    }

    fn trim_front(&mut self, minimum_bytes: usize) -> usize {
        let text = match self {
            Self::Text(text) | Self::Event { text, .. } => text,
        };
        let mut end = minimum_bytes.min(text.len());
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        text.drain(..end);
        end
    }
}

fn stream_chunk(text: &str) -> String {
    let mut chunk = String::with_capacity(text.len() + 1);
    append_stream_chunk(&mut chunk, text);
    chunk
}

fn append_stream_chunk(target: &mut String, text: &str) {
    target.push_str(text);
    if !target.ends_with('\n') {
        target.push('\n');
    }
}

fn append_output_text(target: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !target.is_empty() && !target.ends_with('\n') {
        append_bounded_text(target, "\n");
    }
    append_bounded_text(target, text);
}

pub(crate) fn append_bounded_text(target: &mut String, text: &str) {
    target.push_str(text);
    limit_text(target, OUTPUT_TEXT_LIMIT_BYTES);
}

pub(crate) fn bounded_text(mut text: String) -> String {
    limit_text(&mut text, OUTPUT_TEXT_LIMIT_BYTES);
    text
}

pub(crate) fn bounded_text_with_limit(mut text: String, limit: usize) -> String {
    limit_text(&mut text, limit);
    text
}

pub(crate) fn bounded_output_section(prefix: &str, mut body: String) -> String {
    let section_limit = OUTPUT_TEXT_LIMIT_BYTES.saturating_sub(OUTPUT_TRUNCATED_MARKER.len());
    let mut prefix = prefix.to_owned();
    limit_text(&mut prefix, section_limit);
    let body_limit = section_limit.saturating_sub(prefix.len());
    limit_text(&mut body, body_limit);
    let mut section = String::with_capacity(prefix.len() + body.len());
    section.push_str(&prefix);
    section.push_str(&body);
    section
}

#[cfg(test)]
fn append_bounded_text_with_limit(target: &mut String, text: &str, limit: usize) {
    target.push_str(text);
    limit_text(target, limit);
}

fn limit_text(text: &mut String, limit: usize) {
    if text.len() <= limit {
        return;
    }
    if limit == 0 {
        text.clear();
        return;
    }
    let marker = if OUTPUT_TRUNCATED_MARKER.len() < limit {
        OUTPUT_TRUNCATED_MARKER
    } else {
        ""
    };
    let tail_limit = limit.saturating_sub(marker.len());
    let mut start = text.len().saturating_sub(tail_limit);
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = text[start..].to_owned();
    text.clear();
    text.push_str(marker);
    text.push_str(&tail);
}

#[cfg(test)]
mod tests;
