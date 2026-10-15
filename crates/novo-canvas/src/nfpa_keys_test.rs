//! The bridge dot is drawn for the connected terminals of either pack.

use super::is_bridged;

#[test]
fn bridged_terminals_of_both_packs_carry_the_dot() {
    for pack in ["iec", "nfpa"] {
        assert!(is_bridged(&format!("{pack}/terminals/terminal_bridged")));
        assert!(is_bridged(&format!(
            "{pack}/terminals/terminal_end_bridged"
        )));
        assert!(!is_bridged(&format!("{pack}/terminals/terminal")));
    }
}
