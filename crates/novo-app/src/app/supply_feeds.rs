//! The Three-phase supply macro arrives with its five open supply points
//! (L1, L2, L3, N, PE) bare. In a drawing that has no supply yet, placing it
//! also hangs a potential feed on each, so the drawing has a supply at once
//! and Live view can energise what is wired to the macro's outputs. Where the
//! user finds it: Place macro > Three-phase supply, or the Add supply circuit
//! button on the Live card; the status line then names the next step.

use novo_core::{
    BlockInstanceId, Direction, Endpoint, NetId, Netlist, PageId, Pin, Placement, Point,
    PotentialId, PotentialKind, Project, Result, SymbolRole, Tx,
};
use novo_editor::edit::draw_wire;
use novo_editor::geometry::Target;
use novo_editor::potential::FEED_PE_SYMBOL;

/// A pin or junction and where it is.
type Spot = (Endpoint, Point);

/// One wire to draw: from a supply output to an entry, which the wire
/// reaches from below when the flag is set.
type Join = (Spot, Spot, bool);

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
/// starter's main breaker, its control breaker and its PE terminal. `placed`
/// is the macro just placed: when it is the supply, the entries are looked
/// for anywhere on the page; otherwise only among its own symbols, so an
/// unrelated macro never joins the supply to a starter that was already
/// there. Each entry is joined only when exactly one candidate is open, so a
/// drawing with two starters is left for the user to decide. Returns how
/// many wires it drew.
pub(super) fn join_starter(
    tx: &mut Tx<'_>,
    page: PageId,
    placed: BlockInstanceId,
) -> Result<usize> {
    let (placed_members, placed_is_supply): (Vec<_>, bool) =
        match tx.project().block_instance(placed) {
            Some(i) => (
                i.members.iter().map(|m| m.actual).collect(),
                tx.project()
                    .block_definition(i.definition)
                    .is_some_and(|d| d.name == SUPPLY_MACRO),
            ),
            None => return Ok(0),
        };
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
    // The entries come from the page, or only from the macro just placed.
    let entry = |s: &novo_core::SymbolInstance| {
        !mine(s) && (placed_is_supply || placed_members.iter().any(|m| *m == s.id.into()))
    };
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
            .filter(|s| entry(s))
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
            .filter(|s| entry(s) && key(s, "mcb_1p"))
            .filter_map(|s| open_pin(project, s, "1", Direction::Up).map(|p| vec![p]))
            .collect(),
    );
    let earth = only(
        on_page()
            .filter(|s| entry(s) && key(s, "terminal_pe"))
            .filter_map(|s| open_pin(project, s, "1", Direction::Up).map(|p| vec![p]))
            .collect(),
    );
    // The coil return: the A2 pins of the placed circuit's coils, when they
    // all lie on one net that has no potential of its own. The wire lands on
    // the right end of that net's bus when it has one, else on a free A2 pin.
    let neutral = {
        let netlist = Netlist::build(project);
        let coils: Vec<(Endpoint, Point, NetId)> = on_page()
            .filter(|s| entry(s) && s.symbol.0.contains("/coils/"))
            .filter_map(|s| {
                let i = s
                    .pins
                    .iter()
                    .position(|p| p.name == "A2" && p.direction == Some(Direction::Down))?;
                let end = Endpoint::pin(s.id, i as u16);
                Some((end, project.endpoint_position(end)?, netlist.net_of(end)?))
            })
            .collect();
        let net = coils.first().map(|c| c.2);
        let one_net = coils.iter().all(|c| Some(c.2) == net);
        net.filter(|n| one_net && netlist.net(*n).defined.is_empty())
            .and_then(|n| {
                let bus = project
                    .junctions()
                    .filter(|j| {
                        j.page == page && netlist.net_of(Endpoint::junction(j.id)) == Some(n)
                    })
                    .max_by(|a, b| a.position.x.total_cmp(&b.position.x))
                    .map(|j| (Endpoint::junction(j.id), j.position));
                bus.or_else(|| {
                    coils
                        .iter()
                        .filter(|c| !project.wires().any(|w| w.from == c.0 || w.to == c.0))
                        .max_by(|a, b| a.1.x.total_cmp(&b.1.x))
                        .map(|c| (c.0, c.1))
                })
            })
    };
    // A coil return alone is not a starter: a circuit that only has coils
    // (an E-stop chain) is not joined to the supply.
    let is_starter = phases.is_some() || control.is_some() || earth.is_some();
    let mut pairs: Vec<Join> = Vec::new();
    if let Some(entries) = phases {
        let outs: Vec<_> = ["2", "4", "6"]
            .iter()
            .filter_map(|n| out("mcb_3p", n))
            .collect();
        if outs.len() == 3 {
            pairs.extend(outs.into_iter().zip(entries).map(|(o, e)| (o, e, false)));
        }
    }
    if let (Some(entry), Some(from)) = (control, out("mcb_1p", "2")) {
        pairs.push((from, entry[0], false));
    }
    if let (Some(entry), Some(from)) = (earth, out("terminal_pe", "2")) {
        pairs.push((from, entry[0], false));
    }
    // The coil return enters from below: the wire runs up to the bus.
    if let (Some(entry), Some(from)) = (neutral, out("terminal_n", "2"))
        && is_starter
    {
        pairs.push((from, entry, true));
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
    for (k, ((from, a), (to, b), below)) in pairs.into_iter().enumerate() {
        let k = k as f64;
        let down = a.y + 8.0 + 4.0 * k;
        let column = gap + 4.0 * (5.0 - k);
        let up = if below {
            b.y + 8.0 + 4.0 * k
        } else {
            b.y - 8.0 - 4.0 * k
        };
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

    /// The star-delta starter, then the supply, as a new project gets them;
    /// returns the document and the supply's instance.
    fn starter_then_supply() -> (Document, BlockInstanceId) {
        let mut doc = Document::new(Project::new(""));
        let mut instance = None;
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
            instance = Some(placed.instance);
            Ok(())
        })
        .expect("place");
        (doc, instance.expect("supply instance"))
    }

    /// The potential on pin `name` of the first symbol of device `tag`.
    fn on_pin(project: &Project, tag: &str, name: &str) -> Option<String> {
        let device = project.devices().find(|d| d.tag.to_string() == tag)?.id;
        let s = project
            .symbols()
            .find(|s| matches!(s.role, SymbolRole::Device { device: d, .. } if d == device))?;
        let pin = s.pins.iter().position(|p| p.name == name)?;
        let net = Netlist::build(project);
        let id = net.potential_of(Endpoint::pin(s.id, pin as u16))?;
        project.potential(id).map(|p| p.name.clone())
    }

    #[test]
    fn the_supply_is_joined_to_the_starter_next_to_it() {
        let (mut doc, supply) = starter_then_supply();
        let page = doc.project().pages().next().expect("page").id;
        let mut joined = 0;
        doc.transact("join", |tx| {
            joined = join_starter(tx, page, supply)?;
            Ok(())
        })
        .expect("join");
        assert_eq!(joined, 6, "three phases, the control breaker, PE and N");
        let p = doc.project();
        // The starter's main breaker entries carry L1, L2 and L3 ...
        for (pin, line) in [("1", "L1"), ("3", "L2"), ("5", "L3")] {
            assert_eq!(
                on_pin(p, "-F1", pin).as_deref(),
                Some(line),
                "-F1 pin {pin}"
            );
        }
        // ... its control breaker is fed from the supply's control breaker ...
        assert_eq!(on_pin(p, "-F3", "1").as_deref(), Some("L1"));
        // ... and its PE terminal is on PE.
        let pe = p
            .symbols()
            .filter(|s| s.symbol.0.ends_with("terminal_pe"))
            .filter_map(|s| {
                let i = s.pins.iter().position(|x| x.name == "1")?;
                Netlist::build(p).potential_of(Endpoint::pin(s.id, i as u16))
            })
            .filter_map(|id| p.potential(id).map(|x| x.name.clone()))
            .collect::<Vec<_>>();
        assert!(!pe.is_empty() && pe.iter().all(|n| n == "PE"), "{pe:?}");
        // ... and every coil's return pin A2 is on N.
        let netlist = Netlist::build(p);
        let returns: Vec<_> = p
            .symbols()
            .filter(|s| s.symbol.0.contains("/coils/"))
            .filter_map(|s| {
                let i = s.pins.iter().position(|x| x.name == "A2")?;
                netlist.potential_of(Endpoint::pin(s.id, i as u16))
            })
            .filter_map(|id| p.potential(id).map(|x| x.name.clone()))
            .collect();
        assert!(
            !returns.is_empty() && returns.iter().all(|n| n == "N"),
            "{returns:?}"
        );
        // Nothing is left open at the starter's entries: a second call adds nothing.
        let mut again = 0;
        doc.transact("again", |tx| {
            again = join_starter(tx, page, supply)?;
            Ok(())
        })
        .expect("again");
        assert_eq!(again, 0);
    }

    #[test]
    fn an_unrelated_macro_does_not_join_the_supply_to_a_starter() {
        let (mut doc, _) = starter_then_supply();
        let page = doc.project().pages().next().expect("page").id;
        let wires = doc.project().wires().count();
        let mut joined = 0;
        doc.transact("place", |tx| {
            let origin = novo_workflow::macro_place::free_origin(tx.project(), page, 20.0);
            let placed = place_macro(
                tx,
                macro_named("E-stop chain").definition,
                page,
                origin,
                &PlaceOptions::default(),
            )?;
            joined = join_starter(tx, page, placed.instance)?;
            Ok(())
        })
        .expect("place");
        assert_eq!(joined, 0);
        // The only new wires are the macro's own, none from the supply.
        let supply_outputs_open = doc
            .project()
            .symbols()
            .filter(|s| s.symbol.0.ends_with("mcb_3p"))
            .all(|s| {
                let i = s.pins.iter().position(|p| p.name == "2").expect("pin");
                let end = Endpoint::pin(s.id, i as u16);
                !doc.project().wires().any(|w| w.from == end || w.to == end)
            });
        assert!(supply_outputs_open);
        assert!(doc.project().wires().count() >= wires);
    }
}
