use std::{
    fs,
    path::{Path, PathBuf},
};

use regex::Regex;

pub fn binary_source_path(cwd: &Path, kind: &str, name: &str) -> Option<PathBuf> {
    let manifest = cwd.join("Cargo.toml");
    let source = match kind {
        "lib" => cargo_table_path(&manifest, "lib").or_else(|| Some(cwd.join("src/lib.rs"))),
        "test" => cargo_named_target_path(&manifest, "test", name)
            .or_else(|| Some(cwd.join("tests").join(format!("{name}.rs")))),
        "bin" => cargo_named_target_path(&manifest, "bin", name)
            .or_else(|| Some(cwd.join("src/bin").join(format!("{name}.rs"))))
            .or_else(|| Some(cwd.join("src/main.rs"))),
        "example" => cargo_named_target_path(&manifest, "example", name)
            .or_else(|| Some(cwd.join("examples").join(format!("{name}.rs")))),
        "bench" => cargo_named_target_path(&manifest, "bench", name)
            .or_else(|| Some(cwd.join("benches").join(format!("{name}.rs")))),
        _ => None,
    }?;
    source.exists().then_some(source)
}

pub fn find_test_line(path: &Path, test_name: &str) -> Option<usize> {
    let text = fs::read_to_string(path).ok()?;
    let name = test_name.rsplit("::").next().unwrap_or(test_name);
    let pattern = format!(r"\bfn\s+{}\b", regex::escape(name));
    let regex = Regex::new(&pattern).ok()?;
    text.lines()
        .enumerate()
        .find_map(|(index, line)| regex.is_match(line).then_some(index + 1))
}

fn cargo_table_path(manifest: &Path, table_name: &str) -> Option<PathBuf> {
    let text = fs::read_to_string(manifest).ok()?;
    let mut in_table = false;
    for raw_line in text.lines() {
        let line = strip_comment(raw_line).trim();
        if line.starts_with('[') {
            in_table = line == format!("[{table_name}]");
            continue;
        }
        if in_table && let Some(path) = cargo_string_value(line, "path") {
            return Some(manifest_parent(manifest).join(path));
        }
    }
    None
}

fn cargo_named_target_path(
    manifest: &Path,
    table_name: &str,
    target_name: &str,
) -> Option<PathBuf> {
    let text = fs::read_to_string(manifest).ok()?;
    let mut in_target = false;
    let mut name = None;
    let mut path = None;

    for raw_line in text.lines().chain(["[[__flush__]]"]) {
        let line = strip_comment(raw_line).trim();
        if line.starts_with("[[") {
            if in_target
                && name.as_deref() == Some(target_name)
                && let Some(path) = path.take()
            {
                return Some(manifest_parent(manifest).join(path));
            }
            in_target = line == format!("[[{table_name}]]");
            name = None;
            path = None;
            continue;
        }
        if !in_target {
            continue;
        }
        if let Some(value) = cargo_string_value(line, "name") {
            name = Some(value);
        } else if let Some(value) = cargo_string_value(line, "path") {
            path = Some(value);
        }
    }
    None
}

fn cargo_string_value(line: &str, key: &str) -> Option<String> {
    let (raw_key, raw_value) = line.split_once('=')?;
    if raw_key.trim() != key {
        return None;
    }
    let value = raw_value.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(value.to_owned())
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#')
        .map(|(before, _)| before)
        .unwrap_or(line)
}

fn manifest_parent(manifest: &Path) -> &Path {
    manifest.parent().unwrap_or_else(|| Path::new("."))
}

#[cfg(test)]
mod tests;
