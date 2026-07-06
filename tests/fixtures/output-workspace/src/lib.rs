pub fn marker() -> &'static str {
    "nextdeck-output-fixture"
}

#[cfg(test)]
mod tests {
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
