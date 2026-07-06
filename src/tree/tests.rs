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
    fn branches_remain_expandable_when_filter_hides_all_child_tests() {
        let mut test = discovered_test("demo::demo", "demo", "tests", "ignored");
        test.ignored = true;
        test.status = TestStatus::Ignored;
        let mut tree = Tree::from_tests(vec![test]);
        tree.set_view_filter(TestViewFilter {
            show_success: true,
            show_failed: true,
            show_ignored: false,
            show_skipped: true,
        });

        assert_eq!(visible_labels(&tree), vec![".", "demo"]);

        tree.select_next();
        tree.expand_selected();
        assert_eq!(visible_labels(&tree), vec![".", "demo", "tests"]);

        tree.select_next();
        tree.expand_selected();
        assert_eq!(visible_labels(&tree), vec![".", "demo", "tests"]);
        assert!(tree.selected_node().is_some_and(|node| node.expanded));
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
        assert_eq!(tree.selected_node().map(|node| node.label.as_str()), Some("works"));
    }

    #[test]
    fn running_duration_stays_on_test_leaf() {
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
        assert_eq!(package.display_duration(), None);
        assert_eq!(module.display_duration(), None);
        assert!(test.display_duration().is_some());
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
        }]);

        assert_eq!(tree.root.children[0].label, "demo");
        assert_eq!(tree.root.children[0].children[0].label, "scenario");
        assert_eq!(tree.root.children[0].children[0].children[0].label, "top_level_test");
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
            String::new(),
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
            String::new(),
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
            String::new(),
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

        assert_eq!(tree.selected_node().map(|node| node.label.as_str()), Some("tests"));
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

        assert_eq!(tree.selected_node().map(|node| node.label.as_str()), Some("tests"));
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
        tree.prepare_for_run(&crate::nextest::RunScope::Test {
            name: "tests::selected".to_owned(),
        });
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
            String::new(),
            Some(std::time::Duration::from_millis(12)),
        );
        tree.finish_test(
            &TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: "tests::outside_scope".to_owned(),
            },
            TestStatus::Failed,
            String::new(),
            "old outside stderr".to_owned(),
            Some(std::time::Duration::from_millis(20)),
        );

        tree.prepare_for_run(&crate::nextest::RunScope::Test {
            name: "tests::selected".to_owned(),
        });

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
        let mut tree =
            Tree::from_tests(vec![discovered_test("demo::demo", "demo", "tests", "old")]);
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
                node.started_at = (status == TestStatus::Running).then(Instant::now);
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
        }
    }
