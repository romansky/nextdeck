    use super::*;
    use crate::output_pane::{SearchEditorInput, SearchEditorKey};
    use crate::tree::{DiscoveredTest, TestKey, TestStatus};

    fn app_with_tree(tree: Tree) -> App {
        App::with_settings(tree, AppSettings::default())
    }

    #[test]
    fn command_context_uses_normal_input_without_overlay_by_default() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

        assert_eq!(
            app.command_context(),
            CommandContext {
                input: InputMode::Normal(CommandFocus::Tests),
                overlay: None,
            }
        );

        app.focus = FocusPane::Output;
        assert_eq!(
            app.command_context(),
            CommandContext {
                input: InputMode::Normal(CommandFocus::Output),
                overlay: None,
            }
        );
    }

    #[test]
    fn command_context_keeps_inline_output_search_out_of_overlay_state() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

        app.apply_command(AppCommand::StartOutputSearch);

        assert_eq!(
            app.command_context(),
            CommandContext {
                input: InputMode::OutputSearchInline,
                overlay: None,
            }
        );
    }

    #[test]
    fn command_context_distinguishes_settings_browsing_from_text_input() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

        app.global_settings.modal_open = true;
        assert_eq!(
            app.command_context(),
            CommandContext {
                input: InputMode::SettingsModal,
                overlay: Some(OverlayMode::Settings),
            }
        );

        app.global_settings.open_with_editing = true;
        assert_eq!(
            app.command_context(),
            CommandContext {
                input: InputMode::SettingsOpenWith,
                overlay: Some(OverlayMode::Settings),
            }
        );
    }

    #[test]
    fn command_context_routes_top_help_overlay_to_help_input() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

        app.global_settings.modal_open = true;
        app.show_help = true;

        assert_eq!(
            app.command_context(),
            CommandContext {
                input: InputMode::Help,
                overlay: Some(OverlayMode::Help),
            }
        );
    }

    #[test]
    fn tree_scroll_follows_selection_past_viewport() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(30)));
        expand_all(&mut app.tree.root);
        app.set_viewport_sizes(7, 7);

        for _ in 0..20 {
            app.select_next();
            assert_selection_visible(&app);
        }

        assert!(app.tree_scroll > 0);
    }

    #[test]
    fn tree_scroll_reclamps_when_viewport_height_changes() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(30)));
        expand_all(&mut app.tree.root);
        app.set_viewport_sizes(16, 7);
        app.select_last();
        assert_selection_visible(&app);

        app.set_viewport_sizes(5, 7);
        assert_selection_visible(&app);
    }

    #[test]
    fn resize_tests_pane_updates_settings_and_requests_save() {
        let mut app = App::with_settings(
            Tree::from_tests(test_rows(1)),
            AppSettings {
                tree_width_percent: 45,
                ..AppSettings::default()
            },
        );

        let effect = app.resize_tests_pane(5);

        assert_eq!(app.settings.tree_width_percent, 50);
        assert_eq!(
            effect,
            AppEffect::SaveSettings(AppSettings {
                tree_width_percent: 50,
                ..AppSettings::default()
            })
        );
    }

    #[test]
    fn resize_tests_pane_clamps_to_supported_range() {
        let mut app = App::with_settings(
            Tree::from_tests(test_rows(1)),
            AppSettings {
                tree_width_percent: 25,
                ..AppSettings::default()
            },
        );

        let effect = app.resize_tests_pane(-5);

        assert_eq!(app.settings.tree_width_percent, 25);
        assert_eq!(effect, AppEffect::None);
    }

    #[test]
    fn settings_modal_updates_theme_and_accessibility_settings() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        app.apply_command(AppCommand::OpenSettings);

        app.global_settings.selected = SettingsField::Theme;
        let effect = app.apply_command(AppCommand::SettingsAdjustRight);
        assert_eq!(app.settings.theme_mode, config::ThemePreference::Dark);
        assert_eq!(effect, AppEffect::SaveSettings(app.settings.clone()));

        app.global_settings.selected = SettingsField::ColorBlindMode;
        let effect = app.apply_command(AppCommand::SettingsActivate);
        assert!(app.settings.color_blind_mode);
        assert_eq!(effect, AppEffect::SaveSettings(app.settings.clone()));

        app.global_settings.selected = SettingsField::StorageThreshold;
        let effect = app.apply_command(AppCommand::SettingsAdjustRight);
        assert_eq!(app.settings.storage_low_space_threshold_gb, 11);
        assert_eq!(app.status, "Low disk threshold: 11 GiB");
        assert_eq!(effect, AppEffect::SaveSettings(app.settings.clone()));
    }

    #[test]
    fn settings_modal_edits_open_with_command() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        app.apply_command(AppCommand::OpenSettings);
        app.apply_command(AppCommand::SettingsActivate);
        app.apply_command(AppCommand::SettingsOpenWithEdit(InputFieldInput::char('i')));
        app.apply_command(AppCommand::SettingsOpenWithEdit(InputFieldInput::char('d')));

        let effect = app.apply_command(AppCommand::CommitOpenWithSetting);

        assert_eq!(app.settings.open_with_command.as_deref(), Some("id"));
        assert_eq!(effect, AppEffect::SaveSettings(app.settings.clone()));
    }

    #[test]
    fn settings_modal_appends_to_existing_open_with_command() {
        let mut app = App::with_settings(
            Tree::from_tests(test_rows(1)),
            AppSettings {
                open_with_command: Some("idea".to_owned()),
                ..AppSettings::default()
            },
        );
        app.apply_command(AppCommand::OpenSettings);
        app.apply_command(AppCommand::SettingsActivate);
        app.apply_command(AppCommand::SettingsOpenWithEdit(InputFieldInput::char('X')));

        let effect = app.apply_command(AppCommand::CommitOpenWithSetting);

        assert_eq!(app.settings.open_with_command.as_deref(), Some("ideaX"));
        assert_eq!(effect, AppEffect::SaveSettings(app.settings.clone()));
    }

    #[test]
    fn command_failure_is_visible_and_not_overwritten_by_done_summary() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(2)));
        assert!(app.begin_run(&RunRequest::default()));

        app.apply_run_event(RunEvent::RunnerOutput(
            "nextest failed to start: no such command".to_owned(),
        ));
        app.apply_run_event(RunEvent::RunnerFinished { exit_code: None });

        assert_eq!(app.run.outcome, RunOutcome::CommandFailed);
        assert_eq!(app.run_result_label(), "command failed");
        assert_eq!(app.run_status_label(), "not running");
        assert_eq!(app.status, "Command failed: nextest did not complete");
    }

    #[test]
    fn stop_run_command_only_emits_effect_while_running() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

        assert_eq!(app.apply_command(AppCommand::StopRun), AppEffect::None);
        assert_eq!(app.status, "No run in progress");

        assert!(app.begin_run(&RunRequest::default()));
        assert_eq!(app.apply_command(AppCommand::StopRun), AppEffect::StopRun);
        assert_eq!(app.status, "Stopping run...");
    }

    #[test]
    fn begin_run_refreshes_storage_status() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

        assert!(!app.disk_usage.loading);
        assert!(app.begin_run(&RunRequest::default()));

        assert!(app.disk_usage.loading);
    }

    #[test]
    fn stopped_run_records_stopped_result_and_clears_running_tests() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        assert!(app.begin_run(&RunRequest::default()));
        app.apply_run_event(RunEvent::TestStarted { key: test_key(0) });

        app.apply_run_event(RunEvent::RunnerStopped);

        assert!(!app.running);
        assert_eq!(app.run.outcome, RunOutcome::Stopped);
        assert_eq!(app.run_result_label(), "stopped");
        assert_eq!(app.run_status_label(), "not running");
        assert_eq!(app.tree.status_counts_for_scope(&RunScope::Workspace).running, 0);
        assert!(app.status.starts_with("Stopped:"));
    }

    #[test]
    fn run_phase_starts_as_building_then_switches_to_running_tests() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        assert!(app.begin_run(&RunRequest::default()));

        assert_eq!(app.run.phase, RunPhase::Building);
        assert_eq!(app.run_status_label(), "building");
        assert_eq!(app.status, "Building workspace");
        assert!(app.build_duration().is_some());
        assert_eq!(app.test_duration(), None);

        app.apply_run_event(RunEvent::SuiteStarted { test_count: 1 });

        assert_eq!(app.run.phase, RunPhase::RunningTests);
        assert_eq!(app.run_status_label(), "running tests");
        assert_eq!(app.status, "Running tests for workspace");
        assert!(app.build_duration().is_some());
        assert!(app.test_duration().is_some());
    }

    #[test]
    fn command_failure_before_test_start_records_build_time_only() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        assert!(app.begin_run(&RunRequest::default()));

        app.apply_run_event(RunEvent::RunnerFinished { exit_code: Some(101) });

        assert_eq!(app.run.phase, RunPhase::NotRunning);
        assert_eq!(app.run_status_label(), "not running");
        assert!(app.run_duration().is_some());
        assert!(app.build_duration().is_some());
        assert_eq!(app.test_duration(), None);
    }

    #[test]
    fn failing_test_run_reports_failed_result() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        assert!(app.begin_run(&RunRequest::default()));
        let key = test_key(0);

        app.apply_run_event(RunEvent::TestStarted { key: key.clone() });
        app.apply_run_event(RunEvent::TestFinished {
            key,
            status: TestStatus::Failed,
            stdout: String::new(),
            stderr: "boom".to_owned(),
            duration: Some(Duration::from_millis(7)),
        });
        app.apply_run_event(RunEvent::RunnerFinished {
            exit_code: Some(101),
        });

        assert_eq!(app.run.outcome, RunOutcome::Failed);
        assert_eq!(app.run_result_label(), "failed");
        assert!(app.status.starts_with("Failed:"));
        assert!(app.status.contains("1 failed"));
    }

    #[test]
    fn scoped_run_summary_counts_only_the_scope() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(2)));
        let request = RunRequest {
            scope: RunScope::Test {
                name: "tests::case_00".to_owned(),
            },
        };
        assert!(app.begin_run(&request));
        let key = test_key(0);

        app.apply_run_event(RunEvent::TestStarted { key: key.clone() });
        app.apply_run_event(RunEvent::TestFinished {
            key,
            status: TestStatus::Passed,
            stdout: String::new(),
            stderr: String::new(),
            duration: Some(Duration::from_millis(3)),
        });
        app.apply_run_event(RunEvent::RunnerFinished { exit_code: Some(0) });

        assert_eq!(app.run.outcome, RunOutcome::Passed);
        assert_eq!(app.run_progress(), (1, 1));
        assert_eq!(app.status, "Passed: 1 passed, 0 skipped, 0 ignored");
    }

    #[test]
    fn ignored_start_event_during_workspace_run_stays_ignored() {
        let mut tests = test_rows(2);
        tests[1].ignored = true;
        tests[1].status = TestStatus::Ignored;
        let mut app = app_with_tree(Tree::from_tests(tests));
        assert!(app.begin_run(&RunRequest::default()));

        app.apply_run_event(RunEvent::TestStarted { key: test_key(1) });

        let counts = app.tree.status_counts_for_scope(&RunScope::Workspace);
        assert_eq!(counts.running, 0);
        assert_eq!(counts.ignored, 1);
        assert_eq!(app.run_progress(), (0, 1));
    }

    #[test]
    fn new_run_resets_previous_run_metadata_and_result() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(2)));
        assert!(app.begin_run(&RunRequest::default()));
        app.apply_run_event(RunEvent::RunMetadata {
            run_id: Some("old-run".to_owned()),
            profile: Some("default".to_owned()),
        });
        app.apply_run_event(RunEvent::TestFinished {
            key: test_key(0),
            status: TestStatus::Passed,
            stdout: "stale stdout".to_owned(),
            stderr: String::new(),
            duration: Some(Duration::from_millis(9)),
        });
        app.apply_run_event(RunEvent::TestFinished {
            key: test_key(1),
            status: TestStatus::Failed,
            stdout: String::new(),
            stderr: "stale stderr".to_owned(),
            duration: Some(Duration::from_millis(11)),
        });
        app.apply_run_event(RunEvent::RunnerFinished {
            exit_code: Some(101),
        });
        assert_eq!(app.run.outcome, RunOutcome::Failed);
        app.output_scroll = 10;
        app.output_follow = false;
        app.output_search.current_line = Some(3);

        assert!(app.begin_run(&RunRequest {
            scope: RunScope::Test {
                name: "tests::case_00".to_owned(),
            },
        }));

        assert_eq!(app.run.run_id, None);
        assert_eq!(app.run.outcome, RunOutcome::Running);
        assert_eq!(app.run.exit_code, None);
        assert_eq!(app.run_result_label(), "running");
        assert_eq!(app.run.phase, RunPhase::Building);
        assert_eq!(app.run_status_label(), "building");
        assert!(app.build_duration().is_some());
        assert_eq!(app.test_duration(), None);
        assert_eq!(app.run_progress(), (0, 1));
        assert_eq!(app.output_scroll, 0);
        assert!(app.output_follow);
        assert_eq!(app.output_search.current_line, None);
        assert!(!app.tree.selected_output().contains("stale stdout"));
        assert!(!app.tree.selected_output().contains("stale stderr"));
    }

    #[test]
    fn filter_toggle_during_run_preserves_visible_selection_and_output_state() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
        expand_all(&mut app.tree.root);
        assert!(app.begin_run(&RunRequest::default()));
        app.apply_run_event(RunEvent::TestFinished {
            key: test_key(0),
            status: TestStatus::Passed,
            stdout: String::new(),
            stderr: String::new(),
            duration: Some(Duration::from_millis(5)),
        });
        app.apply_run_event(RunEvent::TestStarted { key: test_key(1) });
        select_visible_path(&mut app, "demo::tests::case_01");
        app.output_scroll = 7;
        app.output_follow = false;
        app.output_search.current_line = Some(2);

        app.apply_command(AppCommand::ToggleShowSuccess);

        assert_eq!(app.tree.selected_path(), "demo::tests::case_01");
        assert_eq!(app.output_scroll, 7);
        assert!(!app.output_follow);
        assert_eq!(app.output_search.current_line, Some(2));
        assert!(app.running);
        assert_eq!(app.run.outcome, RunOutcome::Running);
    }

    #[test]
    fn filter_toggle_resets_output_when_selected_source_is_hidden() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
        expand_all(&mut app.tree.root);
        app.tree.finish_test(
            &test_key(0),
            TestStatus::Passed,
            "old output".to_owned(),
            String::new(),
            Some(Duration::from_millis(5)),
        );
        select_visible_path(&mut app, "demo::tests::case_00");
        app.output_scroll = 7;
        app.output_follow = false;
        app.output_search.current_line = Some(2);

        app.apply_command(AppCommand::ToggleShowSuccess);

        assert_ne!(app.tree.selected_path(), "demo::tests::case_00");
        assert_eq!(app.output_scroll, 0);
        assert!(app.output_follow);
        assert_eq!(app.output_search.current_line, None);
    }

    #[test]
    fn left_and_right_do_not_mutate_tree_when_output_is_focused() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        app.tree.select_next();
        app.focus = FocusPane::Output;
        let before = visible_labels(&app);

        app.apply_command(AppCommand::MoveRight);
        app.apply_command(AppCommand::MoveLeft);

        assert_eq!(visible_labels(&app), before);
        assert_eq!(app.tree.selected_path(), "demo");
    }

    #[test]
    fn output_search_filter_keeps_matching_lines() {
        let mut app = app_with_finished_output("alpha\npanic here\nomega", "");
        app.output_search.query = "panic".to_owned();
        app.output_search.filter = true;

        assert_eq!(app.output_text(), "panic here");
    }

    #[test]
    fn output_search_literal_is_case_insensitive_by_default() {
        let mut app = app_with_finished_output("PANIC\nok", "");
        app.output_search.query = "panic".to_owned();
        app.output_search.filter = true;

        assert_eq!(app.output_text(), "PANIC");

        app.apply_command(AppCommand::ToggleOutputCaseSensitive);

        assert_eq!(app.output_text(), "No output lines match 'panic'");
    }

    #[test]
    fn output_search_regex_filters_and_reports_invalid_regex() {
        let mut app = app_with_finished_output("case_01\ncase_aa\ncase_22", "");
        app.output_search.query = r"case_\d+".to_owned();
        app.output_search.filter = true;
        app.output_search.regex = true;

        assert_eq!(app.output_text(), "case_01\ncase_22");

        app.output_search.query = "(".to_owned();

        assert!(app.output_text().starts_with("Invalid output search regex:"));
    }

    #[test]
    fn output_find_next_and_previous_scroll_to_matching_lines() {
        let mut app = app_with_finished_output("zero\nmatch one\nskip\nmatch two", "");
        app.output_search.query = "match".to_owned();
        app.output_page_size = 2;

        app.apply_command(AppCommand::FindNextOutputMatch);

        assert_eq!(app.output_scroll, 1);
        assert_eq!(app.output_search.current_line, Some(1));

        app.apply_command(AppCommand::FindNextOutputMatch);

        assert_eq!(app.output_scroll, 3);
        assert_eq!(app.output_search.current_line, Some(3));

        app.apply_command(AppCommand::FindPreviousOutputMatch);

        assert_eq!(app.output_scroll, 1);
        assert_eq!(app.output_search.current_line, Some(1));
    }

    #[test]
    fn output_search_input_opens_modal_then_apply_finds_match() {
        let mut app = app_with_finished_output("zero\npanic\nok", "");
        app.output_page_size = 2;

        app.apply_command(AppCommand::StartOutputSearch);
        search_type(&mut app, "px");
        app.apply_command(AppCommand::OutputSearchEdit(SearchEditorInput::new(
            SearchEditorKey::Backspace,
            false,
            false,
            false,
        )));
        search_type(&mut app, "anic");
        assert_eq!(app.output_search.query, "");

        app.apply_command(AppCommand::OpenOutputSearchModal);
        assert!(app.output_search.modal_open);
        app.output_search.modal_focus = SearchModalFocus::Apply;
        app.apply_command(AppCommand::SearchModalActivate);

        assert!(!app.output_search.input_active);
        assert!(!app.output_search.modal_open);
        assert_eq!(app.output_search.query, "panic");
        assert_eq!(app.output_scroll, 1);
    }

    #[test]
    fn output_search_draft_does_not_filter_until_applied() {
        let mut app = app_with_finished_output("alpha\npanic\nomega", "");
        app.output_search.filter = true;

        app.apply_command(AppCommand::StartOutputSearch);
        search_type(&mut app, "panic");

        assert_eq!(app.output_search.query, "");
        assert!(app.output_text().contains("alpha"));
        assert!(app.output_text().contains("omega"));

        app.apply_command(AppCommand::ApplyOutputSearch);

        assert_eq!(app.output_search.query, "panic");
        assert_eq!(app.output_text(), "panic");
    }

    #[test]
    fn output_search_modal_controls_apply_draft_filters() {
        let mut app = app_with_finished_output("case_01\ncase_aa\ncase_22", "");

        app.apply_command(AppCommand::StartOutputSearch);
        search_type(&mut app, r"case_\d+");
        app.apply_command(AppCommand::OpenOutputSearchModal);
        app.output_search.modal_focus = SearchModalFocus::Filter;
        app.apply_command(AppCommand::SearchModalActivate);
        app.output_search.modal_focus = SearchModalFocus::Regex;
        app.apply_command(AppCommand::SearchModalActivate);
        app.apply_command(AppCommand::ApplyOutputSearch);

        assert!(app.output_search.filter);
        assert!(app.output_search.regex);
        assert_eq!(app.output_text(), "case_01\ncase_22");
    }

    #[test]
    fn output_search_clear_keeps_input_active_and_resets_match() {
        let mut app = app_with_finished_output("zero\npanic\nok", "");

        app.apply_command(AppCommand::StartOutputSearch);
        search_type(&mut app, "pa");
        app.apply_command(AppCommand::ApplyOutputSearch);
        assert_eq!(app.output_search.current_line, Some(1));

        app.apply_command(AppCommand::StartOutputSearch);
        app.apply_command(AppCommand::ClearOutputSearch);

        assert!(app.output_search.input_active);
        assert_eq!(app.output_search.draft_query, "");
        assert_eq!(app.output_search.query, "pa");
        assert_eq!(app.output_search.current_line, Some(1));
        assert_eq!(app.status, "Output search draft cleared");
    }

    #[test]
    fn discovery_error_uses_output_scroll_and_search() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
        app.set_viewport_sizes(5, 5);

        app.apply_discovery_event(DiscoveryEvent::Finished(Err(
            "first\nsecond\nneedle\nfourth".to_owned(),
        )));

        assert_eq!(
            app.command_context().input,
            InputMode::Normal(CommandFocus::Output)
        );
        assert_eq!(
            app.command_context().overlay,
            Some(OverlayMode::DiscoveryError)
        );
        app.apply_command(AppCommand::MoveDown);
        assert_eq!(app.output_scroll, 1);

        app.apply_command(AppCommand::StartOutputSearch);
        search_type(&mut app, "needle");
        app.apply_command(AppCommand::ApplyOutputSearch);

        assert_eq!(app.output_search.current_line, Some(4));
        assert_eq!(app.status, "Output match 1/1 for 'needle'");
    }

    #[test]
    fn refresh_tests_retries_after_discovery_error() {
        let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
        app.apply_discovery_event(DiscoveryEvent::Finished(Err("boom".to_owned())));

        let effect = app.apply_command(AppCommand::RefreshTests);

        assert_eq!(effect, AppEffect::StartDiscovery);
        assert!(app.discovery.running);
        assert_eq!(app.discovery.error, None);
        assert_eq!(app.status, "Discovering tests");
    }

    #[test]
    fn output_search_editor_can_insert_at_cursor_and_apply() {
        let mut app = app_with_finished_output("zero\npanic\nok", "");

        app.apply_command(AppCommand::StartOutputSearch);
        search_type(&mut app, "pnic");
        app.apply_command(AppCommand::OutputSearchEdit(SearchEditorInput::new(
            SearchEditorKey::Left,
            false,
            false,
            false,
        )));
        app.apply_command(AppCommand::OutputSearchEdit(SearchEditorInput::new(
            SearchEditorKey::Left,
            false,
            false,
            false,
        )));
        app.apply_command(AppCommand::OutputSearchEdit(SearchEditorInput::new(
            SearchEditorKey::Left,
            false,
            false,
            false,
        )));
        search_type(&mut app, "a");
        app.apply_command(AppCommand::ApplyOutputSearch);

        assert_eq!(app.output_search.query, "panic");
        assert_eq!(app.output_search.current_line, Some(1));
    }

    fn assert_selection_visible(app: &App) {
        let selected = app.tree.selected_index();
        assert!(
            selected >= app.tree_scroll,
            "selected {selected} should be >= scroll {}",
            app.tree_scroll
        );
        assert!(
            selected < app.tree_scroll + app.tree_page_size,
            "selected {selected} should be < scroll {} + page {}",
            app.tree_scroll,
            app.tree_page_size
        );
    }

    fn test_rows(count: usize) -> Vec<DiscoveredTest> {
        (0..count)
            .map(|index| DiscoveredTest {
                key: test_key(index),
                package: "demo".to_owned(),
                binary: "demo".to_owned(),
                binary_kind: "lib".to_owned(),
                cwd: std::path::PathBuf::from("."),
                source_path: None,
                module: Some("tests".to_owned()),
                name: format!("case_{index:02}"),
                full_name: format!("tests::case_{index:02}"),
                status: TestStatus::Pending,
                ignored: false,
            })
            .collect()
    }

    fn test_key(index: usize) -> TestKey {
        TestKey {
            binary_id: Some("demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: format!("tests::case_{index:02}"),
        }
    }

    fn app_with_finished_output(stdout: &str, stderr: &str) -> App {
        let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
        expand_all(&mut app.tree.root);
        app.tree.finish_test(
            &test_key(0),
            TestStatus::Passed,
            stdout.to_owned(),
            stderr.to_owned(),
            None,
        );
        app.tree.select_next();
        app.tree.select_next();
        app.tree.select_next();
        app
    }

    fn search_type(app: &mut App, text: &str) {
        for char in text.chars() {
            app.apply_command(AppCommand::OutputSearchEdit(SearchEditorInput::char(char)));
        }
    }

    fn select_visible_path(app: &mut App, path: &str) {
        app.tree.select_first();
        while app.tree.selected_path() != path {
            let before = app.tree.selected_index();
            app.tree.select_next();
            assert_ne!(app.tree.selected_index(), before, "visible path {path}");
        }
    }

    fn visible_labels(app: &App) -> Vec<String> {
        app.tree
            .visible_rows()
            .iter()
            .map(|row| row.node.label.clone())
            .collect()
    }

    fn expand_all(node: &mut TestNode) {
        node.expanded = true;
        for child in &mut node.children {
            expand_all(child);
        }
    }
