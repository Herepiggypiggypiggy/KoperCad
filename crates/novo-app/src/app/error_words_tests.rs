//! Plain words for OS errors in messages.

use super::error_words::plain;

#[cfg(windows)]
const IN_USE: &str = "Could not save: The process cannot access the file because it is being used by another process. (os error 32)";
#[cfg(not(windows))]
const IN_USE: &str = "Could not save: No space left on device (os error 28)";

#[test]
fn a_known_os_error_becomes_advice_and_keeps_the_start_of_the_message() {
    let text = plain(IN_USE);
    assert!(text.starts_with("Could not save: "), "{text}");
    assert!(!text.contains("os error"), "{text}");
    assert!(
        text.ends_with(" it there and try again") || text.contains("disk is full"),
        "{text}"
    );
}

#[test]
fn an_unknown_code_or_no_code_is_left_alone() {
    let unknown = "Could not save: Something odd (os error 9999)";
    assert_eq!(plain(unknown), unknown);
    let plain_text = "No wires to cut yet: connect two device pins";
    assert_eq!(plain(plain_text), plain_text);
    let not_a_code = "Could not read x (os error y)";
    assert_eq!(plain(not_a_code), not_a_code);
}

#[test]
fn a_message_that_is_only_the_os_error_starts_with_a_capital() {
    #[cfg(windows)]
    let raw = "Access is denied. (os error 5)";
    #[cfg(not(windows))]
    let raw = "Permission denied (os error 13)";
    let text = plain(raw);
    assert!(text.chars().next().unwrap().is_uppercase(), "{text}");
    assert!(text.contains("Pick a folder"), "{text}");
}

#[test]
fn the_app_shows_the_plain_text_and_logs_the_raw_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::gate1_tests::Drive::new(&dir.path().join("untitled"));
    d.app.message(super::Level::Error, IN_USE);
    let shown = &d.app.messages().last().unwrap().text;
    assert!(!shown.contains("os error"), "{shown}");
}
