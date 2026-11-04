use super::heat::is_enclosure_line;

#[test]
fn enclosure_caveats_are_help_lines_but_missing_data_stays_visible() {
    assert!(is_enclosure_line("Enclosure 8455.600: catalogue"));
    assert!(is_enclosure_line(novo_layout::spec::ESTIMATE_NOTICE));
    assert!(!is_enclosure_line("2 devices have no loss data"));
}
