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
