use std::time::Duration;

use nextdeck_test_events::Level;

pub(crate) const OUTPUT_TEXT_LIMIT_BYTES: usize = 512 * 1024;
const OUTPUT_TRUNCATED_MARKER: &str = "[... output truncated; showing tail ...]\n";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestOutput {
    pub entries: Vec<TestOutputEntry>,
    pub duration: Option<Duration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestOutputEntry {
    Text(String),
    Event { level: Level, text: String },
}

impl TestOutput {
    pub fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(TestOutputEntry::Text(existing)) = self.entries.last_mut() {
            append_output_text(existing, text);
        } else {
            self.entries
                .push(TestOutputEntry::Text(bounded_text(text.to_owned())));
        }
    }

    pub fn append_event(&mut self, level: Level, text: &str) {
        if text.is_empty() {
            return;
        }
        self.entries.push(TestOutputEntry::Event {
            level,
            text: bounded_text(text.to_owned()),
        });
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
        let mut text = String::new();
        for entry in &self.entries {
            append_output_text(&mut text, entry.rendered_text());
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text
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
