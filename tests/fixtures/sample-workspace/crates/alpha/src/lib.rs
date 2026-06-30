pub fn add_one(value: i32) -> i32 {
    value + 1
}

#[cfg(test)]
mod tests {
    #[test]
    fn duplicate_name() {
        assert_eq!(super::add_one(1), 2);
    }

    #[test]
    fn alpha_only() {
        assert!(
            std::env::var_os("CARGO_TEST_TUI_FORCE_ALPHA_FAIL").is_none(),
            "forced failure for cargo-test-tui manual verification"
        );
        assert_eq!(super::add_one(2), 3);
    }
}
