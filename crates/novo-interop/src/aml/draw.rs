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
    DeviceId, Direction, Endpoint, JunctionId, PageId, Pin, Placement, Point, Rotation, SymbolId,
    SymbolRole, TerminalId, TextAnchor, TextContent, TextPlacement, Tx, Wire,
};
use novo_symbols::{Library, Pack, Symbol};

use super::read::{Plant, PlantPin, PlantSymbol};
use crate::eplan::pxf::{PxfShape, ShapeKind, background_properties};
use crate::eplan::symbols::match_pins;

const BOX_SYMBOL: &str = "import/box";
const INTERRUPTION: &str = "iec/connectivity/interruption_point";
const INTERRUPTION_ID: &str = "connectivity/interruption_point";
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
        // A tap terminal: one feed side, two tap points.
        ("terminal", 3) => vec!["terminals/terminal_tap"],
        // The power poles of a contactor: a pair of pins per pole.
        ("power_poles", 2) => vec!["contactors/main_1p"],
        ("power_poles", 4) => vec!["contactors/main_2p"],
        ("power_poles", 6) => vec!["contactors/main_3p"],
        ("power_poles", 8) => vec!["contactors/main_4p"],
        // An overload relay's trip contact: 95/96 opens, 97/98 closes.
        ("thermal_switch", 2) => match last_digit {
            Some(7 | 8) => vec!["contacts/thermal_no"],
            _ => vec!["contacts/thermal_nc"],
        },
        ("motor", 2) => vec!["motors/motor_dc"],
        ("motor", 3) => vec!["motors/motor_1ph"],
        ("motor", 4) => vec!["motors/motor_3ph"],
        // With thermistor contacts T1 and T2.
        ("motor", 6) => vec!["motors/motor_3ph_pe_t2"],
        // Six windings ends and earth.
        ("motor", 7) => vec!["motors/motor_3ph_6t"],
        ("plug", 2) => vec!["terminals/plug"],
        ("plug", 4) => vec!["terminals/plug_3p_pe", "terminals/socket_3p_pe"],
        ("plug", 5) => vec!["terminals/plug_3p_n_pe", "terminals/socket_3p_n_pe"],
        ("lamp", 2) => vec!["signalling/lamp"],
        ("earth", 1) => vec!["earth/protective_earth"],
        ("pin" | "plug" | "generic" | "enclosure", 1) => vec!["connectivity/device_pin"],
        ("interruption", 1) => vec![INTERRUPTION_ID],
        _ => Vec::new(),
    }
}

/// What one placed symbol looks like in the project.
struct Drawn {
    key: String,
    placement: Placement,
    pins: Vec<Pin>,
    bridges: Vec<Vec<u16>>,
    pass_through: Vec<Vec<u16>>,
    /// KoperCAD pin of each pin of the file's symbol.
    pin_of: Vec<u16>,
    /// Page position of each pin of the file's symbol: where the file has it.
    pos: Vec<Point>,
    label: String,
    coil: bool,
}

/// How a library symbol sits on the file's pins: orientation and scale.
struct Fit {
    rotation: Rotation,
    mirrored: bool,
    scale: f64,
}

/// The inverse of mirroring then rotating, for an offset.
fn unplace(rotation: Rotation, mirrored: bool, v: Point) -> Point {
    let back = match rotation {
        Rotation::R0 => Rotation::R0,
        Rotation::R90 => Rotation::R270,
        Rotation::R180 => Rotation::R180,
        Rotation::R270 => Rotation::R90,
    };
    let w = back.apply(v);
    if mirrored { Point::new(-w.x, w.y) } else { w }
}

/// The orientation and scale that lay the library pins `lib` (offsets from
/// the first pin, with their directions) closest to the file's `file`
/// offsets, or `None` when no orientation faces the way the file's pins do
/// or the shapes differ too much to be the same symbol.
fn fit_pins(
    lib: &[(Point, Direction)],
    file: &[Point],
    faces: &[Option<Direction>],
) -> Option<Fit> {
    let span = file.iter().map(|p| p.x.hypot(p.y)).fold(0.0, f64::max);
    let mut best: Option<(usize, f64, Fit)> = None;
    for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
        for mirrored in [false, true] {
            let turn = Placement {
                origin: Point::new(0.0, 0.0),
                rotation,
                mirrored,
            };
            let at: Vec<Point> = lib.iter().map(|(p, _)| turn.apply(*p)).collect();
            let (mut dot, mut norm) = (0.0, 0.0);
            for (a, f) in at.iter().zip(file) {
                dot += a.x * f.x + a.y * f.y;
                norm += a.x * a.x + a.y * a.y;
            }
            let scale = if norm > 1e-9 {
                (dot / norm).clamp(0.25, 4.0)
            } else {
                1.0
            };
            let err = at
                .iter()
                .zip(file)
                .map(|(a, f)| (f.x - a.x * scale).hypot(f.y - a.y * scale))
                .fold(0.0, f64::max);
            let miss = lib
                .iter()
                .zip(faces)
                .filter(|((_, d), want)| want.is_some_and(|w| turn.apply_direction(*d) != w))
                .count();
            let better = best
                .as_ref()
                .is_none_or(|(m, e, _)| (miss, err) < (*m, *e - 1e-6));
            if better {
                best = Some((
                    miss,
                    err,
                    Fit {
                        rotation,
                        mirrored,
                        scale,
                    },
                ));
            }
        }
    }
    let (miss, err, fit) = best?;
    (miss == 0 && err <= 0.5 * span + 0.5).then_some(fit)
}

fn design(sym: &PlantSymbol, page_height: f64) -> Drawn {
    let lib = Library::shared(Pack::Iec);
    let (x, y) = sym.at;
    let page_pos = |p: &PlantPin| Point::new(x + p.offset.0, page_height - (y + p.offset.1));
    let spots: Vec<Point> = sym.pins.iter().map(page_pos).collect();
    let names: Vec<String> = sym.pins.iter().map(|p| p.name.clone()).collect();
    let named = names.iter().all(|n| !n.is_empty());
    let terminal = sym.kind == "terminal";
    // Where order is the only meaning: a terminal's two sides, a lamp, a
    // single-pin symbol.
    let positional_ok = terminal
        || matches!(
            sym.kind.as_str(),
            "lamp" | "earth" | "pin" | "plug" | "generic" | "enclosure" | "interruption"
        );
    let faces: Vec<Option<Direction>> = sym.pins.iter().map(|p| direction(&p.faces)).collect();
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
        // The library symbol on the file's pins: turned and scaled to lie on
        // them, so its pins are where the file has them.
        let lib_offsets: Vec<Point> = pins.iter().map(|p| p.offset).collect();
        let lib_at = |j: usize| lib_offsets[conns[j]];
        let lib_rel: Vec<(Point, Direction)> = (0..conns.len())
            .map(|j| {
                let (p, q) = (lib_at(j), lib_at(0));
                (
                    Point::new(p.x - q.x, p.y - q.y),
                    pins[conns[j]].direction.unwrap_or(Direction::Up),
                )
            })
            .collect();
        let file_rel: Vec<Point> = spots
            .iter()
            .map(|p| Point::new(p.x - spots[0].x, p.y - spots[0].y))
            .collect();
        let Some(fit) = fit_pins(&lib_rel, &file_rel, &faces) else {
            continue;
        };
        let turn = Placement {
            origin: Point::new(0.0, 0.0),
            rotation: fit.rotation,
            mirrored: fit.mirrored,
        };
        let base = lib_at(0);
        let scaled0 = Point::new(base.x * fit.scale, base.y * fit.scale);
        let moved = turn.apply(scaled0);
        let origin = Point::new(spots[0].x - moved.x, spots[0].y - moved.y);
        for j in 0..conns.len() {
            let l = lib_at(j);
            // The pin lies where the scaled artwork has it, plus what the
            // file's spacing does not explain (the artwork gets a lead).
            let want = turn.apply(Point::new(l.x * fit.scale, l.y * fit.scale));
            let at = Point::new(origin.x + want.x, origin.y + want.y);
            let off = unplace(
                fit.rotation,
                fit.mirrored,
                Point::new(spots[j].x - at.x, spots[j].y - at.y),
            );
            pins[conns[j]].offset = Point::new(l.x * fit.scale + off.x, l.y * fit.scale + off.y);
            if !sym.pins[j].name.is_empty() {
                pins[conns[j]].name = sym.pins[j].name.clone();
            }
        }
        return Drawn {
            key: format!("iec/{id}"),
            placement: Placement {
                origin,
                rotation: fit.rotation,
                mirrored: fit.mirrored,
            },
            pins,
            bridges,
            pass_through,
            pin_of: conns.iter().map(|&c| c as u16).collect(),
            pos: spots,
            label: String::new(),
            coil: id.starts_with("coils/"),
        };
    }
    // A box around the pins, with the pins where the file has them.
    let (mut lo, mut hi) = (spots[0], spots[0]);
    for p in &spots {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let origin = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
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
            let pin = Pin::new(name, spots[j].x - origin.x, spots[j].y - origin.y);
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
        placement: Placement::at(origin),
        pins,
        bridges,
        pass_through: Vec::new(),
        pin_of: (0..sym.pins.len() as u16).collect(),
        pos: spots,
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
            _ if sym.kind == "interruption" => SymbolRole::Interruption {
                name: sym.net.clone().unwrap_or_default(),
            },
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
        let id = tx.place_symbol(page, d.key.as_str(), d.placement, d.pins.clone(), role)?;
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
        let pos = d
            .pos
            .iter()
            .enumerate()
            .map(|(j, at)| {
                let kp = &d.pins[usize::from(d.pin_of[j])];
                (
                    *at,
                    kp.direction.map(|dir| d.placement.apply_direction(dir)),
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

    out.wires += draw_runs(tx, plant, &placed, pages, new_pages, &mut out.records)?;

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

    // The lines and rectangles are each page's background drawing.
    let mut by_page: BTreeMap<&str, Vec<PxfShape>> = BTreeMap::new();
    for sh in &plant.shapes {
        if new_pages.contains(&sh.page) && pages.contains_key(&sh.page) {
            by_page.entry(sh.page.as_str()).or_default().push(PxfShape {
                page: None,
                kind: if sh.rect {
                    ShapeKind::Rect { a: sh.a, b: sh.b }
                } else {
                    ShapeKind::Line { a: sh.a, b: sh.b }
                },
                pen: None,
                dashed: false,
            });
        }
    }
    for (name, shapes) in by_page {
        let (Some(&(_, page)), Some(props)) = (pages.get(name), background_properties(&shapes))
        else {
            continue;
        };
        // The shapes are y up from the bottom of the sheet; flip with this page's own height.
        let height = tx
            .project()
            .page(page)
            .map_or(297.0, |p| p.format.size_mm().1);
        tx.update::<novo_core::Page>(page, |p| {
            for (k, v) in &props {
                p.properties.insert((*k).to_string(), v.clone());
            }
            p.properties.insert(
                crate::underlay::KEY_HEIGHT.to_string(),
                novo_core::Value::Float(height),
            );
        })?;
        out.records += shapes.len();
    }

    let lib = Library::shared(Pack::Iec);
    let ip_symbol = lib.get(INTERRUPTION);
    let file_has_points = plant.symbols.iter().any(|s| s.kind == "interruption");
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
            for w in members.windows(2).filter(|_| !net.drawn) {
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
            let Some(&(m, mi)) = members
                .last()
                .filter(|_| pages_of_net > 1 && !(net.drawn && file_has_points))
            else {
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

/// A point on a page to a hundredth of a millimetre, to match run ends.
type Key = (i64, i64);

fn key(p: Point) -> Key {
    ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
}

/// Draws the wires the file gives as runs: each run's ends are exact pins,
/// corners or junctions, so a wire starts and ends where the file has it and
/// runs where the file has it. Runs that meet at a corner (a point where
/// exactly two runs meet and no pin sits) are one wire with a bend; three
/// or more make a junction.
fn draw_runs(
    tx: &mut Tx<'_>,
    plant: &Plant,
    placed: &[Option<Placed>],
    pages: &BTreeMap<String, (usize, PageId)>,
    new_pages: &BTreeSet<String>,
    records: &mut usize,
) -> Result<usize, novo_core::ModelError> {
    // Where every placed pin is, by page.
    let mut pins_at: BTreeMap<usize, BTreeMap<Key, Vec<(SymbolId, u16)>>> = BTreeMap::new();
    for pl in placed.iter().flatten() {
        for (pi, (at, _)) in pl.pos.iter().enumerate() {
            pins_at
                .entry(pl.page)
                .or_default()
                .entry(key(*at))
                .or_default()
                .push((pl.id, pl.pin_of[pi]));
        }
    }
    let mut count = 0usize;
    let mut by_page: BTreeMap<&str, Vec<&super::read::PlantSegment>> = BTreeMap::new();
    for s in &plant.segments {
        by_page.entry(s.page.as_str()).or_default().push(s);
    }
    for (name, segments) in by_page {
        let Some(&(page_index, page)) = pages.get(name) else {
            continue;
        };
        if !new_pages.contains(name) {
            continue;
        }
        let height = tx
            .project()
            .page(page)
            .map_or(297.0, |p| p.format.size_mm().1);
        let at = |p: (f64, f64)| Point::new(p.0, height - p.1);
        let here = pins_at.get(&page_index);
        let mut adj: BTreeMap<Key, BTreeSet<Key>> = BTreeMap::new();
        let mut pos: BTreeMap<Key, Point> = BTreeMap::new();
        for s in segments {
            let (a, b) = (at(s.from), at(s.to));
            let (ka, kb) = (key(a), key(b));
            if ka == kb {
                continue;
            }
            pos.entry(ka).or_insert(a);
            pos.entry(kb).or_insert(b);
            adj.entry(ka).or_default().insert(kb);
            adj.entry(kb).or_default().insert(ka);
        }
        let pins_of = |k: &Key| -> &[(SymbolId, u16)] {
            here.and_then(|m| m.get(k)).map_or(&[], Vec::as_slice)
        };
        let mut junctions: BTreeMap<Key, JunctionId> = BTreeMap::new();
        let made = std::cell::Cell::new(0usize);
        let mut end =
            |tx: &mut Tx<'_>, k: Key, n: usize| -> Result<Endpoint, novo_core::ModelError> {
                Ok(match pins_of(&k).first() {
                    Some(&(id, pin)) => Endpoint::pin(id, pin),
                    None if n >= 3 => {
                        let id = match junctions.get(&k) {
                            Some(&id) => id,
                            None => {
                                let id = tx.add_junction(page, pos[&k])?;
                                junctions.insert(k, id);
                                made.set(made.get() + 1);
                                id
                            }
                        };
                        Endpoint::Junction { junction: id }
                    }
                    None => Endpoint::Free { at: pos[&k] },
                })
            };
        let bend = |k: &Key| pins_of(k).is_empty() && adj.get(k).is_some_and(|n| n.len() == 2);
        let mut used: BTreeSet<(Key, Key)> = BTreeSet::new();
        let edge = |a: Key, b: Key| if a < b { (a, b) } else { (b, a) };
        let starts: Vec<Key> = adj.keys().copied().filter(|k| !bend(k)).collect();
        for t in starts {
            for first in adj[&t].clone() {
                if !used.insert(edge(t, first)) {
                    continue;
                }
                let mut route = Vec::new();
                let (mut prev, mut cur) = (t, first);
                let mut closed = false;
                while bend(&cur) {
                    route.push(pos[&cur]);
                    let next = adj[&cur].iter().copied().find(|k| *k != prev);
                    let Some(next) = next else { break };
                    if !used.insert(edge(cur, next)) {
                        closed = true;
                        break;
                    }
                    prev = cur;
                    cur = next;
                }
                if closed || bend(&cur) {
                    continue;
                }
                let a = end(tx, t, adj[&t].len())?;
                let b = end(tx, cur, adj[&cur].len())?;
                if a == b {
                    continue;
                }
                let id = tx.connect_on(page, a, b)?;
                let route = simplify(pos[&t], route, pos[&cur]);
                if !route.is_empty() {
                    tx.update::<Wire>(id, |x| x.route = route)?;
                }
                count += 1;
                *records += 1;
            }
        }
        *records += made.get();
        // Pins that touch (two pins at one point, wired to the first).
        if let Some(m) = here {
            for list in m.values().filter(|l| l.len() > 1) {
                for other in &list[1..] {
                    if *other != list[0] && other.0 != list[0].0 {
                        tx.connect_on(
                            page,
                            Endpoint::pin(list[0].0, list[0].1),
                            Endpoint::pin(other.0, other.1),
                        )?;
                        count += 1;
                        *records += 1;
                    }
                }
            }
        }
    }
    Ok(count)
}

/// The bends of a run from `from` to `to`: points on the straight line
/// between their neighbours are not bends.
fn simplify(from: Point, interior: Vec<Point>, to: Point) -> Vec<Point> {
    let mut all = vec![from];
    all.extend(interior);
    all.push(to);
    let mut i = 1;
    while i + 1 < all.len() {
        let (a, b, c) = (all[i - 1], all[i], all[i + 1]);
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        let along = (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y);
        if cross.abs() < 1e-6 && along >= 0.0 {
            all.remove(i);
        } else {
            i += 1;
        }
    }
    all[1..all.len() - 1].to_vec()
}
