    use super::*;

    #[test]
    fn parses_numstat_counts() {
        assert_eq!(
            parse_numstat("10\t2\tsrc/a.rs\n-\t-\timage.png\n3\t0\tREADME.md\n"),
            DiffStat {
                added: 13,
                deleted: 2,
            }
        );
    }
