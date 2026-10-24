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

/// The open pin `name` on a symbol of `key`, as (symbol, pin index, position).
fn open_pin(
    project: &Project,
    symbol: &novo_core::SymbolInstance,
    name: &str,
    dir: Direction,
) -> Option<(Endpoint, Point)> {
    let index = symbol.pins.iter().position(|p| p.name == name)?;
    if symbol.pins[index].direction != Some(dir) {
        return None;
    }
    let end = Endpoint::pin(symbol.id, index as u16);
    let busy = project.wires().any(|w| w.from == end || w.to == end);
    let at = project.endpoint_position(end)?;
    (!busy).then_some((end, at))
}

/// Draws a wire from each open output of the Three-phase supply on `page` to
/// the open entry of the circuit placed with it: the three line phases of a
/// starter's main breaker, its control breaker and its PE terminal. Each
/// entry is joined only when exactly one candidate is open on the page, so a
/// drawing with two starters is left for the user to decide. Returns how
/// many wires it drew.
pub(super) fn join_starter(tx: &mut Tx<'_>, page: PageId) -> Result<usize> {
    let instances: Vec<_> = tx
        .project()
        .block_instances()
        .filter(|i| i.page == page)
        .filter(|i| {
            tx.project()
                .block_definition(i.definition)
                .is_some_and(|d| d.name == SUPPLY_MACRO)
        })
        .map(|i| i.members.iter().map(|m| m.actual).collect::<Vec<_>>())
        .collect();
    let [members] = instances.as_slice() else {
        return Ok(0);
    };
    let project = tx.project();
    let mine = |s: &novo_core::SymbolInstance| members.iter().any(|m| *m == s.id.into());
    let on_page = || project.symbols().filter(|s| s.page == page);
    let key = |s: &novo_core::SymbolInstance, tail: &str| s.symbol.0.ends_with(tail);
    // The supply's outputs.
    let out = |tail: &str, name: &str| -> Option<(Endpoint, Point)> {
        on_page()
            .filter(|s| mine(s) && key(s, tail))
            .find_map(|s| open_pin(project, s, name, Direction::Down))
    };
    // The entries outside the supply: one candidate or none.
    let only = |found: Vec<Vec<(Endpoint, Point)>>| match found.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    };
    let phases = only(
        on_page()
            .filter(|s| !mine(s))
            .filter_map(|s| {
                let pins: Vec<_> = ["1", "3", "5"]
                    .iter()
                    .filter_map(|n| open_pin(project, s, n, Direction::Up))
                    .collect();
                (pins.len() == 3).then_some(pins)
            })
            .collect(),
    );
    let control = only(
        on_page()
            .filter(|s| !mine(s) && key(s, "mcb_1p"))
            .filter_map(|s| open_pin(project, s, "1", Direction::Up).map(|p| vec![p]))
            .collect(),
    );
    let earth = only(
        on_page()
            .filter(|s| !mine(s) && key(s, "terminal_pe"))
            .filter_map(|s| open_pin(project, s, "1", Direction::Up).map(|p| vec![p]))
            .collect(),
    );
    let mut pairs: Vec<((Endpoint, Point), (Endpoint, Point))> = Vec::new();
    if let Some(entries) = phases {
        let outs: Vec<_> = ["2", "4", "6"]
            .iter()
            .filter_map(|n| out("mcb_3p", n))
            .collect();
        if outs.len() == 3 {
            pairs.extend(outs.into_iter().zip(entries));
        }
    }
    if let (Some(entry), Some(from)) = (control, out("mcb_1p", "2")) {
        pairs.push((from, entry[0]));
    }
    if let (Some(entry), Some(from)) = (earth, out("terminal_pe", "2")) {
        pairs.push((from, entry[0]));
    }
    // The channel: down under the supply, left through the gap between the
    // two circuits, up to above the entries, then over to each entry. Every
    // wire has its own rows and column, so none lies along another.
    let gap = on_page()
        .filter(|s| !mine(s) && !matches!(s.role, SymbolRole::PotentialFeed { .. }))
        .flat_map(|s| {
            (0..s.pins.len())
                .filter_map(|i| project.endpoint_position(Endpoint::pin(s.id, i as u16)))
        })
        .map(|p| p.x)
        .fold(f64::MIN, f64::max)
        + 8.0;
    let mut joined = 0;
    for (k, ((from, a), (to, b))) in pairs.into_iter().enumerate() {
        let k = k as f64;
        let down = a.y + 8.0 + 4.0 * k;
        let column = gap + 4.0 * (4.0 - k);
        let up = b.y - 8.0 - 4.0 * k;
        let corners = if column < a.x - 4.0 {
            vec![
                Point::new(a.x, down),
                Point::new(column, down),
                Point::new(column, up),
                Point::new(b.x, up),
            ]
        } else {
            Vec::new()
        };
        // A route the page refuses is left for the user; the placement stays.
        if draw_wire(
            tx,
            page,
            Target::Endpoint(from, a),
            &corners,
            Target::Endpoint(to, b),
        )
        .is_ok()
        {
            joined += 1;
        }
    }
    Ok(joined)
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

    fn macro_named(name: &str) -> novo_parts::MacroFile {
        starter_macros()
            .into_iter()
            .map(|(_, f)| f)
            .find(|f| f.name == name)
            .expect("a starter macro")
    }

    /// The star-delta starter, then the supply, as a new project gets them.
    fn starter_then_supply() -> Document {
        let mut doc = Document::new(Project::new(""));
        doc.transact("p", |tx| {
            let page = tx.add_page("1")?;
            place_macro(
                tx,
                macro_named("Star-delta starter").definition,
                page,
                Point::new(20.0, 20.0),
                &PlaceOptions::default(),
            )?;
            let origin = novo_workflow::macro_place::free_origin(tx.project(), page, 20.0);
            let placed = place_macro(
                tx,
                supply().definition,
                page,
                origin,
                &PlaceOptions::default(),
            )?;
            feed_open_points(tx, page, placed.instance)?;
            Ok(())
        })
        .expect("place");
        doc
    }

    #[test]
    fn the_supply_is_joined_to_the_starter_next_to_it() {
        let mut doc = starter_then_supply();
        let page = doc.project().pages().next().expect("page").id;
        let mut joined = 0;
        doc.transact("join", |tx| {
            joined = join_starter(tx, page)?;
            Ok(())
        })
        .expect("join");
        assert_eq!(joined, 5, "three phases, the control breaker and PE");
        let p = doc.project();
        let net = Netlist::build(p);
        let named = |end: Endpoint| {
            net.potential_of(end)
                .and_then(|id| p.potential(id))
                .map(|x| x.name.clone())
        };
        // The starter's main breaker now carries L1, L2 and L3.
        let f1 = p
            .symbols()
            .find(|s| s.symbol.0.ends_with("fuse_3p") || s.symbol.0.ends_with("mcb_3p"))
            .map(|s| s.id);
        assert!(f1.is_some());
        let lit: Vec<_> = p
            .symbols()
            .flat_map(|s| {
                s.pins
                    .iter()
                    .enumerate()
                    .map(move |(i, _)| Endpoint::pin(s.id, i as u16))
            })
            .filter_map(named)
            .collect();
        assert!(lit.iter().filter(|n| *n == "L1").count() >= 4);
        // Nothing is left open at the starter's entries: a second call adds nothing.
        let mut again = 0;
        doc.transact("again", |tx| {
            again = join_starter(tx, page)?;
            Ok(())
        })
        .expect("again");
        assert_eq!(again, 0);
    }
}
