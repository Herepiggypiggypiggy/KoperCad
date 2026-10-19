//! Drawing the pages of an AutomationML file's drawing profile: symbols
//! where the file puts them, wires from its nets.
//!
//! The file says which pins are one conductor, so nothing is rebuilt from
//! geometry: each net becomes wires between exactly its pins. A net that
//! reaches pins on several pages gets an interruption point of its name on
//! each page, which KoperCAD pairs by name.
//!
//! Exact: the page, the position and the pins of every symbol, and which
//! pins are one net. Inferred: the artwork (a library symbol whose pin names
//! match, else a labelled box) and the wire routes (plain right-angle runs).

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    DeviceId, Direction, Endpoint, PageId, Pin, Placement, Point, SymbolId, SymbolRole, TerminalId,
    TextAnchor, TextContent, TextPlacement, Tx, Wire,
};
use novo_symbols::{Library, Pack, Symbol};

use super::read::{Plant, PlantPin, PlantSymbol};
use crate::eplan::symbols::match_pins;

const BOX_SYMBOL: &str = "import/box";
const INTERRUPTION: &str = "iec/connectivity/interruption_point";
/// How far beside a pin a page's interruption point sits.
const IP_GAP: f64 = 10.0;
/// How far a second point on the same pin sits from the first.
const IP_STACK: f64 = 8.0;

fn direction(faces: &[String]) -> Option<Direction> {
    faces.first().and_then(|f| match f.as_str() {
        "up" => Some(Direction::Up),
        "down" => Some(Direction::Down),
        "left" => Some(Direction::Left),
        "right" => Some(Direction::Right),
        _ => None,
    })
}

fn to_direction(d: novo_symbols::Direction) -> Direction {
    match d {
        novo_symbols::Direction::Up => Direction::Up,
        novo_symbols::Direction::Down => Direction::Down,
        novo_symbols::Direction::Left => Direction::Left,
        novo_symbols::Direction::Right => Direction::Right,
    }
}

fn model_pins(symbol: &Symbol) -> Vec<Pin> {
    symbol
        .connections
        .iter()
        .map(|c| {
            let pin = Pin::new(c.id.clone(), c.at[0], c.at[1]).with_direction(to_direction(c.dir));
            if c.optional { pin.optional() } else { pin }
        })
        .collect()
}

fn model_groups(symbol: &Symbol) -> (Vec<Vec<u16>>, Vec<Vec<u16>>) {
    let to16 =
        |g: &Vec<usize>| -> Vec<u16> { g.iter().filter_map(|&i| u16::try_from(i).ok()).collect() };
    let bridges: Vec<Vec<u16>> = symbol.bridges().iter().map(to16).collect();
    let pass: Vec<Vec<u16>> = symbol
        .pass_through()
        .paths()
        .iter()
        .map(to16)
        .filter(|g| !bridges.contains(g))
        .collect();
    (bridges, pass)
}

/// KoperCAD symbols (IEC pack, without the prefix) a plain `kind` can be
/// drawn with, best first. Written for KoperCAD; the kinds are device words.
fn candidates(kind: &str, pins: &[PlantPin]) -> Vec<&'static str> {
    let last_digit = pins
        .first()
        .and_then(|p| p.name.chars().last())
        .and_then(|c| c.to_digit(10));
    match (kind, pins.len()) {
        ("coil", 2) => vec!["coils/contactor", "coils/relay"],
        ("contact", 2) => match last_digit {
            Some(1 | 2) => vec!["contacts/nc"],
            _ => vec!["contacts/no"],
        },
        ("contact", 3) => vec!["contacts/changeover"],
        ("terminal", 2) => vec!["terminals/terminal"],
        ("lamp", 2) => vec!["signalling/lamp"],
        ("earth", 1) => vec!["earth/protective_earth"],
        ("pin", 1) => vec!["connectivity/device_pin"],
        _ => Vec::new(),
    }
}

/// What one placed symbol looks like in the project.
struct Drawn {
    key: String,
    origin: Point,
    pins: Vec<Pin>,
    bridges: Vec<Vec<u16>>,
    pass_through: Vec<Vec<u16>>,
    /// KoperCAD pin of each pin of the file's symbol.
    pin_of: Vec<u16>,
    label: String,
    coil: bool,
}

fn design(sym: &PlantSymbol, page_height: f64) -> Drawn {
    let lib = Library::shared(Pack::Iec);
    let (x, y) = sym.at;
    let page_pos = |p: &PlantPin| Point::new(x + p.offset.0, page_height - (y + p.offset.1));
    let names: Vec<String> = sym.pins.iter().map(|p| p.name.clone()).collect();
    let named = names.iter().all(|n| !n.is_empty());
    let terminal = sym.kind == "terminal";
    // Where order is the only meaning: a terminal's two sides, a lamp, a
    // single-pin symbol.
    let positional_ok = terminal || matches!(sym.kind.as_str(), "lamp" | "earth" | "pin");
    for id in candidates(&sym.kind, &sym.pins) {
        let Some(s) = lib.get(&format!("iec/{id}")) else {
            continue;
        };
        if s.connections.len() != sym.pins.len() {
            continue;
        }
        // Pin names decide which pin is which; a symbol whose pins are
        // all positional is accepted only where order is the only meaning
        // (a terminal's two sides, a lamp).
        let conns: Vec<usize> = if named {
            let map = match_pins(&names, s, None);
            let by_name: Vec<Option<usize>> = names
                .iter()
                .map(|n| {
                    map.connection(n)
                        .and_then(|c| s.connections.iter().position(|x| x.id == c))
                })
                .collect();
            if map.is_complete()
                && (!map.positional || positional_ok)
                && by_name.iter().all(Option::is_some)
            {
                by_name.into_iter().flatten().collect()
            } else {
                continue;
            }
        } else if positional_ok {
            (0..sym.pins.len()).collect()
        } else {
            continue;
        };
        let mut distinct = BTreeSet::new();
        if !conns.iter().all(|c| distinct.insert(*c)) {
            continue;
        }
        let mut pins = model_pins(s);
        let (bridges, pass_through) = model_groups(s);
        let first = &sym.pins[0];
        let at = page_pos(first);
        let c0 = conns[0];
        let origin = Point::new(at.x - pins[c0].offset.x, at.y - pins[c0].offset.y);
        for (j, p) in sym.pins.iter().enumerate() {
            if !p.name.is_empty() {
                pins[conns[j]].name = p.name.clone();
            }
        }
        return Drawn {
            key: format!("iec/{id}"),
            origin,
            pins,
            bridges,
            pass_through,
            pin_of: conns.iter().map(|&c| c as u16).collect(),
            label: String::new(),
            coil: id.starts_with("coils/"),
        };
    }
    // A box with the pins where the file has them.
    let pins: Vec<Pin> = sym
        .pins
        .iter()
        .enumerate()
        .map(|(j, p)| {
            let name = if p.name.is_empty() {
                (j + 1).to_string()
            } else {
                p.name.clone()
            };
            let pin = Pin::new(name, p.offset.0, -p.offset.1);
            match direction(&p.faces) {
                Some(d) => pin.with_direction(d),
                None => pin,
            }
        })
        .collect();
    let bridges = if terminal {
        vec![(0..pins.len() as u16).collect()]
    } else {
        Vec::new()
    };
    Drawn {
        key: BOX_SYMBOL.to_string(),
        origin: Point::new(x, page_height - y),
        pins,
        bridges,
        pass_through: Vec::new(),
        pin_of: (0..sym.pins.len() as u16).collect(),
        label: String::new(),
        coil: false,
    }
}

/// Longest interruption point name drawn as the file gives it.
const SHORT_NAME: usize = 8;

/// Short, unique names for the interruption points of a file's nets, in net
/// order. The file's own name is kept when it is short; a long one (the
/// configurator writes a page path and title) would be printed beside every
/// point and over its neighbours, so the net gets a plain `N<number>` and
/// pairs by that.
fn point_names(nets: &[super::read::PlantNet]) -> Vec<String> {
    let short = |n: &str| n.chars().count() <= SHORT_NAME && !n.chars().any(char::is_whitespace);
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut next = 1usize;
    // Two nets of one name stay two nets: a point pairs by its name alone,
    // so the second gets `name-2`, and so on.
    let mut fresh = |base: Option<&str>, taken: &mut BTreeSet<String>| match base {
        Some(b) if taken.insert(b.to_string()) => b.to_string(),
        Some(b) => (2..)
            .map(|k| format!("{b}-{k}"))
            .find(|n| taken.insert(n.clone()))
            .unwrap_or_default(),
        None => loop {
            let name = format!("N{next}");
            next += 1;
            if taken.insert(name.clone()) {
                break name;
            }
        },
    };
    // Short names first, so a generated `N7` never takes a name a net has.
    let mut names: Vec<Option<String>> = vec![None; nets.len()];
    for (i, n) in nets.iter().enumerate() {
        if short(&n.name) {
            names[i] = Some(fresh(Some(&n.name), &mut taken));
        }
    }
    for (i, n) in nets.iter().enumerate() {
        if !short(&n.name) {
            names[i] = Some(fresh(None, &mut taken));
        }
    }
    names.into_iter().map(Option::unwrap_or_default).collect()
}

/// The nearest quarter turn of an angle in degrees.
fn quarter_turns(degrees: f64) -> novo_core::Rotation {
    use novo_core::Rotation;
    match (degrees / 90.0).round().rem_euclid(4.0) as u8 {
        1 => Rotation::R90,
        2 => Rotation::R180,
        3 => Rotation::R270,
        _ => Rotation::R0,
    }
}

/// A right-angle run from `a` to `b`, leaving `a` along its pin direction.
fn route(a: Point, b: Point, leave: Option<Direction>) -> Vec<Point> {
    if (a.x - b.x).abs() < 1e-6 || (a.y - b.y).abs() < 1e-6 {
        return Vec::new();
    }
    match leave {
        Some(Direction::Up | Direction::Down) => vec![Point::new(a.x, b.y)],
        _ => vec![Point::new(b.x, a.y)],
    }
}

/// A symbol that was placed: where its pins are on the page.
struct Placed {
    id: SymbolId,
    page: usize,
    page_id: PageId,
    pin_of: Vec<u16>,
    /// Page position and leaving direction of each file pin.
    pos: Vec<(Point, Option<Direction>)>,
}

/// What drawing created.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct DrawStats {
    pub records: usize,
    pub wires: usize,
    pub interruptions: usize,
}

/// Places the symbols of `plant` whose device is in `devices` (and the
/// device-less ones on the pages in `new_pages`) and wires the nets whose
/// pins were placed. `pages` maps the file's page names to project pages.
pub(super) fn draw(
    tx: &mut Tx<'_>,
    plant: &Plant,
    devices: &BTreeMap<usize, DeviceId>,
    terminals: &BTreeMap<(usize, String), TerminalId>,
    pages: &BTreeMap<String, (usize, PageId)>,
    new_pages: &BTreeSet<String>,
) -> Result<DrawStats, novo_core::ModelError> {
    let mut out = DrawStats::default();
    let mut placed: Vec<Option<Placed>> = Vec::new();
    let mut main_taken: BTreeSet<DeviceId> = BTreeSet::new();
    for sym in &plant.symbols {
        let device = sym.device.and_then(|d| devices.get(&d).copied());
        let wanted = match sym.device {
            Some(_) => device.is_some(),
            None => new_pages.contains(&sym.page),
        };
        let Some(&(page_index, page)) = pages.get(&sym.page).filter(|_| wanted) else {
            placed.push(None);
            continue;
        };
        if sym.pins.is_empty() {
            placed.push(None);
            continue;
        }
        // The file's y is up from the bottom of the sheet, whatever its size.
        let page_height = tx
            .project()
            .page(page)
            .map_or(297.0, |p| p.format.size_mm().1);
        let d = design(sym, page_height);
        let role = match (&sym.terminal, sym.device, device) {
            (Some(name), Some(di), _) => match terminals.get(&(di, name.clone())) {
                Some(&t) => SymbolRole::Terminal { terminals: vec![t] },
                None => SymbolRole::Unassigned,
            },
            (None, Some(_), Some(device)) => SymbolRole::Device {
                device,
                main: d.coil && main_taken.insert(device),
            },
            _ => SymbolRole::Unassigned,
        };
        let id = tx.place_symbol(
            page,
            d.key.as_str(),
            Placement::at(d.origin),
            d.pins.clone(),
            role,
        )?;
        if !d.bridges.is_empty() || !d.pass_through.is_empty() {
            tx.update::<novo_core::SymbolInstance>(id, |x| {
                x.bridges = d.bridges.clone();
                x.pass_through = d.pass_through.clone();
            })?;
        }
        if !d.label.is_empty() {
            tx.add_text(
                page,
                TextContent::Literal {
                    text: d.label.clone(),
                },
                TextPlacement {
                    anchor: TextAnchor::MiddleCenter,
                    ..Default::default()
                },
                Some(id.into()),
            )?;
            out.records += 1;
        }
        let pos = sym
            .pins
            .iter()
            .enumerate()
            .map(|(j, _)| {
                let kp = &d.pins[usize::from(d.pin_of[j])];
                (
                    Point::new(d.origin.x + kp.offset.x, d.origin.y + kp.offset.y),
                    kp.direction,
                )
            })
            .collect();
        placed.push(Some(Placed {
            id,
            page: page_index,
            page_id: page,
            pin_of: d.pin_of,
            pos,
        }));
        out.records += 1;
    }

    // The free texts of the pages this import creates.
    for t in &plant.texts {
        let Some(&(_, page)) = pages.get(&t.page).filter(|_| new_pages.contains(&t.page)) else {
            continue;
        };
        let height = tx
            .project()
            .page(page)
            .map_or(297.0, |p| p.format.size_mm().1);
        let id = tx.add_text(
            page,
            TextContent::Literal {
                text: t.value.clone(),
            },
            TextPlacement {
                offset: Point::new(t.at.0, height - t.at.1),
                anchor: TextAnchor::BottomLeft,
                rotation: quarter_turns(t.rotation),
            },
            None,
        )?;
        if let Some(h) = t.height {
            tx.set_text(id, None, Some(h))?;
        }
        out.records += 1;
    }

    let lib = Library::shared(Pack::Iec);
    let ip_symbol = lib.get(INTERRUPTION);
    let names = point_names(&plant.nets);
    // Every name a point has, so a chain name never equals another net's.
    let mut used: BTreeSet<String> = names.iter().cloned().collect();
    for (net, point_name) in plant.nets.iter().zip(names) {
        // The net's pins by page, only those that were placed.
        let mut by_page: BTreeMap<usize, Vec<(&Placed, usize)>> = BTreeMap::new();
        for &(si, pi) in &net.pins {
            if let Some(Some(p)) = placed.get(si)
                && pi < p.pin_of.len()
            {
                by_page.entry(p.page).or_default().push((p, pi));
            }
        }
        let pages_of_net = by_page.len();
        // The name of the link between page `between - 1` and `between`.
        let link_names: Vec<String> = (1..pages_of_net)
            .map(|between| {
                if pages_of_net == 2 {
                    return point_name.clone();
                }
                let mut name = format!("{point_name}.{between}");
                while !used.insert(name.clone()) {
                    name.push('\'');
                }
                name
            })
            .collect();
        for (rank, members) in by_page.values_mut().enumerate() {
            members.sort_by(|a, b| {
                let (pa, pb) = (a.0.pos[a.1].0, b.0.pos[b.1].0);
                pa.x.total_cmp(&pb.x).then(pa.y.total_cmp(&pb.y))
            });
            for w in members.windows(2) {
                let ((a, ai), (b, bi)) = (w[0], w[1]);
                if a.id == b.id && a.pin_of[ai] == b.pin_of[bi] {
                    continue;
                }
                let id = tx.connect(
                    Endpoint::pin(a.id, a.pin_of[ai]),
                    Endpoint::pin(b.id, b.pin_of[bi]),
                )?;
                let r = route(a.pos[ai].0, b.pos[bi].0, a.pos[ai].1);
                if !r.is_empty() {
                    tx.update::<Wire>(id, |x| x.route = r)?;
                }
                out.wires += 1;
                out.records += 1;
            }
            // A net on several pages continues from page to page: each page
            // has a point towards the previous and one towards the next, a
            // pair by name, so a point names one partner and not every page.
            let Some(&(m, mi)) = members.last().filter(|_| pages_of_net > 1) else {
                continue;
            };
            let link = |between: usize| link_names[between - 1].clone();
            let mut names = Vec::new();
            if rank > 0 {
                names.push(link(rank));
            }
            if rank + 1 < pages_of_net {
                names.push(link(rank + 1));
            }
            for (k, name) in names.into_iter().enumerate() {
                let page = m.page_id;
                let at = m.pos[mi].0;
                let (key, pins) = match ip_symbol {
                    Some(s) => (INTERRUPTION.to_string(), model_pins(s)),
                    None => (BOX_SYMBOL.to_string(), vec![Pin::new("1", 0.0, 0.0)]),
                };
                let off = pins.first().map_or(Point::new(0.0, 0.0), |p| p.offset);
                let origin = Point::new(at.x + IP_GAP - off.x, at.y - off.y + IP_STACK * k as f64);
                let ip = tx.place_symbol(
                    page,
                    key.as_str(),
                    Placement::at(origin),
                    pins,
                    SymbolRole::Interruption { name },
                )?;
                tx.connect(Endpoint::pin(m.id, m.pin_of[mi]), Endpoint::pin(ip, 0))?;
                out.interruptions += 1;
                out.wires += 1;
                out.records += 2;
            }
        }
    }
    Ok(out)
}
