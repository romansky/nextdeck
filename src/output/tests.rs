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
fn display_text_keeps_runner_metadata_out_of_success_output() {
    let mut output = TestOutput {
        duration: Some(Duration::from_millis(7)),
        ..Default::default()
    };
    output.append_text("hello");

    let text = output.display_text();
    assert!(!text.contains("duration:"));
    assert!(text.contains("hello"));
    assert!(!text.contains("stdout"));
    assert!(!text.contains("stderr"));
}

#[test]
fn failed_output_has_a_separate_nextest_summary() {
    let mut output = TestOutput::default();
    output.append_text("panic from test");
    output.append_nextest_failure(Some(Duration::from_millis(7)));

    assert_eq!(
        output.display_text(),
        "panic from test\n\nnextest: failed after 7.00ms\n"
    );
    assert_eq!(output.summary_label(), "text 15 chars, nextest failure");
}

#[test]
fn late_captured_output_stays_before_the_nextest_summary() {
    let mut output = TestOutput::default();
    output.append_text("before failure");
    output.append_nextest_failure(None);
    output.append_event(
        nextdeck_test_events::Level::Error,
        "@ event error late checkpoint",
    );
    output.append_text("late stderr");

    assert_eq!(
        output.display_text(),
        concat!(
            "before failure\n",
            "@ event error late checkpoint\n",
            "late stderr\n\n",
            "nextest: failed\n",
        )
    );
}

#[test]
fn late_output_adds_separation_before_an_existing_summary() {
    let mut output = TestOutput::default();
    output.append_nextest_failure(None);
    output.append_text("late stderr");

    assert_eq!(output.display_text(), "late stderr\n\nnextest: failed\n");
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
    // When Nextdeck launches this test, these frames exercise ordering through
    // nextest's combined stdout/stderr capture.
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
