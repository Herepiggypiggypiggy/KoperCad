use super::whats_new::{Entry, entries, first_run, parse, unseen};
use crate::settings::Settings;

#[test]
fn every_shipped_entry_has_an_id_text_and_a_place() {
    let all = entries();
    assert!(!all.is_empty());
    let mut ids: Vec<_> = all.iter().map(|e| e.id.as_str()).collect();
    for e in &all {
        assert!(!e.title.is_empty() && !e.text.is_empty(), "{}", e.id);
        assert!(!e.whence.is_empty(), "{} says where to find it", e.id);
    }
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), all.len(), "ids are unique");
}

#[test]
fn parse_reads_title_where_and_wrapped_text() {
    let e = parse("<!-- note -->\n## a-1 · Title\nWhere: here\nOne\ntwo.\n");
    assert_eq!(
        e,
        vec![Entry {
            id: "a-1".into(),
            title: "Title".into(),
            whence: "here".into(),
            text: "One two.".into(),
        }]
    );
}

#[test]
fn unseen_is_what_sits_above_the_last_seen_entry() {
    let all = parse("## c · C\nx\n## b · B\nx\n## a · A\nx\n");
    assert_eq!(unseen(&all, "c").len(), 0);
    assert_eq!(unseen(&all, "b").len(), 1);
    assert_eq!(unseen(&all, "a").len(), 2);
    assert_eq!(unseen(&all, "").len(), 3, "unknown id: everything is new");
}

#[test]
fn a_first_run_has_nothing_unseen_but_a_returning_user_does() {
    let mut s = Settings::default();
    assert!(first_run(&s));
    s.onboarding.started = true;
    assert!(!first_run(&s));
}

#[test]
fn the_command_is_in_help_and_runs() {
    assert!(super::workbench::app_runs("help.whats_new"));
    assert!(
        novo_workflow::commands::COMMANDS
            .iter()
            .any(|c| c.id == "help.whats_new")
    );
}
