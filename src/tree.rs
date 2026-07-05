use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{output::TestOutput, state::StatusCounts};

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
    Package { name: String },
    Binary {
        package: String,
        name: String,
        kind: String,
    },
    Module { path: String },
    Test(DiscoveredTest),
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
            expanded: false,
            children: Vec::new(),
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        if matches!(self.kind, NodeKind::Test(_)) {
            return self.output.duration;
        }

        let mut total = Duration::ZERO;
        let mut has_duration = false;
        for child in &self.children {
            if let Some(duration) = child.duration() {
                total += duration;
                has_duration = true;
            }
        }
        has_duration.then_some(total)
    }

    pub fn display_duration(&self) -> Option<Duration> {
        if matches!(self.kind, NodeKind::Test(_)) {
            return self.output.duration.or_else(|| {
                (self.status == TestStatus::Running)
                    .then(|| self.started_at.map(|started_at| started_at.elapsed()))
                    .flatten()
            });
        }

        self.duration()
    }
}

#[derive(Clone, Copy)]
pub struct TreeRow<'a> {
    pub depth: usize,
    pub node: &'a TestNode,
}

pub struct Tree {
    pub root: TestNode,
    pub view_filter: TestViewFilter,
    selected: NodeId,
    runner_output: Vec<String>,
}

impl Tree {
    pub fn from_tests(tests: Vec<DiscoveredTest>) -> Self {
        let mut root = TestNode::new(NodeId::Workspace, "workspace", NodeKind::Workspace);
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
        self.visible_rows()
            .iter()
            .position(|row| row.node.id == self.selected)
            .unwrap_or(0)
    }

    pub fn selected_node(&self) -> Option<&TestNode> {
        self.node(&self.selected)
    }

    pub fn selected_id(&self) -> &NodeId {
        &self.selected
    }

    pub fn select_next(&mut self) {
        let rows = self.visible_rows();
        if !rows.is_empty() {
            let index = self.selected_index().saturating_add(1).min(rows.len() - 1);
            self.selected = rows[index].node.id.clone();
        }
    }

    pub fn select_previous(&mut self) {
        let rows = self.visible_rows();
        if !rows.is_empty() {
            let index = self.selected_index().saturating_sub(1);
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
        let rows = self.visible_rows();
        if !rows.is_empty() {
            let index = self
                .selected_index()
                .saturating_add(page_size.max(1))
                .min(rows.len().saturating_sub(1));
            self.selected = rows[index].node.id.clone();
        }
    }

    pub fn select_previous_page(&mut self, page_size: usize) {
        let rows = self.visible_rows();
        if !rows.is_empty() {
            let index = self.selected_index().saturating_sub(page_size.max(1));
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

    #[cfg(test)]
    pub fn update_status(&mut self, key: &TestKey, status: TestStatus) {
        visit_mut(&mut self.root, &mut |node| {
            if node_matches(node, key) {
                node.status = status;
                node.started_at = (status == TestStatus::Running).then(Instant::now);
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
                node.status = TestStatus::Running;
                node.started_at = Some(Instant::now());
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
                node.output = TestOutput {
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                    duration,
                };
            }
        });
        self.recompute_statuses();
    }

    pub fn stop_running_tests(&mut self) {
        visit_mut(&mut self.root, &mut |node| {
            if node.status == TestStatus::Running {
                node.status = TestStatus::Pending;
                node.started_at = None;
            }
        });
        self.recompute_statuses();
    }

    pub fn append_runner_output(&mut self, line: String) {
        self.runner_output.push(line);
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
                return node.output.display_text();
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

    pub fn failed_test_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        visit(&self.root, &mut |node| {
            if node.status == TestStatus::Failed
                && let NodeKind::Test(test) = &node.kind
            {
                names.push(test.full_name.clone());
            }
        });
        names
    }

    pub fn select_next_failed(&mut self) -> bool {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return false;
        }
        let start = self.selected_index().saturating_add(1);
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
        let rows = self.visible_rows();
        if rows.is_empty() {
            return false;
        }
        let start = self.selected_index();
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
            NodeKind::Test(test.clone()),
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
        | NodeKind::Module { .. } => descendant_test_count(node) > 0,
    }
}

fn node<'a>(current: &'a TestNode, target: &NodeId) -> Option<&'a TestNode> {
    if current.id == *target {
        return Some(current);
    }

    current.children.iter().find_map(|child| node(child, target))
}

fn with_id_mut(
    node: &mut TestNode,
    target: &NodeId,
    f: &mut impl FnMut(&mut TestNode),
) -> bool {
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
        (Failed, _) | (_, Failed) => Failed,
        (Running, _) | (_, Running) => Running,
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

#[cfg(test)]
mod tests {
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
    fn initial_tree_shows_only_one_nested_level() {
        let mut tree = Tree::from_tests(vec![discovered_test(
            "demo::demo",
            "demo",
            "outer::inner",
            "works",
        )]);

        assert_eq!(visible_labels(&tree), vec!["workspace", "demo"]);

        tree.select_next();
        tree.expand_selected();
        assert_eq!(visible_labels(&tree), vec!["workspace", "demo", "outer"]);

        tree.select_next();
        tree.expand_selected();
        assert_eq!(
            visible_labels(&tree),
            vec!["workspace", "demo", "outer", "inner"]
        );
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

        assert_eq!(visible_labels(&tree), vec!["workspace", "demo"]);

        tree.select_next();
        tree.expand_selected();
        assert_eq!(visible_labels(&tree), vec!["workspace", "demo", "tests"]);

        tree.select_next();
        tree.expand_selected();
        assert_eq!(visible_labels(&tree), vec!["workspace", "demo", "tests"]);
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
        tree.update_status(
            &TestKey {
                binary_id: Some("demo::demo".to_owned()),
                event_prefix: Some("demo::demo".to_owned()),
                name: name.to_owned(),
            },
            status,
        );
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
}
