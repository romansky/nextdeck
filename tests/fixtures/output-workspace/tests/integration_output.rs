#[test]
fn integration_pass_prints_output() {
    println!("INTEGRATION_STDOUT: integration pass stdout");
    eprintln!("INTEGRATION_STDERR: integration pass stderr");
}
