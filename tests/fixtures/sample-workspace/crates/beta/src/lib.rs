pub fn double(value: i32) -> i32 {
    value * 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn duplicate_name() {
        assert_eq!(super::double(2), 4);
    }

    #[test]
    fn beta_only() {
        assert_eq!(super::double(3), 6);
    }

    macro_rules! scroll_tests {
        ($($name:ident),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    assert_eq!(super::double(2), 4);
                }
            )*
        };
    }

    scroll_tests!(
        scroll_00, scroll_01, scroll_02, scroll_03, scroll_04, scroll_05, scroll_06, scroll_07,
        scroll_08, scroll_09, scroll_10, scroll_11, scroll_12, scroll_13, scroll_14, scroll_15,
        scroll_16, scroll_17, scroll_18, scroll_19, scroll_20, scroll_21, scroll_22, scroll_23,
        scroll_24, scroll_25, scroll_26, scroll_27, scroll_28, scroll_29,
    );
}
