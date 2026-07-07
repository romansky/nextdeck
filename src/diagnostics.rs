use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct ProcessTracker {
    root_pid: Arc<Mutex<Option<u32>>>,
}

impl ProcessTracker {
    pub fn set(&self, pid: Option<u32>) {
        if let Ok(mut root_pid) = self.root_pid.lock() {
            *root_pid = pid;
        }
    }

    pub fn clear(&self) {
        self.set(None);
    }

    pub fn root_pid(&self) -> Option<u32> {
        self.root_pid.lock().ok().and_then(|root_pid| *root_pid)
    }
}

#[cfg(target_os = "macos")]
pub async fn capture_running_test_snapshot(root_pid: Option<u32>) -> String {
    macos::capture_running_test_snapshot(root_pid).await
}

#[cfg(not(target_os = "macos"))]
pub async fn capture_running_test_snapshot(_root_pid: Option<u32>) -> String {
    "Running test snapshots are not supported on this OS yet.\n\nSupported OS: macOS.\n".to_owned()
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        collections::BTreeSet,
        env, fs,
        path::PathBuf,
        process::{Command, Stdio},
    };

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ProcessInfo {
        pid: u32,
        ppid: u32,
        elapsed: String,
        command: String,
    }

    pub async fn capture_running_test_snapshot(root_pid: Option<u32>) -> String {
        let Some(root_pid) = root_pid else {
            return "No active cargo nextest process was found.\n\nStart a test run and capture while the selected test is still running.\n".to_owned();
        };

        let processes = match load_processes() {
            Ok(processes) => processes,
            Err(error) => return format!("Failed to inspect process tree: {error}\n"),
        };
        let process_tree = process_tree(root_pid, &processes);
        if process_tree.is_empty() {
            return format!(
                "The tracked cargo nextest process {root_pid} is no longer running.\n\nTry capturing again while the selected test is still running.\n"
            );
        }

        let mut text = String::new();
        text.push_str("Running test snapshot\n");
        text.push_str("=====================\n\n");
        text.push_str("Process tree\n");
        text.push_str("------------\n");
        text.push_str(&format_process_tree(root_pid, &process_tree));

        let targets = sample_targets(root_pid, &process_tree);
        text.push_str("\nThread stack samples\n");
        text.push_str("--------------------\n");
        text.push_str("Captured with: sample <pid> 2 -file <tempfile>\n\n");
        for process in targets {
            text.push_str(&format!(
                "pid={} elapsed={} command={}\n",
                process.pid, process.elapsed, process.command
            ));
            text.push_str(&sample_process(process.pid));
            text.push('\n');
        }

        text
    }

    fn load_processes() -> Result<Vec<ProcessInfo>, String> {
        let output = Command::new("ps")
            .args(["-axo", "pid=,ppid=,etime=,command="])
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("failed to run ps: {error}"))?;
        if !output.status.success() {
            return Err(format!("ps exited with {}", output.status));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().filter_map(parse_ps_line).collect())
    }

    fn parse_ps_line(line: &str) -> Option<ProcessInfo> {
        let mut parts = line.split_whitespace();
        let pid = parts.next()?.parse().ok()?;
        let ppid = parts.next()?.parse().ok()?;
        let elapsed = parts.next()?.to_owned();
        let command = parts.collect::<Vec<_>>().join(" ");
        if command.is_empty() {
            return None;
        }
        Some(ProcessInfo {
            pid,
            ppid,
            elapsed,
            command,
        })
    }

    fn process_tree(root_pid: u32, processes: &[ProcessInfo]) -> Vec<ProcessInfo> {
        let mut wanted = BTreeSet::from([root_pid]);
        loop {
            let before = wanted.len();
            for process in processes {
                if wanted.contains(&process.ppid) {
                    wanted.insert(process.pid);
                }
            }
            if wanted.len() == before {
                break;
            }
        }

        processes
            .iter()
            .filter(|process| wanted.contains(&process.pid))
            .cloned()
            .collect()
    }

    fn format_process_tree(root_pid: u32, processes: &[ProcessInfo]) -> String {
        let mut lines = Vec::new();
        for process in processes {
            let depth = process_depth(root_pid, process, processes);
            let indent = "  ".repeat(depth);
            lines.push(format!(
                "{indent}pid={} ppid={} elapsed={} command={}",
                process.pid, process.ppid, process.elapsed, process.command
            ));
        }
        lines.join("\n") + "\n"
    }

    fn process_depth(root_pid: u32, process: &ProcessInfo, processes: &[ProcessInfo]) -> usize {
        let mut depth = 0;
        let mut ppid = process.ppid;
        while ppid != 0 && ppid != root_pid {
            let Some(parent) = processes.iter().find(|candidate| candidate.pid == ppid) else {
                break;
            };
            depth += 1;
            ppid = parent.ppid;
        }
        if process.pid == root_pid {
            0
        } else {
            depth + 1
        }
    }

    fn sample_targets(root_pid: u32, processes: &[ProcessInfo]) -> Vec<&ProcessInfo> {
        let parents = processes
            .iter()
            .map(|process| process.ppid)
            .collect::<BTreeSet<_>>();
        let leaves = processes
            .iter()
            .filter(|process| process.pid != root_pid && !parents.contains(&process.pid))
            .collect::<Vec<_>>();
        if !leaves.is_empty() {
            return leaves;
        }

        let descendants = processes
            .iter()
            .filter(|process| process.pid != root_pid)
            .collect::<Vec<_>>();
        if !descendants.is_empty() {
            return descendants;
        }

        processes
            .iter()
            .filter(|process| process.pid == root_pid)
            .collect()
    }

    fn sample_process(pid: u32) -> String {
        let path = sample_path(pid);
        let output = Command::new("sample")
            .args([pid.to_string(), "2".to_owned(), "-file".to_owned()])
            .arg(&path)
            .stdin(Stdio::null())
            .output();

        let result = match output {
            Ok(output) if output.status.success() => {
                fs::read_to_string(&path).unwrap_or_else(|error| {
                    format!(
                        "sample completed but failed to read {}: {error}\n",
                        path.display()
                    )
                })
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!("sample exited with {}:\n{}\n", output.status, stderr.trim())
            }
            Err(error) => format!("failed to run sample: {error}\n"),
        };
        let _ = fs::remove_file(path);
        result
    }

    fn sample_path(pid: u32) -> PathBuf {
        env::temp_dir().join(format!("nextdeck-sample-{}-{pid}.txt", std::process::id()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_ps_line_with_command_spaces() {
            let process = parse_ps_line(" 123  45  00:01.23 /tmp/test binary --case one").unwrap();

            assert_eq!(process.pid, 123);
            assert_eq!(process.ppid, 45);
            assert_eq!(process.elapsed, "00:01.23");
            assert_eq!(process.command, "/tmp/test binary --case one");
        }

        #[test]
        fn chooses_leaf_descendants_as_sample_targets() {
            let processes = vec![
                ProcessInfo {
                    pid: 1,
                    ppid: 0,
                    elapsed: "00:01".to_owned(),
                    command: "cargo nextest run".to_owned(),
                },
                ProcessInfo {
                    pid: 2,
                    ppid: 1,
                    elapsed: "00:01".to_owned(),
                    command: "nextest-runner".to_owned(),
                },
                ProcessInfo {
                    pid: 3,
                    ppid: 2,
                    elapsed: "00:01".to_owned(),
                    command: "test-binary".to_owned(),
                },
            ];

            let tree = process_tree(1, &processes);
            let targets = sample_targets(1, &tree);

            assert_eq!(
                targets
                    .iter()
                    .map(|process| process.pid)
                    .collect::<Vec<_>>(),
                vec![3]
            );
        }
    }
}
