use super::*;

#[test]
fn captured_text_shows_stdout_without_metadata_headers() {
    let mut output = TestOutput {
        duration: Some(Duration::from_millis(183_940)),
        ..Default::default()
    };
    output.append_text("hello\nworld");

    assert_eq!(output.captured_text(), "hello\nworld\n");
}

#[test]
fn display_text_keeps_duration_metadata_without_stream_headers() {
    let mut output = TestOutput {
        duration: Some(Duration::from_millis(7)),
        ..Default::default()
    };
    output.append_text("hello");

    let text = output.display_text();
    assert!(text.contains("duration:"));
    assert!(text.contains("hello"));
    assert!(!text.contains("stdout"));
    assert!(!text.contains("stderr"));
}

#[test]
fn stream_interleaves_text_and_events_in_append_order() {
    let mut output = TestOutput::default();

    output.append_text("before");
    output.append_event(nextdeck_test_events::Level::Info, "@ event info cache hit");
    output.append_text("after");

    assert_eq!(
        output.captured_text(),
        "before\n@ event info cache hit\nafter\n"
    );
    assert_eq!(output.summary_label(), "text 11 chars, 1 event");
}

#[test]
fn interleaved_entries_share_one_retention_budget() {
    let mut output = TestOutput::default();
    output.append_text(&format!(
        "oldest\n{}",
        "a".repeat(OUTPUT_TEXT_LIMIT_BYTES / 2)
    ));
    output.append_event(
        nextdeck_test_events::Level::Info,
        &format!(
            "@ event info middle {}",
            "b".repeat(OUTPUT_TEXT_LIMIT_BYTES / 2)
        ),
    );
    output.append_text("newest");

    let captured = output.captured_text();
    assert!(captured.len() <= OUTPUT_TEXT_LIMIT_BYTES);
    assert!(captured.starts_with(OUTPUT_TRUNCATED_MARKER));
    assert!(!captured.contains("oldest"));
    assert!(captured.contains("newest"));
}

#[test]
fn interleaved_retention_trims_at_utf8_boundaries() {
    let mut output = TestOutput::default();
    output.append_event(
        nextdeck_test_events::Level::Info,
        &"β".repeat(OUTPUT_TEXT_LIMIT_BYTES),
    );
    output.append_text("tail");

    let captured = output.captured_text();
    assert!(captured.len() <= OUTPUT_TEXT_LIMIT_BYTES);
    assert!(captured.contains("tail"));
}

#[test]
fn adjacent_text_chunks_render_as_one_plain_stream() {
    let mut output = TestOutput::default();

    output.append_text("stdout line");
    output.append_text("stderr line");

    assert_eq!(output.captured_text(), "stdout line\nstderr line\n");
    assert_eq!(output.summary_label(), "text 23 chars");
}

#[test]
fn dogfood_output_captures_stdout_stderr_and_events() {
    // Nextest reports passing-test stdout/stderr as a captured block, so this
    // dogfood signal proves events are attached to test output, not that nextest
    // can provide line-level stdout/event ordering.
    println!("DOGFOOD_OUTPUT stdout before info event");
    nextdeck_test_events::event!(
        level: nextdeck_test_events::Level::Info,
        target: "dogfood-output",
        "stdout reached info event";
        "stream" => "stdout",
        "step" => 1,
    );
    println!("DOGFOOD_OUTPUT stdout after info event");
    eprintln!("DOGFOOD_OUTPUT stderr before warn event");
    nextdeck_test_events::event!(
        level: nextdeck_test_events::Level::Warn,
        target: "dogfood-output",
        "stderr reached warn event";
        "stream" => "stderr",
        "step" => 2,
    );
    println!("DOGFOOD_OUTPUT stdout after warn event");

    let mut output = TestOutput::default();
    output.append_text(concat!(
        "DOGFOOD_OUTPUT stdout before info event\n",
        "DOGFOOD_OUTPUT stdout after info event\n",
        "DOGFOOD_OUTPUT stderr before warn event\n",
        "DOGFOOD_OUTPUT stdout after warn event",
    ));
    output.append_event(
        nextdeck_test_events::Level::Info,
        "@ event info dogfood-output: stdout reached info event",
    );
    output.append_event(
        nextdeck_test_events::Level::Warn,
        "@ event warn dogfood-output: stderr reached warn event",
    );

    let captured = output.captured_text();
    for needle in [
        "DOGFOOD_OUTPUT stdout before info event",
        "DOGFOOD_OUTPUT stdout after info event",
        "DOGFOOD_OUTPUT stderr before warn event",
        "DOGFOOD_OUTPUT stdout after warn event",
        "@ event info dogfood-output: stdout reached info event",
        "@ event warn dogfood-output: stderr reached warn event",
    ] {
        assert!(captured.contains(needle));
    }
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
