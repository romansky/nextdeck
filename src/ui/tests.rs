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
    app.main_output.page_size = 5;
    let text = "one\ntwo";

    assert_eq!(
        app.main_output.status("Output", text),
        "Output [PageUp/PageDown]<#1-2/2> [s]nap:✓"
    );
}

#[test]
fn output_status_shows_clamped_line_ranges() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.main_output.page_size = 3;
    let text = "1\n2\n3\n4\n5\n6";

    app.main_output.scroll = 0;
    assert_eq!(
        app.main_output.status("Output", text),
        "Output [PageUp/PageDown]<#1-3/6> [s]nap:✓"
    );

    app.main_output.scroll = 2;
    app.main_output.follow = false;
    assert_eq!(
        app.main_output.status("Output", text),
        "Output [PageUp/PageDown]<#3-5/6> [s]nap:✗"
    );

    app.main_output.scroll = 3;
    app.main_output.follow = true;
    assert_eq!(
        app.main_output.status("Output", text),
        "Output [PageUp/PageDown]<#4-6/6> [s]nap:✓"
    );
}

#[test]
fn fit_line_prefix_preserves_xtask_text_prefix() {
    assert_eq!(
        fit_line_prefix("Publish the verified package locally", 18),
        "Publish the ver..."
    );
    let fitted = fit_line_prefix("cargo xtask check", 30);
    assert!(fitted.starts_with("cargo xtask check"));
    assert_eq!(fitted.len(), 30);
}

#[test]
fn xtask_list_uses_auto_command_column_width() {
    let theme = Theme::dark();
    let xtasks = XtaskState {
        manifest: Some(crate::xtask::XtaskManifest {
            schema_version: crate::xtask::SCHEMA_VERSION,
            commands: vec![
                xtask_command(
                    "tui-check",
                    "Run local TUI checks expected before publishing",
                ),
                xtask_command(
                    "tui-homebrew-formula",
                    "Generate a Homebrew formula from TUI release artifact checksums",
                ),
            ],
        }),
        loading: false,
        selected_command: 1,
        ..Default::default()
    };

    let lines = xtask_list_lines(&xtasks, &theme, 96);
    let selected = line_text(&lines[1]);

    assert!(selected.starts_with("> tui-homebrew-formula Generate a Homebrew formula"));
    assert!(selected.contains("release artifact checksums"));
}

#[test]
fn xtask_list_caps_long_command_column_width() {
    let theme = Theme::dark();
    let xtasks = XtaskState {
        manifest: Some(crate::xtask::XtaskManifest {
            schema_version: crate::xtask::SCHEMA_VERSION,
            commands: vec![xtask_command(
                "this-command-name-is-too-long-for-the-picker",
                "Visible description still gets space",
            )],
        }),
        loading: false,
        ..Default::default()
    };

    let lines = xtask_list_lines(&xtasks, &theme, 50);
    let text = line_text(&lines[0]);

    assert!(text.starts_with("> this-command-name-is-too-lo... "));
    assert!(text.contains("Visible descr"));
}

#[test]
fn auto_column_layout_sizes_fixed_columns_and_flexes_last_column() {
    let rows = vec![
        vec!["  ", "short", "first description"],
        vec!["  ", "longer-command", "second description"],
    ];
    let layout = AutoColumnLayout::compute(
        &[
            AutoColumn { max_width: Some(2) },
            AutoColumn {
                max_width: Some(30),
            },
            AutoColumn { max_width: None },
        ],
        &rows,
        40,
    );

    assert_eq!(layout.widths, vec![2, 14, 22]);
}

#[test]
fn auto_column_layout_caps_fixed_columns_before_flex_column() {
    let rows = vec![vec![
        "> ",
        "very-very-very-long-command",
        "description keeps the prefix",
    ]];
    let layout = AutoColumnLayout::compute(
        &[
            AutoColumn { max_width: Some(2) },
            AutoColumn {
                max_width: Some(12),
            },
            AutoColumn { max_width: None },
        ],
        &rows,
        30,
    );
    let line = layout.row(&[
        ("> ", Style::default()),
        ("very-very-very-long-command", Style::default()),
        ("description keeps the prefix", Style::default()),
    ]);
    let text = line_text(&line);

    assert_eq!(layout.widths, vec![2, 12, 14]);
    assert!(text.starts_with(">  very-very... descript"));
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

    let run_text = run_details(&app, &Theme::dark(), 80)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    let storage_text = storage_details(&app, &Theme::dark(), 80)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(footer_run_status(&app), app.run_status_label());
    assert!(run_text.contains("run id"));
    assert!(run_text.contains("latest event"));
    assert!(run_text.contains(app.run_status_label()));
    assert!(!run_text.contains("not running"));
    assert!(!run_text.contains("target"));
    assert!(storage_text.contains("Storage"));
    assert!(storage_text.contains("low"));
    assert!(storage_text.contains("available"));
    assert!(storage_text.contains("2.0 KiB"));
    assert!(storage_text.contains("1970-01-01 00:00:00 UTC"));
    assert!(!storage_text.contains("total"));
    assert!(storage_text.contains("/target"));
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
fn disk_cleanup_modal_shows_running_indicator() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.begin_cargo_clean().expect("clean starts");

    let text = disk_cleanup_lines(&app, &Theme::dark())
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("cargo clean running..."));
}

#[test]
fn settings_modal_includes_storage_and_duration_settings() {
    let app = app_with_tree(Tree::from_tests(Vec::new()));

    assert_eq!(settings_value(&app, SettingsField::TreeDuration), "wall");
    assert_eq!(
        settings_value(&app, SettingsField::StorageThreshold),
        "10 GiB"
    );

    let rows = settings_rows(&app);
    let text = ParameterList::new(
        &rows,
        SELECTABLE_FIELD_PREFIX_WIDTH,
        SETTINGS_FIELD_LABEL_WIDTH,
        100,
        parameter_list_styles(&Theme::dark()),
    )
    .render()
    .iter()
    .map(line_text)
    .collect::<Vec<_>>()
    .join("\n");

    assert!(text.contains("# string: env/default, idea, code, cursor, zed, open"));
    assert!(text.contains("# number: 25..70% (default: 45%)"));
    assert!(text.contains("# enum: wall, aggregate (default: wall)"));
    assert!(text.contains("# number: 1..1024 GiB (default: 10 GiB)"));
    assert!(text.contains("# enum: auto, dark, light (default: auto)"));
    assert!(text.contains("# bool: off, on (default: off)"));
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
        "[enter]details [r]un [R]run-custom [o]pen-editor [u]update"
    );
    assert_eq!(
        disk_cleanup_actions(),
        "[c]cargo-clean [r]refresh [esc]close"
    );
    assert_eq!(
        output_actions("[/]search<[            ]>"),
        "[/]search<[            ]> [o]pen-editor"
    );
    assert_eq!(
        discovery_error_actions("[/]search<[            ]> [o]pen-editor"),
        "[u]retry [/]search<[            ]> [o]pen-editor [q]quit"
    );
}

#[test]
fn custom_run_options_render_values_without_accidental_editors() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    let theme = Theme::dark();
    let lines = custom_run_lines(&app.custom_run, &theme, 100);
    let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

    assert!(text.contains("> scope"));
    assert!(text.contains("selected"));
    assert!(text.contains("# enum: selected, workspace, failed (default: selected)"));
    assert!(text.contains("  profile"));
    assert!(text.contains("# enum: default (default: default)"));
    assert!(text.contains("  filterset"));
    assert!(text.contains("# enum: none (default: none; [e] custom)"));
    assert!(text.contains("# enum: profile, pass, fail (default: profile)"));
    assert!(text.contains("# number: profile, 0..20 (default: profile; [e] custom)"));
    assert!(text.contains("# bool: off, on (default: off)"));
    assert!(text.contains("# string: off, rust-lldb --args (default: off; [e] custom)"));
    assert!(text.contains("# number: off, 0..100 (default: off; [e] custom)"));
    assert!(text.contains("# string: off, 30s (default: off; [e] custom)"));
    assert!(!text.contains("stress-durationoff"));
    assert!(!text.contains("[_"));

    app.custom_run.filter = CustomRunFilter::Custom("package(demo)".to_owned());
    let text = custom_run_lines(&app.custom_run, &theme, 100)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("  filterset"));
    assert!(text.contains("custom: package(demo)"));
    assert!(text.contains("# enum: none, custom (default: none)"));

    let narrow = custom_run_lines(&app.custom_run, &theme, 32)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(narrow.contains("# enum: selected"));
}

#[test]
fn xtask_params_use_parameter_component_with_help_details_and_command_preview() {
    let theme = Theme::dark();
    let mut xtasks = XtaskState::default();
    xtasks.set_manifest(crate::xtask::XtaskManifest {
        schema_version: crate::xtask::SCHEMA_VERSION,
        commands: vec![crate::xtask::XtaskCommandSpec {
            name: "ship".to_owned(),
            about: Some("Ship package".to_owned()),
            args: vec![
                crate::xtask::XtaskArgSpec {
                    name: "profile".to_owned(),
                    long: Some("profile".to_owned()),
                    short: None,
                    help: Some("Build profile".to_owned()),
                    required: false,
                    value: crate::xtask::XtaskValueSpec::Enum {
                        values: vec!["debug".to_owned(), "release".to_owned()],
                        default: Some("debug".to_owned()),
                    },
                },
                crate::xtask::XtaskArgSpec {
                    name: "allow-dirty".to_owned(),
                    long: Some("allow-dirty".to_owned()),
                    short: None,
                    help: Some("Allow dirty worktree".to_owned()),
                    required: false,
                    value: crate::xtask::XtaskValueSpec::Bool { default: false },
                },
                crate::xtask::XtaskArgSpec {
                    name: "version".to_owned(),
                    long: Some("version".to_owned()),
                    short: None,
                    help: Some("Release version".to_owned()),
                    required: false,
                    value: crate::xtask::XtaskValueSpec::String { default: None },
                },
            ],
        }],
    });

    let rendered = xtask_param_lines(&xtasks, &theme, 80, true)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>();
    let text = rendered.join("\n");

    assert!(text.contains("> --profile     debug"));
    assert!(text.contains("# Build profile"));
    assert!(text.contains("# enum: debug, release"));
    assert!(text.contains("  --allow-dirty off"));
    assert!(text.contains("# Allow dirty worktree"));
    assert!(text.contains("# bool: off, on (default: off)"));
    assert!(text.contains("  --version     [empty]"));
    assert!(text.contains("# Release version"));
    assert!(text.contains("# string"));

    let first_param = rendered
        .iter()
        .position(|line| line.contains("--profile"))
        .expect("profile param");
    let preview = rendered
        .iter()
        .position(|line| line.contains("cargo xtask ship"))
        .expect("command preview");
    assert!(preview > first_param);
    assert!(text.contains("Ship package"));
}

#[test]
fn test_details_places_run_command_below_options() {
    let app = app_with_tree(Tree::from_tests(Vec::new()));
    let theme = Theme::dark();
    let lines = test_details_modal_lines(&app, &theme);
    let rendered = lines.iter().map(line_text).collect::<Vec<_>>();
    let stress_index = rendered
        .iter()
        .position(|line| line.contains("stress-duration"))
        .expect("stress duration option");
    let command_index = rendered
        .iter()
        .position(|line| line.contains("cargo nextest run"))
        .expect("command preview");

    assert!(command_index > stress_index);
    assert!(
        !rendered
            .iter()
            .any(|line| line.contains("Detected Profiles"))
    );
    assert!(!rendered.iter().any(|line| line.contains("Filter Presets")));
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

    assert_eq!(
        lines.first(),
        Some(&format!("NextDeck {}", env!("CARGO_PKG_VERSION")))
    );
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
            "run custom",
            "run selected scope",
            "update test list",
        ],
    );
    assert_help_order(
        &text,
        &[
            "search output",
            "toggle output case sensitivity",
            "toggle output match filter",
            "next or previous output match",
            "open output as text file",
            "toggle output regex",
            "toggle output snap",
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
        ignore_reason: None,
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
        ignore_reason: None,
    }]);
    tree.start_test(&TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::case".to_owned(),
    });

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
fn test_details_modal_includes_live_info_and_run_command() {
    let test = DiscoveredTest {
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
        ignore_reason: Some("fixture ignored test".to_owned()),
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
    assert!(text.contains("Run"));
    assert!(text.contains("> scope"));
    assert!(text.contains("selected"));
    assert!(text.contains("# enum: selected, workspace, failed (default: selected)"));
    assert!(text.contains("cargo nextest run --run-ignored only -p demo --lib 'tests::case one'"));
    assert!(!text.contains("[esc] close"));
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
        ignore_reason: None,
    }]));
    app.tree.select_next();

    let text = test_details_modal_lines(&app, &Theme::dark())
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("kind     package"));
    assert!(text.contains("package  demo"));
    assert!(text.contains("Run"));
    assert!(text.contains("cargo nextest run -p demo"));
}

#[test]
fn test_details_actions_include_snapshot_for_leaf_tests() {
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
        ignore_reason: None,
    }]));
    expand_all(&mut app.tree.root);

    app.tree.select_next();
    assert_eq!(
        test_details_actions(&app),
        "[up/down]option [left/right]change [e]edit [r/enter]run [esc]close"
    );

    app.tree.select_next();
    app.tree.select_next();
    assert_eq!(
        test_details_actions(&app),
        "[up/down]option [left/right]change [e]edit [r/enter]run [s]snapshot [esc]close"
    );
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
        ignore_reason: None,
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
    assert!(!app.tick().any());
    assert_eq!(app.running_test_spinner(), "⠋");

    app.begin_run(&crate::nextest::RunRequest::default())
        .expect("run starts");
    assert!(app.tick().any());

    assert_eq!(app.running_test_spinner(), "⠙");
}

#[test]
fn output_actions_include_search_flags_when_search_has_value() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.main_output.search.query = "panic".to_owned();
    app.main_output.search.filter = true;
    let text = "panic line";

    assert_eq!(
        app.main_output.search_actions(text),
        "[/]search<[panic       ] 0/1 [C+u]clear [n/N]ext [f]ilter:✓ [r]egex:✗ [c]ase-sensitive:✗>"
    );
}

#[test]
fn output_lines_marks_current_search_result_differently() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    let theme = Theme::dark();
    app.main_output.search.query = "panic".to_owned();
    app.main_output.search.current_line = Some(1);

    let output_view = crate::output_pane::OutputView {
        text: "panic one\npanic two".to_owned(),
        source_lines: vec![0, 1],
    };
    let lines = output_lines(&app.main_output.search, &theme, &output_view);

    assert_eq!(lines[0].spans[0].style, theme.search_match());
    assert_eq!(lines[1].spans[0].style, theme.active_search_match());
}

#[test]
fn output_lines_color_run_result_summaries() {
    let app = app_with_tree(Tree::from_tests(Vec::new()));
    let theme = Theme::dark();
    let output_view = crate::output_pane::OutputView {
        text: "Run passed: 1 passed\nRun failed: 1 failed\nRun command failed: nextest exited with 101\n[event info] fixture: cached\n[event warn] fixture: slow\n[event error] fixture: failed".to_owned(),
        source_lines: vec![0, 1, 2, 3, 4, 5],
    };

    let lines = output_lines(&app.main_output.search, &theme, &output_view);

    assert_eq!(lines[0].style, theme.success());
    assert_eq!(lines[1].style, theme.danger());
    assert_eq!(lines[2].style, theme.danger());
    assert_eq!(lines[3].style, theme.accent());
    assert_eq!(lines[4].style, theme.warning());
    assert_eq!(lines[5].style, theme.danger());
}

#[test]
fn output_lines_marks_only_current_search_range_active() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    let theme = Theme::dark();
    app.main_output.search.query = "panic".to_owned();
    app.main_output.search.current_line = Some(0);
    app.main_output.search.current_range = Some((10, 15));

    let output_view = crate::output_pane::OutputView {
        text: "panic one panic two".to_owned(),
        source_lines: vec![0],
    };
    let lines = output_lines(&app.main_output.search, &theme, &output_view);

    assert_eq!(line_text(&lines[0]), "panic one panic two");
    assert_eq!(lines[0].spans[0].style, theme.search_match());
    assert_eq!(lines[0].spans[2].style, theme.active_search_match());
}

#[test]
fn output_search_box_marks_active_input() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.main_output.search.draft_query = "panic".to_owned();
    app.main_output.search.input_active = true;

    assert_eq!(app.main_output.search.box_text(18), "[panic_            ]");
}

#[test]
fn output_actions_show_submit_and_advanced_hints_while_searching() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.main_output.search.draft_query = "panic".to_owned();
    app.main_output.search.input_active = true;

    assert_eq!(
        app.main_output.search_actions("panic line"),
        "[/]search<[panic_      ] 0/0 [enter]submit [C+enter]advanced [n/N]ext [f]ilter:✗ [r]egex:✗ [c]ase-sensitive:✗>"
    );
}

#[test]
fn output_search_box_keeps_fixed_width_for_long_query() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.main_output.search.query = "abcdefghijklmnopqrstuvwxyz".to_owned();

    assert_eq!(app.main_output.search.box_text(18).len(), 20);
    assert_eq!(app.main_output.search.box_text(18), "[ijklmnopqrstuvwxyz]");
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

fn xtask_command(name: &str, about: &str) -> crate::xtask::XtaskCommandSpec {
    crate::xtask::XtaskCommandSpec {
        name: name.to_owned(),
        about: Some(about.to_owned()),
        args: Vec::new(),
    }
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}
