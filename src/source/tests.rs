use super::*;
use std::{env, fs};

#[test]
fn resolves_custom_integration_test_target_path() {
    let root = temp_dir("custom-target");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"
                [[test]]
                name = "scenario"
                path = "src/tier_scenario.rs"
            "#,
    )
    .expect("write manifest");
    fs::write(root.join("src/tier_scenario.rs"), "").expect("write source");

    assert_eq!(
        binary_source_path(&root, "test", "scenario"),
        Some(root.join("src/tier_scenario.rs"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolves_default_binary_main_when_src_bin_target_is_absent() {
    let root = temp_dir("default-bin-main");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("write manifest");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");

    assert_eq!(
        binary_source_path(&root, "bin", "demo"),
        Some(root.join("src/main.rs"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_locator_resolves_unit_tests_in_module_files() {
    let root = temp_dir("module-unit-test");
    fs::create_dir_all(root.join("src/output")).expect("create module dir");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("write manifest");
    fs::write(root.join("src/main.rs"), "mod output;\nfn main() {}\n").expect("write main");
    fs::write(root.join("src/output.rs"), "mod tests;\n").expect("write module");
    fs::write(
        root.join("src/output/tests.rs"),
        "#[test]\nfn dogfood_output_captures_stdout_stderr_and_events() {}\n",
    )
    .expect("write test module");

    let locator = SourceLocator::new(&root, "bin", "demo");

    assert_eq!(
        locator.path_for_test("output::tests::dogfood_output_captures_stdout_stderr_and_events"),
        Some(root.join("src/output/tests.rs"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn finds_async_test_function_line() {
    let root = temp_dir("find-line");
    fs::create_dir_all(&root).expect("create temp");
    let path = root.join("case.rs");
    fs::write(
        &path,
        "#[tokio::test]\nasync fn first_case() {}\n\n#[test]\nfn second_case() {}\n",
    )
    .expect("write source");

    assert_eq!(find_test_line(&path, "tests::second_case"), Some(5));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn finds_ignore_reason_for_test_function() {
    let root = temp_dir("ignore-reason");
    fs::create_dir_all(&root).expect("create temp");
    let path = root.join("case.rs");
    fs::write(
        &path,
        r#"
            #[cfg(test)]
            mod tests {
                #[test]
                #[ignore = "performance test"]
                fn expensive_case() {}

                #[test]
                fn normal_case() {}
            }
        "#,
    )
    .expect("write source");

    assert_eq!(
        ignore_reason_for_test(&path, "tests::expensive_case").as_deref(),
        Some("performance test")
    );
    assert_eq!(ignore_reason_for_test(&path, "tests::normal_case"), None);

    let _ = fs::remove_dir_all(root);
}

fn temp_dir(name: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "nextdeck-source-test-{name}-{}",
        std::process::id()
    ))
}
