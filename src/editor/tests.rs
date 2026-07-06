    use super::*;

    #[test]
    fn intellij_args_include_line_flag() {
        let mut args = Vec::new();
        append_editor_args("idea", &mut args, "/tmp/test.rs", 42);
        assert_eq!(args, vec!["--line", "42", "/tmp/test.rs"]);
    }

    #[test]
    fn vscode_args_use_goto_format() {
        let mut args = Vec::new();
        append_editor_args("code", &mut args, "/tmp/test.rs", 42);
        assert_eq!(args, vec!["-g", "/tmp/test.rs:42"]);
    }

    #[test]
    fn command_splitter_preserves_quoted_arguments() {
        let (program, args) = editor_command_parts(r#"open -a "IntelliJ IDEA""#);
        assert_eq!(program, "open");
        assert_eq!(args, vec!["-a", "IntelliJ IDEA"]);
    }

    #[test]
    fn editor_alias_maps_intellij_to_idea_cli() {
        let (program, args) = editor_command_parts("intellij");
        assert_eq!(program, "idea");
        assert!(args.is_empty());
    }
