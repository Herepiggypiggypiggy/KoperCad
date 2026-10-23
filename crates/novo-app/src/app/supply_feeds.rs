//! The Three-phase supply macro arrives with its five open supply points
//! (L1, L2, L3, N, PE) bare. In a drawing that has no supply yet, placing it
//! also hangs a potential feed on each, so the drawing has a supply at once
//! and Live view can energise what is wired to the macro's outputs. Where the
//! user finds it: Place macro > Three-phase supply, or the Add supply circuit
//! button on the Live card; the status line then names the next step.

use novo_core::{
    BlockInstanceId, Direction, Endpoint, Netlist, PageId, Pin, Placement, Point, PotentialId,
    PotentialKind, Project, Result, SymbolRole, Tx,
};
use novo_editor::edit::draw_wire;
use novo_editor::geometry::Target;
use novo_editor::potential::FEED_PE_SYMBOL;

/// The macro these feeds belong to.
pub(super) const SUPPLY_MACRO: &str = "Three-phase supply";

/// The open supply points: the symbol's library key tail, the pin, the
/// potential and its kind and volts.
const POINTS: &[(&str, &str, &str, PotentialKind, f64)] = &[
    ("main_switch_3p", "1", "L1", PotentialKind::Line, 400.0),
    ("main_switch_3p", "3", "L2", PotentialKind::Line, 400.0),
    ("main_switch_3p", "5", "L3", PotentialKind::Line, 400.0),
    ("terminal_n", "1", "N", PotentialKind::Neutral, 0.0),
    (
        "terminal_pe",
        "1",
        "PE",
        PotentialKind::ProtectiveEarth,
        0.0,
    ),
];

/// True when the drawing already has a supply, as Live view counts it: a
/// potential feed or a supply terminal.
pub(super) fn has_supply(project: &Project) -> bool {
    novo_sim::explain::has_supply(&Netlist::build(project))
}

fn potential(tx: &mut Tx<'_>, name: &str, kind: PotentialKind, volts: f64) -> Result<PotentialId> {
    if let Some(p) = tx.project().potentials().find(|p| p.name == name) {
        return Ok(p.id);
    }
    let id = tx.add_potential(name, kind)?;
    if volts > 0.0 {
        tx.update::<novo_core::Potential>(id, |p| p.voltage = Some(volts))?;
    }
    Ok(id)
}

/// Hangs a feed on each open supply point of the placed macro. Returns how
/// many it placed.
pub(super) fn feed_open_points(
    tx: &mut Tx<'_>,
    page: PageId,
    instance: BlockInstanceId,
) -> Result<usize> {
    let members: Vec<_> = match tx.project().block_instance(instance) {
        Some(i) => i.members.iter().map(|m| m.actual).collect(),
        None => return Ok(0),
    };
    let mut placed = 0;
    for &(key, pin_name, name, kind, volts) in POINTS {
        let found = tx.project().symbols().find_map(|s| {
            let mine = members.iter().any(|m| *m == s.id.into());
            let pin = s.pins.iter().position(|p| p.name == pin_name)?;
            (mine && s.symbol.0.ends_with(key)).then_some((s.id, pin as u16, s.pins[pin].direction))
        });
        let Some((symbol, pin, dir)) = found else {
            continue;
        };
        let end = Endpoint::pin(symbol, pin);
        let Some(at) = tx.project().endpoint_position(end) else {
            continue;
        };
        let busy = tx.project().wires().any(|w| w.from == end || w.to == end);
        if busy {
            continue;
        }
        let pot = potential(tx, name, kind, volts)?;
        // The feed stands beyond the pin, on the side the wire leaves.
        let down = matches!(dir, Some(Direction::Down));
        let origin = if down {
            Point::new(at.x, at.y + 6.0 - 8.0)
        } else {
            Point::new(at.x, at.y - 6.0 - 8.0)
        };
        let key = if kind == PotentialKind::ProtectiveEarth {
            FEED_PE_SYMBOL
        } else {
            novo_editor::potential::FEED_SYMBOL
        };
        let feed = tx.place_symbol(
            page,
            key,
            Placement::at(origin),
            vec![Pin::new("1", 0.0, 8.0)],
            SymbolRole::PotentialFeed { potential: pot },
        )?;
        let from = Endpoint::pin(feed, 0);
        let Some(a) = tx.project().endpoint_position(from) else {
            continue;
        };
        draw_wire(
            tx,
            page,
            Target::Endpoint(from, a),
            &[],
            Target::Endpoint(end, at),
        )?;
        placed += 1;
    }
    Ok(placed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Project};
    use novo_parts::starter_macros;
    use novo_workflow::macro_place::{PlaceOptions, place_macro};

    fn supply() -> novo_parts::MacroFile {
        starter_macros()
            .into_iter()
            .map(|(_, f)| f)
            .find(|f| f.name == SUPPLY_MACRO)
            .expect("the supply macro")
    }

    #[test]
    fn placing_the_supply_gives_the_drawing_a_supply() {
        let mut doc = Document::new(Project::new(""));
        let mut count = 0;
        doc.transact("p", |tx| {
            let page = tx.add_page("1")?;
            let placed = place_macro(
                tx,
                supply().definition,
                page,
                Point::new(20.0, 20.0),
                &PlaceOptions::default(),
            )?;
            count = feed_open_points(tx, page, placed.instance)?;
            Ok(())
        })
        .expect("place");
        assert_eq!(count, 5);
        let p = doc.project();
        let mut names: Vec<_> = p.potentials().map(|x| x.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["L1", "L2", "L3", "N", "PE"]);
        assert!(has_supply(p));
    }

    fn place(doc: &mut Document) -> usize {
        let mut count = 0;
        doc.transact("p", |tx| {
            let existing = tx.project().pages().next().map(|p| p.id);
            let page = match existing {
                Some(page) => page,
                None => tx.add_page("1")?,
            };
            let placed = place_macro(
                tx,
                supply().definition,
                page,
                Point::new(20.0, 20.0),
                &PlaceOptions::default(),
            )?;
            if !has_supply(tx.project()) {
                count = feed_open_points(tx, page, placed.instance)?;
            }
            Ok(())
        })
        .expect("place");
        count
    }

    #[test]
    fn a_second_placement_adds_no_feeds_and_undo_removes_them() {
        let mut doc = Document::new(Project::new(""));
        assert_eq!(place(&mut doc), 5);
        assert_eq!(place(&mut doc), 0, "the drawing has a supply now");
        assert_eq!(doc.project().potentials().count(), 5);
        let feeds = |d: &Document| {
            d.project()
                .symbols()
                .filter(|s| matches!(s.role, SymbolRole::PotentialFeed { .. }))
                .count()
        };
        assert_eq!(feeds(&doc), 5);
        doc.undo().expect("undo the second placement");
        assert_eq!(feeds(&doc), 5);
        doc.undo().expect("undo the first placement");
        assert_eq!(feeds(&doc), 0);
        assert!(!has_supply(doc.project()));
    }
}
