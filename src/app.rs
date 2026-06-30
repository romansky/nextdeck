use crate::{
    nextest::{NextestClient, RunEvent, RunRequest, RunScope},
    tree::{NodeKind, TestStatus, Tree},
};
use tokio::sync::mpsc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusPane {
    Tree,
    Output,
}

pub struct App {
    pub tree: Tree,
    pub tree_scroll: usize,
    pub status: String,
    pub running: bool,
    pub should_quit: bool,
    pub output_scroll: u16,
    pub output_follow: bool,
    pub focus: FocusPane,
    pub show_help: bool,
    pub tree_page_size: usize,
    pub output_page_size: u16,
    run_rx: Option<mpsc::UnboundedReceiver<RunEvent>>,
}

impl App {
    pub fn new(tree: Tree) -> Self {
        Self {
            tree,
            tree_scroll: 0,
            status: "Ready".to_owned(),
            running: false,
            should_quit: false,
            output_scroll: 0,
            output_follow: true,
            focus: FocusPane::Tree,
            show_help: false,
            tree_page_size: 1,
            output_page_size: 1,
            run_rx: None,
        }
    }

    pub fn prepare_draw(&mut self) {
        self.ensure_tree_selection_visible();
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

    pub fn start_run(&mut self, client: NextestClient, request: RunRequest) {
        if self.running {
            self.status = "Run already in progress".to_owned();
            return;
        }

        self.tree.mark_scope_pending(&request.scope);
        self.status = format!("Running {}", request.scope.label());
        self.running = true;
        self.output_follow = true;

        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            if let Err(error) = client.run(request, tx.clone()).await {
                let _ = tx.send(RunEvent::RunnerOutput(format!(
                    "nextest failed to start: {error}"
                )));
                let _ = tx.send(RunEvent::RunnerFinished { exit_code: None });
            }
        });
        self.run_rx = Some(rx);
    }

    pub fn drain_run_events(&mut self) {
        let mut finished = false;
        while let Some(rx) = &mut self.run_rx {
            match rx.try_recv() {
                Ok(event) => {
                    if self.apply_run_event(event) {
                        finished = true;
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }

        if finished {
            self.running = false;
            self.run_rx = None;
            let counts = self.tree.status_counts();
            self.status = format!(
                "Done: {} passed, {} failed, {} skipped, {} ignored",
                counts.passed, counts.failed, counts.skipped, counts.ignored
            );
        }
    }

    fn apply_run_event(&mut self, event: RunEvent) -> bool {
        match event {
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
        }
    }

    pub fn failed_scope(&self) -> Option<RunScope> {
        let names = self.tree.failed_test_names();
        if names.is_empty() {
            None
        } else {
            Some(RunScope::Failed { names })
        }
    }

    pub fn status_line(&self) -> String {
        let counts = self.tree.status_counts();
        let focus = match self.focus {
            FocusPane::Tree => "tree",
            FocusPane::Output => "output",
        };
        format!(
            "{} | {} | h/? help | {} | {} passed  {} failed  {} running  {} pending",
            self.status,
            focus,
            self.tree.selected_path(),
            counts.passed,
            counts.failed,
            counts.running,
            counts.pending
        )
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
