use super::*;
use crate::command::command_for_input;
use crate::input::InputEvent;
use crate::input_field::{InputFieldInput, InputFieldKey};
use crate::test_events::TestEventRun;
use crate::tree::{DiscoveredTest, TestKey, TestStatus};
use crate::xtask::{
    SCHEMA_VERSION, XtaskArgSpec, XtaskCommandSpec, XtaskDetailFocus, XtaskManifest, XtaskValueSpec,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use nextdeck_test_events::{Level, TestEvent};

fn app_with_tree(tree: Tree) -> App {
    App::with_settings(tree, AppSettings::default())
}

fn output_chunks(text: impl Into<String>) -> Vec<TestOutputChunk> {
    vec![TestOutputChunk::Text(text.into())]
}

#[derive(Clone, Copy, Debug)]
struct TestViewportSizes {
    tree_page_size: usize,
    main_output_page_size: usize,
    xtask_parameters_page_size: usize,
    xtask_output_page_size: usize,
    test_events_output_page_size: usize,
    test_stack_sample_output_page_size: usize,
    test_details_page_size: usize,
}

impl Default for TestViewportSizes {
    fn default() -> Self {
        Self {
            tree_page_size: 1,
            main_output_page_size: 1,
            xtask_parameters_page_size: 1,
            xtask_output_page_size: 1,
            test_events_output_page_size: 1,
            test_stack_sample_output_page_size: 1,
            test_details_page_size: 1,
        }
    }
}

fn test_viewport_metrics(sizes: TestViewportSizes) -> FrameViewportMetrics {
    FrameViewportMetrics::new(vec![
        ViewportSpec::new(ViewportId::Tree, ViewportMetrics::new(sizes.tree_page_size)),
        ViewportSpec::new(
            ViewportId::MainOutput,
            ViewportMetrics::new(sizes.main_output_page_size),
        ),
        ViewportSpec::new(
            ViewportId::XtaskParameters,
            ViewportMetrics::new(sizes.xtask_parameters_page_size),
        ),
        ViewportSpec::new(
            ViewportId::XtaskOutput,
            ViewportMetrics::new(sizes.xtask_output_page_size),
        ),
        ViewportSpec::new(
            ViewportId::TestEventsOutput,
            ViewportMetrics::new(sizes.test_events_output_page_size),
        ),
        ViewportSpec::new(
            ViewportId::TestStackSampleOutput,
            ViewportMetrics::new(sizes.test_stack_sample_output_page_size),
        ),
        ViewportSpec::new(
            ViewportId::TestDetails,
            ViewportMetrics::new(sizes.test_details_page_size),
        ),
    ])
}

fn prepare_test_viewports(app: &mut App, tree_page_size: usize, main_output_page_size: usize) {
    app.prepare_frame(test_viewport_metrics(TestViewportSizes {
        tree_page_size,
        main_output_page_size,
        ..Default::default()
    }));
}

fn sample_xtask_manifest() -> XtaskManifest {
    XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: vec![XtaskCommandSpec {
            name: "ship".to_owned(),
            about: Some("Ship it".to_owned()),
            args: vec![XtaskArgSpec {
                name: "version".to_owned(),
                long: Some("version".to_owned()),
                short: None,
                help: Some("Version".to_owned()),
                required: true,
                value: XtaskValueSpec::String { default: None },
            }],
        }],
    }
}

#[test]
fn startup_effects_are_created_by_app_state() {
    let mut app = App::discovering(AppSettings::default());

    let effects = app.startup_effects();

    assert_eq!(
        effects,
        vec![
            AppEffect::StartDiscovery(RequestId(1)),
            AppEffect::RefreshXtasks(RequestId(1)),
            AppEffect::RefreshDiskUsage(RequestId(1)),
        ]
    );
    assert!(app.discovery.running);
    assert!(app.xtasks.loading);
    assert!(app.disk_usage.loading);
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
fn command_context_uses_test_details_modal_when_open() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

    app.show_test_details = true;

    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::TestDetailsModal,
            overlay: Some(OverlayMode::TestDetails),
        }
    );
}

#[test]
fn command_context_uses_output_modes_inside_stack_sampling_panel() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.show_test_details = true;
    app.test_stack_sample
        .start("Test stack sample: case".to_owned());

    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::TestStackSampleModal,
            overlay: Some(OverlayMode::TestDetails),
        }
    );

    app.apply_command(AppCommand::StartOutputSearch);
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::OutputSearchInline,
            overlay: Some(OverlayMode::TestDetails),
        }
    );
}

#[test]
fn command_context_uses_custom_run_input_inside_test_details() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

    app.apply_command(AppCommand::OpenCustomRun);
    app.custom_run.next_field();
    app.custom_run.next_field();
    app.custom_run.begin_edit_selected();

    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::CustomRunInput,
            overlay: Some(OverlayMode::TestDetails),
        }
    );
}

#[test]
fn command_context_uses_custom_run_modal_inside_test_details() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

    app.apply_command(AppCommand::OpenCustomRun);

    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::CustomRunModal,
            overlay: Some(OverlayMode::TestDetails),
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
fn command_context_uses_xtask_modal_and_input_modes() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

    app.apply_command(AppCommand::OpenXtasks);
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::XtaskModal,
            overlay: Some(OverlayMode::Xtasks),
        }
    );

    app.xtasks.set_manifest(sample_xtask_manifest());
    assert_eq!(
        app.apply_command(AppCommand::OpenSelectedXtask),
        AppEffect::None
    );
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::XtaskCommandModal(XtaskDetailFocus::Parameters),
            overlay: Some(OverlayMode::Xtasks),
        }
    );

    assert_eq!(
        app.apply_command(AppCommand::XtaskActivateArg),
        AppEffect::None
    );
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::XtaskInput,
            overlay: Some(OverlayMode::Xtasks),
        }
    );
}

#[test]
fn xtask_command_frame_tracks_detail_focus() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);

    assert_eq!(app.xtasks.detail_focus, XtaskDetailFocus::Parameters);

    app.apply_command(AppCommand::ToggleXtaskDetailFocus);

    assert_eq!(app.xtasks.detail_focus, XtaskDetailFocus::Output);
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::XtaskCommandModal(XtaskDetailFocus::Output),
            overlay: Some(OverlayMode::Xtasks),
        }
    );

    app.xtasks.output.apply_viewport_page_size(2);
    let request_id = app.xtasks.begin_run("cargo xtask ship".to_owned());
    app.apply_xtask_event(crate::xtask::XtaskEvent::RunOutput {
        request_id,
        chunk: crate::xtask::XtaskRunChunk {
            stream: crate::xtask::XtaskOutputStream::Stdout,
            text: "one\ntwo\nthree\n".to_owned(),
        },
    });
    let followed_scroll = app.xtasks.output.scroll();

    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::LineUp));

    assert_eq!(app.xtasks.detail_focus, XtaskDetailFocus::Output);
    assert_eq!(
        app.xtasks.output.scroll(),
        followed_scroll.saturating_sub(1)
    );
    assert!(!app.xtasks.output.follow());
}

#[test]
fn generic_scroll_reaches_xtask_parameter_footer_lines() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    app.xtasks.open();
    app.xtasks.set_manifest(XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: vec![XtaskCommandSpec {
            name: "ship".to_owned(),
            about: Some("Ship package".to_owned()),
            args: Vec::new(),
        }],
    });
    assert!(app.xtasks.open_detail());
    app.xtasks.apply_parameters_viewport_metrics(2);

    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::PageDown));

    assert_eq!(app.xtasks.parameters_viewport.scroll(), 2);

    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::PageUp));

    assert_eq!(app.xtasks.parameters_viewport.scroll(), 0);
}

#[test]
fn xtask_command_frame_uses_dedicated_output_search_modes() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);
    app.xtasks.output.set_follow(true);

    app.apply_command(AppCommand::StartOutputSearch);
    assert_eq!(app.xtasks.detail_focus, XtaskDetailFocus::Output);
    assert!(!app.xtasks.output.follow());
    assert!(app.xtasks.output.search.input_active);
    assert!(!app.main_output.search.input_active);
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::OutputSearchInline,
            overlay: Some(OverlayMode::Xtasks),
        }
    );

    app.apply_command(AppCommand::OpenOutputSearchModal);
    assert!(app.xtasks.output.search.modal_open);
    assert!(!app.main_output.search.modal_open);
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::OutputSearchModal,
            overlay: Some(OverlayMode::Xtasks),
        }
    );
}

#[test]
fn test_events_modal_opens_from_tests_focus_and_uses_dedicated_output_search() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.begin_test_event_run(TestEventRun {
        id: "run-1".to_owned(),
    });
    app.apply_run_event(RunEvent::TestOutput {
        key: test_key(0),
        output: vec![TestOutputChunk::Event(
            TestEvent::new(Level::Info, "cache hit").with_target("artifact-cache"),
        )],
    });

    assert_eq!(
        app.test_events.latest_event_label(),
        "info artifact-cache: cache hit •"
    );

    let effect = app.apply_command(AppCommand::OpenTestEvents);

    assert_eq!(effect, AppEffect::None);
    assert!(app.test_events.modal_open);
    assert_eq!(
        app.test_events.latest_event_label(),
        "info artifact-cache: cache hit"
    );
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::TestEventsModal(crate::test_events::TestEventsFocus::Runs),
            overlay: Some(OverlayMode::TestEvents),
        }
    );

    app.apply_command(AppCommand::ToggleTestEventsFocus);
    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "cache");
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.test_events.output.search.query, "cache");
    assert_eq!(app.main_output.search.query, "");
    assert!(app.test_events.output_text().contains("cache hit"));
}

#[test]
fn test_event_run_events_are_inlined_into_matching_test_output() {
    nextdeck_test_events::event!(
        "verifying inline test event output";
        "component" => "app",
    );

    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    let event = TestEvent::new(Level::Info, "cache hit").with_target("artifact-cache");

    app.apply_run_event(RunEvent::TestOutput {
        key: test_key(0),
        output: vec![TestOutputChunk::Event(event)],
    });
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();

    let output = app.tree.selected_output();
    assert!(output.contains("@ event info artifact-cache: cache hit"));
}

#[test]
fn live_output_chunks_interleave_with_inline_events() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    let key = test_key(0);
    app.apply_run_event(RunEvent::TestStarted { key: key.clone() });
    app.apply_run_event(RunEvent::TestOutput {
        key: key.clone(),
        output: vec![
            TestOutputChunk::Text("DOGFOOD_OUTPUT stdout before event".to_owned()),
            TestOutputChunk::Event(
                TestEvent::new(Level::Info, "between output chunks").with_target("dogfood-output"),
            ),
        ],
    });
    app.apply_run_event(RunEvent::TestOutput {
        key,
        output: output_chunks(
            "DOGFOOD_OUTPUT stdout after event\nDOGFOOD_OUTPUT stderr after event",
        ),
    });

    let output = app.tree.selected_output();
    let mut cursor = 0;
    for needle in [
        "DOGFOOD_OUTPUT stdout before event",
        "@ event info dogfood-output: between output chunks",
        "DOGFOOD_OUTPUT stdout after event",
        "DOGFOOD_OUTPUT stderr after event",
    ] {
        let offset = output[cursor..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?} after byte {cursor} in:\n{output}"));
        cursor += offset + needle.len();
    }
}

#[test]
fn test_events_run_finishes_with_run_result_label() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    assert!(app.begin_run(&RunRequest::default()).is_some());
    app.begin_test_event_run(TestEventRun {
        id: "run-1".to_owned(),
    });

    app.apply_run_event(RunEvent::RunnerFinished { exit_code: Some(0) });

    assert_eq!(app.test_events.runs[0].status, "passed");
}

#[test]
fn xtask_detail_close_cancels_output_search_interaction() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "draft");
    app.apply_command(AppCommand::OpenOutputSearchModal);
    assert!(app.xtasks.output.search.modal_open);

    app.apply_command(AppCommand::CloseXtaskDetails);

    assert!(!app.xtasks.detail_open);
    assert!(!app.xtasks.output.search.input_active);
    assert!(!app.xtasks.output.search.modal_open);
    assert_eq!(app.xtasks.output.search.draft_query(), "");
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::XtaskModal,
            overlay: Some(OverlayMode::Xtasks),
        }
    );
}

#[test]
fn xtask_modal_builds_run_effect_with_named_args() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);
    app.apply_command(AppCommand::XtaskActivateArg);
    for ch in "1.2.3".chars() {
        app.apply_command(AppCommand::XtaskEdit(InputFieldInput::char(ch)));
    }
    assert_eq!(
        app.apply_command(AppCommand::CommitXtaskEdit),
        AppEffect::None
    );

    let effect = app.apply_command(AppCommand::RunXtask);

    assert_eq!(
        effect,
        AppEffect::RunXtask(
            app.xtasks.run_request_id,
            crate::xtask::XtaskRunRequest {
                command: "ship".to_owned(),
                args: vec!["--version".to_owned(), "1.2.3".to_owned()],
            }
        )
    );
    assert!(app.xtasks.running);
}

#[test]
fn xtask_open_output_uses_xtask_output_without_replacing_main_output() {
    let mut app = app_with_finished_output("main output\n");
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);
    let request_id = app.xtasks.begin_run("cargo xtask ship".to_owned());
    app.apply_xtask_event(crate::xtask::XtaskEvent::RunOutput {
        request_id,
        chunk: crate::xtask::XtaskRunChunk {
            stream: crate::xtask::XtaskOutputStream::Stdout,
            text: "publishing\n".to_owned(),
        },
    });

    assert!(app.output_text().contains("main output"));
    assert!(!app.output_text().contains("cargo xtask ship"));

    let effect = app.apply_command(AppCommand::OpenOutput);

    let AppEffect::OpenOutput(request) = effect else {
        panic!("expected xtask output open effect, got {effect:?}");
    };
    assert_eq!(request.title, "Xtask: ship");
    assert!(request.text.contains("cargo xtask ship"));
    assert!(request.text.contains("publishing"));
    assert!(!request.text.contains("main output"));
}

#[test]
fn xtask_output_search_filter_isolated_from_main_output_search() {
    let mut app = app_with_finished_output("main alpha\nmain beta\n");
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);
    let request_id = app.xtasks.begin_run("cargo xtask ship".to_owned());
    app.apply_xtask_event(crate::xtask::XtaskEvent::RunOutput {
        request_id,
        chunk: crate::xtask::XtaskRunChunk {
            stream: crate::xtask::XtaskOutputStream::Stdout,
            text: "publishing\n".to_owned(),
        },
    });

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "publishing");
    app.apply_command(AppCommand::ApplyOutputSearch);
    app.apply_command(AppCommand::ToggleOutputFilter);

    assert_eq!(app.xtasks.output.search.query, "publishing");
    assert!(app.xtasks.output.search.filter);
    assert_eq!(app.main_output.search.query, "");
    assert!(!app.main_output.search.filter);
    assert!(app.output_text().contains("main alpha"));

    let effect = app.apply_command(AppCommand::OpenOutput);

    let AppEffect::OpenOutput(request) = effect else {
        panic!("expected xtask output open effect, got {effect:?}");
    };
    assert_eq!(request.text, "publishing");
}

#[test]
fn xtask_detail_close_returns_to_command_picker() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);

    assert!(app.xtasks.modal_open);
    assert!(app.xtasks.detail_open);

    app.apply_command(AppCommand::CloseXtaskDetails);

    assert!(app.xtasks.modal_open);
    assert!(!app.xtasks.detail_open);
    assert_eq!(
        app.command_context(),
        CommandContext {
            input: InputMode::XtaskModal,
            overlay: Some(OverlayMode::Xtasks),
        }
    );
}

#[test]
fn tree_scroll_follows_selection_past_viewport() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(30)));
    expand_all(&mut app.tree.root);
    prepare_test_viewports(&mut app, 5, 5);

    for _ in 0..20 {
        app.select_next();
        assert_selection_visible(&app);
    }

    assert!(app.tree_viewport.scroll() > 0);
}

#[test]
fn tree_scroll_reclamps_when_viewport_height_changes() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(30)));
    expand_all(&mut app.tree.root);
    prepare_test_viewports(&mut app, 14, 5);
    app.select_last();
    assert_selection_visible(&app);

    prepare_test_viewports(&mut app, 3, 5);
    assert_selection_visible(&app);
}

#[test]
fn activate_selected_opens_details_for_test_rows() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();

    app.apply_command(AppCommand::ActivateSelected);

    assert!(app.show_test_details);
    assert_eq!(app.status, "Details opened");

    app.apply_command(AppCommand::OpenCustomRun);
    assert!(app.custom_run.open);

    app.apply_command(AppCommand::CloseCustomRun);
    assert!(app.show_test_details);
    assert!(!app.custom_run.open);

    app.apply_command(AppCommand::CloseTestDetails);

    assert!(!app.show_test_details);
}

#[test]
fn test_details_scroll_follows_selected_run_option_with_detail_lines() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();
    app.prepare_frame(test_viewport_metrics(TestViewportSizes {
        test_details_page_size: 6,
        ..Default::default()
    }));

    app.apply_command(AppCommand::OpenCustomRun);

    for _ in 0..crate::custom_run::CustomRunField::ALL.len() {
        let (start, len, _) = app
            .test_details_focused_range()
            .expect("focused run option");
        let scroll = app.custom_run.viewport.scroll();
        assert!(start >= scroll);
        assert!(start + len <= scroll + app.custom_run.viewport.page_size());

        app.apply_command(AppCommand::CustomRunNext);
    }
}

#[test]
fn test_details_page_scroll_is_not_reset_without_resize() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();
    app.prepare_frame(test_viewport_metrics(TestViewportSizes {
        test_details_page_size: 6,
        ..Default::default()
    }));
    app.apply_command(AppCommand::OpenCustomRun);

    let before = app.custom_run.viewport.scroll();
    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::PageDown));
    let after_page_down = app.custom_run.viewport.scroll();
    app.prepare_frame(test_viewport_metrics(TestViewportSizes {
        test_details_page_size: 6,
        ..Default::default()
    }));

    assert!(after_page_down > before);
    assert_eq!(app.custom_run.viewport.scroll(), after_page_down);
}

#[test]
fn activate_selected_opens_details_for_non_test_rows() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    let before = app.tree.visible_rows().len();

    app.apply_command(AppCommand::ActivateSelected);

    assert!(app.show_test_details);
    assert_eq!(app.tree.visible_rows().len(), before);
}

#[test]
fn sample_test_stacks_requires_running_leaf_test() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();

    assert_eq!(
        app.apply_command(AppCommand::SampleTestStacks),
        AppEffect::None
    );
    assert_eq!(app.status, "No test run in progress");

    app.running = true;
    assert_eq!(
        app.apply_command(AppCommand::SampleTestStacks),
        AppEffect::None
    );
    assert_eq!(app.status, "Selected test is not currently running");

    app.tree.start_test(&test_key(0));
    assert_eq!(
        app.apply_command(AppCommand::SampleTestStacks),
        AppEffect::SampleTestStacks(TestStackSampleRequest {
            title: "Test stack sample: demo::tests::case_00".to_owned(),
            selector: crate::diagnostics::TestProcessSelector {
                binary_path: std::path::PathBuf::from("target/debug/deps/demo"),
                full_name: "tests::case_00".to_owned(),
            },
        })
    );
    assert_eq!(app.status, "Sampling running test stacks...");
    assert!(app.test_stack_sample.running);
    assert!(app.test_stack_sample.open);
    assert_eq!(
        app.test_stack_sample.text,
        "Sampling running test stacks..."
    );

    assert_eq!(
        app.apply_command(AppCommand::SampleTestStacks),
        AppEffect::None
    );
    assert_eq!(app.status, "Sampling running test stacks...");

    app.finish_test_stack_sample(Ok("sample output".to_owned()));
    assert!(!app.test_stack_sample.running);
    assert!(!app.test_stack_sample.failed);
    assert_eq!(app.test_stack_sample.text, "sample output");
}

#[test]
fn failed_stack_sample_stays_in_the_sampling_output_panel() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.show_test_details = true;
    app.test_stack_sample
        .start("Test stack sample: case".to_owned());

    app.finish_test_stack_sample(Err("selected process exited".to_owned()));

    assert!(app.test_stack_sample.open);
    assert!(!app.test_stack_sample.running);
    assert!(app.test_stack_sample.failed);
    assert_eq!(
        app.test_stack_sample.text,
        "Stack sampling failed: selected process exited"
    );
    assert_eq!(app.status, "Stack sampling failed");

    app.apply_command(AppCommand::CloseTestStackSample);
    assert!(app.show_test_details);
    assert!(!app.test_stack_sample.open);
}

#[test]
fn stack_sample_panel_routes_search_scroll_and_open_to_its_output() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.show_test_details = true;
    app.test_stack_sample
        .start("Test stack sample: case".to_owned());
    app.finish_test_stack_sample(Ok("alpha\nbeta\nalpha two\n".to_owned()));
    app.test_stack_sample.output.apply_viewport_page_size(1);

    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::LineUp));
    assert_eq!(app.test_stack_sample.output.scroll(), 1);
    assert_eq!(app.main_output.scroll(), 0);

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "alpha");
    app.apply_command(AppCommand::ApplyOutputSearch);
    app.apply_command(AppCommand::ToggleOutputFilter);

    let AppEffect::OpenOutput(request) = app.apply_command(AppCommand::OpenOutput) else {
        panic!("expected sampling output open effect");
    };
    assert_eq!(request.title, "Test stack sample: case");
    assert_eq!(request.text, "alpha\nalpha two");
    assert_eq!(app.main_output.search.query, "");
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
fn repeated_shift_arrow_resizes_never_start_a_run() {
    let mut app = App::with_settings(
        Tree::from_tests(test_rows(1)),
        AppSettings {
            tree_width_percent: 50,
            ..AppSettings::default()
        },
    );

    for index in 0..200 {
        let code = if index % 2 == 0 {
            KeyCode::Right
        } else {
            KeyCode::Left
        };
        let event = InputEvent::Terminal(Event::Key(KeyEvent::new(code, KeyModifiers::SHIFT)));
        let command = command_for_input(&event, app.command_context());

        assert!(
            matches!(
                command,
                AppCommand::WidenTestsPane | AppCommand::NarrowTestsPane
            ),
            "shift-arrow mapped to {command:?} at index {index}"
        );

        let effect = app.apply_command(command);

        assert!(
            !matches!(effect, AppEffect::StartRun(_)),
            "shift-arrow emitted StartRun at index {index}"
        );
        assert!(!app.running, "app started running at index {index}");
    }

    assert_eq!(app.settings.tree_width_percent, 50);
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

    app.global_settings.selected = SettingsField::OutputPoll;
    let effect = app.apply_command(AppCommand::SettingsAdjustRight);
    assert_eq!(app.settings.test_output_poll_interval_ms, 1250);
    assert_eq!(app.status, "Output poll interval: 1250 ms");
    assert_eq!(effect, AppEffect::SaveSettings(app.settings.clone()));

    app.global_settings.selected = SettingsField::TreeDuration;
    let effect = app.apply_command(AppCommand::SettingsAdjustRight);
    assert_eq!(
        app.settings.tree_duration_mode,
        config::TreeDurationMode::Aggregate
    );
    assert_eq!(app.status, "Tests time: aggregate");
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
    assert!(app.begin_run(&RunRequest::default()).is_some());

    app.apply_run_event(RunEvent::RunnerOutput(
        "nextest failed to start: no such command".to_owned(),
    ));
    app.apply_run_event(RunEvent::RunnerFinished { exit_code: None });

    assert_eq!(app.run.outcome, RunOutcome::CommandFailed);
    assert_eq!(app.run_result_label(), "command failed");
    assert_eq!(app.run_status_label(), "idle");
    assert_eq!(app.status, "Command failed: nextest did not complete");
}

#[test]
fn stop_run_command_only_emits_effect_while_running() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

    assert_eq!(app.apply_command(AppCommand::StopRun), AppEffect::None);
    assert_eq!(app.status, "No run in progress");

    assert!(app.begin_run(&RunRequest::default()).is_some());
    assert_eq!(app.apply_command(AppCommand::StopRun), AppEffect::StopRun);
    assert_eq!(app.status, "Stopping run...");
}

#[test]
fn begin_run_refreshes_storage_status() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));

    assert!(!app.disk_usage.loading);
    assert!(app.begin_run(&RunRequest::default()).is_some());

    assert!(app.disk_usage.loading);
}

#[test]
fn cargo_clean_success_requests_disk_usage_refresh() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    let clean_request_id = app.begin_cargo_clean().expect("clean starts");

    let effect = app.apply_cargo_clean(clean_request_id, Ok(()));

    assert_eq!(effect, AppEffect::RefreshDiskUsage(RequestId(1)));
    assert!(!app.disk_cleanup.running);
    assert!(app.disk_usage.loading);
    assert_eq!(app.status, "cargo clean completed");
}

#[test]
fn cargo_clean_failure_does_not_refresh_disk_usage() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    let clean_request_id = app.begin_cargo_clean().expect("clean starts");

    let effect = app.apply_cargo_clean(clean_request_id, Err("boom".to_owned()));

    assert_eq!(effect, AppEffect::None);
    assert!(!app.disk_cleanup.running);
    assert!(!app.disk_usage.loading);
    assert_eq!(app.status, "cargo clean failed: boom");
}

#[test]
fn stopped_run_records_stopped_result_and_clears_running_tests() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    assert!(app.begin_run(&RunRequest::default()).is_some());
    app.apply_run_event(RunEvent::TestStarted { key: test_key(0) });

    app.apply_run_event(RunEvent::RunnerStopped);

    assert!(!app.running);
    assert_eq!(app.run.outcome, RunOutcome::Stopped);
    assert_eq!(app.run_result_label(), "stopped");
    assert_eq!(app.run_status_label(), "idle");
    assert_eq!(
        app.tree
            .status_counts_for_scope(&RunScope::Workspace)
            .running,
        0
    );
    assert!(app.status.starts_with("Stopped:"));
}

#[test]
fn run_phase_starts_as_building_then_switches_to_running_tests() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    assert!(app.begin_run(&RunRequest::default()).is_some());

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
    assert!(app.begin_run(&RunRequest::default()).is_some());

    app.apply_run_event(RunEvent::RunnerFinished {
        exit_code: Some(101),
    });

    assert_eq!(app.run.phase, RunPhase::NotRunning);
    assert_eq!(app.run_status_label(), "idle");
    assert!(app.run_duration().is_some());
    assert!(app.build_duration().is_some());
    assert_eq!(app.test_duration(), None);
    assert!(
        app.output_text()
            .contains("Run command failed: nextest exited with 101")
    );
}

#[test]
fn command_failure_summary_includes_command_hint_and_runner_output() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    assert!(app.begin_run(&RunRequest::default()).is_some());

    app.apply_run_event(RunEvent::RunnerOutput(
        "error: no tests to run; pass --no-tests=pass to accept this".to_owned(),
    ));
    app.apply_run_event(RunEvent::RunnerFinished { exit_code: Some(4) });

    assert_eq!(
        app.status,
        "Command failed: nextest exited with 4 (no tests to run)"
    );
    let output = app.output_text();
    assert!(output.contains("Run command failed: nextest exited with 4"));
    assert!(output.contains("Hint: exit code 4 usually means nextest found no tests to run."));
    assert!(output.contains("Command:\ncargo nextest run"));
    assert!(
        output.contains(
            "Runner output:\nerror: no tests to run; pass --no-tests=pass to accept this"
        )
    );
}

#[test]
fn failing_test_run_reports_failed_result() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    assert!(app.begin_run(&RunRequest::default()).is_some());
    let key = test_key(0);

    app.apply_run_event(RunEvent::TestStarted { key: key.clone() });
    app.apply_run_event(RunEvent::TestFinished {
        key,
        status: TestStatus::Failed,
        output: output_chunks("boom"),
        duration: Some(Duration::from_millis(7)),
    });
    app.apply_run_event(RunEvent::RunnerFinished {
        exit_code: Some(101),
    });

    assert_eq!(app.run.outcome, RunOutcome::Failed);
    assert_eq!(app.run_result_label(), "failed");
    assert!(app.status.starts_with("Failed:"));
    assert!(app.status.contains("1 failed"));
    assert!(app.output_text().contains("Run failed: 0 passed, 1 failed"));
}

#[test]
fn scoped_run_summary_counts_only_the_scope() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(2)));
    let request = RunRequest::new(run_scope_test(0));
    assert!(app.begin_run(&request).is_some());
    let key = test_key(0);

    app.apply_run_event(RunEvent::TestStarted { key: key.clone() });
    app.apply_run_event(RunEvent::TestFinished {
        key,
        status: TestStatus::Passed,
        output: Vec::new(),
        duration: Some(Duration::from_millis(3)),
    });
    app.apply_run_event(RunEvent::RunnerFinished { exit_code: Some(0) });

    assert_eq!(app.run.outcome, RunOutcome::Passed);
    assert_eq!(app.run_progress(), (1, 1));
    assert_eq!(app.status, "Passed: 1 passed, 0 skipped, 0 ignored");
    assert!(
        app.output_text()
            .contains("Run passed: 1 passed, 0 skipped, 0 ignored")
    );
}

#[test]
fn ignored_start_event_during_workspace_run_stays_ignored() {
    let mut tests = test_rows(2);
    tests[1].ignored = true;
    tests[1].status = TestStatus::Ignored;
    let mut app = app_with_tree(Tree::from_tests(tests));
    assert!(app.begin_run(&RunRequest::default()).is_some());

    app.apply_run_event(RunEvent::TestStarted { key: test_key(1) });

    let counts = app.tree.status_counts_for_scope(&RunScope::Workspace);
    assert_eq!(counts.running, 0);
    assert_eq!(counts.ignored, 1);
    assert_eq!(app.run_progress(), (0, 1));
}

#[test]
fn custom_run_opens_and_runs_selected_scope_with_options() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();

    assert_eq!(
        app.apply_command(AppCommand::OpenCustomRun),
        AppEffect::None
    );
    assert!(app.show_test_details);
    app.custom_run.options.no_capture = true;

    let effect = app.apply_command(AppCommand::RunCustom);

    match effect {
        AppEffect::StartRun(request) => {
            assert!(matches!(request.scope, RunScope::Test(_)));
            assert!(request.options.no_capture);
        }
        other => panic!("unexpected effect: {other:?}"),
    }
    assert!(!app.show_test_details);
}

#[test]
fn custom_run_defaults_selected_ignored_test_to_run_ignored_only() {
    let mut tests = test_rows(1);
    tests[0].ignored = true;
    let mut app = app_with_tree(Tree::from_tests(tests));
    expand_all(&mut app.tree.root);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();

    let request = app.custom_run_request().expect("custom run request");

    assert_eq!(request.options.ignored, RunIgnored::Only);
    assert!(matches!(request.scope, RunScope::Test(_)));
}

#[test]
fn custom_run_debugger_requires_single_test_scope() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.apply_command(AppCommand::OpenCustomRun);
    app.custom_run.options.debugger = Some("rust-lldb --args".to_owned());

    assert_eq!(app.apply_command(AppCommand::RunCustom), AppEffect::None);

    assert_eq!(app.status, "Debugger requires a single selected test");
    assert!(app.show_test_details);
}

#[test]
fn new_run_resets_previous_run_metadata_and_result() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(2)));
    assert!(app.begin_run(&RunRequest::default()).is_some());
    app.apply_run_event(RunEvent::RunMetadata {
        run_id: Some("old-run".to_owned()),
        profile: Some("default".to_owned()),
    });
    app.apply_run_event(RunEvent::TestFinished {
        key: test_key(0),
        status: TestStatus::Passed,
        output: output_chunks("stale stdout"),
        duration: Some(Duration::from_millis(9)),
    });
    app.apply_run_event(RunEvent::TestFinished {
        key: test_key(1),
        status: TestStatus::Failed,
        output: output_chunks("stale stderr"),
        duration: Some(Duration::from_millis(11)),
    });
    app.apply_run_event(RunEvent::RunnerFinished {
        exit_code: Some(101),
    });
    assert_eq!(app.run.outcome, RunOutcome::Failed);
    app.main_output.set_scroll(10);
    app.main_output.set_follow(false);
    app.main_output.search.current_line = Some(3);

    assert!(app.begin_run(&RunRequest::new(run_scope_test(0))).is_some());

    assert_eq!(app.run.run_id, None);
    assert_eq!(app.run.outcome, RunOutcome::Running);
    assert_eq!(app.run.exit_code, None);
    assert_eq!(app.run_result_label(), "running");
    assert_eq!(app.run.phase, RunPhase::Building);
    assert_eq!(app.run_status_label(), "building");
    assert!(app.build_duration().is_some());
    assert_eq!(app.test_duration(), None);
    assert_eq!(app.run_progress(), (0, 1));
    assert_eq!(app.main_output.scroll(), 0);
    assert!(app.main_output.follow());
    assert_eq!(app.main_output.search.current_line, None);
    assert!(!app.tree.selected_output().contains("stale stdout"));
    assert!(!app.tree.selected_output().contains("stale stderr"));
}

#[test]
fn filter_toggle_during_run_preserves_visible_selection_and_output_state() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
    expand_all(&mut app.tree.root);
    assert!(app.begin_run(&RunRequest::default()).is_some());
    app.apply_run_event(RunEvent::TestFinished {
        key: test_key(0),
        status: TestStatus::Passed,
        output: Vec::new(),
        duration: Some(Duration::from_millis(5)),
    });
    app.apply_run_event(RunEvent::TestStarted { key: test_key(1) });
    select_visible_path(&mut app, "demo::tests::case_01");
    app.main_output.apply_content_len(20);
    app.main_output.set_scroll(7);
    app.main_output.set_follow(false);
    app.main_output.search.current_line = Some(2);

    app.apply_command(AppCommand::ToggleShowSuccess);

    assert_eq!(app.tree.selected_path(), "demo::tests::case_01");
    assert_eq!(app.main_output.scroll(), 7);
    assert!(!app.main_output.follow());
    assert_eq!(app.main_output.search.current_line, Some(2));
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
        Some(Duration::from_millis(5)),
    );
    select_visible_path(&mut app, "demo::tests::case_00");
    app.main_output.set_scroll(7);
    app.main_output.set_follow(false);
    app.main_output.search.current_line = Some(2);

    app.apply_command(AppCommand::ToggleShowSuccess);

    assert_ne!(app.tree.selected_path(), "demo::tests::case_00");
    assert_eq!(app.main_output.scroll(), 0);
    assert!(app.main_output.follow());
    assert_eq!(app.main_output.search.current_line, None);
}

#[test]
fn output_snap_toggle_jumps_to_bottom_and_can_disable_following() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree.finish_test(
        &test_key(0),
        TestStatus::Failed,
        "one\ntwo\nthree\nfour\nfive".to_owned(),
        Some(Duration::from_millis(5)),
    );
    select_visible_path(&mut app, "demo::tests::case_00");
    app.focus = FocusPane::Output;
    app.main_output.apply_viewport_page_size(2);
    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::Bottom));
    let bottom_scroll = app.main_output.scroll();

    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::PageUp));

    assert!(!app.main_output.follow());
    assert!(app.main_output.scroll() < bottom_scroll);

    app.apply_command(AppCommand::ToggleOutputSnap);

    assert!(app.main_output.follow());
    assert_eq!(app.main_output.scroll(), bottom_scroll);
    assert_eq!(app.status, "Output snap-bottom: on");

    app.apply_command(AppCommand::ToggleOutputSnap);

    assert!(!app.main_output.follow());
    assert_eq!(app.main_output.scroll(), bottom_scroll);
    assert_eq!(app.status, "Output snap-bottom: off");
}

#[test]
fn output_snap_follows_polled_chunks_until_the_user_scrolls() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    select_visible_path(&mut app, "demo::tests::case_00");
    app.focus = FocusPane::Output;
    let key = test_key(0);

    app.apply_run_event(RunEvent::TestStarted { key: key.clone() });
    app.apply_run_event(RunEvent::TestOutput {
        key: key.clone(),
        output: output_chunks("one\ntwo\nthree\nfour\nfive"),
    });
    prepare_test_viewports(&mut app, 4, 2);
    let first_bottom = scroll::max_scroll(app.output_view().line_count(), 2);

    assert!(app.main_output.follow());
    assert_eq!(app.main_output.scroll() as usize, first_bottom);

    app.apply_run_event(RunEvent::TestOutput {
        key: key.clone(),
        output: output_chunks("six\nseven"),
    });
    prepare_test_viewports(&mut app, 4, 2);
    let second_bottom = scroll::max_scroll(app.output_view().line_count(), 2);

    assert!(second_bottom > first_bottom);
    assert_eq!(app.main_output.scroll() as usize, second_bottom);

    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::PageUp));
    let manual_scroll = app.main_output.scroll();
    app.apply_run_event(RunEvent::TestOutput {
        key,
        output: output_chunks("eight\nnine"),
    });
    prepare_test_viewports(&mut app, 4, 2);

    assert!(!app.main_output.follow());
    assert_eq!(app.main_output.scroll(), manual_scroll);
}

#[test]
fn output_snap_toggle_routes_to_xtask_output_when_detail_is_open() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    app.xtasks.set_manifest(sample_xtask_manifest());
    app.apply_command(AppCommand::OpenXtasks);
    app.apply_command(AppCommand::OpenSelectedXtask);
    app.xtasks.output.apply_viewport_page_size(2);
    let request_id = app.xtasks.begin_run("cargo xtask tui-check".to_owned());
    app.apply_xtask_event(crate::xtask::XtaskEvent::RunOutput {
        request_id,
        chunk: crate::xtask::XtaskRunChunk {
            stream: crate::xtask::XtaskOutputStream::Stdout,
            text: "one\ntwo\nthree\nfour\nfive\n".to_owned(),
        },
    });
    let bottom_scroll = app.xtasks.output.scroll();
    let line_count = app
        .xtasks
        .output
        .output_view(&app.xtasks.output_text())
        .line_count();
    app.xtasks
        .output
        .apply_scroll(scroll::ScrollAction::LineUp, line_count);
    app.main_output.set_follow(true);

    app.apply_command(AppCommand::ToggleOutputSnap);

    assert!(app.xtasks.output.follow());
    assert_eq!(app.xtasks.output.scroll(), bottom_scroll);
    assert!(app.main_output.follow());
    assert_eq!(app.status, "Output snap-bottom: on");
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
    let mut app = app_with_finished_output("alpha\npanic here\nomega");
    app.main_output.search.query = "panic".to_owned();
    app.main_output.search.filter = true;

    assert_eq!(app.output_text(), "panic here");
}

#[test]
fn output_search_literal_is_case_insensitive_by_default() {
    let mut app = app_with_finished_output("PANIC\nok");
    app.main_output.search.query = "panic".to_owned();
    app.main_output.search.filter = true;
    app.main_output.set_follow(true);

    assert_eq!(app.output_text(), "PANIC");

    app.apply_command(AppCommand::ToggleOutputCaseSensitive);

    assert_eq!(app.output_text(), "No output lines match 'panic'");
    assert!(!app.main_output.follow());
}

#[test]
fn output_search_regex_filters_and_reports_invalid_regex() {
    let mut app = app_with_finished_output("case_01\ncase_aa\ncase_22");
    app.main_output.search.query = r"case_\d+".to_owned();
    app.main_output.search.filter = true;
    app.main_output.search.regex = true;

    assert_eq!(app.output_text(), "case_01\ncase_22");

    app.main_output.search.query = "(".to_owned();

    assert!(
        app.output_text()
            .starts_with("Invalid output search regex:")
    );
}

#[test]
fn output_find_next_and_previous_scroll_to_matching_lines() {
    let mut app = app_with_finished_output("zero\nmatch one\nskip\nmatch two");
    app.main_output.search.query = "match".to_owned();
    app.main_output.apply_viewport_page_size(2);
    app.main_output.set_follow(true);

    app.apply_command(AppCommand::FindNextOutputMatch);

    assert_eq!(app.main_output.scroll(), 0);
    assert_eq!(app.main_output.search.current_line, Some(1));
    assert!(!app.main_output.follow());

    app.apply_command(AppCommand::FindNextOutputMatch);

    assert_eq!(app.main_output.scroll(), 2);
    assert_eq!(app.main_output.search.current_line, Some(3));

    app.apply_command(AppCommand::FindPreviousOutputMatch);

    assert_eq!(app.main_output.scroll(), 1);
    assert_eq!(app.main_output.search.current_line, Some(1));
}

#[test]
fn output_find_reveals_match_with_scrolloff_context() {
    let mut app =
        app_with_finished_output("line0\nline1\nline2\nline3\nline4\nline5\nline6\nneedle\nline8");
    app.main_output.search.query = "needle".to_owned();
    app.main_output.apply_viewport_page_size(6);

    app.apply_command(AppCommand::FindNextOutputMatch);

    assert_eq!(app.main_output.search.current_line, Some(7));
    assert_eq!(app.main_output.scroll(), 3);
}

#[test]
fn output_find_next_steps_between_matches_on_same_line() {
    let mut app = app_with_finished_output("zero\nmatch one match two\nskip");
    app.main_output.search.query = "match".to_owned();
    app.main_output.apply_viewport_page_size(2);

    app.apply_command(AppCommand::FindNextOutputMatch);

    assert_eq!(app.main_output.scroll(), 0);
    assert_eq!(app.main_output.search.current_line, Some(1));
    assert_eq!(app.main_output.search.current_range, Some((0, 5)));
    assert!(app.status.contains("1/2"));

    app.apply_command(AppCommand::FindNextOutputMatch);

    assert_eq!(app.main_output.scroll(), 0);
    assert_eq!(app.main_output.search.current_line, Some(1));
    assert_eq!(app.main_output.search.current_range, Some((10, 15)));
    assert!(app.status.contains("2/2"));
}

#[test]
fn output_match_navigation_without_query_disables_snap() {
    let mut app = app_with_finished_output("zero\npanic\nok");
    app.main_output.set_follow(true);

    app.apply_command(AppCommand::FindNextOutputMatch);

    assert!(!app.main_output.follow());
    assert_eq!(app.status, "No output search query");
}

#[test]
fn output_filter_preserves_current_source_match_and_restores_global_scroll() {
    let mut app = app_with_finished_output("zero\nmatch one\nskip\nmatch two");
    app.main_output.search.query = "match".to_owned();
    app.main_output.search.current_line = Some(3);
    app.main_output.set_scroll(3);
    app.main_output.set_follow(true);

    app.apply_command(AppCommand::ToggleOutputFilter);

    assert!(app.main_output.search.filter);
    assert_eq!(app.main_output.search.current_line, Some(3));
    assert_eq!(app.output_text(), "match one\nmatch two");
    assert_eq!(app.main_output.scroll(), 1);
    assert!(!app.main_output.follow());

    app.main_output.set_follow(true);
    app.apply_command(AppCommand::ToggleOutputFilter);

    assert!(!app.main_output.search.filter);
    assert_eq!(app.main_output.search.current_line, Some(3));
    assert_eq!(app.output_text(), "zero\nmatch one\nskip\nmatch two\n");
    assert_eq!(app.main_output.scroll(), 3);
    assert!(!app.main_output.follow());
}

#[test]
fn output_search_apply_filter_preserves_existing_match() {
    let mut app = app_with_finished_output("zero\nmatch one\nskip\nmatch two");
    app.main_output.search.query = "match".to_owned();
    app.main_output.search.current_line = Some(3);

    app.apply_command(AppCommand::StartOutputSearch);
    app.apply_command(AppCommand::OpenOutputSearchModal);
    app.main_output.search.modal_focus = SearchModalFocus::Filter;
    app.apply_command(AppCommand::SearchModalActivate);
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert!(app.main_output.search.filter);
    assert_eq!(app.main_output.search.current_line, Some(3));
    assert_eq!(app.main_output.scroll(), 1);
    assert_eq!(app.status, "Output match 2/2 for 'match'");
}

#[test]
fn output_search_input_opens_modal_then_apply_finds_match() {
    let mut app = app_with_finished_output("zero\npanic\nok");
    app.main_output.apply_viewport_page_size(2);
    app.main_output.set_follow(true);

    app.apply_command(AppCommand::StartOutputSearch);
    assert!(!app.main_output.follow());

    app.main_output.set_follow(true);
    search_type(&mut app, "px");
    app.apply_command(AppCommand::OutputSearchEdit(InputFieldInput::new(
        InputFieldKey::Backspace,
    )));
    search_type(&mut app, "anic");
    assert_eq!(app.main_output.search.query, "");

    app.apply_command(AppCommand::OpenOutputSearchModal);
    assert!(app.main_output.search.modal_open);
    assert!(!app.main_output.follow());

    app.main_output.set_follow(true);
    app.apply_command(AppCommand::SearchModalActivate);

    assert!(!app.main_output.search.input_active);
    assert!(!app.main_output.search.modal_open);
    assert_eq!(app.main_output.search.query, "panic");
    assert_eq!(app.main_output.scroll(), 0);
    assert!(!app.main_output.follow());
}

#[test]
fn output_search_reopen_places_cursor_at_end_of_existing_query() {
    let mut app = app_with_finished_output("zero\nstdo\nok");

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "stde");
    app.apply_command(AppCommand::ApplyOutputSearch);
    app.apply_command(AppCommand::StartOutputSearch);

    app.apply_command(AppCommand::OutputSearchEdit(InputFieldInput::new(
        InputFieldKey::Backspace,
    )));
    assert_eq!(app.main_output.search.draft_query(), "std");

    search_type(&mut app, "o");
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.main_output.search.query, "stdo");
    assert_eq!(app.main_output.search.current_line, Some(1));
}

#[test]
fn output_search_reopen_preserves_cursor_for_unchanged_query() {
    let mut app = app_with_finished_output("zero\npanXic\nok");

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "panic");
    for _ in 0..2 {
        app.apply_command(AppCommand::OutputSearchEdit(InputFieldInput::new(
            InputFieldKey::Left,
        )));
    }
    app.apply_command(AppCommand::ApplyOutputSearch);
    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "X");
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.main_output.search.query, "panXic");
    assert_eq!(app.main_output.search.current_line, Some(1));
}

#[test]
fn output_search_draft_does_not_filter_until_applied() {
    let mut app = app_with_finished_output("alpha\npanic\nomega");
    app.main_output.search.filter = true;

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "panic");

    assert_eq!(app.main_output.search.query, "");
    assert!(app.output_text().contains("alpha"));
    assert!(app.output_text().contains("omega"));

    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.main_output.search.query, "panic");
    assert_eq!(app.output_text(), "panic");
}

#[test]
fn output_search_modal_controls_apply_draft_filters() {
    let mut app = app_with_finished_output("case_01\ncase_aa\ncase_22");

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, r"case_\d+");
    app.apply_command(AppCommand::OpenOutputSearchModal);
    app.main_output.search.modal_focus = SearchModalFocus::Filter;
    app.apply_command(AppCommand::SearchModalActivate);
    app.main_output.search.modal_focus = SearchModalFocus::Regex;
    app.apply_command(AppCommand::SearchModalActivate);
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert!(app.main_output.search.filter);
    assert!(app.main_output.search.regex);
    assert_eq!(app.output_text(), "case_01\ncase_22");
}

#[test]
fn output_search_clear_keeps_input_active_and_resets_match() {
    let mut app = app_with_finished_output("zero\npanic\nok");

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "pa");
    app.apply_command(AppCommand::ApplyOutputSearch);
    assert_eq!(app.main_output.search.current_line, Some(1));

    app.apply_command(AppCommand::StartOutputSearch);
    app.apply_command(AppCommand::ClearOutputSearch);

    assert!(app.main_output.search.input_active);
    assert_eq!(app.main_output.search.draft_query(), "");
    assert_eq!(app.main_output.search.query, "pa");
    assert_eq!(app.main_output.search.current_line, Some(1));
    assert_eq!(app.status, "Output search draft cleared");

    app.apply_command(AppCommand::OpenOutputSearchModal);

    assert_eq!(app.main_output.search.draft_query(), "");
}

#[test]
fn output_search_clear_preserves_visible_source_line() {
    let mut app = app_with_finished_output("zero\nmatch one\nskip\nmatch two\nlater");
    app.main_output.search.query = "match".to_owned();
    app.main_output.search.filter = true;
    app.main_output.apply_viewport_metrics(1, 2);
    app.main_output.set_scroll(1);

    app.apply_command(AppCommand::ClearOutputSearch);

    assert_eq!(app.main_output.search.query, "");
    assert_eq!(
        app.output_text(),
        "zero\nmatch one\nskip\nmatch two\nlater\n"
    );
    assert_eq!(app.main_output.scroll(), 3);
    assert_eq!(app.status, "Output search cleared");
}

#[test]
fn output_search_apply_empty_query_preserves_visible_source_line() {
    let mut app = app_with_finished_output("zero\nmatch one\nskip\nmatch two\nlater");
    app.main_output.search.query = "match".to_owned();
    app.main_output.search.filter = true;
    app.main_output.apply_viewport_metrics(1, 2);
    app.main_output.set_scroll(1);

    app.apply_command(AppCommand::StartOutputSearch);
    app.apply_command(AppCommand::ClearOutputSearch);
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.main_output.search.query, "");
    assert_eq!(
        app.output_text(),
        "zero\nmatch one\nskip\nmatch two\nlater\n"
    );
    assert_eq!(app.main_output.scroll(), 3);
    assert_eq!(app.status, "Output search cleared");
}

#[test]
fn discovery_error_uses_output_scroll_and_search() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
    prepare_test_viewports(&mut app, 3, 3);

    app.apply_discovery_event(
        app.discovery.request_id,
        DiscoveryEvent::Finished(Err("first\nsecond\nneedle\nfourth".to_owned())),
    );

    assert_eq!(
        app.command_context().input,
        InputMode::Normal(CommandFocus::Output)
    );
    assert_eq!(
        app.command_context().overlay,
        Some(OverlayMode::DiscoveryError)
    );
    app.apply_command(AppCommand::Scroll(scroll::ScrollAction::LineDown));
    assert_eq!(app.main_output.scroll(), 1);

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "needle");
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.main_output.search.current_line, Some(4));
    assert_eq!(app.status, "Output match 1/1 for 'needle'");
}

#[test]
fn refresh_tests_retries_after_discovery_error() {
    let mut app = app_with_tree(Tree::from_tests(test_rows(3)));
    app.apply_discovery_event(
        app.discovery.request_id,
        DiscoveryEvent::Finished(Err("boom".to_owned())),
    );

    let effect = app.apply_command(AppCommand::RefreshTests);

    assert_eq!(effect, AppEffect::StartDiscovery(app.discovery.request_id));
    assert!(app.discovery.running);
    assert_eq!(app.discovery.error, None);
    assert_eq!(app.status, "Discovering tests");
}

#[test]
fn stale_discovery_event_is_ignored() {
    let mut app = App::discovering(crate::config::AppSettings::default());
    let stale_request_id = app.discovery.request_id;
    let current_request_id = app.begin_discovery();

    assert!(!app.apply_discovery_event(
        stale_request_id,
        DiscoveryEvent::Finished(Err("old failure".to_owned())),
    ));

    assert!(app.discovery.running);
    assert_eq!(app.discovery.error, None);
    assert_eq!(app.status, "Discovering tests");

    assert!(app.apply_discovery_event(
        current_request_id,
        DiscoveryEvent::Finished(Ok(DiscoveryOutput {
            tests: test_rows(1),
            run_config: crate::nextest::RunConfig::default(),
        })),
    ));
    assert!(!app.discovery.running);
    assert_eq!(app.discovery.error, None);
}

#[test]
fn stale_disk_usage_result_is_ignored() {
    let mut app = app_with_tree(Tree::from_tests(Vec::new()));
    let stale_request_id = app.begin_disk_usage_scan();
    let current_request_id = app.begin_disk_usage_scan();

    app.apply_disk_usage(stale_request_id, Err("old scan failed".to_owned()));

    assert!(app.disk_usage.loading);
    assert_eq!(app.disk_usage.error, None);
    assert_eq!(app.status, "Ready");

    app.apply_disk_usage(current_request_id, Err("current scan failed".to_owned()));

    assert!(!app.disk_usage.loading);
    assert_eq!(app.disk_usage.error, Some("current scan failed".to_owned()));
    assert_eq!(app.status, "Disk usage failed: current scan failed");
}

#[test]
fn output_search_editor_can_insert_at_cursor_and_apply() {
    let mut app = app_with_finished_output("zero\npanic\nok");

    app.apply_command(AppCommand::StartOutputSearch);
    search_type(&mut app, "pnic");
    for _ in 0..3 {
        app.apply_command(AppCommand::OutputSearchEdit(InputFieldInput::new(
            InputFieldKey::Left,
        )));
    }
    search_type(&mut app, "a");
    app.apply_command(AppCommand::ApplyOutputSearch);

    assert_eq!(app.main_output.search.query, "panic");
    assert_eq!(app.main_output.search.current_line, Some(1));
}

fn assert_selection_visible(app: &App) {
    let selected = app.tree.selected_index();
    let scroll = app.tree_viewport.scroll();
    let page_size = app.tree_viewport.page_size();
    assert!(
        selected >= scroll,
        "selected {selected} should be >= scroll {scroll}"
    );
    assert!(
        selected < scroll + page_size,
        "selected {selected} should be < scroll {scroll} + page {page_size}"
    );
}

fn test_rows(count: usize) -> Vec<DiscoveredTest> {
    (0..count)
        .map(|index| DiscoveredTest {
            key: test_key(index),
            package: "demo".to_owned(),
            binary: "demo".to_owned(),
            binary_kind: "lib".to_owned(),
            binary_path: std::path::PathBuf::from("target/debug/deps/demo"),
            cwd: std::path::PathBuf::from("."),
            source_path: None,
            module: Some("tests".to_owned()),
            name: format!("case_{index:02}"),
            full_name: format!("tests::case_{index:02}"),
            status: TestStatus::Pending,
            ignored: false,
            ignore_reason: None,
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

fn run_scope_test(index: usize) -> RunScope {
    RunScope::Test(TestSelector {
        target: TargetSelector {
            package: "demo".to_owned(),
            name: "demo".to_owned(),
            kind: "lib".to_owned(),
        },
        name: format!("tests::case_{index:02}"),
    })
}

fn app_with_finished_output(output: &str) -> App {
    let mut app = app_with_tree(Tree::from_tests(test_rows(1)));
    expand_all(&mut app.tree.root);
    app.tree
        .finish_test(&test_key(0), TestStatus::Passed, output.to_owned(), None);
    app.tree.select_next();
    app.tree.select_next();
    app.tree.select_next();
    app
}

fn search_type(app: &mut App, text: &str) {
    for char in text.chars() {
        app.apply_command(AppCommand::OutputSearchEdit(InputFieldInput::char(char)));
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
