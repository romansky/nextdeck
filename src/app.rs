use std::time::{Duration, Instant};

use crate::{
    command::{AppCommand, CommandContext, CommandFocus, InputMode, OverlayMode},
    config::{
        self, AppSettings, STORAGE_LOW_SPACE_THRESHOLD_STEP_GB, TREE_WIDTH_STEP_PERCENT,
    },
    disk_usage::{DiskCleanupState, DiskUsageSnapshot, DiskUsageState},
    editor::SourceLocation,
    git_status::GitStatus,
    input_field::InputFieldInput,
    nextest::{DiscoveryEvent, RunEvent, RunRequest, RunScope},
    output_pane::{
        OutputSearchState, SearchDirection, SearchEditorInput, SearchEditorKey, SearchModalFocus,
    },
    scroll,
    settings::{GlobalSettingsState, SettingsField},
    source,
    state::StatusCounts,
    tree::{DiscoveredTest, NodeId, NodeKind, SelectionChange, TestNode, TestViewFilter, Tree},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusPane {
    Tree,
    Output,
}

pub struct App {
    pub tree: Tree,
    pub tree_scroll: usize,
    pub status: String,
    pub key_echo: Option<KeyEcho>,
    pub running: bool,
    pub should_quit: bool,
    pub output_scroll: u16,
    pub output_follow: bool,
    pub output_search: OutputSearchState,
    pub focus: FocusPane,
    pub show_help: bool,
    pub tree_page_size: usize,
    pub output_page_size: u16,
    pub discovery: DiscoveryState,
    pub git_status: GitStatus,
    pub disk_usage: DiskUsageState,
    pub disk_cleanup: DiskCleanupState,
    pub global_settings: GlobalSettingsState,
    pub run: RunState,
    pub settings: AppSettings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyEcho {
    pub text: String,
    ticks_remaining: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryState {
    pub running: bool,
    pub ticks: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RunState {
    pub active: bool,
    pub phase: RunPhase,
    pub run_id: Option<String>,
    pub profile: String,
    pub scope: RunScope,
    pub outcome: RunOutcome,
    pub exit_code: Option<i32>,
    started_at: Option<Instant>,
    tests_started_at: Option<Instant>,
    build_duration: Option<Duration>,
    test_duration: Option<Duration>,
    finished_duration: Option<Duration>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RunPhase {
    #[default]
    NotRunning,
    Building,
    RunningTests,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RunOutcome {
    #[default]
    NotStarted,
    Running,
    Passed,
    Failed,
    CommandFailed,
    Stopped,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            active: false,
            phase: RunPhase::NotRunning,
            run_id: None,
            profile: "default".to_owned(),
            scope: RunScope::Workspace,
            outcome: RunOutcome::NotStarted,
            exit_code: None,
            started_at: None,
            tests_started_at: None,
            build_duration: None,
            test_duration: None,
            finished_duration: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppEffect {
    None,
    SaveSettings(AppSettings),
    StartDiscovery,
    StartRun(RunRequest),
    StopRun,
    OpenSource(SourceLocation),
    OpenOutput(OutputOpenRequest),
    RefreshDiskUsage,
    RunCargoClean,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OutputOpenRequest {
    pub title: String,
    pub text: String,
}

impl App {
    #[cfg(test)]
    pub fn new(tree: Tree) -> Self {
        Self::with_settings(tree, AppSettings::default())
    }

    pub fn with_settings(tree: Tree, settings: AppSettings) -> Self {
        Self {
            tree,
            tree_scroll: 0,
            status: "Ready".to_owned(),
            key_echo: None,
            running: false,
            should_quit: false,
            output_scroll: 0,
            output_follow: true,
            output_search: OutputSearchState::default(),
            focus: FocusPane::Tree,
            show_help: false,
            tree_page_size: 1,
            output_page_size: 1,
            discovery: DiscoveryState::default(),
            git_status: GitStatus::unknown(),
            disk_usage: DiskUsageState::default(),
            disk_cleanup: DiskCleanupState::default(),
            global_settings: GlobalSettingsState::default(),
            run: RunState::default(),
            settings: settings.normalized(),
        }
    }

    pub fn discovering(settings: AppSettings) -> Self {
        let mut app = Self::with_settings(Tree::from_tests(Vec::new()), settings);
        app.begin_discovery();
        app
    }

    pub fn prepare_frame(&mut self, tree_height: u16, output_height: u16) {
        self.set_viewport_sizes(tree_height, output_height);
        let line_count = self.output_text().lines().count().max(1);
        self.set_output_line_count(line_count);
    }

    pub fn set_viewport_sizes(&mut self, tree_height: u16, output_height: u16) {
        self.tree_page_size = tree_height.saturating_sub(2).max(1) as usize;
        self.output_page_size = output_height.saturating_sub(2).max(1);
        self.ensure_tree_selection_visible();
        self.clamp_output_scroll();
    }

    pub fn set_output_line_count(&mut self, line_count: usize) {
        let max_scroll = self.max_output_scroll(line_count);
        if self.output_follow {
            self.output_scroll = max_scroll;
        } else {
            self.output_scroll = self.output_scroll.min(max_scroll);
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Tree => FocusPane::Output,
            FocusPane::Output => FocusPane::Tree,
        };
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn on_resize(&mut self) {
        self.ensure_tree_selection_visible();
        self.clamp_output_scroll();
    }

    pub fn command_context(&self) -> CommandContext {
        let focus = match self.command_focus() {
            FocusPane::Tree => CommandFocus::Tests,
            FocusPane::Output => CommandFocus::Output,
        };
        let overlay = if self.show_help {
            Some(OverlayMode::Help)
        } else if self.global_settings.modal_open {
            Some(OverlayMode::Settings)
        } else if self.disk_cleanup.modal_open {
            Some(OverlayMode::DiskCleanup)
        } else if self.output_search.modal_open {
            Some(OverlayMode::OutputSearch)
        } else if self.discovery.running {
            Some(OverlayMode::Discovery)
        } else if self.discovery.error.is_some() {
            Some(OverlayMode::DiscoveryError)
        } else {
            None
        };
        let input = match overlay {
            Some(OverlayMode::Help) => InputMode::Help,
            Some(OverlayMode::Settings) if self.global_settings.open_with_editing => {
                InputMode::SettingsOpenWith
            }
            Some(OverlayMode::Settings) => InputMode::SettingsModal,
            Some(OverlayMode::DiskCleanup) => InputMode::DiskCleanupModal,
            Some(OverlayMode::OutputSearch) => InputMode::OutputSearchModal,
            Some(OverlayMode::Discovery) => InputMode::DiscoveryRunning,
            Some(OverlayMode::DiscoveryError) => InputMode::Normal(CommandFocus::Output),
            None if self.output_search.input_active => InputMode::OutputSearchInline,
            None => InputMode::Normal(focus),
        };

        CommandContext {
            input,
            overlay,
        }
    }

    pub fn record_key(&mut self, text: impl Into<String>) {
        self.key_echo = Some(KeyEcho {
            text: text.into(),
            ticks_remaining: 8,
        });
    }

    pub fn tick(&mut self) {
        if self.discovery.running {
            self.discovery.ticks = self.discovery.ticks.saturating_add(1);
        }
        if let Some(echo) = &mut self.key_echo {
            echo.ticks_remaining = echo.ticks_remaining.saturating_sub(1);
            if echo.ticks_remaining == 0 {
                self.key_echo = None;
            }
        }
    }

    pub fn begin_discovery(&mut self) {
        self.discovery = DiscoveryState {
            running: true,
            ticks: 0,
            error: None,
        };
        self.status = "Discovering tests".to_owned();
    }

    pub fn apply_discovery_event(&mut self, event: DiscoveryEvent) -> bool {
        match event {
            DiscoveryEvent::Finished(Ok(tests)) => {
                let count = tests.len();
                self.tree.refresh_from_tests(tests);
                self.tree_scroll = 0;
                self.reset_output_for_source_change();
                self.discovery.running = false;
                self.discovery.error = None;
                self.status = format!("Discovered {count} test(s)");
                true
            }
            DiscoveryEvent::Finished(Err(error)) => {
                self.discovery.running = false;
                self.discovery.error = Some(error);
                self.focus = FocusPane::Output;
                self.reset_output_for_modal();
                self.status = "Discovery failed".to_owned();
                false
            }
        }
    }

    pub fn apply_git_status(&mut self, git_status: GitStatus) {
        self.git_status = git_status;
    }

    pub fn begin_disk_usage_scan(&mut self) {
        self.disk_usage.begin_scan();
    }

    pub fn apply_disk_usage(&mut self, result: Result<DiskUsageSnapshot, String>) {
        if let Some(error) = self.disk_usage.apply_result(result).err() {
            self.status = format!("Disk usage failed: {error}");
        }
    }

    pub fn begin_cargo_clean(&mut self) -> bool {
        if !self.disk_cleanup.begin_clean() {
            self.status = "cargo clean already running".to_owned();
            return false;
        }
        self.status = "Running cargo clean".to_owned();
        true
    }

    pub fn apply_cargo_clean(&mut self, result: Result<(), String>) -> bool {
        match &result {
            Ok(()) => self.status = "cargo clean completed".to_owned(),
            Err(error) => self.status = format!("cargo clean failed: {error}"),
        }
        self.disk_cleanup.apply_result(result)
    }

    fn save_settings_effect(&mut self) -> AppEffect {
        self.settings = self.settings.clone().normalized();
        AppEffect::SaveSettings(self.settings.clone())
    }

    pub fn open_global_settings(&mut self) {
        self.global_settings.open(&self.settings);
        self.status = "Settings opened".to_owned();
    }

    pub fn close_global_settings(&mut self) {
        self.global_settings.close();
        self.status = "Settings closed".to_owned();
    }

    fn sync_settings_open_with(&mut self) {
        self.global_settings.sync_open_with(&self.settings);
    }

    fn select_next_setting(&mut self) {
        self.global_settings.select_next();
    }

    fn select_previous_setting(&mut self) {
        self.global_settings.select_previous();
    }

    fn begin_edit_open_with_setting(&mut self) {
        self.global_settings.begin_open_with_edit(&self.settings);
        self.status = "Editing open-with command".to_owned();
    }

    fn edit_open_with_setting(&mut self, input: InputFieldInput) {
        self.global_settings.edit_open_with(input);
    }

    fn commit_open_with_setting(&mut self) -> AppEffect {
        self.global_settings.open_with_editing = false;
        self.settings.open_with_command = Some(self.global_settings.open_with_text());
        self.settings = self.settings.clone().normalized();
        self.sync_settings_open_with();
        self.status = format!("Open with: {}", self.settings.open_with_label());
        self.save_settings_effect()
    }

    fn cancel_open_with_setting(&mut self) {
        self.global_settings.cancel_open_with_edit(&self.settings);
        self.status = "Open-with edit canceled".to_owned();
    }

    fn clear_open_with_setting(&mut self) -> AppEffect {
        if self.global_settings.open_with_editing {
            self.global_settings.clear_open_with_draft();
            self.status = "Open-with draft cleared".to_owned();
            return AppEffect::None;
        }
        self.settings.open_with_command = None;
        self.sync_settings_open_with();
        self.status = "Open with: env/default".to_owned();
        self.save_settings_effect()
    }

    fn cycle_open_with_setting(&mut self, direction: i8) -> AppEffect {
        const PRESETS: &[Option<&str>] = &[
            None,
            Some("idea"),
            Some("code"),
            Some("cursor"),
            Some("zed"),
            Some("open"),
        ];
        let current = self.settings.open_with_command.as_deref();
        let index = PRESETS
            .iter()
            .position(|preset| *preset == current)
            .unwrap_or(0);
        let next = if direction < 0 {
            index.checked_sub(1).unwrap_or(PRESETS.len() - 1)
        } else {
            (index + 1) % PRESETS.len()
        };
        self.settings.open_with_command = PRESETS[next].map(ToOwned::to_owned);
        self.sync_settings_open_with();
        self.status = format!("Open with: {}", self.settings.open_with_label());
        self.save_settings_effect()
    }

    fn adjust_selected_setting(&mut self, direction: i8) -> AppEffect {
        match self.global_settings.selected {
            SettingsField::OpenWith => self.cycle_open_with_setting(direction),
            SettingsField::TreeWidth => {
                let delta = if direction < 0 {
                    -(TREE_WIDTH_STEP_PERCENT as i16)
                } else {
                    TREE_WIDTH_STEP_PERCENT as i16
                };
                self.resize_tests_pane(delta)
            }
            SettingsField::StorageThreshold => {
                let delta = if direction < 0 {
                    -(STORAGE_LOW_SPACE_THRESHOLD_STEP_GB as i16)
                } else {
                    STORAGE_LOW_SPACE_THRESHOLD_STEP_GB as i16
                };
                self.settings.storage_low_space_threshold_gb =
                    config::resize_storage_low_space_threshold(
                        self.settings.storage_low_space_threshold_gb,
                        delta,
                    );
                self.status = format!(
                    "Low disk threshold: {} GiB",
                    self.settings.storage_low_space_threshold_gb
                );
                self.save_settings_effect()
            }
            SettingsField::Theme => {
                self.settings.theme_mode = if direction < 0 {
                    self.settings.theme_mode.previous()
                } else {
                    self.settings.theme_mode.next()
                };
                self.status = format!("Theme: {}", self.settings.theme_mode.label());
                self.save_settings_effect()
            }
            SettingsField::ColorBlindMode => {
                self.settings.color_blind_mode = !self.settings.color_blind_mode;
                self.status = format!(
                    "Color-blind mode: {}",
                    if self.settings.color_blind_mode { "on" } else { "off" }
                );
                self.save_settings_effect()
            }
        }
    }

    fn activate_selected_setting(&mut self) -> AppEffect {
        match self.global_settings.selected {
            SettingsField::OpenWith => {
                self.begin_edit_open_with_setting();
                AppEffect::None
            }
            SettingsField::ColorBlindMode => self.adjust_selected_setting(1),
            SettingsField::TreeWidth | SettingsField::StorageThreshold | SettingsField::Theme => {
                self.adjust_selected_setting(1)
            }
        }
    }

    pub fn discovery_spinner(&self) -> &'static str {
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        FRAMES[self.discovery.ticks % FRAMES.len()]
    }

    pub fn discovery_elapsed_seconds(&self) -> usize {
        self.discovery.ticks / 4
    }

    pub fn apply_command(&mut self, command: AppCommand) -> AppEffect {
        match command {
            AppCommand::Noop => AppEffect::None,
            AppCommand::Quit => {
                self.should_quit = true;
                AppEffect::None
            }
            AppCommand::StopRun => {
                if self.running {
                    self.status = "Stopping run...".to_owned();
                    AppEffect::StopRun
                } else {
                    self.status = "No run in progress".to_owned();
                    AppEffect::None
                }
            }
            AppCommand::Resize => {
                self.on_resize();
                AppEffect::None
            }
            AppCommand::ToggleHelp => {
                self.toggle_help();
                self.status = if self.show_help {
                    "Help opened".to_owned()
                } else {
                    "Help closed".to_owned()
                };
                AppEffect::None
            }
            AppCommand::CloseHelp => {
                self.show_help = false;
                self.status = "Help closed".to_owned();
                AppEffect::None
            }
            AppCommand::ToggleFocus => {
                self.toggle_focus();
                AppEffect::None
            }
            AppCommand::MoveUp => {
                match self.command_focus() {
                    FocusPane::Tree => self.select_previous(),
                    FocusPane::Output => self.scroll_output_up(1),
                }
                AppEffect::None
            }
            AppCommand::MoveDown => {
                match self.command_focus() {
                    FocusPane::Tree => self.select_next(),
                    FocusPane::Output => self.scroll_output_down(1),
                }
                AppEffect::None
            }
            AppCommand::MoveLeft => {
                if self.command_focus() == FocusPane::Tree {
                    self.collapse_selected();
                }
                AppEffect::None
            }
            AppCommand::MoveRight => {
                if self.command_focus() == FocusPane::Tree {
                    self.expand_selected();
                }
                AppEffect::None
            }
            AppCommand::ToggleSelected => {
                self.toggle_selected();
                AppEffect::None
            }
            AppCommand::MoveHome => {
                match self.command_focus() {
                    FocusPane::Tree => self.select_first(),
                    FocusPane::Output => self.scroll_output_up(u16::MAX),
                }
                AppEffect::None
            }
            AppCommand::MoveEnd => {
                match self.command_focus() {
                    FocusPane::Tree => self.select_last(),
                    FocusPane::Output => self.scroll_output_bottom(),
                }
                AppEffect::None
            }
            AppCommand::PageUp => {
                match self.command_focus() {
                    FocusPane::Tree => self.select_previous_page(),
                    FocusPane::Output => self.scroll_output_up(self.output_page_size),
                }
                AppEffect::None
            }
            AppCommand::PageDown => {
                match self.command_focus() {
                    FocusPane::Tree => self.select_next_page(),
                    FocusPane::Output => self.scroll_output_down(self.output_page_size),
                }
                AppEffect::None
            }
            AppCommand::NarrowTestsPane => {
                self.resize_tests_pane(-(TREE_WIDTH_STEP_PERCENT as i16))
            }
            AppCommand::WidenTestsPane => self.resize_tests_pane(TREE_WIDTH_STEP_PERCENT as i16),
            AppCommand::RefreshTests => {
                if self.discovery.running {
                    self.status = "Discovery already in progress".to_owned();
                    AppEffect::None
                } else if self.running {
                    self.status = "Run in progress".to_owned();
                    AppEffect::None
                } else {
                    self.begin_discovery();
                    AppEffect::StartDiscovery
                }
            }
            AppCommand::RunSelected => {
                if self.discovery.running {
                    self.status = "Discovering tests".to_owned();
                    AppEffect::None
                } else {
                    AppEffect::StartRun(RunRequest {
                        scope: self.selected_scope(),
                    })
                }
            }
            AppCommand::RunFailed => {
                if self.discovery.running {
                    self.status = "Discovering tests".to_owned();
                    return AppEffect::None;
                }
                if let Some(scope) = self.failed_scope() {
                    AppEffect::StartRun(RunRequest { scope })
                } else {
                    self.status = "No failed tests to rerun".to_owned();
                    AppEffect::None
                }
            }
            AppCommand::OpenSource => self.open_source_effect(),
            AppCommand::OpenOutput => self.open_output_effect(),
            AppCommand::OpenSettings => {
                self.open_global_settings();
                AppEffect::None
            }
            AppCommand::CloseSettings => {
                self.close_global_settings();
                AppEffect::None
            }
            AppCommand::SettingsNext => {
                self.select_next_setting();
                AppEffect::None
            }
            AppCommand::SettingsPrevious => {
                self.select_previous_setting();
                AppEffect::None
            }
            AppCommand::SettingsAdjustLeft => self.adjust_selected_setting(-1),
            AppCommand::SettingsAdjustRight => self.adjust_selected_setting(1),
            AppCommand::SettingsActivate => self.activate_selected_setting(),
            AppCommand::SettingsOpenWithEdit(input) => {
                self.edit_open_with_setting(input);
                AppEffect::None
            }
            AppCommand::CommitOpenWithSetting => self.commit_open_with_setting(),
            AppCommand::CancelOpenWithSetting => {
                self.cancel_open_with_setting();
                AppEffect::None
            }
            AppCommand::ClearOpenWithSetting => self.clear_open_with_setting(),
            AppCommand::RefreshDiskUsage => {
                self.begin_disk_usage_scan();
                AppEffect::RefreshDiskUsage
            }
            AppCommand::OpenDiskCleanup => {
                self.disk_cleanup.modal_open = true;
                self.status = "Disk cleanup opened".to_owned();
                AppEffect::None
            }
            AppCommand::CloseDiskCleanup => {
                self.disk_cleanup.modal_open = false;
                self.status = "Disk cleanup closed".to_owned();
                AppEffect::None
            }
            AppCommand::RunCargoClean => {
                if self.begin_cargo_clean() {
                    AppEffect::RunCargoClean
                } else {
                    AppEffect::None
                }
            }
            AppCommand::ToggleShowSuccess => {
                self.toggle_show_success();
                AppEffect::None
            }
            AppCommand::ToggleShowFailed => {
                self.toggle_show_failed();
                AppEffect::None
            }
            AppCommand::ToggleShowIgnored => {
                self.toggle_show_ignored();
                AppEffect::None
            }
            AppCommand::ToggleShowSkipped => {
                self.toggle_show_skipped();
                AppEffect::None
            }
            AppCommand::SelectNextFailed => {
                self.select_next_failed();
                AppEffect::None
            }
            AppCommand::SelectPreviousFailed => {
                self.select_previous_failed();
                AppEffect::None
            }
            AppCommand::StartOutputSearch => {
                self.start_output_search();
                AppEffect::None
            }
            AppCommand::OpenOutputSearchModal => {
                self.open_output_search_modal();
                AppEffect::None
            }
            AppCommand::OutputSearchEdit(input) => {
                self.edit_output_search(input);
                AppEffect::None
            }
            AppCommand::ClearOutputSearch => {
                self.clear_output_search();
                AppEffect::None
            }
            AppCommand::ApplyOutputSearch => {
                self.apply_output_search();
                AppEffect::None
            }
            AppCommand::CancelOutputSearch => {
                self.cancel_output_search();
                AppEffect::None
            }
            AppCommand::SearchModalNextControl => {
                self.output_search.modal_focus = self.output_search.modal_focus.next();
                AppEffect::None
            }
            AppCommand::SearchModalPreviousControl => {
                self.output_search.modal_focus = self.output_search.modal_focus.previous();
                AppEffect::None
            }
            AppCommand::SearchModalActivate => {
                self.activate_output_search_modal_control();
                AppEffect::None
            }
            AppCommand::FindNextOutputMatch => {
                self.find_output_match(SearchDirection::Next);
                AppEffect::None
            }
            AppCommand::FindPreviousOutputMatch => {
                self.find_output_match(SearchDirection::Previous);
                AppEffect::None
            }
            AppCommand::ToggleOutputFilter => {
                self.toggle_output_filter();
                AppEffect::None
            }
            AppCommand::ToggleOutputRegex => {
                self.toggle_output_regex();
                AppEffect::None
            }
            AppCommand::ToggleOutputCaseSensitive => {
                self.toggle_output_case_sensitive();
                AppEffect::None
            }
            AppCommand::ReportStatus(status) => {
                self.status = status;
                AppEffect::None
            }
        }
    }

    pub fn select_next(&mut self) {
        self.with_selection_reset(|tree| tree.select_next());
    }

    pub fn select_previous(&mut self) {
        self.with_selection_reset(|tree| tree.select_previous());
    }

    pub fn select_first(&mut self) {
        self.with_selection_reset(|tree| tree.select_first());
    }

    pub fn select_last(&mut self) {
        self.with_selection_reset(|tree| tree.select_last());
    }

    pub fn select_next_page(&mut self) {
        let page_size = self.tree_page_size;
        self.with_selection_reset(|tree| tree.select_next_page(page_size));
    }

    pub fn select_previous_page(&mut self) {
        let page_size = self.tree_page_size;
        self.with_selection_reset(|tree| tree.select_previous_page(page_size));
    }

    pub fn toggle_selected(&mut self) {
        self.with_selection_reset(|tree| tree.toggle_selected());
    }

    pub fn expand_selected(&mut self) {
        self.with_selection_reset(|tree| tree.expand_selected());
    }

    pub fn collapse_selected(&mut self) {
        self.with_selection_reset(|tree| tree.collapse_selected_or_parent());
    }

    pub fn select_next_failed(&mut self) {
        let before = self.tree.selected_id().clone();
        if !self.tree.select_next_failed() {
            self.status = "No failed test visible".to_owned();
        }
        self.after_selection_action(before);
    }

    pub fn select_previous_failed(&mut self) {
        let before = self.tree.selected_id().clone();
        if !self.tree.select_previous_failed() {
            self.status = "No failed test visible".to_owned();
        }
        self.after_selection_action(before);
    }

    pub fn toggle_show_success(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_success = !filter.show_success;
        let enabled = filter.show_success;
        self.apply_tree_filter_change(filter);
        self.status = format!(
            "Show successful tests: {}",
            if enabled { "on" } else { "off" }
        );
    }

    pub fn toggle_show_failed(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_failed = !filter.show_failed;
        let enabled = filter.show_failed;
        self.apply_tree_filter_change(filter);
        self.status = format!(
            "Show failed tests: {}",
            if enabled { "on" } else { "off" }
        );
    }

    pub fn toggle_show_ignored(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_ignored = !filter.show_ignored;
        let enabled = filter.show_ignored;
        self.apply_tree_filter_change(filter);
        self.status = format!(
            "Show ignored tests: {}",
            if enabled { "on" } else { "off" }
        );
    }

    pub fn toggle_show_skipped(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_skipped = !filter.show_skipped;
        let enabled = filter.show_skipped;
        self.apply_tree_filter_change(filter);
        self.status = format!(
            "Show skipped tests: {}",
            if enabled { "on" } else { "off" }
        );
    }

    pub fn resize_tests_pane(&mut self, delta: i16) -> AppEffect {
        let before = self.settings.tree_width_percent;
        let after = config::resize_tree_width(before, delta);
        if before == after {
            self.status = format!("Tests pane width: {after}%");
            return AppEffect::None;
        }

        self.settings.tree_width_percent = after;
        self.ensure_tree_selection_visible();
        self.clamp_output_scroll();
        self.status = format!("Tests pane width: {after}%");
        AppEffect::SaveSettings(self.settings.clone())
    }

    pub fn scroll_output_up(&mut self, amount: u16) {
        self.output_scroll = scroll::up(self.output_scroll as usize, amount as usize) as u16;
        self.output_follow = false;
    }

    pub fn scroll_output_down(&mut self, amount: u16) {
        self.output_scroll = scroll::down(
            self.output_scroll as usize,
            amount as usize,
            usize::from(u16::MAX) + 1,
            1,
        ) as u16;
        self.output_follow = false;
    }

    pub fn scroll_output_bottom(&mut self) {
        self.output_follow = true;
    }

    pub fn output_text(&self) -> String {
        let text = self.output_source_text();
        self.output_search.filtered_text(&text)
    }

    pub fn output_search_error(&self) -> Option<String> {
        self.output_search.error()
    }

    pub fn selected_scope(&self) -> RunScope {
        let Some(node) = self.tree.selected_node() else {
            return RunScope::Workspace;
        };
        match &node.kind {
            NodeKind::Workspace => RunScope::Workspace,
            NodeKind::Package { name } => RunScope::Package { name: name.clone() },
            NodeKind::Binary {
                package,
                name,
                kind,
            } => RunScope::Binary {
                package: package.clone(),
                name: name.clone(),
                kind: kind.clone(),
            },
            NodeKind::Module { path } => RunScope::Module { path: path.clone() },
            NodeKind::Test(test) => RunScope::Test {
                name: test.full_name.clone(),
            },
        }
    }

    pub fn begin_run(&mut self, request: &RunRequest) -> bool {
        if self.running {
            self.status = "Run already in progress".to_owned();
            return false;
        }

        self.reset_for_run(request);
        self.begin_disk_usage_scan();
        true
    }

    pub fn apply_run_event(&mut self, event: RunEvent) {
        let finished = match event {
            RunEvent::RunMetadata { run_id, profile } => {
                if let Some(run_id) = run_id {
                    self.run.run_id = Some(run_id);
                }
                if let Some(profile) = profile {
                    self.run.profile = profile;
                }
                None
            }
            RunEvent::SuiteStarted { test_count } => {
                self.mark_tests_running();
                self.tree
                    .append_runner_output(format!("Starting {test_count} test(s)"));
                None
            }
            RunEvent::TestStarted { key } => {
                self.mark_tests_running();
                self.tree.start_test(&key);
                None
            }
            RunEvent::TestFinished {
                key,
                status,
                stdout,
                stderr,
                duration,
            } => {
                self.mark_tests_running();
                self.tree
                    .finish_test(&key, status, stdout, stderr, duration);
                None
            }
            RunEvent::RunnerOutput(line) => {
                self.tree.append_runner_output(line);
                None
            }
            RunEvent::RunnerFinished { exit_code } => Some((exit_code, false)),
            RunEvent::RunnerStopped => Some((None, true)),
        };

        if let Some((exit_code, stopped)) = finished {
            self.running = false;
            self.run.active = false;
            self.finish_run_timers();
            self.run.phase = RunPhase::NotRunning;
            self.run.exit_code = exit_code;
            if stopped {
                self.tree.stop_running_tests();
            }
            let counts = self.tree.status_counts_for_scope(&self.run.scope);
            self.run.outcome = if stopped {
                RunOutcome::Stopped
            } else {
                run_outcome(exit_code, counts)
            };
            self.status = run_summary_status(self.run.outcome, counts, exit_code);
        }
    }

    pub fn run_status_label(&self) -> &'static str {
        match self.run.phase {
            RunPhase::Building => "building",
            RunPhase::RunningTests => "running tests",
            RunPhase::NotRunning => "not running",
        }
    }

    pub fn run_result_label(&self) -> &'static str {
        match self.run.outcome {
            RunOutcome::NotStarted => "-",
            RunOutcome::Running => "running",
            RunOutcome::Passed => "passed",
            RunOutcome::Failed => "failed",
            RunOutcome::CommandFailed => "command failed",
            RunOutcome::Stopped => "stopped",
        }
    }

    pub fn run_duration(&self) -> Option<Duration> {
        if self.run.active {
            self.run.started_at.map(|started_at| started_at.elapsed())
        } else {
            self.run.finished_duration
        }
    }

    pub fn build_duration(&self) -> Option<Duration> {
        match self.run.phase {
            RunPhase::Building => self.run.started_at.map(|started_at| started_at.elapsed()),
            RunPhase::RunningTests | RunPhase::NotRunning => self.run.build_duration,
        }
    }

    pub fn test_duration(&self) -> Option<Duration> {
        match self.run.phase {
            RunPhase::RunningTests => self
                .run
                .tests_started_at
                .map(|tests_started_at| tests_started_at.elapsed()),
            RunPhase::Building | RunPhase::NotRunning => self.run.test_duration,
        }
    }

    pub fn run_progress(&self) -> (usize, usize) {
        self.tree.progress_for_scope(&self.run.scope)
    }

    pub fn failed_scope(&self) -> Option<RunScope> {
        let names = self.tree.failed_test_names();
        if names.is_empty() {
            None
        } else {
            Some(RunScope::Failed { names })
        }
    }

    fn open_source_effect(&mut self) -> AppEffect {
        let Some(location) = self.selected_source_location() else {
            self.status = "No source path available for selection".to_owned();
            return AppEffect::None;
        };
        self.status = format!("Opening {}", location.path.display());
        AppEffect::OpenSource(location)
    }

    fn open_output_effect(&mut self) -> AppEffect {
        let title = if self.discovery.error.is_some() {
            "Discovery failed".to_owned()
        } else {
            self.tree.selected_path()
        };
        let text = self.output_text();
        self.status = "Opening output".to_owned();
        AppEffect::OpenOutput(OutputOpenRequest { title, text })
    }

    fn command_focus(&self) -> FocusPane {
        if self.discovery.error.is_some() {
            FocusPane::Output
        } else {
            self.focus
        }
    }

    fn output_source_text(&self) -> String {
        if let Some(error) = &self.discovery.error {
            discovery_error_output(error)
        } else {
            self.tree.selected_output()
        }
    }

    fn selected_source_location(&self) -> Option<SourceLocation> {
        let node = self.tree.selected_node()?;
        match &node.kind {
            NodeKind::Test(test) => source_location_for_test(test, true),
            NodeKind::Binary { .. } | NodeKind::Module { .. } => {
                let test = first_descendant_test(node)?;
                source_location_for_test(test, false)
            }
            NodeKind::Workspace | NodeKind::Package { .. } => None,
        }
    }

    fn with_selection_reset(&mut self, action: impl FnOnce(&mut Tree)) {
        let before = self.tree.selected_id().clone();
        action(&mut self.tree);
        self.after_selection_action(before);
    }

    fn after_selection_action(&mut self, before: NodeId) {
        let after = self.tree.selected_id().clone();
        self.ensure_tree_selection_visible();
        if before != after {
            self.reset_output_for_source_change();
        }
    }

    fn apply_tree_filter_change(&mut self, filter: TestViewFilter) {
        let selection_change = self.tree.set_view_filter_preserving_selection(filter);
        self.ensure_tree_selection_visible();
        self.after_tree_view_changed(selection_change);
    }

    fn after_tree_view_changed(&mut self, selection_change: SelectionChange) {
        if selection_change.changed() {
            self.reset_output_for_source_change();
        } else {
            self.clamp_output_scroll();
        }
    }

    fn ensure_tree_selection_visible(&mut self) {
        let rows_len = self.tree.visible_rows().len();
        self.tree_scroll = scroll::ensure_visible(
            self.tree_scroll,
            self.tree.selected_index(),
            rows_len,
            self.tree_page_size,
        );
    }

    fn clamp_output_scroll(&mut self) {
        if self.output_follow {
            self.output_scroll = 0;
        }
    }

    fn max_output_scroll(&self, line_count: usize) -> u16 {
        scroll::max_scroll(line_count, self.output_page_size as usize).min(u16::MAX as usize) as u16
    }

    fn reset_for_run(&mut self, request: &RunRequest) {
        self.tree.prepare_for_run(&request.scope);
        self.status = format!("Building {}", request.scope.label());
        self.running = true;
        self.run.active = true;
        self.run.phase = RunPhase::Building;
        self.run.run_id = None;
        self.run.scope = request.scope.clone();
        self.run.outcome = RunOutcome::Running;
        self.run.exit_code = None;
        self.run.started_at = Some(Instant::now());
        self.run.tests_started_at = None;
        self.run.build_duration = None;
        self.run.test_duration = None;
        self.run.finished_duration = None;
        self.reset_output_for_source_change();
    }

    fn reset_output_for_source_change(&mut self) {
        self.output_scroll = 0;
        self.output_follow = true;
        self.output_search.current_line = None;
    }

    fn reset_output_for_modal(&mut self) {
        self.output_scroll = 0;
        self.output_follow = false;
        self.output_search.current_line = None;
    }

    fn mark_tests_running(&mut self) {
        if !self.run.active {
            return;
        }

        if self.run.phase == RunPhase::RunningTests {
            return;
        }

        let now = Instant::now();
        self.run.build_duration = self
            .run
            .started_at
            .map(|started_at| now.duration_since(started_at));
        self.run.tests_started_at = Some(now);
        self.run.phase = RunPhase::RunningTests;
        self.status = format!("Running tests for {}", self.run.scope.label());
    }

    fn finish_run_timers(&mut self) {
        let Some(started_at) = self.run.started_at else {
            return;
        };

        let total = started_at.elapsed();
        self.run.finished_duration = Some(total);
        match self.run.phase {
            RunPhase::Building => {
                self.run.build_duration = Some(total);
                self.run.test_duration = None;
            }
            RunPhase::RunningTests => {
                if self.run.build_duration.is_none() {
                    self.run.build_duration = self
                        .run
                        .tests_started_at
                        .map(|tests_started_at| tests_started_at.duration_since(started_at));
                }
                self.run.test_duration = self
                    .run
                    .tests_started_at
                    .map(|tests_started_at| tests_started_at.elapsed());
            }
            RunPhase::NotRunning => {}
        }
    }

    fn start_output_search(&mut self) {
        self.focus = FocusPane::Output;
        self.output_search.sync_draft_from_applied();
        self.output_search.input_active = true;
        self.output_search.modal_open = false;
        self.status = output_search_prompt(&self.output_search);
    }

    fn edit_output_search(&mut self, input: SearchEditorInput) {
        if self.output_search.modal_open
            && self.output_search.modal_focus != SearchModalFocus::Query
        {
            return;
        }
        if self.output_search.input_active || self.output_search.modal_open {
            self.output_search.edit_draft(input);
            self.status = output_search_prompt(&self.output_search);
        }
    }

    fn clear_output_search(&mut self) {
        if self.output_search.input_active || self.output_search.modal_open {
            self.output_search.clear_draft();
            self.status = "Output search draft cleared".to_owned();
            return;
        }
        self.output_search.sync_draft_from_applied();
        self.output_search.clear_draft();
        self.output_search.apply_draft();
        self.output_search.current_line = None;
        self.output_scroll = 0;
        self.output_follow = false;
        self.status = "Output search cleared".to_owned();
    }

    fn open_output_search_modal(&mut self) {
        self.output_search.input_active = false;
        self.output_search.modal_open = true;
        self.output_search.modal_focus = SearchModalFocus::Query;
        if self.output_search.draft_query.is_empty() && !self.output_search.query.is_empty() {
            self.output_search.sync_draft_from_applied();
        }
        self.status = "Output search options".to_owned();
    }

    fn apply_output_search(&mut self) {
        self.output_search.apply_draft();
        self.output_search.input_active = false;
        self.output_search.modal_open = false;
        if self.output_search.query.is_empty() {
            self.output_search.current_line = None;
            self.output_scroll = 0;
            self.output_follow = false;
            self.status = "Output search cleared".to_owned();
        } else {
            self.find_output_match(SearchDirection::Next);
        }
    }

    fn cancel_output_search(&mut self) {
        self.output_search.input_active = false;
        self.output_search.modal_open = false;
        self.output_search.sync_draft_from_applied();
        self.status = "Output search cancelled".to_owned();
    }

    fn activate_output_search_modal_control(&mut self) {
        match self.output_search.modal_focus {
            SearchModalFocus::Query => {
                self.output_search.edit_draft(SearchEditorInput::new(
                    SearchEditorKey::Enter,
                    false,
                    false,
                    false,
                ));
            }
            SearchModalFocus::Clear => self.output_search.clear_draft(),
            SearchModalFocus::Apply => self.apply_output_search(),
            SearchModalFocus::Filter => {
                self.output_search.draft_filter = !self.output_search.draft_filter;
            }
            SearchModalFocus::Regex => {
                self.output_search.draft_regex = !self.output_search.draft_regex;
            }
            SearchModalFocus::CaseSensitive => {
                self.output_search.draft_case_sensitive = !self.output_search.draft_case_sensitive;
            }
        }
    }

    fn toggle_output_filter(&mut self) {
        if self.output_search.modal_open {
            self.output_search.draft_filter = !self.output_search.draft_filter;
            return;
        }
        self.output_search.filter = !self.output_search.filter;
        self.output_search.sync_draft_from_applied();
        self.output_search.current_line = None;
        self.output_scroll = 0;
        self.output_follow = false;
        self.status = format!(
            "Output filter: {}",
            if self.output_search.filter { "on" } else { "off" }
        );
    }

    fn toggle_output_regex(&mut self) {
        if self.output_search.modal_open {
            self.output_search.draft_regex = !self.output_search.draft_regex;
            return;
        }
        self.output_search.regex = !self.output_search.regex;
        self.output_search.sync_draft_from_applied();
        self.output_search.current_line = None;
        self.status = match self.output_search_error() {
            Some(error) => format!("Invalid output search regex: {error}"),
            None => format!(
                "Output regex: {}",
                if self.output_search.regex { "on" } else { "off" }
            ),
        };
    }

    fn toggle_output_case_sensitive(&mut self) {
        if self.output_search.modal_open {
            self.output_search.draft_case_sensitive = !self.output_search.draft_case_sensitive;
            return;
        }
        self.output_search.case_sensitive = !self.output_search.case_sensitive;
        self.output_search.sync_draft_from_applied();
        self.output_search.current_line = None;
        self.status = format!(
            "Output case sensitive: {}",
            if self.output_search.case_sensitive {
                "on"
            } else {
                "off"
            }
        );
    }

    fn find_output_match(&mut self, direction: SearchDirection) {
        let matches = match self.output_search_match_lines() {
            Ok(matches) => matches,
            Err(error) => {
                self.status = format!("Invalid output search regex: {error}");
                return;
            }
        };

        if self.output_search.query.is_empty() {
            self.status = "No output search query".to_owned();
            return;
        }

        if matches.is_empty() {
            self.output_search.current_line = None;
            self.status = format!("No output matches for '{}'", self.output_search.query);
            return;
        }

        let text = self.output_text();
        let output_match = self
            .output_search
            .next_match(&text, direction)
            .expect("matches already validated")
            .expect("matches already non-empty");
        self.output_search.current_line = Some(output_match.line);
        self.output_scroll = output_match.line.min(u16::MAX as usize) as u16;
        self.output_follow = false;
        self.status = format!(
            "Output match {}/{} for '{}'",
            output_match.index + 1,
            output_match.total,
            self.output_search.query
        );
    }

    fn output_search_match_lines(&self) -> Result<Vec<usize>, String> {
        let text = self.output_text();
        self.output_search.match_lines(&text)
    }
}

fn first_descendant_test(node: &TestNode) -> Option<&DiscoveredTest> {
    if let NodeKind::Test(test) = &node.kind {
        return Some(test);
    }
    node.children.iter().find_map(first_descendant_test)
}

fn source_location_for_test(test: &DiscoveredTest, include_line: bool) -> Option<SourceLocation> {
    let path = test.source_path.clone()?;
    let line = include_line
        .then(|| source::find_test_line(&path, &test.full_name))
        .flatten();
    Some(SourceLocation { path, line })
}

fn output_search_prompt(search: &OutputSearchState) -> String {
    search.prompt()
}

fn discovery_error_output(error: &str) -> String {
    format!("Discovery failed\n\n{error}")
}

fn run_outcome(exit_code: Option<i32>, counts: StatusCounts) -> RunOutcome {
    match exit_code {
        Some(0) if counts.failed == 0 => RunOutcome::Passed,
        Some(_) if counts.failed > 0 => RunOutcome::Failed,
        Some(_) | None => RunOutcome::CommandFailed,
    }
}

fn run_summary_status(outcome: RunOutcome, counts: StatusCounts, exit_code: Option<i32>) -> String {
    match outcome {
        RunOutcome::Passed => format!(
            "Passed: {} passed, {} skipped, {} ignored",
            counts.passed, counts.skipped, counts.ignored
        ),
        RunOutcome::Failed => format!(
            "Failed: {} passed, {} failed, {} skipped, {} ignored",
            counts.passed, counts.failed, counts.skipped, counts.ignored
        ),
        RunOutcome::CommandFailed => match exit_code {
            Some(code) => format!("Command failed: nextest exited with {code}"),
            None => "Command failed: nextest did not complete".to_owned(),
        },
        RunOutcome::Stopped => format!(
            "Stopped: {} passed, {} failed, {} pending, {} skipped, {} ignored",
            counts.passed, counts.failed, counts.pending, counts.skipped, counts.ignored
        ),
        RunOutcome::Running => "Running tests".to_owned(),
        RunOutcome::NotStarted => "No run yet".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output_pane::{SearchEditorInput, SearchEditorKey};
    use crate::tree::{DiscoveredTest, TestKey, TestStatus};

    #[test]
    fn command_context_uses_normal_input_without_overlay_by_default() {
        let mut app = App::new(Tree::from_tests(test_rows(1)));

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
        let mut app = App::new(Tree::from_tests(test_rows(1)));

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
        let mut app = App::new(Tree::from_tests(test_rows(1)));

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
        let mut app = App::new(Tree::from_tests(test_rows(1)));

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
        let mut app = App::new(Tree::from_tests(test_rows(30)));
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
        let mut app = App::new(Tree::from_tests(test_rows(30)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
        let mut app = App::new(Tree::from_tests(test_rows(2)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));

        assert_eq!(app.apply_command(AppCommand::StopRun), AppEffect::None);
        assert_eq!(app.status, "No run in progress");

        assert!(app.begin_run(&RunRequest::default()));
        assert_eq!(app.apply_command(AppCommand::StopRun), AppEffect::StopRun);
        assert_eq!(app.status, "Stopping run...");
    }

    #[test]
    fn begin_run_refreshes_storage_status() {
        let mut app = App::new(Tree::from_tests(test_rows(1)));

        assert!(!app.disk_usage.loading);
        assert!(app.begin_run(&RunRequest::default()));

        assert!(app.disk_usage.loading);
    }

    #[test]
    fn stopped_run_records_stopped_result_and_clears_running_tests() {
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
        let mut app = App::new(Tree::from_tests(test_rows(2)));
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
        let mut app = App::new(Tree::from_tests(tests));
        assert!(app.begin_run(&RunRequest::default()));

        app.apply_run_event(RunEvent::TestStarted { key: test_key(1) });

        let counts = app.tree.status_counts_for_scope(&RunScope::Workspace);
        assert_eq!(counts.running, 0);
        assert_eq!(counts.ignored, 1);
        assert_eq!(app.run_progress(), (0, 1));
    }

    #[test]
    fn new_run_resets_previous_run_metadata_and_result() {
        let mut app = App::new(Tree::from_tests(test_rows(2)));
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
        let mut app = App::new(Tree::from_tests(test_rows(3)));
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
        let mut app = App::new(Tree::from_tests(test_rows(3)));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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

        assert_eq!(app.output_scroll, 2);
        assert_eq!(app.output_search.current_line, Some(2));

        app.apply_command(AppCommand::FindNextOutputMatch);

        assert_eq!(app.output_scroll, 4);
        assert_eq!(app.output_search.current_line, Some(4));

        app.apply_command(AppCommand::FindPreviousOutputMatch);

        assert_eq!(app.output_scroll, 2);
        assert_eq!(app.output_search.current_line, Some(2));
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
        assert_eq!(app.output_scroll, 2);
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
        assert_eq!(app.output_search.current_line, Some(2));

        app.apply_command(AppCommand::StartOutputSearch);
        app.apply_command(AppCommand::ClearOutputSearch);

        assert!(app.output_search.input_active);
        assert_eq!(app.output_search.draft_query, "");
        assert_eq!(app.output_search.query, "pa");
        assert_eq!(app.output_search.current_line, Some(2));
        assert_eq!(app.status, "Output search draft cleared");
    }

    #[test]
    fn discovery_error_uses_output_scroll_and_search() {
        let mut app = App::new(Tree::from_tests(test_rows(3)));
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
        let mut app = App::new(Tree::from_tests(test_rows(3)));
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
        assert_eq!(app.output_search.current_line, Some(2));
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
        let mut app = App::new(Tree::from_tests(test_rows(1)));
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
}
