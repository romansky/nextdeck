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
        assert_eq!(super::add_one(2), 3);
    }
}
