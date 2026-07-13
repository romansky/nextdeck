use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestProcessSelector {
    pub binary_path: PathBuf,
    pub full_name: String,
}

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
pub fn sample_running_test_stacks(
    root_pid: Option<u32>,
    selector: &TestProcessSelector,
) -> Result<String, String> {
    macos::sample_running_test_stacks(root_pid, selector)
}

#[cfg(not(target_os = "macos"))]
pub fn sample_running_test_stacks(
    _root_pid: Option<u32>,
    _selector: &TestProcessSelector,
) -> Result<String, String> {
    Err(
        "Running test stack sampling is not supported on this OS yet. Supported OS: macOS."
            .to_owned(),
    )
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        collections::BTreeSet,
        env, fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    use rustc_demangle::try_demangle;

    use super::TestProcessSelector;

    const SAMPLE_DURATION_SECONDS: &str = "1";
    const SAMPLE_INTERVAL_MILLIS: &str = "10";
    const MAX_SAMPLED_PROCESSES: usize = 3;
    const MAX_TEST_THREADS: usize = 4;
    const MAX_CHILD_THREADS: usize = 2;
    const MAX_PATHS_PER_THREAD: usize = 2;
    const MAX_FRAMES: usize = 8;
    const MAX_FRAME_CHARS: usize = 180;
    const MAX_REPORT_BYTES: usize = 6 * 1024;

    #[derive(Clone, Debug, PartialEq)]
    struct ProcessInfo {
        pid: u32,
        ppid: u32,
        pgid: u32,
        elapsed: String,
        state: String,
        cpu_percent: f64,
        rss_kib: u64,
        command: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct SamplePath {
        samples: usize,
        frames: Vec<String>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ThreadSample {
        label: String,
        samples: usize,
        paths: Vec<SamplePath>,
        similar: usize,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct CompactSample {
        text: String,
        thread_count: usize,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct ResourceSnapshot {
        cpu_percent: f64,
        rss_kib: u64,
        process_count: usize,
    }

    pub fn sample_running_test_stacks(
        root_pid: Option<u32>,
        selector: &TestProcessSelector,
    ) -> Result<String, String> {
        let Some(root_pid) = root_pid else {
            return Err("No active cargo-nextest process.".to_owned());
        };

        let processes = load_processes()
            .map_err(|error| format!("Could not inspect test processes: {error}"))?;
        let selected = resolve_selected_process(root_pid, selector, &processes)?;
        let targets = sample_targets(selected, &processes);
        let omitted = process_group_descendants(selected, &processes)
            .len()
            .saturating_sub(targets.len());
        let resources_before = resource_snapshot(selected.pgid, &processes);
        let started = Instant::now();

        let mut body = String::new();
        let mut sampled_threads = 0;
        let mut failed_samples = 0;

        for (index, process) in targets.iter().enumerate() {
            let max_threads = if process.pid == selected.pid {
                body.push_str("\nstacks:\n");
                MAX_TEST_THREADS
            } else {
                body.push_str(&format!(
                    "\nchild: pid={} name={}\n",
                    process.pid,
                    process_name(&process.command)
                ));
                MAX_CHILD_THREADS
            };
            match sample_process(process.pid) {
                Ok(raw) => {
                    let compact = compact_sample_output(&raw, &selector.full_name, max_threads);
                    sampled_threads += compact.thread_count;
                    body.push_str(&compact.text);
                }
                Err(error) => {
                    if process.pid == selected.pid {
                        return Err(format!("Could not sample selected test: {error}"));
                    }
                    failed_samples += 1;
                    body.push_str(&format!("  sample failed: {error}\n"));
                }
            }
            if index + 1 == MAX_SAMPLED_PROCESSES && omitted > 0 {
                body.push_str(&format!("\n{omitted} additional process(es) omitted\n"));
            }
        }

        let resources_after = load_processes()
            .ok()
            .map(|processes| resource_snapshot(selected.pgid, &processes));
        let resources = format_resource_line(
            resources_before,
            resources_after,
            started.elapsed(),
            sampled_threads,
            omitted == 0 && failed_samples == 0,
        );
        let mut text = format!(
            "test: {}\npid: {}  pgid: {}  elapsed: {}  state: {}\nsample: {}s/{}ms\n{resources}\n",
            selector.full_name,
            selected.pid,
            selected.pgid,
            selected.elapsed,
            display_state(&selected.state),
            SAMPLE_DURATION_SECONDS,
            SAMPLE_INTERVAL_MILLIS,
        );
        text.push_str(&body);

        Ok(truncate_report(text))
    }

    fn load_processes() -> Result<Vec<ProcessInfo>, String> {
        let output = Command::new("ps")
            .args(["-axo", "pid=,ppid=,pgid=,etime=,state=,%cpu=,rss=,command="])
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
        let pgid = parts.next()?.parse().ok()?;
        let elapsed = parts.next()?.to_owned();
        let state = parts.next()?.to_owned();
        let cpu_percent = parts.next()?.parse().ok()?;
        let rss_kib = parts.next()?.parse().ok()?;
        let command = parts.collect::<Vec<_>>().join(" ");
        if command.is_empty() {
            return None;
        }
        Some(ProcessInfo {
            pid,
            ppid,
            pgid,
            elapsed,
            state,
            cpu_percent,
            rss_kib,
            command,
        })
    }

    fn resource_snapshot(pgid: u32, processes: &[ProcessInfo]) -> ResourceSnapshot {
        processes
            .iter()
            .filter(|process| process.pgid == pgid)
            .fold(
                ResourceSnapshot {
                    cpu_percent: 0.0,
                    rss_kib: 0,
                    process_count: 0,
                },
                |mut total, process| {
                    total.cpu_percent += process.cpu_percent;
                    total.rss_kib = total.rss_kib.saturating_add(process.rss_kib);
                    total.process_count += 1;
                    total
                },
            )
    }

    fn process_tree(root_pid: u32, processes: &[ProcessInfo]) -> Vec<&ProcessInfo> {
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
            .collect()
    }

    fn resolve_selected_process<'a>(
        root_pid: u32,
        selector: &TestProcessSelector,
        processes: &'a [ProcessInfo],
    ) -> Result<&'a ProcessInfo, String> {
        let expected_path = selector
            .binary_path
            .canonicalize()
            .unwrap_or_else(|_| selector.binary_path.clone());
        let mut matches = process_tree(root_pid, processes)
            .into_iter()
            .filter(|process| process.pid != root_pid)
            .filter(|process| process_matches(process, &expected_path, &selector.full_name));
        let Some(selected) = matches.next() else {
            return Err(format!(
                "No live process found for selected test {}",
                selector.full_name
            ));
        };
        if matches.next().is_some() {
            return Err(format!(
                "Multiple live processes found for selected test {}",
                selector.full_name
            ));
        }
        Ok(selected)
    }

    fn process_matches(process: &ProcessInfo, binary_path: &Path, full_name: &str) -> bool {
        let binary = binary_path.to_string_lossy();
        let Some(args) = process.command.strip_prefix(binary.as_ref()) else {
            return false;
        };
        args.starts_with(char::is_whitespace) && has_exact_test_arg(args, full_name)
    }

    fn has_exact_test_arg(args: &str, full_name: &str) -> bool {
        let Some(value) = args.split_once("--exact ").map(|(_, value)| value) else {
            return false;
        };
        value
            .strip_prefix(full_name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    }

    fn process_group_descendants<'a>(
        selected: &ProcessInfo,
        processes: &'a [ProcessInfo],
    ) -> Vec<&'a ProcessInfo> {
        process_tree(selected.pid, processes)
            .into_iter()
            .filter(|process| process.pid == selected.pid || process.pgid == selected.pgid)
            .collect()
    }

    fn sample_targets<'a>(
        selected: &'a ProcessInfo,
        processes: &'a [ProcessInfo],
    ) -> Vec<&'a ProcessInfo> {
        let mut scoped = process_group_descendants(selected, processes);
        scoped.sort_by_key(|process| (process.pid != selected.pid, process.pid));
        scoped.truncate(MAX_SAMPLED_PROCESSES);
        scoped
    }

    fn process_name(command: &str) -> String {
        let executable = command.split(" --exact ").next().unwrap_or(command);
        Path::new(executable)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_owned()
    }

    fn display_state(state: &str) -> &'static str {
        match state.chars().next() {
            Some('R') => "running",
            Some('S' | 'I') => "sleeping",
            Some('D' | 'U') => "blocked",
            Some('T') => "stopped",
            Some('Z') => "zombie",
            _ => "unknown",
        }
    }

    fn format_resource_line(
        before: ResourceSnapshot,
        after: Option<ResourceSnapshot>,
        elapsed: Duration,
        sampled_threads: usize,
        threads_complete: bool,
    ) -> String {
        let exited = after.is_some_and(|snapshot| snapshot.process_count == 0);
        let current = after
            .filter(|snapshot| snapshot.process_count > 0)
            .unwrap_or(before);
        let mut rss = format_bytes(current.rss_kib);
        if let Some(after) = after
            && before.process_count > 0
            && after.process_count > 0
        {
            let delta = i128::from(after.rss_kib) - i128::from(before.rss_kib);
            if delta.unsigned_abs() >= 1024 {
                let delta_mib = delta as f64 / 1024.0;
                rss.push_str(&format!(
                    " ({delta_mib:+.0} MiB/{:.1}s)",
                    elapsed.as_secs_f64()
                ));
            }
        }
        let threads = if threads_complete {
            format!("threads={sampled_threads}")
        } else {
            format!("threads>={sampled_threads}")
        };
        let exited = if exited { "  exited" } else { "" };
        format!(
            "resources: cpu~{:.1}%  rss~{rss}  processes={}  {threads}{exited}",
            current.cpu_percent, current.process_count
        )
    }

    fn format_bytes(rss_kib: u64) -> String {
        if rss_kib < 1024 {
            format!("{rss_kib} KiB")
        } else if rss_kib < 1024 * 1024 {
            format!("{} MiB", rss_kib.div_ceil(1024))
        } else {
            format!("{:.1} GiB", rss_kib as f64 / (1024.0 * 1024.0))
        }
    }

    fn sample_process(pid: u32) -> Result<String, String> {
        let path = sample_path(pid);
        let output = Command::new("sample")
            .args([
                pid.to_string(),
                SAMPLE_DURATION_SECONDS.to_owned(),
                SAMPLE_INTERVAL_MILLIS.to_owned(),
                "-file".to_owned(),
            ])
            .arg(&path)
            .stdin(Stdio::null())
            .output();

        let result = match output {
            Ok(output) if output.status.success() => fs::read_to_string(&path)
                .map_err(|error| format!("could not read sample output: {error}")),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let detail = stderr.lines().last().unwrap_or_default().trim();
                if detail.is_empty() {
                    Err(format!("sample exited with {}", output.status))
                } else {
                    Err(detail.to_owned())
                }
            }
            Err(error) => Err(format!("could not run sample: {error}")),
        };
        let _ = fs::remove_file(path);
        result
    }

    fn compact_sample_output(raw: &str, full_name: &str, max_threads: usize) -> CompactSample {
        let mut threads = parse_call_graph(raw);
        let thread_count = threads.len();
        threads.sort_by_key(|thread| {
            (
                thread_priority(thread, full_name),
                std::cmp::Reverse(thread.samples),
            )
        });

        let mut selected = Vec::<ThreadSample>::new();
        for mut thread in threads {
            prepare_paths(&mut thread, full_name);
            if thread.paths.is_empty() {
                continue;
            }
            if let Some(existing) = selected
                .iter_mut()
                .find(|existing| same_stack_shape(existing, &thread))
            {
                existing.similar += 1;
                continue;
            }
            selected.push(thread);
            if selected.len() == max_threads {
                break;
            }
        }

        if selected.is_empty() {
            return CompactSample {
                text: "  no thread stacks found\n".to_owned(),
                thread_count,
            };
        }

        let mut text = String::new();
        for thread in selected {
            let label = display_thread_label(&thread.label, full_name);
            text.push_str(&format!("\n{label} [{} samples", thread.samples));
            if thread.similar > 0 {
                text.push_str(&format!(", {} similar thread(s)", thread.similar));
            }
            text.push_str("]\n");
            if thread.paths.len() == 1 {
                push_frames(&mut text, &thread.paths[0].frames, "  ");
            } else {
                for path in thread.paths {
                    let percent = path.samples.saturating_mul(100) / thread.samples.max(1);
                    text.push_str(&format!("  {percent}%\n"));
                    push_frames(&mut text, &path.frames, "    ");
                }
            }
        }
        CompactSample { text, thread_count }
    }

    fn parse_call_graph(raw: &str) -> Vec<ThreadSample> {
        let mut in_call_graph = false;
        let mut threads = Vec::new();
        let mut current = None::<ThreadBuilder>;

        for line in raw.lines() {
            if line.trim() == "Call graph:" {
                in_call_graph = true;
                continue;
            }
            if !in_call_graph {
                continue;
            }
            if line.starts_with("Total number in stack")
                || line.starts_with("Sort by top of stack")
                || line.starts_with("Binary Images:")
            {
                break;
            }
            if let Some((indent, samples, label)) = parse_thread_header(line) {
                if let Some(thread) = current.take() {
                    threads.push(thread.finish());
                }
                current = Some(ThreadBuilder {
                    label,
                    samples,
                    frame_indent: indent + 2,
                    stack: Vec::new(),
                    paths: Vec::new(),
                    last_depth: None,
                    last_samples: 0,
                });
            } else if let Some((indent, samples, frame)) = parse_frame(line)
                && let Some(thread) = &mut current
            {
                thread.push_frame(indent, samples, frame);
            }
        }
        if let Some(thread) = current {
            threads.push(thread.finish());
        }
        threads
    }

    struct ThreadBuilder {
        label: String,
        samples: usize,
        frame_indent: usize,
        stack: Vec<String>,
        paths: Vec<SamplePath>,
        last_depth: Option<usize>,
        last_samples: usize,
    }

    impl ThreadBuilder {
        fn push_frame(&mut self, indent: usize, samples: usize, frame: String) {
            let depth = indent.saturating_sub(self.frame_indent) / 2;
            if self
                .last_depth
                .is_some_and(|last_depth| depth <= last_depth)
            {
                self.finish_path();
            }
            self.stack.truncate(depth);
            if self.stack.last() != Some(&frame) {
                self.stack.push(frame);
            }
            self.last_depth = Some(depth);
            self.last_samples = samples;
        }

        fn finish_path(&mut self) {
            if !self.stack.is_empty() {
                self.paths.push(SamplePath {
                    samples: self.last_samples,
                    frames: self.stack.clone(),
                });
            }
        }

        fn finish(mut self) -> ThreadSample {
            self.finish_path();
            ThreadSample {
                label: self.label,
                samples: self.samples,
                paths: self.paths,
                similar: 0,
            }
        }
    }

    fn parse_thread_header(line: &str) -> Option<(usize, usize, String)> {
        let indent = count_position(line)?;
        let line = &line[indent..];
        let (samples, rest) = line.split_once(char::is_whitespace)?;
        let samples = samples.parse().ok()?;
        let rest = rest.trim_start();
        rest.starts_with("Thread_")
            .then(|| (indent, samples, rest.to_owned()))
    }

    fn parse_frame(line: &str) -> Option<(usize, usize, String)> {
        let indent = count_position(line)?;
        let line = &line[indent..];
        let (samples, frame) = line.split_once(char::is_whitespace)?;
        let samples = samples.parse::<usize>().ok()?;
        let frame = frame
            .trim_start()
            .split_once("  (in ")
            .map(|(frame, _)| frame)
            .unwrap_or(frame)
            .trim();
        if frame.is_empty() || frame.starts_with("0x") {
            return None;
        }
        Some((
            indent,
            samples,
            limit_chars(demangle_frame(frame), MAX_FRAME_CHARS),
        ))
    }

    fn count_position(line: &str) -> Option<usize> {
        line.char_indices()
            .find_map(|(index, character)| character.is_ascii_digit().then_some(index))
    }

    fn demangle_frame(frame: &str) -> String {
        let (symbol, suffix) = frame
            .split_once(char::is_whitespace)
            .map_or((frame, ""), |(symbol, suffix)| (symbol, suffix));
        let demangled = try_demangle(symbol)
            .ok()
            .or_else(|| {
                symbol
                    .strip_prefix('_')
                    .and_then(|symbol| try_demangle(symbol).ok())
            })
            .map(|symbol| format!("{symbol:#}"))
            .unwrap_or_else(|| symbol.to_owned());
        if suffix.is_empty() {
            demangled
        } else {
            format!("{demangled} {suffix}")
        }
    }

    fn thread_priority(thread: &ThreadSample, full_name: &str) -> u8 {
        if thread.label.contains(full_name)
            || thread
                .paths
                .iter()
                .flat_map(|path| &path.frames)
                .any(|frame| frame.contains(full_name))
        {
            0
        } else if thread
            .paths
            .iter()
            .flat_map(|path| &path.frames)
            .any(|frame| is_application_frame(frame))
        {
            1
        } else if is_test_harness_thread(&thread.label) {
            2
        } else {
            3
        }
    }

    fn is_application_frame(frame: &str) -> bool {
        let frame = frame.trim_start();
        frame.contains("::")
            && ![
                "<alloc::", "alloc::", "<core::", "core::", "<mio::", "mio::", "<std::", "std::",
                "<test::", "test::", "<tokio::", "tokio::",
            ]
            .iter()
            .any(|prefix| frame.starts_with(prefix))
    }

    fn is_test_harness_thread(label: &str) -> bool {
        label.contains("main-thread") || label.contains("DispatchQueue_1")
    }

    fn prepare_paths(thread: &mut ThreadSample, full_name: &str) {
        let mut paths = Vec::<SamplePath>::new();
        for mut path in std::mem::take(&mut thread.paths) {
            path.frames = select_frames(path.frames, full_name);
            if path.frames.is_empty() {
                continue;
            }
            if let Some(existing) = paths
                .iter_mut()
                .find(|existing| existing.frames == path.frames)
            {
                existing.samples = existing.samples.saturating_add(path.samples);
            } else {
                paths.push(path);
            }
        }
        paths.sort_by_key(|path| std::cmp::Reverse(path.samples));
        paths.truncate(MAX_PATHS_PER_THREAD);
        thread.paths = paths;
    }

    fn same_stack_shape(left: &ThreadSample, right: &ThreadSample) -> bool {
        left.paths.len() == right.paths.len()
            && left
                .paths
                .iter()
                .zip(&right.paths)
                .all(|(left, right)| left.frames == right.frames)
    }

    fn push_frames(text: &mut String, frames: &[String], indent: &str) {
        for frame in frames {
            text.push_str(indent);
            text.push_str(frame);
            text.push('\n');
        }
    }

    fn select_frames(mut frames: Vec<String>, full_name: &str) -> Vec<String> {
        if frames.len() <= MAX_FRAMES {
            return frames;
        }
        let selected_frame = frames
            .iter()
            .take(frames.len() - MAX_FRAMES)
            .rposition(|frame| frame.contains(full_name))
            .map(|index| frames[index].clone());
        let mut tail = frames.split_off(frames.len() - MAX_FRAMES);
        if let Some(selected_frame) = selected_frame
            && !tail.iter().any(|frame| frame.contains(full_name))
        {
            tail.remove(0);
            tail.insert(0, selected_frame);
        }
        tail
    }

    fn display_thread_label(label: &str, full_name: &str) -> String {
        if label.contains(full_name) {
            "test thread".to_owned()
        } else if is_test_harness_thread(label) {
            "test harness".to_owned()
        } else if let Some((_, name)) = label.split_once(':') {
            name.trim().to_owned()
        } else {
            label
                .split_whitespace()
                .next()
                .unwrap_or("thread")
                .to_owned()
        }
    }

    fn limit_chars(text: String, limit: usize) -> String {
        if text.chars().count() <= limit {
            return text;
        }
        let mut shortened = text
            .chars()
            .take(limit.saturating_sub(3))
            .collect::<String>();
        shortened.push_str("...");
        shortened
    }

    fn truncate_report(mut text: String) -> String {
        if text.len() <= MAX_REPORT_BYTES {
            return text;
        }
        const MARKER: &str = "\n... stack sample truncated ...\n";
        let limit = MAX_REPORT_BYTES.saturating_sub(MARKER.len());
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(MARKER);
        text
    }

    fn sample_path(pid: u32) -> PathBuf {
        env::temp_dir().join(format!("nextdeck-sample-{}-{pid}.txt", std::process::id()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn process(pid: u32, ppid: u32, pgid: u32, command: &str) -> ProcessInfo {
            ProcessInfo {
                pid,
                ppid,
                pgid,
                elapsed: "00:12".to_owned(),
                state: "S".to_owned(),
                cpu_percent: 1.5,
                rss_kib: 2048,
                command: command.to_owned(),
            }
        }

        #[test]
        fn parses_ps_line_with_process_group_and_command_spaces() {
            let process = parse_ps_line(
                " 123 45 123 00:01.23 S 12.5 4096 /tmp/test binary --exact tests::case --nocapture",
            )
            .unwrap();

            assert_eq!(process.pid, 123);
            assert_eq!(process.ppid, 45);
            assert_eq!(process.pgid, 123);
            assert_eq!(process.state, "S");
            assert_eq!(process.cpu_percent, 12.5);
            assert_eq!(process.rss_kib, 4096);
            assert_eq!(
                process.command,
                "/tmp/test binary --exact tests::case --nocapture"
            );
        }

        #[test]
        fn resolves_only_the_selected_test_process() {
            let processes = vec![
                process(10, 1, 10, "/usr/bin/cargo-nextest nextest run"),
                process(
                    20,
                    10,
                    20,
                    "/tmp/demo-test --exact tests::selected --nocapture",
                ),
                process(
                    30,
                    10,
                    30,
                    "/tmp/demo-test --exact tests::other --nocapture",
                ),
            ];
            let selector = TestProcessSelector {
                binary_path: PathBuf::from("/tmp/demo-test"),
                full_name: "tests::selected".to_owned(),
            };

            let selected = resolve_selected_process(10, &selector, &processes).unwrap();

            assert_eq!(selected.pid, 20);
            assert_eq!(selected.pgid, 20);
        }

        #[test]
        fn samples_test_process_before_same_group_children() {
            let processes = vec![
                process(10, 1, 10, "cargo-nextest"),
                process(20, 10, 20, "/tmp/demo-test --exact tests::selected"),
                process(21, 20, 20, "selected-child"),
                process(22, 20, 22, "detached-child"),
                process(30, 10, 30, "/tmp/demo-test --exact tests::other"),
            ];

            let targets = sample_targets(&processes[1], &processes);

            assert_eq!(
                targets
                    .iter()
                    .map(|process| process.pid)
                    .collect::<Vec<_>>(),
                vec![20, 21]
            );
        }

        #[test]
        fn compacts_call_graph_and_demangles_rust_frames() {
            let raw = r#"Analysis metadata that should be omitted
Call graph:
    100 Thread_1 DispatchQueue_1: com.apple.main-thread
    + 100 _ZN4test17run_tests_console17h1234567890abcdefE  (in demo) + 1 [0x1]
    +   100 semaphore_wait_trap  (in libsystem_kernel.dylib) + 8 [0x2]
    100 Thread_2: tests::selected
      100 _ZN4demo5tests8selected17h1234567890abcdefE  (in demo) + 1 [0x3]
        100 nanosleep  (in libsystem_c.dylib) + 1 [0x4]
    100 Thread_3: tokio-rt-worker
      100 tokio::runtime::blocking::task::BlockingTask::poll  (in demo) + 1 [0x5]
        100 scenario::harness::lock::acquire_e2e_disk_lock  (in demo) + 1 [0x6]
          100 capakit_core::local_file_lock::acquire_advisory_flock  (in demo) + 1 [0x7]
Total number in stack (recursive counted multiple, when >=5):
Binary Images:
    thousands of irrelevant bytes
"#;

            let compact = compact_sample_output(raw, "tests::selected", MAX_TEST_THREADS);

            assert_eq!(compact.thread_count, 3);
            assert!(compact.text.contains("test thread [100 samples]"));
            assert!(compact.text.contains("demo::tests::selected"));
            assert!(compact.text.contains("test harness [100 samples]"));
            assert!(compact.text.contains("test::run_tests_console"));
            assert!(compact.text.contains("nanosleep"));
            let worker = compact.text.find("tokio-rt-worker").expect("worker stack");
            let harness = compact
                .text
                .find("test harness")
                .expect("test harness stack");
            assert!(worker < harness);
            assert!(!compact.text.contains("_ZN"));
            assert!(!compact.text.contains("Binary Images"));
            assert!(!compact.text.contains("0x"));

            let v0 = demangle_frame(
                "_RNvNtCseYM68cHKaSo_22nextdeck_sample_review5testss_18slow_selected_test",
            );
            assert!(v0.contains("nextdeck_sample_review::tests::slow_selected_test"));
        }

        #[test]
        fn preserves_the_two_hottest_call_paths() {
            let raw = r#"Call graph:
    100 Thread_2: tests::selected
    + 100 tests::selected
    +   60 hot_parent
    +     60 hot_leaf
    +   30 cold_parent
    +     30 cold_leaf
    +   10 omitted_parent
    +     10 omitted_leaf
Total number in stack (recursive counted multiple, when >=5):
"#;

            let compact = compact_sample_output(raw, "tests::selected", MAX_TEST_THREADS);

            assert!(compact.text.contains("  60%\n    tests::selected"));
            assert!(compact.text.contains("hot_parent\n    hot_leaf"));
            assert!(compact.text.contains("  30%\n    tests::selected"));
            assert!(compact.text.contains("cold_parent\n    cold_leaf"));
            assert!(!compact.text.contains("hot_leaf\n    cold_parent"));
            assert!(!compact.text.contains("omitted_parent"));
        }

        #[test]
        fn formats_compact_process_group_resources() {
            let before = ResourceSnapshot {
                cpu_percent: 42.25,
                rss_kib: 128 * 1024,
                process_count: 2,
            };
            let after = ResourceSnapshot {
                cpu_percent: 96.0,
                rss_kib: 136 * 1024,
                process_count: 2,
            };

            let line =
                format_resource_line(before, Some(after), Duration::from_secs_f64(2.1), 14, true);

            assert_eq!(
                line,
                "resources: cpu~96.0%  rss~136 MiB (+8 MiB/2.1s)  processes=2  threads=14"
            );
        }

        #[test]
        fn omits_small_rss_changes_and_marks_partial_thread_counts() {
            let before = ResourceSnapshot {
                cpu_percent: 1.0,
                rss_kib: 2048,
                process_count: 1,
            };
            let after = ResourceSnapshot {
                cpu_percent: 2.0,
                rss_kib: 2560,
                process_count: 1,
            };

            let line = format_resource_line(before, Some(after), Duration::from_secs(1), 4, false);

            assert_eq!(
                line,
                "resources: cpu~2.0%  rss~3 MiB  processes=1  threads>=4"
            );
        }

        #[test]
        fn caps_report_size() {
            let report = truncate_report("x".repeat(MAX_REPORT_BYTES * 2));

            assert!(report.len() <= MAX_REPORT_BYTES);
            assert!(report.ends_with("... stack sample truncated ...\n"));
        }
    }
}
