use std::time::Duration;

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
}

impl Default for TestViewFilter {
    fn default() -> Self {
        Self {
            show_success: true,
            show_failed: true,
            show_ignored: true,
        }
    }
}

impl TestViewFilter {
    fn allows(self, status: TestStatus) -> bool {
        match status {
            TestStatus::Passed => self.show_success,
            TestStatus::Failed => self.show_failed,
            TestStatus::Ignored => self.show_ignored,
            TestStatus::Pending | TestStatus::Running | TestStatus::Skipped => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Workspace,
    Package { name: String },
    Module { path: String },
    Test(DiscoveredTest),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestNode {
    pub label: String,
    pub kind: NodeKind,
    pub status: TestStatus,
    pub output: TestOutput,
    pub expanded: bool,
    pub children: Vec<TestNode>,
}

impl TestNode {
    fn new(label: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            label: label.into(),
            kind,
            status: TestStatus::Pending,
            output: TestOutput::default(),
            expanded: true,
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
}

pub struct Tree {
    pub root: TestNode,
    pub view_filter: TestViewFilter,
    selected: usize,
    runner_output: Vec<String>,
}

impl Tree {
    pub fn from_tests(tests: Vec<DiscoveredTest>) -> Self {
        let mut tree = Self {
            root: TestNode::new("workspace", NodeKind::Workspace),
            view_filter: TestViewFilter::default(),
            selected: 0,
            runner_output: Vec::new(),
        };
        for test in tests {
            tree.insert_test(test);
        }
        tree.recompute_statuses();
        tree
    }

    pub fn visible_rows(&self) -> Vec<(usize, &TestNode)> {
        let mut rows = Vec::new();
        collect_visible(&self.root, 0, self.view_filter, true, &mut rows);
        rows
    }

    pub fn set_view_filter(&mut self, filter: TestViewFilter) {
        self.view_filter = filter;
        self.clamp_selection();
    }

    pub fn selected_index(&self) -> usize {
        self.selected
            .min(self.visible_rows().len().saturating_sub(1))
    }

    pub fn selected_node(&self) -> Option<&TestNode> {
        let rows = self.visible_rows();
        rows.get(self.selected_index()).map(|(_, node)| *node)
    }

    pub fn select_next(&mut self) {
        let len = self.visible_rows().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        let len = self.visible_rows().len();
        self.selected = len.saturating_sub(1);
    }

    pub fn select_next_page(&mut self, page_size: usize) {
        let len = self.visible_rows().len();
        if len > 0 {
            self.selected = self
                .selected
                .saturating_add(page_size.max(1))
                .min(len.saturating_sub(1));
        }
    }

    pub fn select_previous_page(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size.max(1));
    }

    pub fn toggle_selected(&mut self) {
        self.with_selected_mut(|node| node.expanded = !node.expanded);
        self.clamp_selection();
    }

    pub fn expand_selected(&mut self) {
        self.with_selected_mut(|node| node.expanded = true);
        self.clamp_selection();
    }

    pub fn collapse_selected(&mut self) {
        self.with_selected_mut(|node| node.expanded = false);
        self.clamp_selection();
    }

    pub fn mark_scope_pending(&mut self, scope: &crate::nextest::RunScope) {
        visit_mut(&mut self.root, &mut |node| {
            if let NodeKind::Test(test) = &node.kind {
                if test.ignored {
                    node.status = TestStatus::Ignored;
                } else if scope.matches_test(test) {
                    node.status = TestStatus::Pending;
                    node.output = TestOutput::default();
                } else {
                    node.status = TestStatus::Skipped;
                }
            }
        });
        self.recompute_statuses();
    }

    pub fn update_status(&mut self, key: &TestKey, status: TestStatus) {
        visit_mut(&mut self.root, &mut |node| {
            if node_matches(node, key) {
                node.status = status;
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
                node.output = TestOutput {
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                    duration,
                };
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
            .find_map(|(index, (_, node))| (node.status == TestStatus::Failed).then_some(index))
        {
            self.selected = index;
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
        if let Some(index) = indices.find(|index| rows[*index].1.status == TestStatus::Failed) {
            self.selected = index;
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
            NodeKind::Module { path } => path.clone(),
            NodeKind::Test(test) => format!("{}::{}", test.package, test.full_name),
        }
    }

    pub fn status_counts(&self) -> StatusCounts {
        let mut counts = StatusCounts::default();
        visit(&self.root, &mut |node| {
            if matches!(node.kind, NodeKind::Test(_)) {
                match node.status {
                    TestStatus::Pending => counts.pending += 1,
                    TestStatus::Running => counts.running += 1,
                    TestStatus::Passed => counts.passed += 1,
                    TestStatus::Failed => counts.failed += 1,
                    TestStatus::Ignored => counts.ignored += 1,
                    TestStatus::Skipped => counts.skipped += 1,
                }
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
                &test.package,
                NodeKind::Package {
                    name: test.package.clone(),
                },
            ));
            self.root.children.len() - 1
        });

        let mut parent = &mut self.root.children[package_index];
        let mut module_path = String::new();
        if let Some(module) = &test.module {
            for part in module.split("::") {
                if !module_path.is_empty() {
                    module_path.push_str("::");
                }
                module_path.push_str(part);
                let index = child_position(parent, part).unwrap_or_else(|| {
                    parent.children.push(TestNode::new(
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

        let mut node = TestNode::new(test.name.clone(), NodeKind::Test(test.clone()));
        node.status = test.status;
        node.expanded = false;
        parent.children.push(node);
    }

    fn recompute_statuses(&mut self) {
        recompute_node_status(&mut self.root);
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected_index();
    }

    fn with_selected_mut(&mut self, mut f: impl FnMut(&mut TestNode)) {
        let target = self.selected_index();
        let mut current = 0;
        let _ = with_visible_mut(&mut self.root, target, &mut current, &mut f);
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
    rows: &mut Vec<(usize, &'a TestNode)>,
) {
    if !force_include && !node_has_visible_tests(node, filter) {
        return;
    }

    rows.push((depth, node));
    if node.expanded {
        for child in &node.children {
            collect_visible(child, depth + 1, filter, false, rows);
        }
    }
}

fn node_has_visible_tests(node: &TestNode, filter: TestViewFilter) -> bool {
    match &node.kind {
        NodeKind::Test(_) => filter.allows(node.status),
        NodeKind::Workspace | NodeKind::Package { .. } | NodeKind::Module { .. } => node
            .children
            .iter()
            .any(|child| node_has_visible_tests(child, filter)),
    }
}

fn with_visible_mut(
    node: &mut TestNode,
    target: usize,
    current: &mut usize,
    f: &mut impl FnMut(&mut TestNode),
) -> bool {
    if *current == target {
        f(node);
        return true;
    }
    *current += 1;

    if node.expanded {
        for child in &mut node.children {
            if with_visible_mut(child, target, current, f) {
                return true;
            }
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

        tree.set_view_filter(TestViewFilter {
            show_success: false,
            show_failed: true,
            show_ignored: true,
        });
        let labels = visible_labels(&tree);
        assert!(!labels.contains(&"passed".to_owned()));
        assert!(labels.contains(&"failed".to_owned()));
        assert!(labels.contains(&"pending".to_owned()));

        tree.set_view_filter(TestViewFilter {
            show_success: true,
            show_failed: false,
            show_ignored: true,
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

        assert_eq!(tree.root.status, TestStatus::Passed);
        let labels = tree
            .visible_rows()
            .into_iter()
            .map(|(_, node)| (node.label.clone(), node.status))
            .collect::<Vec<_>>();
        assert!(labels.contains(&("tests".to_owned(), TestStatus::Passed)));
    }

    #[test]
    fn view_filter_hides_ignored_test_rows() {
        let mut tree = Tree::from_tests(vec![
            discovered_test("demo::demo", "demo", "tests", "ignored"),
            discovered_test("demo::demo", "demo", "tests", "pending"),
        ]);
        set_test_status(&mut tree, "tests::ignored", TestStatus::Ignored);

        tree.set_view_filter(TestViewFilter {
            show_success: true,
            show_failed: true,
            show_ignored: false,
        });

        let labels = visible_labels(&tree);
        assert!(!labels.contains(&"ignored".to_owned()));
        assert!(labels.contains(&"pending".to_owned()));
    }

    #[test]
    fn refresh_from_tests_preserves_view_filter() {
        let mut tree =
            Tree::from_tests(vec![discovered_test("demo::demo", "demo", "tests", "old")]);
        tree.set_view_filter(TestViewFilter {
            show_success: false,
            show_failed: true,
            show_ignored: false,
        });

        tree.refresh_from_tests(vec![discovered_test("demo::demo", "demo", "tests", "new")]);

        assert!(!tree.view_filter.show_success);
        assert!(tree.view_filter.show_failed);
        assert!(!tree.view_filter.show_ignored);
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
            .map(|(_, node)| node.label.clone())
            .collect()
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
            module: Some(module.to_owned()),
            name: name.to_owned(),
            full_name: format!("{module}::{name}"),
            status: TestStatus::Pending,
            ignored: false,
        }
    }
}
