#[cfg(test)]
mod tests {
    trait CompilerErrorMarker {}

    impl CompilerErrorMarker for u8 {}
    impl CompilerErrorMarker for u16 {}

    fn ambiguous_type_for_nextdeck<T: CompilerErrorMarker>() {}

    struct TrailingCompilerWarnings;

    impl TrailingCompilerWarnings {
        fn warning_00() {}
        fn warning_01() {}
        fn warning_02() {}
        fn warning_03() {}
        fn warning_04() {}
        fn warning_05() {}
        fn warning_06() {}
        fn warning_07() {}
        fn warning_08() {}
        fn warning_09() {}
        fn warning_10() {}
        fn warning_11() {}
        fn warning_12() {}
        fn warning_13() {}
        fn warning_14() {}
        fn warning_15() {}
        fn warning_16() {}
        fn warning_17() {}
        fn warning_18() {}
        fn warning_19() {}
        fn warning_20() {}
        fn warning_21() {}
        fn warning_22() {}
        fn warning_23() {}
    }

    #[test]
    fn never_runs_because_the_test_binary_does_not_compile() {
        ambiguous_type_for_nextdeck::<_>();
    }
}
