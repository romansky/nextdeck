use super::*;
use crossterm::event::KeyEvent;

const fn help_context() -> CommandContext {
    CommandContext {
        input: InputMode::Help,
        overlay: Some(OverlayMode::Help),
    }
}

const fn discovery_running_context() -> CommandContext {
    CommandContext {
        input: InputMode::DiscoveryRunning,
        overlay: Some(OverlayMode::Discovery),
    }
}

const fn settings_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::SettingsModal,
        overlay: Some(OverlayMode::Settings),
    }
}

const fn settings_open_with_input_context() -> CommandContext {
    CommandContext {
        input: InputMode::SettingsOpenWith,
        overlay: Some(OverlayMode::Settings),
    }
}

const fn custom_run_input_context() -> CommandContext {
    CommandContext {
        input: InputMode::CustomRunInput,
        overlay: Some(OverlayMode::TestDetails),
    }
}

const fn disk_cleanup_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::DiskCleanupModal,
        overlay: Some(OverlayMode::DiskCleanup),
    }
}

const fn xtask_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::XtaskModal,
        overlay: Some(OverlayMode::Xtasks),
    }
}

const fn xtask_command_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::XtaskCommandModal(XtaskDetailFocus::Parameters),
        overlay: Some(OverlayMode::Xtasks),
    }
}

const fn xtask_output_context() -> CommandContext {
    CommandContext {
        input: InputMode::XtaskCommandModal(XtaskDetailFocus::Output),
        overlay: Some(OverlayMode::Xtasks),
    }
}

const fn xtask_input_context() -> CommandContext {
    CommandContext {
        input: InputMode::XtaskInput,
        overlay: Some(OverlayMode::Xtasks),
    }
}

const fn test_details_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::TestDetailsModal,
        overlay: Some(OverlayMode::TestDetails),
    }
}

const fn output_search_modal_context() -> CommandContext {
    CommandContext {
        input: InputMode::OutputSearchModal,
        overlay: Some(OverlayMode::OutputSearch),
    }
}

const fn output_search_inline_context() -> CommandContext {
    CommandContext {
        input: InputMode::OutputSearchInline,
        overlay: None,
    }
}

#[test]
fn maps_normalized_question_mark_to_help() {
    assert_eq!(
        command_for_key(KeyCode::Char('?'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleHelp
    );
    assert_eq!(
        command_for_key(KeyCode::Char('?'), KeyModifiers::SHIFT, CommandFocus::Tests),
        AppCommand::ToggleHelp
    );
    assert_eq!(
        command_for_key(KeyCode::Char('/'), KeyModifiers::SHIFT, CommandFocus::Tests),
        AppCommand::ToggleHelp
    );
}

#[test]
fn maps_fallback_help_keys() {
    assert_eq!(
        command_for_key(KeyCode::Char('h'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleHelp
    );
    assert_eq!(
        command_for_key(KeyCode::F(1), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleHelp
    );
}

#[test]
fn plain_slash_searches_output_only_when_output_is_focused() {
    assert_eq!(
        command_for_key(KeyCode::Char('/'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::Noop
    );
    assert_eq!(
        command_for_key(KeyCode::Char('/'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::StartOutputSearch
    );
}

#[test]
fn plain_s_toggles_snap_only_when_output_is_focused() {
    assert_eq!(
        command_for_key(KeyCode::Char('s'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleShowSkipped
    );
    assert_eq!(
        command_for_key(KeyCode::Char('s'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::ToggleOutputSnap
    );
}

#[test]
fn maps_ctrl_c_to_stop_run() {
    assert_eq!(
        command_for_key(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            CommandFocus::Tests
        ),
        AppCommand::StopRun
    );
    assert_eq!(
        command_for_key(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            CommandFocus::Output
        ),
        AppCommand::StopRun
    );
}

#[test]
fn ctrl_c_stops_run_in_search_and_help_contexts() {
    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    )));

    assert_eq!(
        command_for_input(&event, output_search_inline_context()),
        AppCommand::StopRun
    );
    assert_eq!(
        command_for_input(&event, help_context()),
        AppCommand::StopRun
    );
}

#[test]
fn maps_refresh_and_view_filter_keys() {
    assert_eq!(
        command_for_key(KeyCode::Char('u'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::RefreshTests
    );
    assert_eq!(
        command_for_key(KeyCode::Char('u'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::RefreshTests
    );
    assert_eq!(
        command_for_key(KeyCode::Char('p'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleShowSuccess
    );
    assert_eq!(
        command_for_key(KeyCode::Char('f'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleShowFailed
    );
    assert_eq!(
        command_for_key(KeyCode::Char('i'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleShowIgnored
    );
    assert_eq!(
        command_for_key(KeyCode::Char('s'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleShowSkipped
    );
    assert_eq!(
        command_for_key(KeyCode::Char('j'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::SelectNextFailed
    );
    assert_eq!(
        command_for_key(KeyCode::Char('J'), KeyModifiers::SHIFT, CommandFocus::Tests),
        AppCommand::SelectPreviousFailed
    );
    assert_eq!(
        command_for_key(KeyCode::Char('o'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::OpenSource
    );
    assert_eq!(
        command_for_key(KeyCode::Char('R'), KeyModifiers::SHIFT, CommandFocus::Tests),
        AppCommand::OpenCustomRun
    );
}

#[test]
fn test_details_modal_maps_snapshot_key() {
    assert_eq!(
        command_for_test_details_modal(KeyCode::Char('s')),
        AppCommand::CaptureTestSnapshot
    );
    assert_eq!(
        command_for_test_details_modal(KeyCode::Down),
        AppCommand::CustomRunNext
    );
    assert_eq!(
        command_for_test_details_modal(KeyCode::Left),
        AppCommand::CustomRunAdjustLeft
    );
    assert_eq!(
        command_for_test_details_modal(KeyCode::Char('e')),
        AppCommand::CustomRunActivate
    );
    assert_eq!(
        command_for_test_details_modal(KeyCode::Enter),
        AppCommand::RunCustom
    );
    assert_eq!(
        command_for_test_details_modal(KeyCode::Esc),
        AppCommand::CloseTestDetails
    );
}

#[test]
fn tests_focus_splits_enter_details_from_space_toggle() {
    assert_eq!(
        command_for_key(KeyCode::Enter, KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ActivateSelected
    );
    assert_eq!(
        command_for_key(KeyCode::Char(' '), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::ToggleSelected
    );
}

#[test]
fn maps_disk_usage_keys_across_focus_modes() {
    assert_eq!(
        command_for_key(KeyCode::Char('d'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::RefreshDiskUsage
    );
    assert_eq!(
        command_for_key(
            KeyCode::Char('D'),
            KeyModifiers::SHIFT,
            CommandFocus::Output
        ),
        AppCommand::OpenDiskCleanup
    );
}

#[test]
fn maps_global_settings_key_across_focus_modes() {
    assert_eq!(
        command_for_key(KeyCode::Char(','), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::OpenSettings
    );
    assert_eq!(
        command_for_key(KeyCode::Char(','), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::OpenSettings
    );
}

#[test]
fn maps_global_xtasks_key_across_focus_modes() {
    assert_eq!(
        command_for_key(KeyCode::Char('x'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::OpenXtasks
    );
    assert_eq!(
        command_for_key(KeyCode::Char('x'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::OpenXtasks
    );
}

#[test]
fn settings_modal_uses_settings_commands() {
    let context = settings_modal_context();
    let next = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(command_for_input(&next, context), AppCommand::SettingsNext);

    let edit = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&edit, context),
        AppCommand::SettingsActivate
    );

    let close = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&close, context),
        AppCommand::CloseSettings
    );

    let q = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&q, context), AppCommand::Noop);

    let old_clear = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&old_clear, context), AppCommand::Noop);
}

#[test]
fn settings_open_with_input_accepts_text_and_commit() {
    let context = settings_open_with_input_context();
    let char = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('i'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&char, context),
        AppCommand::SettingsOpenWithEdit(InputFieldInput::char('i'))
    );

    let enter = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&enter, context),
        AppCommand::CommitOpenWithSetting
    );

    let old_clear = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(command_for_input(&old_clear, context), AppCommand::Noop);
}

#[test]
fn settings_open_with_input_ignores_modified_navigation() {
    let context = settings_open_with_input_context();
    let ctrl_left = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Left,
        KeyModifiers::CONTROL,
    )));
    let super_v = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('v'),
        KeyModifiers::SUPER,
    )));

    assert_eq!(command_for_input(&ctrl_left, context), AppCommand::Noop);
    assert_eq!(command_for_input(&super_v, context), AppCommand::Noop);
}

#[test]
fn discovery_running_blocks_normal_tui_commands() {
    let context = discovery_running_context();
    let down = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(command_for_input(&down, context), AppCommand::Noop);

    let esc = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(command_for_input(&esc, context), AppCommand::Noop);

    let quit = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&quit, context), AppCommand::Quit);
}

#[test]
fn disk_cleanup_modal_uses_cleanup_commands() {
    let context = disk_cleanup_modal_context();
    let clean = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&clean, context),
        AppCommand::RunCargoClean
    );

    let refresh = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('r'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&refresh, context),
        AppCommand::RefreshDiskUsage
    );

    let close = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&close, context),
        AppCommand::CloseDiskCleanup
    );

    let q = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&q, context), AppCommand::Noop);
}

#[test]
fn test_details_modal_uses_custom_run_commands() {
    let context = test_details_modal_context();
    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))),
            context
        ),
        AppCommand::CustomRunNext
    );
    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE
            ))),
            context
        ),
        AppCommand::RunCustom
    );
    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(KeyEvent::new(
                KeyCode::Char('e'),
                KeyModifiers::NONE,
            ))),
            context
        ),
        AppCommand::CustomRunActivate
    );
    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))),
            context
        ),
        AppCommand::CloseTestDetails
    );
}

#[test]
fn custom_run_input_commits_or_cancels_editing() {
    let context = custom_run_input_context();
    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE
            ))),
            context
        ),
        AppCommand::CommitCustomRunEdit
    );
    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))),
            context
        ),
        AppCommand::CancelCustomRunEdit
    );
}

#[test]
fn xtask_modal_uses_xtask_commands() {
    let context = xtask_modal_context();
    let next = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&next, context),
        AppCommand::XtaskNextCommand
    );

    let open = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&open, context),
        AppCommand::OpenSelectedXtask
    );

    let refresh = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&refresh, context),
        AppCommand::RefreshXtasks
    );

    let run = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('r'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&run, context), AppCommand::Noop);

    let close = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(command_for_input(&close, context), AppCommand::CloseXtasks);
}

#[test]
fn xtask_command_modal_uses_parameter_and_output_commands() {
    let context = xtask_command_modal_context();
    let arg = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&arg, context),
        AppCommand::ToggleXtaskDetailFocus
    );

    let next_arg =
        InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&next_arg, context),
        AppCommand::XtaskNextArg
    );

    let adjust = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&adjust, context),
        AppCommand::XtaskAdjustRight
    );

    let edit = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('e'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&edit, context),
        AppCommand::XtaskActivateArg
    );

    let run = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('r'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&run, context), AppCommand::RunXtask);

    let output_context = xtask_output_context();

    let line_down =
        InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&line_down, output_context),
        AppCommand::XtaskOutputLineDown
    );

    let snap = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&snap, output_context),
        AppCommand::ToggleOutputSnap
    );

    let regex = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('r'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(
        command_for_input(&regex, output_context),
        AppCommand::ToggleOutputRegex
    );

    let search = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&search, output_context),
        AppCommand::StartOutputSearch
    );

    let next_match = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('n'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&next_match, output_context),
        AppCommand::FindNextOutputMatch
    );

    let filter = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('f'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&filter, output_context),
        AppCommand::ToggleOutputFilter
    );

    let open_output = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('o'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&open_output, output_context),
        AppCommand::OpenOutput
    );

    let page = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::PageDown,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&page, output_context),
        AppCommand::XtaskOutputPageDown
    );

    let close = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&close, context),
        AppCommand::CloseXtaskDetails
    );

    let back = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('b'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&back, context), AppCommand::Noop);
}

#[test]
fn xtask_input_accepts_text_and_commit() {
    let context = xtask_input_context();
    let char = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('v'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&char, context),
        AppCommand::XtaskEdit(InputFieldInput::char('v'))
    );

    let enter = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&enter, context),
        AppCommand::CommitXtaskEdit
    );
}

#[test]
fn test_details_modal_routes_run_options_and_close_keys() {
    let context = test_details_modal_context();
    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&event, context),
        AppCommand::CloseTestDetails
    );

    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&event, context),
        AppCommand::CustomRunNext
    );

    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&event, context), AppCommand::RunCustom);

    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    )));
    assert_eq!(command_for_input(&event, context), AppCommand::Noop);
}

#[test]
fn maps_tests_pane_resize_keys() {
    assert_eq!(
        command_for_key(KeyCode::Left, KeyModifiers::SHIFT, CommandFocus::Tests),
        AppCommand::NarrowTestsPane
    );
    assert_eq!(
        command_for_key(KeyCode::Right, KeyModifiers::SHIFT, CommandFocus::Tests),
        AppCommand::WidenTestsPane
    );
    assert_eq!(
        command_for_key(KeyCode::Char('['), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::NarrowTestsPane
    );
    assert_eq!(
        command_for_key(KeyCode::Char(']'), KeyModifiers::NONE, CommandFocus::Tests),
        AppCommand::WidenTestsPane
    );
}

#[test]
fn output_focus_uses_output_search_commands() {
    assert_eq!(
        command_for_key(KeyCode::Char('f'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::ToggleOutputFilter
    );
    assert_eq!(
        command_for_key(KeyCode::Char('r'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::ToggleOutputRegex
    );
    assert_eq!(
        command_for_key(KeyCode::Char('c'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::ToggleOutputCaseSensitive
    );
    assert_eq!(
        command_for_key(KeyCode::Char('n'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::FindNextOutputMatch
    );
    assert_eq!(
        command_for_key(
            KeyCode::Char('N'),
            KeyModifiers::SHIFT,
            CommandFocus::Output
        ),
        AppCommand::FindPreviousOutputMatch
    );
    assert_eq!(
        command_for_key(KeyCode::Char('o'), KeyModifiers::NONE, CommandFocus::Output),
        AppCommand::OpenOutput
    );
}

#[test]
fn command_metadata_drives_ticker_labels() {
    assert_eq!(AppCommand::RunSelected.ticker_label(), Some("run"));
    assert_eq!(AppCommand::OpenCustomRun.ticker_label(), Some("run-custom"));
    assert_eq!(AppCommand::ToggleOutputRegex.ticker_label(), Some("regex"));
    assert_eq!(AppCommand::CloseHelp.ticker_label(), Some("close help"));
}

#[test]
fn command_metadata_contains_help_entries() {
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::Navigation
            && info.keys == "Tab"
            && info.label == "switch tree/output focus"
    }));
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::Global
            && info.keys == "h/?/F1"
            && info.label == "open or close help"
    }));
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::Runs && info.keys == "r" && info.label == "run selected scope"
    }));
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::Global
            && info.keys == "Ctrl+C"
            && info.label == "stop running tests"
    }));
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::Global && info.keys == "x" && info.label == "open xtasks"
    }));
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::Output && info.keys == "/" && info.label == "search output"
    }));
    assert!(command_infos().iter().any(|info| {
        info.group == CommandGroup::View && info.keys == "f" && info.label == "toggle failed tests"
    }));
}

#[test]
fn output_search_input_accepts_text_and_controls() {
    let context = output_search_inline_context();
    let text = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('p'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&text, context),
        AppCommand::OutputSearchEdit(SearchEditorInput::char('p'))
    );

    let backspace = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Backspace,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&backspace, context),
        AppCommand::OutputSearchEdit(SearchEditorInput::new(
            SearchEditorKey::Backspace,
            false,
            false,
            false,
        ))
    );

    let left = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&left, context),
        AppCommand::OutputSearchEdit(SearchEditorInput::new(
            SearchEditorKey::Left,
            false,
            false,
            false,
        ))
    );

    let clear = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(
        command_for_input(&clear, context),
        AppCommand::ClearOutputSearch
    );

    let enter = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&enter, context),
        AppCommand::ApplyOutputSearch
    );

    let advanced = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::CONTROL,
    )));
    assert_eq!(
        command_for_input(&advanced, context),
        AppCommand::OpenOutputSearchModal
    );

    let mac_advanced = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::SUPER,
    )));
    assert_eq!(
        command_for_input(&mac_advanced, context),
        AppCommand::OpenOutputSearchModal
    );
}

#[test]
fn output_search_modal_accepts_navigation_and_apply_keys() {
    let context = output_search_modal_context();

    let tab = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
    assert_eq!(
        command_for_input(&tab, context),
        AppCommand::SearchModalNextControl
    );

    let enter = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        command_for_input(&enter, context),
        AppCommand::SearchModalActivate
    );

    let apply = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::CONTROL,
    )));
    assert_eq!(
        command_for_input(&apply, context),
        AppCommand::ApplyOutputSearch
    );
}

#[test]
fn help_context_only_closes_on_close_keys() {
    let context = help_context();
    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(command_for_input(&event, context), AppCommand::Noop);

    let event = InputEvent::Terminal(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(command_for_input(&event, context), AppCommand::CloseHelp);
}

#[test]
fn ignores_non_press_key_events() {
    let repeat = InputEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('h'),
        KeyModifiers::NONE,
    )));
    let mut repeat = match repeat {
        InputEvent::Terminal(Event::Key(key)) => key,
        _ => unreachable!(),
    };
    repeat.kind = KeyEventKind::Repeat;

    assert_eq!(
        command_for_input(
            &InputEvent::Terminal(Event::Key(repeat)),
            CommandContext::default()
        ),
        AppCommand::Noop
    );
}

#[test]
fn resize_is_a_command() {
    let event = InputEvent::Terminal(Event::Resize(80, 24));
    assert_eq!(
        command_for_input(&event, CommandContext::default()),
        AppCommand::Resize
    );
}
