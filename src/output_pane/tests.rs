    use super::*;

    #[test]
    fn filters_literal_matches_case_insensitively_by_default() {
        let search = OutputSearchState {
            query: "panic".to_owned(),
            filter: true,
            ..OutputSearchState::default()
        };

        assert_eq!(search.filtered_view("ok\nPANIC\nfine").text, "PANIC");
    }

    #[test]
    fn filtered_view_preserves_source_line_mapping() {
        let search = OutputSearchState {
            query: "panic".to_owned(),
            filter: true,
            ..OutputSearchState::default()
        };

        let view = search.filtered_view("ok\nPANIC\nfine\npanic again");

        assert_eq!(view.text, "PANIC\npanic again");
        assert_eq!(view.source_lines, vec![1, 3]);
        assert_eq!(view.line_index_for_source_line(3), Some(1));
    }

    #[test]
    fn finds_next_and_previous_matches() {
        let mut search = OutputSearchState {
            query: "case".to_owned(),
            ..OutputSearchState::default()
        };

        let next = search
            .next_match("case_1\nother\ncase_2", SearchDirection::Next)
            .expect("valid search")
            .expect("match");
        assert_eq!(next.line, 0);
        assert_eq!(next.index, 0);
        assert_eq!(next.total, 2);

        search.current_line = Some(next.line);
        let previous = search
            .next_match("case_1\nother\ncase_2", SearchDirection::Previous)
            .expect("valid search")
            .expect("match");
        assert_eq!(previous.line, 2);
    }

    #[test]
    fn search_box_view_is_fixed_width_and_marks_active_input() {
        let search = OutputSearchState {
            draft_query: "panic".to_owned(),
            input_active: true,
            ..OutputSearchState::default()
        };

        assert_eq!(search.box_text(18), "[panic_            ]");
        assert_eq!(search.box_text(18).len(), 20);
    }

    #[test]
    fn search_box_view_truncates_long_query_from_left() {
        let search = OutputSearchState {
            query: "abcdefghijklmnopqrstuvwxyz".to_owned(),
            ..OutputSearchState::default()
        };

        assert_eq!(search.box_text(18), "[ijklmnopqrstuvwxyz]");
    }

    #[test]
    fn search_box_view_marks_invalid_regex() {
        let search = OutputSearchState {
            query: "(".to_owned(),
            regex: true,
            ..OutputSearchState::default()
        };

        assert!(search.view("anything").invalid);
        assert!(search.view("anything").title_fragment().contains("!regex"));
    }

    #[test]
    fn match_ranges_find_literal_ranges_case_insensitively() {
        let search = OutputSearchState {
            query: "panic".to_owned(),
            ..OutputSearchState::default()
        };

        assert_eq!(
            search.match_ranges("PANIC then panic").expect("ranges"),
            vec![(0, 5), (11, 16)]
        );
    }

    #[test]
    fn match_ranges_find_regex_ranges() {
        let search = OutputSearchState {
            query: r"case_\d+".to_owned(),
            regex: true,
            ..OutputSearchState::default()
        };

        assert_eq!(
            search.match_ranges("case_01 case_aa case_22").expect("ranges"),
            vec![(0, 7), (16, 23)]
        );
    }
