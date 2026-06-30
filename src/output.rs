use std::time::Duration;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestOutput {
    pub stdout: String,
    pub stderr: String,
    pub duration: Option<Duration>,
}

impl TestOutput {
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
