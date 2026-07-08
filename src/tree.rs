use std::{
    path::{Component, PathBuf},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    config::TreeDurationMode,
    output::{TestOutput, append_bounded_text, bounded_text},
    state::StatusCounts,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TestKey {
    pub binary_id: Option<String>,
    pub event_prefix: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveredTest {
    pub key: TestKey,
    pub package: String,
    pub binary: String,
    pub binary_kind: String,
    pub cwd: PathBuf,
    pub source_path: Option<PathBuf>,
    pub module: Option<String>,
    pub name: String,
    pub full_name: String,
    pub status: TestStatus,
    pub ignored: bool,
    pub ignore_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Ignored,
    Skipped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestViewFilter {
    pub show_success: bool,
    pub show_failed: bool,
    pub show_ignored: bool,
    pub show_skipped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionChange {
    Unchanged,
    Changed,
}

impl SelectionChange {
    pub fn changed(self) -> bool {
        self == Self::Changed
    }
}

impl Default for TestViewFilter {
    fn default() -> Self {
        Self {
            show_success: true,
            show_failed: true,
            show_ignored: true,
            show_skipped: true,
        }
    }
}

impl TestViewFilter {
    fn allows(self, status: TestStatus) -> bool {
        match status {
            TestStatus::Passed => self.show_success,
            TestStatus::Failed => self.show_failed,
            TestStatus::Ignored => self.show_ignored,
            TestStatus::Skipped => self.show_skipped,
            TestStatus::Pending | TestStatus::Running => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Workspace,
    Package {
        name: String,
    },
    Binary {
        package: String,
        name: String,
        kind: String,
    },
    Module {
        path: String,
    },
    Test(Box<DiscoveredTest>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeId {
    Workspace,
    Package {
        name: String,
    },
    Binary {
        package: String,
        name: String,
        kind: String,
    },
    Module {
        package: String,
        binary: String,
        kind: String,
        path: String,
    },
    Test {
        key: TestKey,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestNode {
    pub id: NodeId,
    pub label: String,
    pub kind: NodeKind,
    pub status: TestStatus,
    pub output: TestOutput,
    pub started_at: Option<Instant>,
    pub run_started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub expanded: bool,
    pub children: Vec<TestNode>,
}

impl TestNode {
    fn new(id: NodeId, label: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            status: TestStatus::Pending,
            output: TestOutput::default(),
            started_at: None,
            run_started_at: None,
            finished_at: None,
            expanded: false,
            children: Vec::new(),
        }
    }

    pub fn display_duration(&self, mode: TreeDurationMode) -> Option<Duration> {
        self.display_duration_at(mode, Instant::now())
    }

    fn display_duration_at(&self, mode: TreeDurationMode, now: Instant) -> Option<Duration> {
        if matches!(self.kind, NodeKind::Test(_)) {
            return self.test_display_duration_at(now);
        }

        match mode {
            TreeDurationMode::Wall => self
                .duration_span_at(now)
                .map(|(started_at, finished_at)| finished_at.duration_since(started_at))
                .or_else(|| self.aggregate_duration_at(now)),
            TreeDurationMode::Aggregate => self.aggregate_duration_at(now),
        }
    }

    fn test_display_duration_at(&self, now: Instant) -> Option<Duration> {
        self.output.duration.or_else(|| {
            (self.status == TestStatus::Running)
                .then(|| {
                    self.started_at
                        .map(|started_at| now.duration_since(started_at))
                })
                .flatten()
        })
    }

    fn aggregate_duration_at(&self, now: Instant) -> Option<Duration> {
        let mut total = Duration::ZERO;
        let mut has_duration = false;
        for child in &self.children {
            if let Some(duration) = child.display_duration_at(TreeDurationMode::Aggregate, now) {
                total += duration;
                has_duration = true;
            }
        }
        has_duration.then_some(total)
    }

    fn duration_span_at(&self, now: Instant) -> Option<(Instant, Instant)> {
        if matches!(self.kind, NodeKind::Test(_)) {
            if self.status == TestStatus::Running {
                return self.started_at.map(|started_at| (started_at, now));
            }
            let finished_at = self.finished_at?;
            let started_at = self.run_started_at.or_else(|| {
                self.output
                    .duration
                    .and_then(|duration| finished_at.checked_sub(duration))
            })?;
            return Some((started_at, finished_at));
        }

        let mut started_at = None;
        let mut finished_at = None;
        for child in &self.children {
            let Some((child_started, child_finished)) = child.duration_span_at(now) else {
                continue;
            };
            started_at = Some(
                started_at.map_or(child_started, |current: Instant| current.min(child_started)),
            );
            finished_at = Some(finished_at.map_or(child_finished, |current: Instant| {
                current.max(child_finished)
            }));
        }
        started_at.zip(finished_at)
    }
}

#[derive(Clone, Copy)]
pub struct TreeRow<'a> {
    pub depth: usize,
    pub node: &'a TestNode,
}

pub struct VisibleTreeRows<'a> {
    pub rows: Vec<TreeRow<'a>>,
    pub selected_index: usize,
}

pub struct Tree {
    pub root: TestNode,
    pub view_filter: TestViewFilter,
    selected: NodeId,
    runner_output: Vec<String>,
}

impl Tree {
    pub fn from_tests(tests: Vec<DiscoveredTest>) -> Self {
        let mut root = TestNode::new(
            NodeId::Workspace,
            workspace_label(&tests),
            NodeKind::Workspace,
        );
        root.expanded = true;
        let mut tree = Self {
            root,
            view_filter: TestViewFilter::default(),
            selected: NodeId::Workspace,
            runner_output: Vec::new(),
        };
        for test in tests {
            tree.insert_test(test);
        }
        tree.recompute_statuses();
        tree
    }

    pub fn visible_rows(&self) -> Vec<TreeRow<'_>> {
        let mut rows = Vec::new();
        collect_visible(&self.root, 0, self.view_filter, true, &mut rows);
        rows
    }

    pub fn visible_rows_with_selection(&self) -> VisibleTreeRows<'_> {
        let rows = self.visible_rows();
        let selected_index = rows
            .iter()
            .position(|row| row.node.id == self.selected)
            .unwrap_or(0);
        VisibleTreeRows {
            rows,
            selected_index,
        }
    }

    pub fn set_view_filter(&mut self, filter: TestViewFilter) {
        self.set_view_filter_preserving_selection(filter);
    }

    pub fn set_view_filter_preserving_selection(
        &mut self,
        filter: TestViewFilter,
    ) -> SelectionChange {
        let before = self.selected.clone();
        self.view_filter = filter;
        if !self.is_selected_visible() {
            self.clamp_selection();
        }
        if self.selected == before {
            SelectionChange::Unchanged
        } else {
            SelectionChange::Changed
        }
    }

    pub fn selected_index(&self) -> usize {
        self.visible_rows_with_selection().selected_index
    }

    pub fn selected_node(&self) -> Option<&TestNode> {
        self.node(&self.selected)
    }

    pub fn selected_id(&self) -> &NodeId {
        &self.selected
    }

    pub fn select_next(&mut self) {
        let visible = self.visible_rows_with_selection();
        let rows = visible.rows;
        if !rows.is_empty() {
            let index = visible.selected_index.saturating_add(1).min(rows.len() - 1);
            self.selected = rows[index].node.id.clone();
        }
    }

    pub fn select_previous(&mut self) {
        let visible = self.visible_rows_with_selection();
        let rows = visible.rows;
        if !rows.is_empty() {
            let index = visible.selected_index.saturating_sub(1);
            self.selected = rows[index].node.id.clone();
        }
    }

    pub fn select_first(&mut self) {
        if let Some(row) = self.visible_rows().first() {
            self.selected = row.node.id.clone();
        }
    }

    pub fn select_last(&mut self) {
        if let Some(row) = self.visible_rows().last() {
            self.selected = row.node.id.clone();
        }
    }

    pub fn select_next_page(&mut self, page_size: usize) {
        let visible = self.visible_rows_with_selection();
        let rows = visible.rows;
        if !rows.is_empty() {
            let index = visible
                .selected_index
                .saturating_add(page_size.max(1))
                .min(rows.len().saturating_sub(1));
            self.selected = rows[index].node.id.clone();
        }
    }

    pub fn select_previous_page(&mut self, page_size: usize) {
        let visible = self.visible_rows_with_selection();
        let rows = visible.rows;
        if !rows.is_empty() {
            let index = visible.selected_index.saturating_sub(page_size.max(1));
            self.selected = rows[index].node.id.clone();
        }
    }

    pub fn toggle_selected(&mut self) {
        self.with_selected_mut(|node| {
            if !node.children.is_empty() {
                node.expanded = !node.expanded;
            }
        });
        self.clamp_selection();
    }

    pub fn expand_selected(&mut self) {
        self.with_selected_mut(|node| {
            if !node.children.is_empty() {
                node.expanded = true;
            }
        });
        self.clamp_selection();
    }

    pub fn collapse_selected_or_parent(&mut self) {
        let target = {
            let rows = self.visible_rows();
            let selected = self.selected_index();
            let Some(selected_row) = rows.get(selected) else {
                return;
            };
            let selected_depth = selected_row.depth;
            let selected_node = selected_row.node;

            if !selected_node.children.is_empty() && selected_node.expanded {
                Some(selected_node.id.clone())
            } else {
                rows[..selected]
                    .iter()
                    .rev()
                    .find_map(|row| (row.depth < selected_depth).then_some(row.node.id.clone()))
            }
        };

        let Some(target) = target else {
            return;
        };

        self.with_id_mut(&target, |node| node.expanded = false);
        self.selected = target;
        self.clamp_selection();
    }

    pub fn prepare_for_run(&mut self, scope: &crate::nextest::RunScope) {
        self.clear_runner_output();
        visit_mut(&mut self.root, &mut |node| {
            if let NodeKind::Test(test) = &node.kind {
                node.output = TestOutput::default();
                node.started_at = None;
                node.run_started_at = None;
                node.finished_at = None;
                if test.ignored {
                    node.status = TestStatus::Ignored;
                } else if scope.matches_test(test) {
                    node.status = TestStatus::Pending;
                } else {
                    node.status = TestStatus::Skipped;
                }
            }
        });
        self.recompute_statuses();
    }

    pub fn start_test(&mut self, key: &TestKey) {
        visit_mut(&mut self.root, &mut |node| {
            if node_matches(node, key)
                && let NodeKind::Test(test) = &node.kind
                && !test.ignored
            {
                let now = Instant::now();
                node.status = TestStatus::Running;
                node.started_at = Some(now);
                node.run_started_at = Some(now);
                node.finished_at = None;
            }
        });
        self.recompute_statuses();
    }

    pub fn finish_test(
        &mut self,
        key: &TestKey,
        status: TestStatus,
        stdout: String,
        stderr: String,
        duration: Option<Duration>,
    ) {
        visit_mut(&mut self.root, &mut |node| {
            if node_matches(node, key) {
                node.status = status;
                node.started_at = None;
                node.finished_at = Some(Instant::now());
                let stdout = merge_finished_output(&node.output.stdout, &stdout);
                let stderr = merge_finished_output(&node.output.stderr, &stderr);
                node.output = TestOutput {
                    stdout,
                    stderr,
                    duration,
                };
            }
        });
        self.recompute_statuses();
    }

    pub fn append_test_output(&mut self, key: &TestKey, stdout: String, stderr: String) {
        visit_mut(&mut self.root, &mut |node| {
            if node_matches(node, key) {
                append_output_text(&mut node.output.stdout, &stdout);
                append_output_text(&mut node.output.stderr, &stderr);
            }
        });
    }

    pub fn append_test_event(
        &mut self,
        event: &nextdeck_test_events::TestEvent,
        line: &str,
    ) -> bool {
        let Some(target) = self.event_target_node_id(event) else {
            return false;
        };
        self.with_id_mut(&target, |node| {
            append_output_text(&mut node.output.stdout, line);
        });
        true
    }

    pub fn stop_running_tests(&mut self) {
        visit_mut(&mut self.root, &mut |node| {
            if node.status == TestStatus::Running {
                node.status = TestStatus::Pending;
                node.started_at = None;
                node.run_started_at = None;
                node.finished_at = None;
            }
        });
        self.recompute_statuses();
    }

    pub fn append_runner_output(&mut self, line: String) {
        self.runner_output.push(bounded_text(line));
        if self.runner_output.len() > 500 {
            self.runner_output.drain(0..100);
        }
    }

    fn clear_runner_output(&mut self) {
        self.runner_output.clear();
    }

    pub fn refresh_from_tests(&mut self, tests: Vec<DiscoveredTest>) {
        let filter = self.view_filter;
        *self = Self::from_tests(tests);
        self.set_view_filter(filter);
    }

    pub fn selected_output(&self) -> String {
        if let Some(node) = self.selected_node() {
            if matches!(node.kind, NodeKind::Test(_)) {
                return node.output.captured_text();
            }
            return descendant_outputs(node);
        }

        if self.runner_output.is_empty() {
            "Select a test to inspect captured output. Press r to run, R to rerun failures, q to quit."
                .to_owned()
        } else {
            self.runner_output.join("\n")
        }
    }

    pub fn failed_test_selectors(&self) -> Vec<crate::nextest::TestSelector> {
        let mut tests = Vec::new();
        visit(&self.root, &mut |node| {
            if node.status == TestStatus::Failed
                && let NodeKind::Test(test) = &node.kind
            {
                tests.push(crate::nextest::TestSelector::from_test(test));
            }
        });
        tests
    }

    pub fn select_next_failed(&mut self) -> bool {
        let visible = self.visible_rows_with_selection();
        let rows = visible.rows;
        if rows.is_empty() {
            return false;
        }
        let start = visible.selected_index.saturating_add(1);
        if let Some(index) = rows
            .iter()
            .enumerate()
            .skip(start)
            .chain(rows.iter().enumerate().take(start))
            .find_map(|(index, row)| (row.node.status == TestStatus::Failed).then_some(index))
        {
            self.selected = rows[index].node.id.clone();
            true
        } else {
            false
        }
    }

    pub fn select_previous_failed(&mut self) -> bool {
        let visible = self.visible_rows_with_selection();
        let rows = visible.rows;
        if rows.is_empty() {
            return false;
        }
        let start = visible.selected_index;
        let mut indices = (0..start).rev().chain((start + 1..rows.len()).rev());
        if let Some(index) = indices.find(|index| rows[*index].node.status == TestStatus::Failed) {
            self.selected = rows[index].node.id.clone();
            true
        } else {
            false
        }
    }

    pub fn selected_path(&self) -> String {
        let Some(node) = self.selected_node() else {
            return "workspace".to_owned();
        };

        match &node.kind {
            NodeKind::Workspace => "workspace".to_owned(),
            NodeKind::Package { name } => name.clone(),
            NodeKind::Binary { package, name, .. } => format!("{package}::{name}"),
            NodeKind::Module { path } => path.clone(),
            NodeKind::Test(test) => format!("{}::{}", test.package, test.full_name),
        }
    }

    pub fn status_counts_for_scope(&self, scope: &crate::nextest::RunScope) -> StatusCounts {
        let mut counts = StatusCounts::default();
        visit(&self.root, &mut |node| {
            if let NodeKind::Test(test) = &node.kind
                && scope.matches_test(test)
            {
                add_status_count(&mut counts, node.status);
            }
        });
        counts
    }

    pub fn progress_for_scope(&self, scope: &crate::nextest::RunScope) -> (usize, usize) {
        let mut finished = 0;
        let mut total = 0;
        visit(&self.root, &mut |node| {
            if let NodeKind::Test(test) = &node.kind
                && scope.matches_test(test)
                && !test.ignored
            {
                total += 1;
                if matches!(
                    node.status,
                    TestStatus::Passed | TestStatus::Failed | TestStatus::Skipped
                ) {
                    finished += 1;
                }
            }
        });
        (finished, total)
    }

    fn insert_test(&mut self, test: DiscoveredTest) {
        let package_index = child_position(&self.root, &test.package).unwrap_or_else(|| {
            self.root.children.push(TestNode::new(
                NodeId::Package {
                    name: test.package.clone(),
                },
                &test.package,
                NodeKind::Package {
                    name: test.package.clone(),
                },
            ));
            self.root.children.len() - 1
        });

        let mut parent = &mut self.root.children[package_index];
        if test.binary_kind == "test" {
            let index = child_position(parent, &test.binary).unwrap_or_else(|| {
                parent.children.push(TestNode::new(
                    NodeId::Binary {
                        package: test.package.clone(),
                        name: test.binary.clone(),
                        kind: test.binary_kind.clone(),
                    },
                    &test.binary,
                    NodeKind::Binary {
                        package: test.package.clone(),
                        name: test.binary.clone(),
                        kind: test.binary_kind.clone(),
                    },
                ));
                parent.children.len() - 1
            });
            parent = &mut parent.children[index];
        }

        let mut module_path = String::new();
        if let Some(module) = &test.module {
            for part in module.split("::") {
                if !module_path.is_empty() {
                    module_path.push_str("::");
                }
                module_path.push_str(part);
                let index = child_position(parent, part).unwrap_or_else(|| {
                    parent.children.push(TestNode::new(
                        NodeId::Module {
                            package: test.package.clone(),
                            binary: test.binary.clone(),
                            kind: test.binary_kind.clone(),
                            path: module_path.clone(),
                        },
                        part,
                        NodeKind::Module {
                            path: module_path.clone(),
                        },
                    ));
                    parent.children.len() - 1
                });
                parent = &mut parent.children[index];
            }
        }

        let mut node = TestNode::new(
            NodeId::Test {
                key: test.key.clone(),
            },
            test.name.clone(),
            NodeKind::Test(Box::new(test.clone())),
        );
        node.status = test.status;
        node.expanded = false;
        parent.children.push(node);
    }

    fn recompute_statuses(&mut self) {
        recompute_node_status(&mut self.root);
    }

    fn clamp_selection(&mut self) {
        if !self.is_selected_visible() {
            self.selected = self
                .visible_rows()
                .first()
                .map(|row| row.node.id.clone())
                .unwrap_or(NodeId::Workspace);
        }
    }

    fn with_selected_mut(&mut self, f: impl FnMut(&mut TestNode)) {
        let target = self.selected.clone();
        self.with_id_mut(&target, f);
    }

    fn with_id_mut(&mut self, target: &NodeId, mut f: impl FnMut(&mut TestNode)) {
        let _ = with_id_mut(&mut self.root, target, &mut f);
    }

    fn node(&self, id: &NodeId) -> Option<&TestNode> {
        node(&self.root, id)
    }

    fn event_target_node_id(&self, event: &nextdeck_test_events::TestEvent) -> Option<NodeId> {
        event_thread_name(event)
            .and_then(|name| {
                unique_test_node_id(&self.root, |node, test| {
                    event_thread_matches_test(name, node, test)
                })
            })
            .or_else(|| {
                unique_test_node_id(&self.root, |node, _| node.status == TestStatus::Running)
            })
    }

    fn is_selected_visible(&self) -> bool {
        self.visible_rows()
            .iter()
            .any(|row| row.node.id == self.selected)
    }
}

fn add_status_count(counts: &mut StatusCounts, status: TestStatus) {
    match status {
        TestStatus::Pending => counts.pending += 1,
        TestStatus::Running => counts.running += 1,
        TestStatus::Passed => counts.passed += 1,
        TestStatus::Failed => counts.failed += 1,
        TestStatus::Ignored => counts.ignored += 1,
        TestStatus::Skipped => counts.skipped += 1,
    }
}

fn child_position(parent: &TestNode, label: &str) -> Option<usize> {
    parent
        .children
        .iter()
        .position(|child| child.label == label)
}

fn append_output_text(target: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !target.is_empty() && !target.ends_with('\n') {
        append_bounded_text(target, "\n");
    }
    append_bounded_text(target, text);
}

fn merge_finished_output(existing: &str, finished: &str) -> String {
    if finished.is_empty() {
        return existing.to_owned();
    }
    if existing.is_empty() {
        return bounded_text(finished.to_owned());
    }
    let mut merged = existing.to_owned();
    append_output_text(&mut merged, finished);
    merged
}

fn event_thread_name(event: &nextdeck_test_events::TestEvent) -> Option<&str> {
    event
        .thread
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| !matches!(*name, "main" | "tokio-runtime-worker"))
}

fn unique_test_node_id(
    root: &TestNode,
    mut predicate: impl FnMut(&TestNode, &DiscoveredTest) -> bool,
) -> Option<NodeId> {
    let mut matches = Vec::new();
    visit(root, &mut |node| {
        if let NodeKind::Test(test) = &node.kind
            && predicate(node, test)
        {
            matches.push(node.id.clone());
        }
    });
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

fn event_thread_matches_test(name: &str, node: &TestNode, test: &DiscoveredTest) -> bool {
    node_matches(
        node,
        &TestKey {
            binary_id: None,
            event_prefix: None,
            name: name.to_owned(),
        },
    ) || test.full_name == name
        || test
            .full_name
            .strip_suffix(test.name.as_str())
            .is_some_and(|prefix| prefix.ends_with("::") && test.name == name)
}

fn collect_visible<'a>(
    node: &'a TestNode,
    depth: usize,
    filter: TestViewFilter,
    force_include: bool,
    rows: &mut Vec<TreeRow<'a>>,
) {
    if !force_include && !node_has_visible_tests(node, filter) {
        return;
    }

    rows.push(TreeRow { depth, node });
    if node.expanded {
        for child in &node.children {
            collect_visible(child, depth + 1, filter, false, rows);
        }
    }
}

fn node_has_visible_tests(node: &TestNode, filter: TestViewFilter) -> bool {
    match &node.kind {
        NodeKind::Test(_) => filter.allows(node.status),
        NodeKind::Workspace
        | NodeKind::Package { .. }
        | NodeKind::Binary { .. }
        | NodeKind::Module { .. } => node
            .children
            .iter()
            .any(|child| node_has_visible_tests(child, filter)),
    }
}

fn node<'a>(current: &'a TestNode, target: &NodeId) -> Option<&'a TestNode> {
    if current.id == *target {
        return Some(current);
    }

    current
        .children
        .iter()
        .find_map(|child| node(child, target))
}

fn with_id_mut(node: &mut TestNode, target: &NodeId, f: &mut impl FnMut(&mut TestNode)) -> bool {
    if node.id == *target {
        f(node);
        return true;
    }

    for child in &mut node.children {
        if with_id_mut(child, target, f) {
            return true;
        }
    }
    false
}

fn visit(node: &TestNode, f: &mut impl FnMut(&TestNode)) {
    f(node);
    for child in &node.children {
        visit(child, f);
    }
}

fn visit_mut(node: &mut TestNode, f: &mut impl FnMut(&mut TestNode)) {
    f(node);
    for child in &mut node.children {
        visit_mut(child, f);
    }
}

fn recompute_node_status(node: &mut TestNode) -> TestStatus {
    if matches!(node.kind, NodeKind::Test(_)) {
        return node.status;
    }

    let mut aggregate = None;
    for child in &mut node.children {
        let status = recompute_node_status(child);
        aggregate = Some(match aggregate {
            Some(current) => merge_status(current, status),
            None => status,
        });
    }
    let aggregate = aggregate.unwrap_or(TestStatus::Pending);
    node.status = aggregate;
    aggregate
}

fn merge_status(current: TestStatus, next: TestStatus) -> TestStatus {
    use TestStatus::{Failed, Ignored, Passed, Pending, Running, Skipped};
    match (current, next) {
        (Running, _) | (_, Running) => Running,
        (Failed, _) | (_, Failed) => Failed,
        (Pending, _) | (_, Pending) => Pending,
        (Skipped, _) | (_, Skipped) => Skipped,
        (Ignored, Passed) | (Passed, Ignored) | (Ignored, Ignored) => Ignored,
        (Passed, Passed) => Passed,
    }
}

fn node_matches(node: &TestNode, key: &TestKey) -> bool {
    let NodeKind::Test(test) = &node.kind else {
        return false;
    };

    if let Some(binary_id) = &key.binary_id {
        test.key.binary_id.as_deref() == Some(binary_id.as_str()) && test.key.name == key.name
    } else if let Some(event_prefix) = &key.event_prefix {
        test.key.event_prefix.as_deref() == Some(event_prefix.as_str()) && test.key.name == key.name
    } else {
        test.key.name == key.name
    }
}

fn descendant_outputs(node: &TestNode) -> String {
    let mut outputs = Vec::new();
    visit(node, &mut |child| {
        if let NodeKind::Test(test) = &child.kind
            && has_observable_output(child)
        {
            outputs.push(format!(
                "{}::{} [{}]\n{}",
                test.package,
                test.full_name,
                status_label(child.status),
                child.output.display_text()
            ));
        }
    });

    if outputs.is_empty() {
        if descendant_test_count(node) == 0 {
            "No tests under this selection".to_owned()
        } else {
            "No captured output for tests under this selection yet".to_owned()
        }
    } else {
        outputs.join("\n\n")
    }
}

fn has_observable_output(node: &TestNode) -> bool {
    node.status != TestStatus::Pending
        || node.output.duration.is_some()
        || !node.output.stdout.is_empty()
        || !node.output.stderr.is_empty()
}

fn descendant_test_count(node: &TestNode) -> usize {
    let mut count = 0;
    visit(node, &mut |child| {
        if matches!(child.kind, NodeKind::Test(_)) {
            count += 1;
        }
    });
    count
}

fn status_label(status: TestStatus) -> &'static str {
    match status {
        TestStatus::Pending => "pending",
        TestStatus::Running => "running",
        TestStatus::Passed => "passed",
        TestStatus::Failed => "failed",
        TestStatus::Ignored => "ignored",
        TestStatus::Skipped => "skipped",
    }
}

fn workspace_label(tests: &[DiscoveredTest]) -> String {
    let Some(path) = common_cwd(tests) else {
        return "workspace".to_owned();
    };
    if path.as_os_str().is_empty() {
        "workspace".to_owned()
    } else {
        path.display().to_string()
    }
}

fn common_cwd(tests: &[DiscoveredTest]) -> Option<PathBuf> {
    let mut iter = tests.iter();
    let first = iter.next()?.cwd.clone();
    let mut common = first.components().collect::<Vec<_>>();

    for test in iter {
        let other = test.cwd.components().collect::<Vec<_>>();
        let shared_len = common
            .iter()
            .zip(other.iter())
            .take_while(|(left, right)| left == right)
            .count();
        common.truncate(shared_len);
    }

    Some(components_to_path(&common))
}

fn components_to_path(components: &[Component<'_>]) -> PathBuf {
    let mut path = PathBuf::new();
    for component in components {
        path.push(component.as_os_str());
    }
    path
}

#[cfg(test)]
mod tests;
