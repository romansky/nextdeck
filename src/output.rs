use std::time::Duration;

pub(crate) const OUTPUT_TEXT_LIMIT_BYTES: usize = 512 * 1024;
const OUTPUT_TRUNCATED_MARKER: &str = "[... output truncated; showing tail ...]\n";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestOutput {
    pub stdout: String,
    pub stderr: String,
    pub duration: Option<Duration>,
}

impl TestOutput {
    pub fn captured_text(&self) -> String {
        let mut text = String::new();
        if !self.stdout.is_empty() {
            text.push_str(&self.stdout);
            text.push('\n');
        }
        if !self.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&self.stderr);
            text.push('\n');
        }
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
        if !self.stdout.is_empty() {
            text.push_str("stdout\n");
            text.push_str(&self.stdout);
            text.push('\n');
        }
        if !self.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("stderr\n");
            text.push_str(&self.stderr);
            text.push('\n');
        }
        if text.trim().is_empty() {
            "No output captured".to_owned()
        } else {
            text
        }
    }
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
