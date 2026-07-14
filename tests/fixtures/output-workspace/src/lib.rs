pub fn marker() -> &'static str {
    "nextdeck-output-fixture"
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    #[test]
    fn pass_prints_stdout_and_stderr() {
        println!("PASS_STDOUT: lib pass stdout");
        eprintln!("PASS_STDERR: lib pass stderr");
    }

    #[test]
    fn pass_prints_child_like_output() {
        println!("CHILD_STDOUT: command started");
        println!("    indented child output");
        eprintln!("CHILD_STDERR: command warning");
    }

    #[test]
    fn pass_prints_slow_output_for_info_poll() {
        println!("SLOW_PREVIEW: before poll");
        thread::sleep(Duration::from_millis(2200));
        println!("SLOW_FINAL: after poll");
    }

    #[test]
    fn pass_emits_nextdeck_event() {
        nextdeck_helper::event!(
            level: nextdeck_helper::Level::Info,
            target: "fixture",
            "event from fixture";
            "phase" => "arrange",
        );
        println!("EVENT_STDOUT: fixture stdout");
    }

    #[test]
    fn fail_prints_stdout_and_stderr() {
        println!("FAIL_STDOUT: lib fail stdout");
        eprintln!("FAIL_STDERR: lib fail stderr");
        panic!("FAIL_PANIC: expected fixture failure");
    }

    #[test]
    #[ignore = "fixture ignored test"]
    fn ignored_prints_when_explicitly_run() {
        println!("IGNORED_STDOUT: ignored stdout");
    }
}
