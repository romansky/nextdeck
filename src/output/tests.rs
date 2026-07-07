use super::*;

#[test]
fn captured_text_shows_stdout_without_metadata_headers() {
    let output = TestOutput {
        stdout: "hello\nworld".to_owned(),
        stderr: String::new(),
        duration: Some(Duration::from_millis(183_940)),
    };

    assert_eq!(output.captured_text(), "hello\nworld\n");
}

#[test]
fn display_text_keeps_detailed_metadata_headers() {
    let output = TestOutput {
        stdout: "hello".to_owned(),
        stderr: String::new(),
        duration: Some(Duration::from_millis(7)),
    };

    let text = output.display_text();
    assert!(text.contains("duration:"));
    assert!(text.contains("stdout\nhello"));
}

#[test]
fn bounded_text_keeps_tail_at_utf8_boundary() {
    let mut text = "alpha".to_owned();

    append_bounded_text_with_limit(&mut text, "βeta-gamma-delta", 24);

    assert!(text.len() <= 24);
    assert!(text.contains("delta"));
    assert!(text.is_char_boundary(text.len()));
}

#[test]
fn bounded_text_uses_marker_when_limit_allows_it() {
    let mut text = "a".repeat(80);

    append_bounded_text_with_limit(&mut text, "tail", 64);

    assert!(text.starts_with("[... output truncated"));
    assert!(text.ends_with("tail"));
}
