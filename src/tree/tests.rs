use super::*;

#[test]
fn builds_package_module_test_tree() {
    let tree = Tree::from_tests(vec![DiscoveredTest {
        key: TestKey {
            binary_id: Some("demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "a::b::works".to_owned(),
        },
        package: "demo".to_owned(),
        binary: "demo".to_owned(),
        binary_kind: "lib".to_owned(),
        cwd: PathBuf::from("."),
        source_path: None,
        module: Some("a::b".to_owned()),
        name: "works".to_owned(),
        full_name: "a::b::works".to_owned(),
        status: TestStatus::Pending,
        ignored: false,
        ignore_reason: None,
    }]);

    assert_eq!(tree.root.children[0].label, "demo");
    assert_eq!(tree.root.children[0].children[0].label, "a");
    assert_eq!(tree.root.children[0].children[0].children[0].label, "b");
    assert_eq!(
        tree.root.children[0].children[0].children[0].children[0].label,
        "works"
    );
}

#[test]
fn workspace_label_uses_discovered_cwd() {
    let mut test = discovered_test("demo::demo", "demo", "tests", "works");
    test.cwd = PathBuf::from("/Users/roman/Code/demo");

    let tree = Tree::from_tests(vec![test]);

    assert_eq!(tree.root.label, "/Users/roman/Code/demo");
}

#[test]
fn workspace_label_uses_common_cwd_for_multiple_packages() {
    let mut first = discovered_test("app::app", "app", "tests", "works");
    first.cwd = PathBuf::from("/Users/roman/Code/workspace/app");
    let mut second = discovered_test("core::core", "core", "tests", "works");
    second.cwd = PathBuf::from("/Users/roman/Code/workspace/core");

    let tree = Tree::from_tests(vec![first, second]);

    assert_eq!(tree.root.label, "/Users/roman/Code/workspace");
}

#[test]
fn empty_tree_keeps_workspace_label() {
    let tree = Tree::from_tests(Vec::new());

    assert_eq!(tree.root.label, "workspace");
}

#[test]
fn initial_tree_shows_only_one_nested_level() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "outer::inner",
        "works",
    )]);

    assert_eq!(visible_labels(&tree), vec![".", "demo"]);

    tree.select_next();
    tree.expand_selected();
    assert_eq!(visible_labels(&tree), vec![".", "demo", "outer"]);

    tree.select_next();
    tree.expand_selected();
    assert_eq!(visible_labels(&tree), vec![".", "demo", "outer", "inner"]);
}

#[test]
fn view_filter_prunes_empty_parent_branches() {
    let mut ignored = discovered_test("demo::demo", "demo", "tests", "ignored");
    ignored.ignored = true;
    ignored.status = TestStatus::Ignored;
    let mut tree = Tree::from_tests(vec![ignored]);
    tree.set_view_filter(TestViewFilter {
        show_success: true,
        show_failed: true,
        show_ignored: false,
        show_skipped: true,
    });

    assert_eq!(visible_labels(&tree), vec!["."]);
}

#[test]
fn view_filter_keeps_parent_branches_with_visible_descendants() {
    let mut ignored = discovered_test("demo::demo", "demo", "tests", "ignored");
    ignored.ignored = true;
    ignored.status = TestStatus::Ignored;
    let visible = discovered_test("demo::demo", "demo", "tests", "pending");
    let mut tree = Tree::from_tests(vec![ignored, visible]);
    expand_all(&mut tree);

    tree.set_view_filter(TestViewFilter {
        show_success: true,
        show_failed: true,
        show_ignored: false,
        show_skipped: true,
    });

    assert_eq!(visible_labels(&tree), vec![".", "demo", "tests", "pending"]);
}

#[test]
fn expand_selected_expands_only_the_selected_branch() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "alpha", "one"),
        discovered_test("demo::demo", "demo", "beta", "two"),
    ]);

    tree.select_next();
    tree.expand_selected();
    select_label(&mut tree, "alpha");
    tree.expand_selected();

    let labels = visible_labels(&tree);
    assert!(labels.contains(&"one".to_owned()));
    assert!(!labels.contains(&"two".to_owned()));
}

#[test]
fn expand_selected_on_test_leaf_keeps_tree_unchanged() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    expand_all(&mut tree);
    select_label(&mut tree, "works");
    let before = visible_labels(&tree);

    tree.expand_selected();

    assert_eq!(visible_labels(&tree), before);
    assert_eq!(
        tree.selected_node().map(|node| node.label.as_str()),
        Some("works")
    );
}

#[test]
fn running_duration_rolls_up_to_parent_rows() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);

    set_test_status(&mut tree, "tests::works", TestStatus::Running);

    let package = &tree.root.children[0];
    let module = &package.children[0];
    let test = &module.children[0];
    assert!(
        tree.root
            .display_duration(crate::config::TreeDurationMode::Wall)
            .is_some()
    );
    assert!(
        package
            .display_duration(crate::config::TreeDurationMode::Wall)
            .is_some()
    );
    assert!(
        module
            .display_duration(crate::config::TreeDurationMode::Wall)
            .is_some()
    );
    assert!(
        test.display_duration(crate::config::TreeDurationMode::Wall)
            .is_some()
    );
}

#[test]
fn parent_duration_mode_can_use_wall_span_or_aggregate_sum() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "one"),
        discovered_test("demo::demo", "demo", "tests", "two"),
    ]);
    let now = Instant::now();
    set_finished_span(
        &mut tree,
        "tests::one",
        now,
        now + Duration::from_millis(20),
        Duration::from_millis(20),
    );
    set_finished_span(
        &mut tree,
        "tests::two",
        now + Duration::from_millis(10),
        now + Duration::from_millis(30),
        Duration::from_millis(20),
    );

    assert_eq!(
        tree.root
            .display_duration_at(crate::config::TreeDurationMode::Wall, now),
        Some(Duration::from_millis(30))
    );
    assert_eq!(
        tree.root
            .display_duration_at(crate::config::TreeDurationMode::Aggregate, now),
        Some(Duration::from_millis(40))
    );
}

#[test]
fn stop_running_tests_returns_running_leaves_to_pending() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    set_test_status(&mut tree, "tests::works", TestStatus::Running);

    tree.stop_running_tests();

    let test = &tree.root.children[0].children[0].children[0];
    assert_eq!(test.status, TestStatus::Pending);
    assert_eq!(test.started_at, None);
    assert_eq!(tree.root.status, TestStatus::Pending);
}

#[test]
fn ignored_test_start_event_does_not_mark_running() {
    let mut test = discovered_test("demo::demo", "demo", "tests", "ignored");
    test.ignored = true;
    test.status = TestStatus::Ignored;
    let mut tree = Tree::from_tests(vec![test]);

    tree.start_test(&TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::ignored".to_owned(),
    });

    let ignored = &tree.root.children[0].children[0];
    assert_eq!(ignored.status, TestStatus::Ignored);
    assert_eq!(ignored.started_at, None);
}

#[test]
fn integration_test_targets_are_grouped_under_binary_name() {
    let tree = Tree::from_tests(vec![DiscoveredTest {
        key: TestKey {
            binary_id: Some("demo::scenario".to_owned()),
            event_prefix: Some("demo::scenario".to_owned()),
            name: "top_level_test".to_owned(),
        },
        package: "demo".to_owned(),
        binary: "scenario".to_owned(),
        binary_kind: "test".to_owned(),
        cwd: PathBuf::from("."),
        source_path: Some(PathBuf::from("src/tier_scenario.rs")),
        module: None,
        name: "top_level_test".to_owned(),
        full_name: "top_level_test".to_owned(),
        status: TestStatus::Pending,
        ignored: false,
        ignore_reason: None,
    }]);

    assert_eq!(tree.root.children[0].label, "demo");
    assert_eq!(tree.root.children[0].children[0].label, "scenario");
    assert_eq!(
        tree.root.children[0].children[0].children[0].label,
        "top_level_test"
    );
}

#[test]
fn module_output_shows_descendant_tests_not_runner_output() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "works"),
        discovered_test("demo::demo", "demo", "tests", "also_works"),
    ]);

    tree.finish_test(
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::works".to_owned(),
        },
        TestStatus::Passed,
        "hello from works".to_owned(),
        Some(std::time::Duration::from_millis(12)),
    );
    tree.finish_test(
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::also_works".to_owned(),
        },
        TestStatus::Passed,
        "hello from also_works".to_owned(),
        Some(std::time::Duration::from_millis(20)),
    );
    tree.append_runner_output("unrelated runner line".to_owned());

    tree.select_next();
    tree.select_next();

    let output = tree.selected_output();
    assert!(output.contains("demo::tests::works [passed]"));
    assert!(output.contains("hello from works"));
    assert!(output.contains("demo::tests::also_works [passed]"));
    assert!(output.contains("hello from also_works"));
    assert!(!output.contains("unrelated runner line"));
}

#[test]
fn focused_test_output_shows_captured_stream_without_metadata_headers() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);

    tree.finish_test(
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::works".to_owned(),
        },
        TestStatus::Passed,
        "hello from stdout".to_owned(),
        Some(std::time::Duration::from_millis(12)),
    );

    tree.select_next();
    tree.expand_selected();
    tree.select_next();
    tree.expand_selected();
    tree.select_next();

    assert_eq!(tree.selected_output(), "hello from stdout\n");
}

#[test]
fn late_success_output_is_attached_to_finished_test() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::works".to_owned(),
    };

    tree.finish_test(
        &key,
        TestStatus::Passed,
        String::new(),
        Some(std::time::Duration::from_millis(12)),
    );
    tree.append_test_output(&key, "late stdout".to_owned());
    expand_all(&mut tree);
    select_label(&mut tree, "works");

    assert_eq!(tree.selected_output(), "late stdout\n");
}

#[test]
fn early_success_output_survives_empty_finished_event() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::works".to_owned(),
    };

    tree.append_test_output(&key, "early stdout".to_owned());
    tree.finish_test(
        &key,
        TestStatus::Passed,
        String::new(),
        Some(std::time::Duration::from_millis(12)),
    );
    expand_all(&mut tree);
    select_label(&mut tree, "works");

    assert_eq!(tree.selected_output(), "early stdout\n");
}

#[test]
fn inline_event_survives_finished_event_with_stdout() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::works".to_owned(),
    };
    let event = nextdeck_test_events::TestEvent::new(nextdeck_test_events::Level::Info, "hit");

    assert!(tree.append_test_event(&key, &event, "@ event info cache: hit"));
    tree.finish_test(
        &key,
        TestStatus::Failed,
        "final stdout".to_owned(),
        Some(std::time::Duration::from_millis(12)),
    );
    expand_all(&mut tree);
    select_label(&mut tree, "works");

    let output = tree.selected_output();
    assert!(output.contains("@ event info cache: hit"));
    assert!(output.contains("final stdout"));
    assert!(
        output
            .find("@ event info cache: hit")
            .is_some_and(|event_index| output
                .find("final stdout")
                .is_some_and(|stdout_index| event_index < stdout_index))
    );
}

#[test]
fn appended_event_bubbles_to_target_ancestors() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "alpha", "one"),
        discovered_test("demo::demo", "demo", "beta", "two"),
    ]);
    let event = nextdeck_test_events::TestEvent::new(nextdeck_test_events::Level::Warn, "slow");
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "alpha::one".to_owned(),
    };

    assert!(!tree.root.has_events);
    assert!(tree.append_test_event(&key, &event, "@ event warn alpha: slow"));

    let package = &tree.root.children[0];
    let alpha = &package.children[0];
    let alpha_test = &alpha.children[0];
    let beta = &package.children[1];
    let beta_test = &beta.children[0];

    assert!(tree.root.has_events);
    assert!(package.has_events);
    assert!(alpha.has_events);
    assert!(alpha_test.has_events);
    assert!(!beta.has_events);
    assert!(!beta_test.has_events);
}

#[test]
fn prepare_for_run_clears_event_bubbles() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    let event = nextdeck_test_events::TestEvent::new(nextdeck_test_events::Level::Info, "hit");
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::works".to_owned(),
    };

    assert!(tree.append_test_event(&key, &event, "@ event info cache: hit"));
    assert!(tree.root.has_events);

    tree.prepare_for_run(&crate::nextest::RunScope::Workspace);

    assert!(!tree.root.has_events);
    assert!(!tree.root.children[0].has_events);
    assert!(!tree.root.children[0].children[0].has_events);
    assert!(!tree.root.children[0].children[0].children[0].has_events);
}

#[test]
fn appended_test_output_is_bounded() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::works".to_owned(),
    };

    tree.append_test_output(
        &key,
        "x".repeat(crate::output::OUTPUT_TEXT_LIMIT_BYTES + 1024),
    );
    tree.append_test_output(&key, "tail".to_owned());
    expand_all(&mut tree);
    select_label(&mut tree, "works");

    let output = tree.selected_output();
    assert!(output.contains("[... output truncated"));
    assert!(output.contains("tail"));
    assert!(output.len() <= crate::output::OUTPUT_TEXT_LIMIT_BYTES + 1);
}

#[test]
fn parent_output_is_bounded_across_descendants() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "first"),
        discovered_test("demo::demo", "demo", "tests", "second"),
    ]);
    for name in ["tests::first", "tests::second"] {
        tree.append_test_output(
            &TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: name.to_owned(),
            },
            format!(
                "{name}\n{}",
                "x".repeat(crate::output::OUTPUT_TEXT_LIMIT_BYTES)
            ),
        );
    }

    let output = tree.selected_output();
    assert!(output.len() <= crate::output::OUTPUT_TEXT_LIMIT_BYTES);
    assert!(output.contains("tests::second"));
}

#[test]
fn pending_module_output_stays_scoped_and_short() {
    let mut tree = Tree::from_tests(vec![discovered_test(
        "demo::demo",
        "demo",
        "tests",
        "works",
    )]);
    tree.append_runner_output("unrelated runner line".to_owned());

    tree.select_next();
    tree.select_next();

    let output = tree.selected_output();
    assert_eq!(
        output,
        "No captured output for tests under this selection yet"
    );
}

#[test]
fn view_filter_hides_passed_and_failed_test_rows() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "passed"),
        discovered_test("demo::demo", "demo", "tests", "failed"),
        discovered_test("demo::demo", "demo", "tests", "pending"),
    ]);
    set_test_status(&mut tree, "tests::passed", TestStatus::Passed);
    set_test_status(&mut tree, "tests::failed", TestStatus::Failed);
    expand_all(&mut tree);

    tree.set_view_filter(TestViewFilter {
        show_success: false,
        show_failed: true,
        show_ignored: true,
        show_skipped: true,
    });
    let labels = visible_labels(&tree);
    assert!(!labels.contains(&"passed".to_owned()));
    assert!(labels.contains(&"failed".to_owned()));
    assert!(labels.contains(&"pending".to_owned()));

    tree.set_view_filter(TestViewFilter {
        show_success: true,
        show_failed: false,
        show_ignored: true,
        show_skipped: true,
    });
    let labels = visible_labels(&tree);
    assert!(labels.contains(&"passed".to_owned()));
    assert!(!labels.contains(&"failed".to_owned()));
    assert!(labels.contains(&"pending".to_owned()));
}

#[test]
fn parent_status_passes_when_all_children_pass() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "one"),
        discovered_test("demo::demo", "demo", "tests", "two"),
    ]);

    set_test_status(&mut tree, "tests::one", TestStatus::Passed);
    set_test_status(&mut tree, "tests::two", TestStatus::Passed);
    expand_all(&mut tree);

    assert_eq!(tree.root.status, TestStatus::Passed);
    let labels = tree
        .visible_rows()
        .into_iter()
        .map(|row| (row.node.label.clone(), row.node.status))
        .collect::<Vec<_>>();
    assert!(labels.contains(&("tests".to_owned(), TestStatus::Passed)));
}

#[test]
fn parent_status_runs_when_any_descendant_is_running() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "one"),
        discovered_test("demo::demo", "demo", "tests", "two"),
    ]);

    set_test_status(&mut tree, "tests::one", TestStatus::Failed);
    set_test_status(&mut tree, "tests::two", TestStatus::Running);
    expand_all(&mut tree);

    assert_eq!(tree.root.status, TestStatus::Running);
    let labels = tree
        .visible_rows()
        .into_iter()
        .map(|row| (row.node.label.clone(), row.node.status))
        .collect::<Vec<_>>();
    assert!(labels.contains(&("demo".to_owned(), TestStatus::Running)));
    assert!(labels.contains(&("tests".to_owned(), TestStatus::Running)));
}

#[test]
fn collapse_selected_or_parent_collapses_parent_when_test_is_selected() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "one"),
        discovered_test("demo::demo", "demo", "tests", "two"),
    ]);
    expand_all(&mut tree);
    select_label(&mut tree, "one");

    tree.collapse_selected_or_parent();

    assert_eq!(
        tree.selected_node().map(|node| node.label.as_str()),
        Some("tests")
    );
    let labels = visible_labels(&tree);
    assert!(!labels.contains(&"one".to_owned()));
    assert!(!labels.contains(&"two".to_owned()));
}

#[test]
fn collapse_selected_or_parent_collapses_selected_branch_first() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "one"),
        discovered_test("demo::demo", "demo", "tests", "two"),
    ]);
    expand_all(&mut tree);
    select_label(&mut tree, "tests");

    tree.collapse_selected_or_parent();

    assert_eq!(
        tree.selected_node().map(|node| node.label.as_str()),
        Some("tests")
    );
    let labels = visible_labels(&tree);
    assert!(!labels.contains(&"one".to_owned()));
    assert!(!labels.contains(&"two".to_owned()));
}

#[test]
fn view_filter_hides_ignored_test_rows() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "ignored"),
        discovered_test("demo::demo", "demo", "tests", "pending"),
    ]);
    set_test_status(&mut tree, "tests::ignored", TestStatus::Ignored);
    expand_all(&mut tree);

    tree.set_view_filter(TestViewFilter {
        show_success: true,
        show_failed: true,
        show_ignored: false,
        show_skipped: true,
    });

    let labels = visible_labels(&tree);
    assert!(!labels.contains(&"ignored".to_owned()));
    assert!(labels.contains(&"pending".to_owned()));
}

#[test]
fn view_filter_hides_skipped_test_rows() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "skipped"),
        discovered_test("demo::demo", "demo", "tests", "pending"),
    ]);
    set_test_status(&mut tree, "tests::skipped", TestStatus::Skipped);
    expand_all(&mut tree);

    tree.set_view_filter(TestViewFilter {
        show_success: true,
        show_failed: true,
        show_ignored: true,
        show_skipped: false,
    });

    let labels = visible_labels(&tree);
    assert!(!labels.contains(&"skipped".to_owned()));
    assert!(labels.contains(&"pending".to_owned()));
}

#[test]
fn view_filter_hides_tests_skipped_by_scoped_run() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "selected"),
        discovered_test("demo::demo", "demo", "tests", "outside_scope"),
    ]);
    tree.prepare_for_run(&run_scope_test("tests::selected"));
    expand_all(&mut tree);

    tree.set_view_filter(TestViewFilter {
        show_success: true,
        show_failed: true,
        show_ignored: true,
        show_skipped: false,
    });

    let labels = visible_labels(&tree);
    assert!(labels.contains(&"selected".to_owned()));
    assert!(!labels.contains(&"outside_scope".to_owned()));
}

#[test]
fn prepare_for_run_clears_previous_results_and_outputs() {
    let mut tree = Tree::from_tests(vec![
        discovered_test("demo::demo", "demo", "tests", "selected"),
        discovered_test("demo::demo", "demo", "tests", "outside_scope"),
    ]);
    tree.finish_test(
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::selected".to_owned(),
        },
        TestStatus::Passed,
        "old selected stdout".to_owned(),
        Some(std::time::Duration::from_millis(12)),
    );
    tree.finish_test(
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::outside_scope".to_owned(),
        },
        TestStatus::Failed,
        "old outside stderr".to_owned(),
        Some(std::time::Duration::from_millis(20)),
    );

    tree.prepare_for_run(&run_scope_test("tests::selected"));

    let output = tree.selected_output();
    assert!(!output.contains("old selected stdout"));
    assert!(!output.contains("old outside stderr"));
    assert_eq!(
        tree.status_counts_for_scope(&crate::nextest::RunScope::Workspace),
        StatusCounts {
            pending: 1,
            skipped: 1,
            ..StatusCounts::default()
        }
    );
}

#[test]
fn refresh_from_tests_preserves_view_filter() {
    let mut tree = Tree::from_tests(vec![discovered_test("demo::demo", "demo", "tests", "old")]);
    tree.set_view_filter(TestViewFilter {
        show_success: false,
        show_failed: true,
        show_ignored: false,
        show_skipped: false,
    });

    tree.refresh_from_tests(vec![discovered_test("demo::demo", "demo", "tests", "new")]);
    expand_all(&mut tree);

    assert!(!tree.view_filter.show_success);
    assert!(tree.view_filter.show_failed);
    assert!(!tree.view_filter.show_ignored);
    assert!(!tree.view_filter.show_skipped);
    assert!(visible_labels(&tree).contains(&"new".to_owned()));
}

fn set_test_status(tree: &mut Tree, name: &str, status: TestStatus) {
    set_tree_status(
        tree,
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: name.to_owned(),
        },
        status,
    );
}

fn set_tree_status(tree: &mut Tree, key: &TestKey, status: TestStatus) {
    visit_mut(&mut tree.root, &mut |node| {
        if node_matches(node, key) {
            node.status = status;
            let now = Instant::now();
            node.started_at = (status == TestStatus::Running).then_some(now);
            node.run_started_at = (status == TestStatus::Running).then_some(now);
        }
    });
    tree.recompute_statuses();
}

fn set_finished_span(
    tree: &mut Tree,
    name: &str,
    started_at: Instant,
    finished_at: Instant,
    duration: Duration,
) {
    set_tree_status(
        tree,
        &TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: name.to_owned(),
        },
        TestStatus::Passed,
    );
    visit_mut(&mut tree.root, &mut |node| {
        if let NodeKind::Test(test) = &node.kind
            && test.full_name == name
        {
            node.run_started_at = Some(started_at);
            node.finished_at = Some(finished_at);
            node.output.duration = Some(duration);
        }
    });
    tree.recompute_statuses();
}

fn visible_labels(tree: &Tree) -> Vec<String> {
    tree.visible_rows()
        .iter()
        .map(|row| row.node.label.clone())
        .collect()
}

fn select_label(tree: &mut Tree, label: &str) {
    tree.selected = tree
        .visible_rows()
        .iter()
        .find_map(|row| (row.node.label == label).then(|| row.node.id.clone()))
        .expect("visible label");
}

fn expand_all(tree: &mut Tree) {
    visit_mut(&mut tree.root, &mut |node| node.expanded = true);
}

fn discovered_test(binary_id: &str, package: &str, module: &str, name: &str) -> DiscoveredTest {
    DiscoveredTest {
        key: TestKey {
            binary_id: Some(binary_id.to_owned()),
            event_prefix: Some(binary_id.to_owned()),
            name: format!("{module}::{name}"),
        },
        package: package.to_owned(),
        binary: package.to_owned(),
        binary_kind: "lib".to_owned(),
        cwd: PathBuf::from("."),
        source_path: None,
        module: Some(module.to_owned()),
        name: name.to_owned(),
        full_name: format!("{module}::{name}"),
        status: TestStatus::Pending,
        ignored: false,
        ignore_reason: None,
    }
}

fn run_scope_test(name: &str) -> crate::nextest::RunScope {
    crate::nextest::RunScope::Test(crate::nextest::TestSelector {
        target: crate::nextest::TargetSelector {
            package: "demo".to_owned(),
            name: "demo".to_owned(),
            kind: "lib".to_owned(),
        },
        name: name.to_owned(),
    })
}
