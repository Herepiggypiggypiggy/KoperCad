//! The EPLAN-style checks: one test each on small hand-built projects.

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn build<R>(edit: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> (Project, R) {
    let mut project = Project::default();
    let (r, _) = project.transact("build", edit).unwrap();
    (project, r)
}

fn of_kind(problems: &[Problem], kind: ProblemKind) -> Vec<&Problem> {
    problems.iter().filter(|p| p.kind == kind).collect()
}

fn interruption(tx: &mut Tx<'_>, page: PageId, name: &str) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/connectivity/interruption",
        Placement::at(Point::new(0.0, 0.0)),
        vec![Pin::new("1", 0.0, 0.0)],
        SymbolRole::Interruption { name: name.into() },
    )
}

#[test]
fn three_interruption_points_of_one_name_are_ambiguous() {
    let (project, ids) = build(|tx| {
        let p1 = tx.add_page("1")?;
        let p2 = tx.add_page("2")?;
        let a = interruption(tx, p1, "A")?;
        let b = interruption(tx, p2, "A")?;
        let c = interruption(tx, p2, "A")?;
        // A clean pair and a lone one are not ambiguous.
        interruption(tx, p1, "B")?;
        interruption(tx, p2, "B")?;
        Ok(vec![a, b, c])
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::AmbiguousInterruption);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[0].at.len(), ids.len());
    assert!(found[0].message.contains("A is used 3 times"));
}

fn two_wires_between(
    tx: &mut Tx<'_>,
    page: PageId,
    numbers: [&str; 2],
    same_net: bool,
) -> Result<()> {
    let d = tx.add_device(tag("-K1"), "")?;
    let sym = tx.place_symbol(
        page,
        "iec/coils/relay",
        Placement::at(Point::new(0.0, 0.0)),
        vec![
            Pin::new("A1", 0.0, 0.0),
            Pin::new("A2", 0.0, 16.0),
            Pin::new("B1", 10.0, 0.0),
            Pin::new("B2", 10.0, 16.0),
        ],
        SymbolRole::Device {
            device: d,
            main: true,
        },
    )?;
    let pins = if same_net {
        [(0u16, 1u16), (1, 2)]
    } else {
        [(0, 1), (2, 3)]
    };
    for (n, (from, to)) in numbers.into_iter().zip(pins) {
        let w = tx.connect(Endpoint::pin(sym, from), Endpoint::pin(sym, to))?;
        tx.update(w, |w: &mut Wire| w.number = Some(n.to_owned()))?;
    }
    Ok(())
}

#[test]
fn one_wire_number_on_two_nets() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        two_wires_between(tx, page, ["101", "101"], false)
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::WireNumber);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert!(found[0].message.contains("101 is on more than one net"));
}

#[test]
fn different_numbers_on_one_net_are_fine() {
    // The two sides of a device or terminal share a net but are separate
    // connections with their own numbers.
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        two_wires_between(tx, page, ["101", "102"], true)
    });
    assert!(of_kind(&check(&project), ProblemKind::WireNumber).is_empty());
}

#[test]
fn distinct_numbers_on_distinct_nets_are_fine() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        two_wires_between(tx, page, ["101", "102"], false)
    });
    assert!(of_kind(&check(&project), ProblemKind::WireNumber).is_empty());
}

#[test]
fn a_device_without_a_designation() {
    let (project, device) = build(|tx| {
        tx.add_device(tag("-K1"), "ok")?;
        tx.add_device(DeviceTag::default(), "nameless")
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::UntaggedDevice);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].at, vec![AnyId::from(device)]);
}

#[test]
fn two_pages_with_one_name() {
    let (project, ()) = build(|tx| {
        tx.add_page("1")?;
        tx.add_page("1")?;
        tx.add_page("2")?;
        Ok(())
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::DuplicatePage);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].at.len(), 2);
}

#[test]
fn a_junction_with_no_wire() {
    let (project, junction) = build(|tx| {
        let page = tx.add_page("1")?;
        tx.add_junction(page, Point::new(5.0, 5.0))
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::DanglingJunction);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].at, vec![AnyId::from(junction)]);
}

#[test]
fn a_terminal_without_a_name() {
    let (project, terminal) = build(|tx| {
        let strip = tx.add_terminal_strip(tag("-X1"))?;
        tx.add_terminal(strip, "1")?;
        tx.add_terminal(strip, " ")
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::UnnamedTerminal);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].at, vec![AnyId::from(terminal)]);
}

#[test]
fn one_short_tag_under_two_structures_is_info() {
    let (project, ()) = build(|tx| {
        tx.add_device(tag("=A1-K1"), "")?;
        tx.add_device(tag("=A2-K1"), "")?;
        tx.add_device(tag("=A1-K2"), "")?;
        Ok(())
    });
    let problems = check(&project);
    let found = of_kind(&problems, ProblemKind::SameShortTag);
    assert_eq!(found.len(), 1, "{problems:#?}");
    assert_eq!(found[0].severity, Severity::Info);
    assert!(found[0].message.starts_with("-K1 is used under =A1, =A2"));
}
