use std::time::{Duration, Instant};

use crate::{
    command::{AppCommand, CommandContext},
    config::{self, AppSettings, TREE_WIDTH_STEP_PERCENT},
    git_status::GitStatus,
    nextest::{DiscoveryEvent, RunEvent, RunRequest, RunScope},
    tree::{NodeKind, TestStatus, Tree},
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
    pub focus: FocusPane,
    pub show_help: bool,
    pub tree_page_size: usize,
    pub output_page_size: u16,
    pub discovery: DiscoveryState,
    pub git_status: GitStatus,
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
    pub run_id: Option<String>,
    pub profile: String,
    pub scope: RunScope,
    started_at: Option<Instant>,
    finished_duration: Option<Duration>,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            active: false,
            run_id: None,
            profile: "default".to_owned(),
            scope: RunScope::Workspace,
            started_at: None,
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
            focus: FocusPane::Tree,
            show_help: false,
            tree_page_size: 1,
            output_page_size: 1,
            discovery: DiscoveryState::default(),
            git_status: GitStatus::unknown(),
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
        let line_count = self.tree.selected_output().lines().count().max(1);
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
        CommandContext {
            help_visible: self.show_help,
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
                self.output_scroll = 0;
                self.output_follow = true;
                self.discovery.running = false;
                self.discovery.error = None;
                self.status = format!("Discovered {count} test(s)");
                true
            }
            DiscoveryEvent::Finished(Err(error)) => {
                self.discovery.running = false;
                self.discovery.error = Some(error);
                self.status = "Discovery failed".to_owned();
                false
            }
        }
    }

    pub fn apply_git_status(&mut self, git_status: GitStatus) {
        self.git_status = git_status;
    }

    pub fn is_discovering(&self) -> bool {
        self.discovery.running
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
                match self.focus {
                    FocusPane::Tree => self.select_previous(),
                    FocusPane::Output => self.scroll_output_up(1),
                }
                AppEffect::None
            }
            AppCommand::MoveDown => {
                match self.focus {
                    FocusPane::Tree => self.select_next(),
                    FocusPane::Output => self.scroll_output_down(1),
                }
                AppEffect::None
            }
            AppCommand::MoveLeft => {
                self.collapse_selected();
                AppEffect::None
            }
            AppCommand::MoveRight => {
                self.expand_selected();
                AppEffect::None
            }
            AppCommand::ToggleSelected => {
                self.toggle_selected();
                AppEffect::None
            }
            AppCommand::MoveHome => {
                match self.focus {
                    FocusPane::Tree => self.select_first(),
                    FocusPane::Output => self.scroll_output_up(u16::MAX),
                }
                AppEffect::None
            }
            AppCommand::MoveEnd => {
                match self.focus {
                    FocusPane::Tree => self.select_last(),
                    FocusPane::Output => self.scroll_output_bottom(),
                }
                AppEffect::None
            }
            AppCommand::PageUp => {
                match self.focus {
                    FocusPane::Tree => self.select_previous_page(),
                    FocusPane::Output => self.scroll_output_up(self.output_page_size),
                }
                AppEffect::None
            }
            AppCommand::PageDown => {
                match self.focus {
                    FocusPane::Tree => self.select_next_page(),
                    FocusPane::Output => self.scroll_output_down(self.output_page_size),
                }
                AppEffect::None
            }
            AppCommand::NarrowTestsPane => {
                self.resize_tests_pane(-(TREE_WIDTH_STEP_PERCENT as i16))
            }
            AppCommand::WidenTestsPane => {
                self.resize_tests_pane(TREE_WIDTH_STEP_PERCENT as i16)
            }
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
            AppCommand::SelectNextFailed => {
                self.select_next_failed();
                AppEffect::None
            }
            AppCommand::SelectPreviousFailed => {
                self.select_previous_failed();
                AppEffect::None
            }
            AppCommand::SearchNavigationPending => {
                self.status = "Search navigation is planned for phase 3".to_owned();
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
        self.with_selection_reset(|tree| tree.collapse_selected());
    }

    pub fn select_next_failed(&mut self) {
        let before = self.tree.selected_index();
        if !self.tree.select_next_failed() {
            self.status = "No failed test visible".to_owned();
        }
        self.after_selection_action(before);
    }

    pub fn select_previous_failed(&mut self) {
        let before = self.tree.selected_index();
        if !self.tree.select_previous_failed() {
            self.status = "No failed test visible".to_owned();
        }
        self.after_selection_action(before);
    }

    pub fn toggle_show_success(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_success = !filter.show_success;
        self.tree.set_view_filter(filter);
        self.ensure_tree_selection_visible();
        self.output_scroll = 0;
        self.output_follow = true;
        self.status = format!(
            "Show successful tests: {}",
            if filter.show_success { "on" } else { "off" }
        );
    }

    pub fn toggle_show_failed(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_failed = !filter.show_failed;
        self.tree.set_view_filter(filter);
        self.ensure_tree_selection_visible();
        self.output_scroll = 0;
        self.output_follow = true;
        self.status = format!(
            "Show failed tests: {}",
            if filter.show_failed { "on" } else { "off" }
        );
    }

    pub fn toggle_show_ignored(&mut self) {
        let mut filter = self.tree.view_filter;
        filter.show_ignored = !filter.show_ignored;
        self.tree.set_view_filter(filter);
        self.ensure_tree_selection_visible();
        self.output_scroll = 0;
        self.output_follow = true;
        self.status = format!(
            "Show ignored tests: {}",
            if filter.show_ignored { "on" } else { "off" }
        );
    }

    pub fn resize_tests_pane(&mut self, delta: i16) -> AppEffect {
        let before = self.settings.tree_width_percent;
        let after = config::clamp_tree_width(before.saturating_add_signed(delta));
        if before == after {
            self.status = format!("Tests pane width: {after}%");
            return AppEffect::None;
        }

        self.settings.tree_width_percent = after;
        self.ensure_tree_selection_visible();
        self.clamp_output_scroll();
        self.status = format!("Tests pane width: {after}%");
        AppEffect::SaveSettings(self.settings)
    }

    pub fn scroll_output_up(&mut self, amount: u16) {
        self.output_scroll = self.output_scroll.saturating_sub(amount.max(1));
        self.output_follow = false;
    }

    pub fn scroll_output_down(&mut self, amount: u16) {
        self.output_scroll = self.output_scroll.saturating_add(amount.max(1));
    }

    pub fn scroll_output_bottom(&mut self) {
        self.output_follow = true;
    }

    pub fn selected_scope(&self) -> RunScope {
        let Some(node) = self.tree.selected_node() else {
            return RunScope::Workspace;
        };
        match &node.kind {
            NodeKind::Workspace => RunScope::Workspace,
            NodeKind::Package { name } => RunScope::Package { name: name.clone() },
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

        self.tree.mark_scope_pending(&request.scope);
        self.status = format!("Running {}", request.scope.label());
        self.running = true;
        self.run.active = true;
        self.run.run_id = None;
        self.run.scope = request.scope.clone();
        self.run.started_at = Some(Instant::now());
        self.run.finished_duration = None;
        self.output_follow = true;
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
                false
            }
            RunEvent::TestStarted { key } => {
                self.tree.update_status(&key, TestStatus::Running);
                false
            }
            RunEvent::TestFinished {
                key,
                status,
                stdout,
                stderr,
                duration,
            } => {
                self.tree
                    .finish_test(&key, status, stdout, stderr, duration);
                false
            }
            RunEvent::RunnerOutput(line) => {
                self.tree.append_runner_output(line);
                false
            }
            RunEvent::RunnerFinished { exit_code } => {
                self.status = match exit_code {
                    Some(code) => format!("nextest exited with {code}"),
                    None => "nextest process ended".to_owned(),
                };
                true
            }
        };

        if finished {
            self.running = false;
            self.run.active = false;
            self.run.finished_duration = self
                .run
                .started_at
                .map(|started_at| started_at.elapsed());
            let counts = self.tree.status_counts();
            self.status = format!(
                "Done: {} passed, {} failed, {} skipped, {} ignored",
                counts.passed, counts.failed, counts.skipped, counts.ignored
            );
        }
    }

    pub fn run_status_label(&self) -> &'static str {
        if self.run.active {
            "running"
        } else if self.run.finished_duration.is_some() {
            "complete"
        } else {
            "not running"
        }
    }

    pub fn run_duration(&self) -> Option<Duration> {
        if self.run.active {
            self.run.started_at.map(|started_at| started_at.elapsed())
        } else {
            self.run.finished_duration
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

    fn with_selection_reset(&mut self, action: impl FnOnce(&mut Tree)) {
        let before = self.tree.selected_index();
        action(&mut self.tree);
        self.after_selection_action(before);
    }

    fn after_selection_action(&mut self, before: usize) {
        let after = self.tree.selected_index();
        self.ensure_tree_selection_visible();
        if before != after {
            self.output_scroll = 0;
            self.output_follow = true;
        }
    }

    fn ensure_tree_selection_visible(&mut self) {
        let rows_len = self.tree.visible_rows().len();
        if rows_len == 0 {
            self.tree_scroll = 0;
            return;
        }

        let selected = self.tree.selected_index();
        let viewport = self.tree_page_size.max(1);
        if selected < self.tree_scroll {
            self.tree_scroll = selected;
        } else if selected >= self.tree_scroll.saturating_add(viewport) {
            self.tree_scroll = selected.saturating_add(1).saturating_sub(viewport);
        }

        let max_scroll = rows_len.saturating_sub(viewport);
        self.tree_scroll = self.tree_scroll.min(max_scroll);
    }

    fn clamp_output_scroll(&mut self) {
        if self.output_follow {
            self.output_scroll = 0;
        }
    }

    fn max_output_scroll(&self, line_count: usize) -> u16 {
        line_count
            .saturating_sub(self.output_page_size as usize)
            .min(u16::MAX as usize) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{DiscoveredTest, TestKey};

    #[test]
    fn tree_scroll_follows_selection_past_viewport() {
        let mut app = App::new(Tree::from_tests(test_rows(30)));
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
            },
        );

        let effect = app.resize_tests_pane(5);

        assert_eq!(app.settings.tree_width_percent, 50);
        assert_eq!(
            effect,
            AppEffect::SaveSettings(AppSettings {
                tree_width_percent: 50,
            })
        );
    }

    #[test]
    fn resize_tests_pane_clamps_to_supported_range() {
        let mut app = App::with_settings(
            Tree::from_tests(test_rows(1)),
            AppSettings {
                tree_width_percent: 25,
            },
        );

        let effect = app.resize_tests_pane(-5);

        assert_eq!(app.settings.tree_width_percent, 25);
        assert_eq!(effect, AppEffect::None);
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
                key: TestKey {
                    binary_id: Some("demo".to_owned()),
                    event_prefix: Some("demo::demo".to_owned()),
                    name: format!("tests::case_{index:02}"),
                },
                package: "demo".to_owned(),
                binary: "demo".to_owned(),
                module: Some("tests".to_owned()),
                name: format!("case_{index:02}"),
                full_name: format!("tests::case_{index:02}"),
                status: TestStatus::Pending,
                ignored: false,
            })
            .collect()
    }
}
