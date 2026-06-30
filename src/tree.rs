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

impl TestStatus {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Pending => "o",
            Self::Running => ">",
            Self::Passed => "+",
            Self::Failed => "x",
            Self::Ignored => "-",
            Self::Skipped => "~",
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
}

pub struct Tree {
    pub root: TestNode,
    selected: usize,
    runner_output: Vec<String>,
}

impl Tree {
    pub fn from_tests(tests: Vec<DiscoveredTest>) -> Self {
        let mut tree = Self {
            root: TestNode::new("workspace", NodeKind::Workspace),
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
        collect_visible(&self.root, 0, &mut rows);
        rows
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

    pub fn selected_output(&self) -> String {
        if let Some(node) = self.selected_node() {
            if matches!(node.kind, NodeKind::Test(_)) {
                return node.output.display_text();
            }
            let failures = failed_descendants(node);
            if !failures.is_empty() {
                return failures.join("\n\n");
            }
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
            if node.status == TestStatus::Failed {
                if let NodeKind::Test(test) = &node.kind {
                    names.push(test.full_name.clone());
                }
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

fn collect_visible<'a>(node: &'a TestNode, depth: usize, rows: &mut Vec<(usize, &'a TestNode)>) {
    rows.push((depth, node));
    if node.expanded {
        for child in &node.children {
            collect_visible(child, depth + 1, rows);
        }
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

    let mut aggregate = TestStatus::Pending;
    for child in &mut node.children {
        let status = recompute_node_status(child);
        aggregate = merge_status(aggregate, status);
    }
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

fn failed_descendants(node: &TestNode) -> Vec<String> {
    let mut failures = Vec::new();
    visit(node, &mut |child| {
        if child.status == TestStatus::Failed {
            if let NodeKind::Test(test) = &child.kind {
                let mut text = format!("{}::{}\n", test.package, test.full_name);
                text.push_str(&child.output.display_text());
                failures.push(text);
            }
        }
    });
    failures
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
}
