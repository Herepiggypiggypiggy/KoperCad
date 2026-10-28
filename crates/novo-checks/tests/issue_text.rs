//! Issue texts name what they are about in the words of the drawing: a wire
//! by its ends and page, a symbol without a tag in words, never by library id.

use novo_checks::{ProblemKind, check};
use novo_core::*;

fn build<R>(edit: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> (Project, R) {
    let mut project = Project::default();
    let (r, _) = project.transact("build", edit).unwrap();
    (project, r)
}

#[test]
fn a_symbol_without_a_tag_is_named_in_words_not_by_library_id() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        tx.place_symbol(
            page,
            "iec/terminals/terminal_pe",
            Placement::at(Point::new(0.0, 0.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
            SymbolRole::Unassigned,
        )?;
        Ok(())
    });
    let problems = check(&project);
    assert!(!problems.is_empty());
    for p in &problems {
        assert!(!p.message.contains("iec/"), "{}", p.message);
    }
    let free = problems
        .iter()
        .find(|p| p.kind == ProblemKind::UnconnectedPin)
        .unwrap();
    assert_eq!(free.message, "terminal PE (no tag): 1, 2 not connected");
}

fn ip(tx: &mut Tx<'_>, page: PageId, name: &str) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/connectivity/interruption_point",
        Placement::at(Point::new(0.0, 0.0)),
        vec![Pin::new("1", 0.0, 0.0)],
        SymbolRole::Interruption { name: name.into() },
    )
}

#[test]
fn a_lone_interruption_point_names_the_ones_that_wait_on_other_pages() {
    let (project, ()) = build(|tx| {
        let p1 = tx.add_page("1")?;
        let p2 = tx.add_page("2a")?;
        ip(tx, p1, "IP1")?;
        ip(tx, p2, "IP2")?;
        // A pair and a lone point on the same page as IP1 add nothing.
        ip(tx, p1, "IP3")?;
        ip(tx, p2, "IP3")?;
        Ok(())
    });
    let lone: Vec<String> = check(&project)
        .into_iter()
        .filter(|p| p.kind == ProblemKind::LoneInterruption)
        .map(|p| p.message)
        .collect();
    assert_eq!(lone.len(), 2, "{lone:#?}");
    assert!(
        lone[0].starts_with(
            "Interruption point IP1 has no partner; IP2 on page 2a has no partner either."
        ),
        "{}",
        lone[0]
    );
    assert!(lone[0].contains("Link cross reference"));
    assert!(lone[1].contains("IP1 on page 1"), "{}", lone[1]);
}

#[test]
fn a_lone_interruption_point_with_no_other_waiting_keeps_the_short_text() {
    let (project, ()) = build(|tx| {
        let p1 = tx.add_page("1")?;
        ip(tx, p1, "IP1")?;
        Ok(())
    });
    let lone: Vec<String> = check(&project)
        .into_iter()
        .filter(|p| p.kind == ProblemKind::LoneInterruption)
        .map(|p| p.message)
        .collect();
    assert_eq!(lone, ["Interruption point IP1 has no partner"]);
}
