    use super::*;
    use crate::disk_usage::{DiskUsageEntry, DiskUsageSnapshot};
    use crate::tree::{DiscoveredTest, TestKey, TestNode, TestStatus, Tree};
    use std::path::PathBuf;

    fn app_with_tree(tree: Tree) -> App {
        App::with_settings(tree, crate::config::AppSettings::default())
    }

    #[test]
    fn output_status_shows_all_when_text_fits() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;
        let text = "one\ntwo";

        assert_eq!(
            output_status(&app, text, text),
            "Output <lines: 1-2/2> <search: [            ] 0/0 [n]ext [f]ilter:✗ [r]egex:✗ [c]ase-sensitive:✗>"
        );
    }

    #[test]
    fn output_status_shows_clamped_line_ranges() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.output_page_size = 3;
        let text = "1\n2\n3\n4\n5\n6";

        app.output_scroll = 0;
        assert_eq!(
            output_status(&app, text, text),
            "Output <lines: 1-3/6> <search: [            ] 0/0 [n]ext [f]ilter:✗ [r]egex:✗ [c]ase-sensitive:✗>"
        );

        app.output_scroll = 2;
        assert_eq!(
            output_status(&app, text, text),
            "Output <lines: 3-5/6> <search: [            ] 0/0 [n]ext [f]ilter:✗ [r]egex:✗ [c]ase-sensitive:✗>"
        );

        app.output_scroll = 3;
        assert_eq!(
            output_status(&app, text, text),
            "Output <lines: 4-6/6> <search: [            ] 0/0 [n]ext [f]ilter:✗ [r]egex:✗ [c]ase-sensitive:✗>"
        );
    }

    #[test]
    fn filter_hint_includes_toggle_key() {
        assert_eq!(filter_hint("pass", "p", true), "[p]ass:✓");
        assert_eq!(filter_hint("fail", "f", false), "[f]ail:✗");
        assert_eq!(filter_hint("ignore", "i", false), "[i]gnore:✗");
    }

    #[test]
    fn tests_status_includes_filter_hints() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.tree.view_filter.show_ignored = false;

        assert_eq!(
            tests_status(&app),
            "Tests <filters: [p]ass:✓ [f]ail:✓ [i]gnore:✗ [s]kip:✓>"
        );
    }

    #[test]
    fn info_status_includes_disk_state() {
        let app = app_with_tree(Tree::from_tests(Vec::new()));

        assert_eq!(info_status(&app), "Info");
    }

    #[test]
    fn info_columns_keep_run_and_storage_details_separate() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.disk_usage.snapshot = Some(DiskUsageSnapshot {
            entries: vec![DiskUsageEntry {
                label: "target",
                path: PathBuf::from("target"),
                bytes: 1024,
            }],
            available_bytes: Some(2048),
            updated_at: std::time::UNIX_EPOCH,
        });
        app.settings.storage_low_space_threshold_gb = 1;

        let run_text = run_details(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");
        let storage_text = storage_details(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(footer_run_status(&app), app.run_status_label());
        assert!(run_text.contains("run id"));
        assert!(run_text.contains(app.run_status_label()));
        assert!(!run_text.contains("not running"));
        assert!(!run_text.contains("target"));
        assert!(storage_text.contains("Storage"));
        assert!(storage_text.contains("low"));
        assert!(storage_text.contains("available"));
        assert!(storage_text.contains("2.0 KiB"));
        assert!(storage_text.contains("1970-01-01 00:00:00 UTC"));
        assert!(storage_text.contains("target"));
        assert!(storage_text.contains("1.0 KiB"));
    }

    #[test]
    fn storage_status_reports_healthy_when_available_space_exceeds_threshold() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.disk_usage.snapshot = Some(DiskUsageSnapshot {
            entries: Vec::new(),
            available_bytes: Some(11 * 1024 * 1024 * 1024),
            updated_at: std::time::UNIX_EPOCH,
        });

        assert_eq!(storage_status(&app), "healthy");
    }

    #[test]
    fn disk_cleanup_modal_shows_detailed_target_row_without_summary_duplicate() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.disk_usage.snapshot = Some(DiskUsageSnapshot {
            entries: vec![DiskUsageEntry {
                label: "target",
                path: PathBuf::from("/workspace/target"),
                bytes: 1024,
            }],
            available_bytes: None,
            updated_at: std::time::UNIX_EPOCH,
        });

        let text = disk_cleanup_lines(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!text.contains("target 1.0 KiB\n"));
        assert!(text.contains("/workspace/target"));
    }

    #[test]
    fn settings_modal_includes_storage_and_duration_settings() {
        let app = app_with_tree(Tree::from_tests(Vec::new()));

        assert_eq!(
            settings_value(&app, SettingsField::TreeDuration),
            "wall"
        );
        assert_eq!(
            settings_value(&app, SettingsField::StorageThreshold),
            "10 GiB"
        );
    }

    #[test]
    fn footer_includes_run_and_storage_status_before_key() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.disk_usage.snapshot = Some(DiskUsageSnapshot {
            entries: Vec::new(),
            available_bytes: Some(11 * 1024 * 1024 * 1024),
            updated_at: std::time::UNIX_EPOCH,
        });

        let text = status_spans(&app, &Theme::dark())
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(text.contains(" | run idle | storage healthy | key "));
    }

    #[test]
    fn panel_actions_describe_local_commands() {
        assert_eq!(
            tests_actions(),
            "actions: [enter]details [r]un [R]failed [o]pen-editor [u]update"
        );
        assert_eq!(info_actions(), "actions: [d]disk-refresh [D]cleanup");
        assert_eq!(
            disk_cleanup_actions(),
            "actions: [c]cargo-clean [r]refresh [esc]close"
        );
        assert_eq!(
            output_actions(),
            "actions: [/]search [n]ext [N]prev [o]pen-editor"
        );
        assert_eq!(
            discovery_error_actions(),
            "actions: [u]retry [/]search [n]ext [N]prev [o]pen-editor [q]quit"
        );
    }

    #[test]
    fn pane_focus_is_suppressed_while_modal_is_visible() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.focus = FocusPane::Tree;
        assert!(pane_focused(&app, FocusPane::Tree));

        app.discovery.running = true;
        assert!(!pane_focused(&app, FocusPane::Tree));

        app.discovery.running = false;
        app.discovery.error = Some("boom".to_owned());
        assert!(!pane_focused(&app, FocusPane::Tree));

        app.discovery.error = None;
        app.show_help = true;
        app.focus = FocusPane::Output;
        assert!(!pane_focused(&app, FocusPane::Output));

        app.show_help = false;
        app.show_test_details = true;
        app.focus = FocusPane::Tree;
        assert!(!pane_focused(&app, FocusPane::Tree));
    }

    #[test]
    fn help_text_uses_contextual_sections() {
        let theme = Theme::dark();
        let text = help_text(&theme, FocusPane::Tree);
        let lines = text.iter().map(line_text).collect::<Vec<_>>();

        assert_eq!(lines.first(), Some(&format!("NextDeck {}", env!("CARGO_PKG_VERSION"))));
        assert!(lines.contains(&"Global".to_owned()));
        assert!(lines.contains(&"  Navigation".to_owned()));
        assert!(lines.contains(&"Tests".to_owned()));
        assert!(lines.contains(&"  Runs".to_owned()));
        assert!(lines.contains(&"  View".to_owned()));
        assert!(lines.contains(&"Output".to_owned()));
        assert!(lines.iter().any(|line| line.contains("h/?/F1")));
        assert!(lines.iter().any(|line| line.contains("q")));
    }

    #[test]
    fn help_text_sorts_commands_alpha_numerically_within_groups() {
        let theme = Theme::dark();
        let text = help_text(&theme, FocusPane::Tree);

        assert_help_order(
            &text,
            &[
                "open selected details",
                "first or last row",
                "collapse or expand",
                "page active pane",
                "narrow tests pane",
                "widen tests pane",
                "toggle selected branch",
                "switch tree/output focus",
                "move selection",
            ],
        );
        assert_help_order(
            &text,
            &[
                "open global settings",
                "stop running tests",
                "open disk cleanup",
                "refresh disk usage",
                "open or close help",
                "quit",
            ],
        );
        assert_help_order(
            &text,
            &[
                "next or previous failure",
                "open selected test source",
                "rerun failures",
                "run selected scope",
                "update test list",
            ],
        );
        assert_help_order(
            &text,
            &[
                "search output",
                "toggle output case sensitivity",
                "follow output bottom",
                "toggle output match filter",
                "next or previous output match",
                "open output as text file",
                "toggle output regex",
            ],
        );
    }

    #[test]
    fn help_text_dims_inactive_pane_commands() {
        let theme = Theme::dark();
        let tests_help = help_text(&theme, FocusPane::Tree);
        let output_help = help_text(&theme, FocusPane::Output);

        assert_eq!(
            help_line_with_label(&tests_help, "search output").spans[1].style,
            theme.muted()
        );
        assert_eq!(
            help_line_with_label(&tests_help, "run selected scope").spans[1].style,
            theme.accent()
        );
        assert_eq!(
            help_line_with_label(&output_help, "run selected scope").spans[1].style,
            theme.muted()
        );
        assert_eq!(
            help_line_with_label(&output_help, "search output").spans[1].style,
            theme.accent()
        );
    }

    #[test]
    fn tree_leading_fields_have_no_status_gap() {
        let tree = Tree::from_tests(vec![DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: "case".to_owned(),
            full_name: "tests::case".to_owned(),
            status: TestStatus::Pending,
            ignored: false,
        }]);

        assert_eq!(
            tree_leading_fields(0, &tree.root, config::TreeDurationMode::Wall),
            "v [        ] "
        );
    }

    #[test]
    fn running_duration_field_rolls_up_to_parent_rows() {
        let mut tree = Tree::from_tests(vec![DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: "case".to_owned(),
            full_name: "tests::case".to_owned(),
            status: TestStatus::Pending,
            ignored: false,
        }]);
        tree.start_test(
            &TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
        );

        let package = &tree.root.children[0];
        let module = &package.children[0];
        let test = &module.children[0];
        assert_ne!(
            tree_leading_fields(1, package, config::TreeDurationMode::Wall),
            "  > [        ] "
        );
        assert_ne!(
            tree_leading_fields(2, module, config::TreeDurationMode::Wall),
            "    > [        ] "
        );
        assert_ne!(
            tree_leading_fields(3, test, config::TreeDurationMode::Wall),
            "        [        ] "
        );
    }

    #[test]
    fn test_details_modal_includes_live_info_and_manual_command() {
        let mut test = DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case one".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: Some(PathBuf::from("src/lib.rs")),
            module: Some("tests".to_owned()),
            name: "case one".to_owned(),
            full_name: "tests::case one".to_owned(),
            status: TestStatus::Pending,
            ignored: true,
        };
        let key = test.key.clone();
        let mut app = app_with_tree(Tree::from_tests(vec![test.clone()]));
        expand_all(&mut app.tree.root);
        app.tree.finish_test(
            &key,
            TestStatus::Passed,
            "hello".to_owned(),
            String::new(),
            Some(Duration::from_millis(250)),
        );
        app.tree.select_next();
        app.tree.select_next();
        app.tree.select_next();

        let text = test_details_modal_lines(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("tests::case one"));
        assert!(text.contains("status   passed"));
        assert!(text.contains("duration 0.250s"));
        assert!(text.contains("output   stdout 5 chars"));
        assert!(text.contains(
            "cargo    cargo nextest run -p demo --lib --run-ignored only 'tests::case one'"
        ));
        assert!(!text.contains("[esc] close"));

        test.full_name = "tests::case_two".to_owned();
        assert_eq!(
            manual_test_command(&test),
            "cargo nextest run -p demo --lib --run-ignored only tests::case_two"
        );
    }

    #[test]
    fn test_details_modal_for_parent_includes_scoped_run_command() {
        let mut app = app_with_tree(Tree::from_tests(vec![DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: "case".to_owned(),
            full_name: "tests::case".to_owned(),
            status: TestStatus::Pending,
            ignored: false,
        }]));
        app.tree.select_next();

        let text = test_details_modal_lines(&app, &Theme::dark())
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("kind     package"));
        assert!(text.contains("package  demo"));
        assert!(text.contains("cargo    cargo nextest run -p demo"));
    }

    fn expand_all(node: &mut TestNode) {
        node.expanded = true;
        for child in &mut node.children {
            expand_all(child);
        }
    }

    #[test]
    fn running_row_label_shows_spinner_after_name() {
        let mut tree = Tree::from_tests(vec![DiscoveredTest {
            key: TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::case".to_owned(),
            },
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            cwd: PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: "case".to_owned(),
            full_name: "tests::case".to_owned(),
            status: TestStatus::Pending,
            ignored: false,
        }]);
        let key = TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::case".to_owned(),
        };
        tree.start_test(&key);

        let package = &tree.root.children[0];
        let module = &package.children[0];
        let test = &module.children[0];

        assert_eq!(node_label(&tree.root, "⠋"), ". ⠋");
        assert_eq!(node_label(package, "⠋"), "demo ⠋");
        assert_eq!(node_label(module, "⠋"), "tests ⠋");
        assert_eq!(node_label(test, "⠋"), "case ⠋");
    }

    #[test]
    fn running_test_spinner_advances_with_app_tick() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));

        assert_eq!(app.running_test_spinner(), "⠋");

        app.tick();

        assert_eq!(app.running_test_spinner(), "⠙");
    }

    #[test]
    fn output_status_includes_search_flags() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.output_page_size = 5;
        app.output_search.query = "panic".to_owned();
        app.output_search.filter = true;
        let text = "panic line";

        assert_eq!(
            output_status(&app, text, text),
            "Output <lines: 1-1/1> <search: [panic       ] 0/1 [n]ext [f]ilter:✓ [r]egex:✗ [c]ase-sensitive:✗>"
        );
    }

    #[test]
    fn output_lines_marks_current_search_result_differently() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        let theme = Theme::dark();
        app.output_search.query = "panic".to_owned();
        app.output_search.current_line = Some(1);

        let output_view = crate::output_pane::OutputView {
            text: "panic one\npanic two".to_owned(),
            source_lines: vec![0, 1],
        };
        let lines = output_lines(&app, &theme, &output_view);

        assert_eq!(lines[0].spans[0].style, theme.search_match());
        assert_eq!(lines[1].spans[0].style, theme.active_search_match());
    }

    #[test]
    fn output_search_box_marks_active_input() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.output_search.draft_query = "panic".to_owned();
        app.output_search.input_active = true;

        assert_eq!(app.output_search.box_text(18), "[panic_            ]");
    }

    #[test]
    fn output_status_shows_submit_and_advanced_hints_while_searching() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.output_search.draft_query = "panic".to_owned();
        app.output_search.input_active = true;

        assert_eq!(
            output_status(&app, "panic line", "panic line"),
            "Output <lines: 1-1/1> <search: [panic_      ] 0/0 [enter]submit [C+enter]advanced [n]ext [f]ilter:✗ [r]egex:✗ [c]ase-sensitive:✗>"
        );
    }

    #[test]
    fn output_search_box_keeps_fixed_width_for_long_query() {
        let mut app = app_with_tree(Tree::from_tests(Vec::new()));
        app.output_search.query = "abcdefghijklmnopqrstuvwxyz".to_owned();

        assert_eq!(app.output_search.box_text(18).len(), 20);
        assert_eq!(app.output_search.box_text(18), "[ijklmnopqrstuvwxyz]");
    }

    fn help_line_with_label<'a>(lines: &'a [Line<'a>], label: &str) -> &'a Line<'a> {
        lines
            .iter()
            .find(|line| line_text(line).contains(label))
            .expect("help line")
    }

    fn assert_help_order(lines: &[Line<'_>], labels: &[&str]) {
        let indexes = labels
            .iter()
            .map(|label| help_line_index(lines, label))
            .collect::<Vec<_>>();
        let mut sorted = indexes.clone();
        sorted.sort_unstable();
        assert_eq!(indexes, sorted);
    }

    fn help_line_index(lines: &[Line<'_>], label: &str) -> usize {
        lines
            .iter()
            .position(|line| line_text(line).contains(label))
            .expect("help line")
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }
