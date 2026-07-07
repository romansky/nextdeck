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
