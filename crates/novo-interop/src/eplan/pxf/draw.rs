//! Drawing the pages of a project export: symbols where EPLAN put them,
//! wires where EPLAN's rules join connection points.
//!
//! EPLAN stores few wires. This module places every symbol placement that
//! the embedded library explains (its connection points, their positions and
//! directions), rebuilds the nets by EPLAN's rules ([`crate::eplan::nets`])
//! and draws each net as wires between its points on each page.
//!
//! Exact: where each placement sits, how many connection points it has and
//! where they are, which points are one net (the rules' result), device and
//! terminal tags, interruption point names.
//! The artwork: a placement whose library symbol the export draws (lines,
//! arcs, circles, polygons, fixed texts) is drawn with that drawing, which
//! travels on the placed symbol (`novo_symbols::drawn`), and its pins sit
//! where EPLAN has them. Where a KoperCAD symbol of the same function and
//! pin count exists the placed symbol keeps its meaning (coil, contact,
//! bridges, simulation behaviour) but not its picture. A placement the
//! export has no drawing for falls back to that KoperCAD symbol (its pins
//! land where the symbol has them) or a labelled box. Wire routes and
//! junctions are EPLAN's own runs and nodes ([`super::runs`]). Interruption
//! points face any way.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Direction, Pin, Point, Rotation};
use novo_symbols::{Graphic, Library, Pack, Symbol, drawn};

use super::library::{DOWN, LEFT, LibPort, Match, RIGHT, UP};
use super::model::{DeviceRole, PageClass, PxfFile};
use super::runs::{RunEnd, RunPoint, wires_from_runs};
use crate::eplan::nets::{
    ConnGraph, ConnPoint, Dir, Interruption, NetIssue, PinKey, ReconNets, rebuild_nets,
};
use crate::eplan::symbols::{EplanSymbol, Port, SymbolMapper, Target, fit_geometry};

/// EPLAN is y up from the bottom of the sheet; KoperCAD is y down from the
/// top of an A3 landscape page.
pub(super) const PAGE_HEIGHT: f64 = 297.0;

pub(super) const BOX_SYMBOL: &str = "import/box";
const INTERRUPTION: &str = "iec/connectivity/interruption_point";

/// What a drawn symbol stands for in the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Role {
    /// A function of the device with this tag (as EPLAN writes it).
    Device {
        tag: String,
        coil: bool,
    },
    /// A terminal of a strip.
    Terminal {
        strip: String,
        name: String,
    },
    Interruption(String),
    Plain,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DrawnSymbol {
    /// Index into the file's pages.
    pub page: usize,
    pub key: String,
    pub origin: Point,
    pub pins: Vec<Pin>,
    pub bridges: Vec<Vec<u16>>,
    pub pass_through: Vec<Vec<u16>>,
    pub role: Role,
    /// Text shown beside a labelled box: the function the placement stands
    /// for. Empty for library symbols, which carry their own artwork.
    pub label: String,
    /// EPLAN's drawing of the symbol as `novo_symbols::drawn` text.
    pub art: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DrawnWire {
    pub page: usize,
    pub from: DrawnEnd,
    pub to: DrawnEnd,
    /// Corners between the ends: EPLAN's own nodes.
    pub route: Vec<Point>,
}

/// What a drawn wire ends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DrawnEnd {
    /// A pin of the drawn symbol (index, pin).
    Pin(usize, u16),
    /// An entry of [`Drawing::junctions`].
    Junction(usize),
}

/// What the drawing step did, for the report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrawStats {
    pub pages: usize,
    pub library_symbols: usize,
    pub box_symbols: usize,
    /// Placements drawn with EPLAN's own drawing of the symbol.
    pub art_symbols: usize,
    /// Of those, placements whose matrix turns or mirrors the symbol.
    pub art_turned: usize,
    /// Connection points drawn more than 0.05 mm from where the file has them.
    pub pins_off: usize,
    /// Connection points checked.
    pub pins_checked: usize,
    /// Function names of placements the export has no drawing for, with counts.
    pub no_art_functions: BTreeMap<String, usize>,
    /// Placements the embedded library could not explain: not drawn.
    pub unplaced: usize,
    /// Placements drawn although the library only loosely matched.
    pub loose: usize,
    /// Function names of the loosely matched placements, with counts.
    pub loose_functions: BTreeMap<String, usize>,
    pub interruptions: usize,
    pub nets: usize,
    pub wires: usize,
    /// Connection dots wires end on (T and cross nodes with three runs).
    pub junctions: usize,
    /// Nodes that carry no wire: stubs and dots with fewer than three runs.
    pub dots_dropped: usize,
    /// Function names drawn as boxes, with counts.
    pub box_functions: BTreeMap<String, usize>,
    /// The same, told apart by tag letters and pin designations in order.
    pub box_kinds: BTreeMap<(String, String, Vec<String>, usize), usize>,
    /// What the net rebuild found questionable, by kind.
    pub issues: BTreeMap<&'static str, usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Drawing {
    pub symbols: Vec<DrawnSymbol>,
    /// Connection dots at T and cross joints: page index and position.
    pub junctions: Vec<(usize, Point)>,
    pub wires: Vec<DrawnWire>,
    pub stats: DrawStats,
    /// The nets EPLAN's rules give, and which drawn pin each point is.
    pub recon: ReconNets,
    pub keys: BTreeMap<PinKey, (usize, u16)>,
}

fn dirs_of(bits: u8) -> Vec<Dir> {
    let mut v = Vec::new();
    for (bit, d) in [
        (UP, Dir::Up),
        (RIGHT, Dir::Right),
        (DOWN, Dir::Down),
        (LEFT, Dir::Left),
    ] {
        if bits & bit != 0 {
            v.push(d);
        }
    }
    v
}

/// The way a wire leaves a point, as seen on the page. EPLAN's bits name the
/// side a wire attaches from, and up is up in both programs: only the
/// coordinates flip (EPLAN y up, KoperCAD y down), the words do not.
fn direction_of(bits: u8) -> Option<Direction> {
    dirs_of(bits).first().map(|d| match d {
        Dir::Up => Direction::Up,
        Dir::Down => Direction::Down,
        Dir::Left => Direction::Left,
        Dir::Right => Direction::Right,
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

/// The KoperCAD symbol (IEC pack, without the pack prefix) a function of
/// the crane configurator's library stands for, by the words of its name.
/// Written for KoperCAD; the names are plain device words.
fn by_name(function: &str) -> Option<&'static str> {
    let f = function.to_lowercase();
    // (needle, must not contain, symbol)
    const ROWS: &[(&str, &str, &str)] = &[
        ("delayed closing", "", "contacts/no_delay_on"),
        ("delayed opening", "", "contacts/no_delay_off"),
        ("thermal release", "nc", "contacts/thermal_nc"),
        ("limit switch, nc", "", "contacts/position_nc"),
        ("limit switch, no", "", "contacts/position_no"),
        ("limit switch, change-over", "", "contacts/changeover"),
        ("change-over", "", "contacts/changeover"),
        ("power no contact", "", "contactors/main_1p"),
        ("power nc contact", "", "contacts/nc"),
        ("nc auxiliary contact", "", "contacts/nc"),
        ("no auxiliary contact", "", "contacts/no"),
        ("coil for power contactor", "", "coils/contactor"),
        ("coil, general", "", "coils/relay"),
        ("lamp", "", "signalling/lamp"),
        ("brake", "", "coils/brake"),
        ("safety fuse", "", "protection/fuse"),
        ("rectifier", "", "passive/bridge_rectifier"),
        ("double circuit breaker", "", "protection/mcb_2p"),
        (
            "ground fault current circuit breaker, two-pole",
            "",
            "protection/rcd_2p",
        ),
        (
            "ground fault current circuit breaker, four-pole",
            "",
            "protection/rcd_4p",
        ),
        (
            "motor overload switch three-pole",
            "",
            "protection/motor_protection_breaker",
        ),
        ("transformer", "", "transformers/transformer_1ph"),
    ];
    ROWS.iter()
        .find(|(needle, not, _)| f.contains(needle) && (not.is_empty() || !f.contains(not)))
        .map(|r| r.2)
}

/// Longest interruption point name drawn as the file gives it.
const SHORT_POINT_NAME: usize = 8;

/// The name an interruption point carries in the file: its tag, else its text.
fn point_name(ip: &crate::eplan::pxf::PxfInterruption) -> String {
    if ip.tag.is_empty() {
        ip.text.clone()
    } else {
        ip.tag.full()
    }
}

/// The name each interruption point of the drawn pages is shown with. EPLAN
/// names a point after its whole destination path, and a name that long is
/// printed beside every point, over its neighbours; a long name (or one with
/// spaces) gets a plain `N<number>` instead. The mapping is one to one, so
/// KoperCAD pairs the points exactly as the file's own names do. Short names
/// are kept and never taken by a generated one.
fn shown_point_names(file: &PxfFile, drawn: &BTreeSet<usize>) -> BTreeMap<String, String> {
    let mut names: Vec<String> = Vec::new();
    for ip in &file.interruptions {
        if !ip.page.is_some_and(|p| drawn.contains(&p)) {
            continue;
        }
        let name = point_name(ip);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let short =
        |n: &str| n.chars().count() <= SHORT_POINT_NAME && !n.chars().any(char::is_whitespace);
    let mut out: BTreeMap<String, String> = names
        .iter()
        .filter(|n| short(n))
        .map(|n| (n.clone(), n.clone()))
        .collect();
    let mut taken: BTreeSet<String> = out.values().cloned().collect();
    let mut next = 1usize;
    for n in names.iter().filter(|n| !short(n)) {
        let fresh = loop {
            let c = format!("N{next}");
            next += 1;
            if taken.insert(c.clone()) {
                break c;
            }
        };
        out.insert(n.clone(), fresh);
    }
    out
}

struct Placed {
    symbol: DrawnSymbol,
    /// Where each KoperCAD pin sits on the page.
    page_pos: BTreeMap<u16, Point>,
}

/// Builds the drawing of every circuit-diagram page of `file`.
pub(super) fn draw(file: &PxfFile, mapper: &SymbolMapper, only: &BTreeSet<usize>) -> Drawing {
    let lib = Library::shared(Pack::Iec);
    let mut out = Drawing::default();
    let mut graph = ConnGraph::default();
    let mut placed: Vec<Placed> = Vec::new();
    let mut by_key: BTreeMap<PinKey, (usize, u16, usize)> = BTreeMap::new();
    let drawn_pages: BTreeSet<usize> = file
        .pages
        .iter()
        .enumerate()
        .filter(|(i, p)| p.class() == PageClass::Schematic && only.contains(i))
        .map(|(i, _)| i)
        .collect();
    out.stats.pages = drawn_pages.len();

    for (di, d) in file.devices.iter().enumerate() {
        let (Some(page), Some((x, y)), Some(sref)) = (d.page, d.position, d.symbol) else {
            continue;
        };
        if !drawn_pages.contains(&page) || d.role == DeviceRole::Cable {
            continue;
        }
        let key3 = (sref.number, sref.variant, sref.library);
        let found =
            file.library
                .symbol(d.function_def, d.library_id, key3, d.variant, d.pins.len());
        let Some((lsym, quality)) = found else {
            out.stats.unplaced += 1;
            continue;
        };
        let Some(ports) = lsym.variants.get(&(d.variant as u32)) else {
            out.stats.unplaced += 1;
            continue;
        };
        let function = file
            .library
            .function(key3)
            .map(|f| f.name.get("en_US").to_string())
            .unwrap_or_default();
        if quality > Match::SameFamily {
            out.stats.loose += 1;
            *out.stats
                .loose_functions
                .entry(function.clone())
                .or_default() += 1;
        }
        let owner = format!("d{di}");
        let role = match d.role {
            DeviceRole::Terminal => Role::Terminal {
                strip: d.tag.full(),
                name: d.terminal_text.trim().to_string(),
            },
            _ if d.tag.is_empty() => Role::Plain,
            _ => Role::Device {
                tag: d.tag.full(),
                coil: false,
            },
        };
        let conductor = match &role {
            Role::Terminal { strip, name } => Some(format!("T:{strip}:{name}")),
            _ => None,
        };
        // The variant's own drawing and points, turned by the placement's
        // matrix (the identity in every file seen so far).
        let turned = d.matrix != [1.0, 0.0, 0.0, 1.0];
        let moved: Vec<LibPort> = ports.iter().map(|p| turn_port(p, d.matrix)).collect();
        let ports: &[LibPort] = &moved;
        let art_g: Option<Vec<Graphic>> = lsym
            .art
            .get(&(d.variant as u32))
            .filter(|a| !a.is_empty())
            .map(|a| turn_art(a, d.matrix));
        let art_text = art_g.as_deref().map(drawn::encode);
        if art_g.is_none() {
            *out.stats
                .no_art_functions
                .entry(if function.is_empty() {
                    "no function name".to_string()
                } else {
                    function.clone()
                })
                .or_default() += 1;
        }
        if art_g.is_some() {
            out.stats.art_symbols += 1;
            if turned {
                out.stats.art_turned += 1;
            }
        }
        let pin_names: Vec<String> = (0..ports.len())
            .map(|i| d.pins.get(i).cloned().unwrap_or_default())
            .collect();

        // Artwork: a library symbol of the same pin count, else a box.
        let (lib_symbol, port_conn) = pick_library_symbol(
            lib,
            mapper,
            &function,
            &d.tag.letters,
            &pin_names,
            ports,
            matches!(role, Role::Terminal { .. }),
        );
        let origin_pos = |first: &LibPort| Point::new(x + first.x, PAGE_HEIGHT - (y + first.y));
        let first = ports.first().copied().unwrap_or(LibPort {
            index: 0,
            x: 0.0,
            y: 0.0,
            dirs: 0,
        });
        let mut box_label = String::new();
        let (key, origin, pins, bridges, pass_through, pin_of_port) = match lib_symbol {
            Some(sym) => {
                let pins = model_pins(sym);
                let (bridges, pass) = model_groups(sym);
                let c0 = port_conn[0];
                let at = origin_pos(&first);
                let mut pins = pins;
                let origin = if art_g.is_some() {
                    // EPLAN's own pins: the symbol sits at its insertion point.
                    for (j, p) in ports.iter().enumerate() {
                        let pin = &mut pins[port_conn[j]];
                        pin.offset = Point::new(p.x, -p.y);
                        if let Some(dir) = direction_of(p.dirs) {
                            pin.direction = Some(dir);
                        }
                    }
                    Point::new(x, PAGE_HEIGHT - y)
                } else {
                    Point::new(at.x - pins[c0].offset.x, at.y - pins[c0].offset.y)
                };
                let mut map = BTreeMap::new();
                for (j, p) in ports.iter().enumerate() {
                    map.insert(p.index, port_conn[j] as u16);
                    if !pin_names[j].is_empty() {
                        pins[port_conn[j]].name = pin_names[j].clone();
                    }
                }
                out.stats.library_symbols += 1;
                (
                    format!("iec/{}", sym.id.trim_start_matches("iec/")),
                    origin,
                    pins,
                    bridges,
                    pass,
                    map,
                )
            }
            None => {
                let origin = Point::new(x, PAGE_HEIGHT - y);
                let mut map = BTreeMap::new();
                let pins: Vec<Pin> = ports
                    .iter()
                    .enumerate()
                    .map(|(j, p)| {
                        map.insert(p.index, j as u16);
                        let name = if pin_names[j].is_empty() {
                            (j + 1).to_string()
                        } else {
                            pin_names[j].clone()
                        };
                        let pin = Pin::new(name, p.x, -p.y);
                        match direction_of(p.dirs) {
                            Some(dir) => pin.with_direction(dir),
                            None => pin,
                        }
                    })
                    .collect();
                let bridges = if conductor.is_some() {
                    vec![(0..pins.len() as u16).collect()]
                } else {
                    Vec::new()
                };
                out.stats.box_symbols += 1;
                let label = if function.is_empty() {
                    "no function name".to_string()
                } else {
                    function.clone()
                };
                *out.stats.box_functions.entry(label).or_default() += 1;
                if art_g.is_none() {
                    box_label = box_caption(&function, d.type_text.get("en_US"), &d.part_number);
                }
                *out.stats
                    .box_kinds
                    .entry((
                        function.clone(),
                        d.tag.letters.clone(),
                        d.pins.clone(),
                        ports.len(),
                    ))
                    .or_default() += 1;
                (
                    BOX_SYMBOL.to_string(),
                    origin,
                    pins,
                    bridges,
                    Vec::new(),
                    map,
                )
            }
        };

        let role = match role {
            Role::Device { tag, .. } => Role::Device {
                coil: key.starts_with("iec/coils/"),
                tag,
            },
            r => r,
        };
        let idx = placed.len();
        let mut page_pos = BTreeMap::new();
        for p in ports {
            let pk = PinKey(format!("{owner}:{}", p.index));
            let mut cp = ConnPoint::pin(&pk.0, page as u32, x + p.x, y + p.y, Dir::Up);
            cp.facing = dirs_of(p.dirs);
            cp.owner = Some(owner.clone());
            cp.conductor = conductor.clone();
            graph.points.push(cp);
            let pin = pin_of_port[&p.index];
            by_key.insert(pk, (idx, pin, page));
            let kp = &pins[pin as usize];
            let at = Point::new(origin.x + kp.offset.x, origin.y + kp.offset.y);
            let want = Point::new(x + p.x, PAGE_HEIGHT - (y + p.y));
            out.stats.pins_checked += 1;
            if (at.x - want.x).hypot(at.y - want.y) > 0.05 {
                out.stats.pins_off += 1;
            }
            page_pos.insert(pin, at);
        }
        placed.push(Placed {
            symbol: DrawnSymbol {
                page,
                key,
                origin,
                pins,
                bridges,
                pass_through,
                role,
                label: box_label,
                art: art_text,
            },
            page_pos,
        });
    }

    // Connection nodes: legs by kind, found by looking at the drawings.
    for (ni, n) in file.nodes.iter().enumerate() {
        let (Some(page), Some((x, y))) = (n.page, n.position) else {
            continue;
        };
        if !drawn_pages.contains(&page) {
            continue;
        }
        let legs: &[Dir] = match (n.kind, n.variant) {
            (64, _) => &[Dir::Right, Dir::Down, Dir::Left],
            (66, _) => &[Dir::Up, Dir::Right, Dir::Down],
            (67, _) => &[Dir::Up, Dir::Right, Dir::Down, Dir::Left],
            (72, _) => &[Dir::Up, Dir::Right, Dir::Left],
            (70, 0) => &[Dir::Right, Dir::Down],
            (70, 1) => &[Dir::Up, Dir::Right],
            (70, 2) => &[Dir::Up, Dir::Left],
            (70, 3) => &[Dir::Down, Dir::Left],
            _ => &[Dir::Up, Dir::Right, Dir::Down, Dir::Left],
        };
        graph
            .points
            .push(ConnPoint::node(&format!("n{ni}"), page as u32, x, y, legs));
    }

    // Interruption points: one connection that faces any way, so whatever
    // faces it joins it.
    let lib_ip = lib.get(INTERRUPTION);
    let shown = shown_point_names(file, &drawn_pages);
    for (ii, ip) in file.interruptions.iter().enumerate() {
        let (Some(page), Some((x, y))) = (ip.page, ip.position) else {
            continue;
        };
        if !drawn_pages.contains(&page) {
            continue;
        }
        let name = point_name(ip);
        let pk = PinKey(format!("i{ii}"));
        let mut cp = ConnPoint::pin(&pk.0, page as u32, x, y, Dir::Up);
        cp.facing = vec![Dir::Up, Dir::Right, Dir::Down, Dir::Left];
        cp.owner = Some(pk.0.clone());
        graph.points.push(cp);
        graph.interruptions.push(Interruption {
            point: pk.clone(),
            name: name.clone(),
            seq: ii as u32,
        });
        let (pins, bridges, pass, key) = match lib_ip {
            Some(s) => {
                let (b, p) = model_groups(s);
                (model_pins(s), b, p, INTERRUPTION.to_string())
            }
            None => (
                vec![Pin::new("1", 0.0, 0.0)],
                Vec::new(),
                Vec::new(),
                BOX_SYMBOL.to_string(),
            ),
        };
        let off = pins.first().map_or(Point::new(0.0, 0.0), |p| p.offset);
        let origin = Point::new(x - off.x, PAGE_HEIGHT - y - off.y);
        let idx = placed.len();
        by_key.insert(pk, (idx, 0, page));
        placed.push(Placed {
            symbol: DrawnSymbol {
                page,
                key,
                origin,
                pins,
                bridges,
                pass_through: pass,
                role: Role::Interruption(shown.get(&name).cloned().unwrap_or(name)),
                label: String::new(),
                art: None,
            },
            page_pos: BTreeMap::from([(0, Point::new(x, PAGE_HEIGHT - y))]),
        });
        out.stats.interruptions += 1;
    }

    // Nets, then wires per page.
    let nets = rebuild_nets(&graph);
    for issue in &nets.issues {
        let what = match issue {
            NetIssue::UnpairedInterruption { .. } => "interruption points without a partner",
            NetIssue::AmbiguousInterruption { .. } => "interruption names used more than twice",
            NetIssue::DanglingLeg { .. } => "node legs that reach nothing",
            NetIssue::DanglingLineEnd { .. } => "line ends that touch nothing",
            NetIssue::OpenPin { .. } => "connection points that face a direction and join nothing",
            NetIssue::BlockedRun { .. } => "connection runs stopped by another point",
            NetIssue::SkewLine { .. } => "slanted lines",
            NetIssue::DuplicateKey { .. } => "duplicate point keys",
            NetIssue::UnknownPin { .. } => "unknown point keys",
        };
        *out.stats.issues.entry(what).or_default() += 1;
    }
    out.stats.nets = nets.connected_nets();

    // Wires are the runs EPLAN implies, through its nodes; T and cross nodes
    // become junctions the wires end on.
    let mut run_points: Vec<RunPoint> = Vec::with_capacity(graph.points.len());
    let mut run_pin: Vec<Option<(usize, u16)>> = Vec::with_capacity(graph.points.len());
    for p in &graph.points {
        let (at, node, leaves, pin) = match by_key.get(&p.key) {
            Some(&(si, pin, _)) if !p.node => {
                let Some(&at) = placed[si].page_pos.get(&pin) else {
                    continue;
                };
                let leaves = placed[si].symbol.pins[pin as usize].direction;
                (at, false, leaves, Some((si, pin)))
            }
            _ if p.node => (Point::new(p.at.x, PAGE_HEIGHT - p.at.y), true, None, None),
            _ => continue,
        };
        run_points.push(RunPoint {
            key: p.key.clone(),
            page: p.page as usize,
            at,
            node,
            leaves,
        });
        run_pin.push(pin);
    }
    let built = wires_from_runs(&run_points, &nets.runs);
    out.junctions = built.junctions;
    out.stats.junctions = out.junctions.len();
    out.stats.dots_dropped = built.dropped_nodes;
    let end = |e: RunEnd| match e {
        RunEnd::Pin(i) => run_pin[i].map(|(si, pin)| DrawnEnd::Pin(si, pin)),
        RunEnd::Junction(j) => Some(DrawnEnd::Junction(j)),
    };
    for w in built.wires {
        if let (Some(from), Some(to)) = (end(w.from), end(w.to)) {
            out.wires.push(DrawnWire {
                page: w.page,
                from,
                to,
                route: w.route,
            });
        }
    }
    out.stats.wires = out.wires.len();
    out.keys = by_key
        .iter()
        .map(|(k, &(si, pin, _))| (k.clone(), (si, pin)))
        .collect();
    out.recon = nets;
    out.symbols = placed.into_iter().map(|p| p.symbol).collect();
    out
}

/// A library symbol that can stand for the placement, with which of its
/// connections each EPLAN port takes (in port order); `None` when no symbol
/// of the same pin count fits and the placement becomes a box.
fn pick_library_symbol(
    lib: &'static Library,
    mapper: &SymbolMapper,
    function: &str,
    letters: &str,
    pin_names: &[String],
    ports: &[LibPort],
    terminal: bool,
) -> (Option<&'static Symbol>, Vec<usize>) {
    let named = pin_names.iter().all(|p| !p.is_empty());
    let id = if terminal {
        (ports.len() == 2).then_some("terminals/terminal")
    } else {
        by_name(function)
    };
    let mut candidates: Vec<&'static Symbol> = Vec::new();
    if let Some(id) = id
        && let Some(s) = lib.get(&format!("iec/{id}"))
    {
        candidates.push(s);
    }
    if !function.is_empty() {
        let source = EplanSymbol {
            key: None,
            function: Some(function.to_string()),
            letters: Some(letters.to_string()),
            pins: if named {
                pin_names.to_vec()
            } else {
                Vec::new()
            },
        };
        // The point count tells the members of a family apart (the variable
        // motor, the terminal with 3 points), then the plain name.
        let resolved = mapper.map_fitting(&source, ports.len()).or_else(|| {
            Some(mapper.map(&source)).filter(|r| r.basis == crate::eplan::symbols::Basis::Name)
        });
        if let Some(r) = resolved
            && let Target::Library(id) = &r.target
            && let Some(s) = lib.get(id)
        {
            candidates.push(s);
        }
    }
    for s in candidates {
        if s.connections.len() != ports.len() {
            continue;
        }
        // Where the pin names say nothing in KoperCAD (`48Vac`, `0V48`), the
        // arrangement of the points does: the primary of a transformer is on
        // one side, the secondary on the other. Only an unturned, unmirrored
        // fit is used, since the symbol is placed as it is.
        let by_position = || -> Vec<usize> {
            if ports.len() >= 3 {
                let points: Vec<Port> = ports
                    .iter()
                    .map(|p| Port {
                        // EPLAN's y is up; the fit's is down.
                        at: Point::new(p.x, -p.y),
                        dir: direction_of(p.dirs),
                    })
                    .collect();
                if let Some(f) = fit_geometry(&points, s)
                    && f.rotation == Rotation::R0
                    && !f.mirrored
                {
                    return f.connection;
                }
            }
            (0..ports.len()).collect()
        };
        let conns: Vec<usize> = if named {
            let map = crate::eplan::symbols::match_pins(pin_names, s, None);
            if map.is_complete() && !map.positional {
                let by_id: Vec<Option<usize>> = pin_names
                    .iter()
                    .map(|p| {
                        map.connection(p)
                            .and_then(|c| s.connections.iter().position(|x| x.id == c))
                    })
                    .collect();
                if by_id.iter().all(Option::is_some) {
                    by_id.into_iter().flatten().collect()
                } else {
                    by_position()
                }
            } else {
                by_position()
            }
        } else {
            by_position()
        };
        let distinct: BTreeSet<usize> = conns.iter().copied().collect();
        if distinct.len() == ports.len() {
            return (Some(s), conns);
        }
    }
    (None, Vec::new())
}

/// The text beside a labelled box. A black box has no function worth showing,
/// so the placement's own words come first: the first line of its type text,
/// then its part number; the function name is the last resort.
fn box_caption(function: &str, type_text: &str, part_number: &str) -> String {
    let type_line = type_text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    if !type_line.is_empty() {
        type_line.to_string()
    } else if !part_number.is_empty() {
        part_number.to_string()
    } else {
        function.to_string()
    }
}

/// A connection point turned by a placement matrix (`[a, b, c, d]`, y up).
fn turn_port(p: &LibPort, m: [f64; 4]) -> LibPort {
    if m == [1.0, 0.0, 0.0, 1.0] {
        return *p;
    }
    let (x, y) = (m[0] * p.x + m[1] * p.y, m[2] * p.x + m[3] * p.y);
    let mut dirs = 0u8;
    for (bit, v) in [
        (UP, (0.0, 1.0)),
        (RIGHT, (1.0, 0.0)),
        (DOWN, (0.0, -1.0)),
        (LEFT, (-1.0, 0.0)),
    ] {
        if p.dirs & bit != 0 {
            let (vx, vy) = (m[0] * v.0 + m[1] * v.1, m[2] * v.0 + m[3] * v.1);
            dirs |= if vx.abs() >= vy.abs() {
                if vx >= 0.0 { RIGHT } else { LEFT }
            } else if vy >= 0.0 {
                UP
            } else {
                DOWN
            };
        }
    }
    LibPort { x, y, dirs, ..*p }
}

/// A symbol drawing (y down) turned by a placement matrix (y up). Curves
/// become polylines when the matrix is not the identity.
fn turn_art(art: &[Graphic], m: [f64; 4]) -> Vec<Graphic> {
    if m == [1.0, 0.0, 0.0, 1.0] {
        return art.to_vec();
    }
    // The matrix in y-down coordinates.
    let (a, b, c, d) = (m[0], -m[1], -m[2], m[3]);
    let t = |p: [f64; 2]| [a * p[0] + b * p[1], c * p[0] + d * p[1]];
    let scale = (a * d - b * c).abs().sqrt();
    let mut out = Vec::with_capacity(art.len());
    for g in art {
        out.push(match g {
            Graphic::Line { from, to, stroke } => Graphic::Line {
                from: t(*from),
                to: t(*to),
                stroke: *stroke,
            },
            Graphic::Polyline {
                points,
                closed,
                fill,
                stroke,
            } => Graphic::Polyline {
                points: points.iter().map(|p| t(*p)).collect(),
                closed: *closed,
                fill: *fill,
                stroke: *stroke,
            },
            Graphic::Rect {
                at,
                size,
                fill,
                stroke,
            } => Graphic::Polyline {
                points: [
                    *at,
                    [at[0] + size[0], at[1]],
                    [at[0] + size[0], at[1] + size[1]],
                    [at[0], at[1] + size[1]],
                ]
                .iter()
                .map(|p| t(*p))
                .collect(),
                closed: true,
                fill: *fill,
                stroke: *stroke,
            },
            Graphic::Circle {
                center,
                r,
                fill,
                stroke,
            } => Graphic::Circle {
                center: t(*center),
                r: r * scale,
                fill: *fill,
                stroke: *stroke,
            },
            Graphic::Ellipse {
                center,
                rx,
                ry,
                fill,
                stroke,
            } => Graphic::Ellipse {
                center: t(*center),
                rx: rx * scale,
                ry: ry * scale,
                fill: *fill,
                stroke: *stroke,
            },
            Graphic::Arc {
                center,
                r,
                start,
                end,
                stroke,
            } => Graphic::Polyline {
                points: (0..=24)
                    .map(|i| {
                        let u = (start + (end - start) * f64::from(i) / 24.0).to_radians();
                        t([center[0] + r * u.cos(), center[1] + r * u.sin()])
                    })
                    .collect(),
                closed: false,
                fill: false,
                stroke: *stroke,
            },
            Graphic::Text {
                at,
                text,
                size,
                align,
            } => Graphic::Text {
                at: t(*at),
                text: text.clone(),
                size: *size,
                align: *align,
            },
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::library::{DOWN, UP};
    use super::*;
    use crate::eplan::pxf::PxfInterruption;
    use crate::eplan::symbols::SymbolMapper;
    use novo_symbols::{Library, Pack};

    fn point(page: usize, text: &str) -> PxfInterruption {
        PxfInterruption {
            id: None,
            page: Some(page),
            tag: Default::default(),
            text: text.to_string(),
            position: Some((10.0, 10.0)),
            variant: 0,
        }
    }

    #[test]
    fn a_box_is_captioned_by_the_placements_own_words_first() {
        assert_eq!(
            box_caption("Black box", "\n Safety relay\n230 V", "MAKER.RELAY"),
            "Safety relay"
        );
        assert_eq!(box_caption("Black box", "", "MAKER.RELAY"), "MAKER.RELAY");
        assert_eq!(box_caption("Black box", " ", ""), "Black box");
    }

    fn port(index: u32, x: f64, y: f64, dirs: u8) -> LibPort {
        LibPort { index, x, y, dirs }
    }

    #[test]
    fn a_transformer_with_pe_is_fitted_by_where_its_points_sit() {
        // Listed secondary, primary, secondary, primary, earth, as a
        // crane drawing has them (y up: the secondary and the earth point at
        // the bottom, leaving downward); the designations (`48Vac`, `0V48`)
        // mean nothing in KoperCAD, the arrangement does.
        let lib = Library::shared(Pack::Iec);
        let mapper = SymbolMapper::new(Pack::Iec);
        let ports = [
            port(0, 0.0, -8.0, DOWN),
            port(1, 0.0, 8.0, UP),
            port(2, 16.0, -8.0, DOWN),
            port(3, 16.0, 8.0, UP),
            port(4, 24.0, -8.0, DOWN),
        ];
        let pins: Vec<String> = ["48Vac", "L1", "0V48", "L2", "PE"]
            .map(String::from)
            .to_vec();
        let (symbol, conns) = pick_library_symbol(
            lib,
            &mapper,
            "Transformer, 5 connection points with PE",
            "T",
            &pins,
            &ports,
            false,
        );
        let symbol = symbol.expect("the library has a transformer with PE");
        assert_eq!(symbol.id, "iec/transformers/transformer_1ph_pe");
        let ids: Vec<&str> = conns
            .iter()
            .map(|&j| symbol.connections[j].id.as_str())
            .collect();
        assert_eq!(ids, ["2.1", "1.1", "2.2", "1.2", "PE"]);
    }

    #[test]
    fn a_long_point_name_is_shown_short_and_pairs_one_to_one() {
        let long = "=A1+CAB1/14 Hoist motors slow / fast.7";
        let mut file = crate::eplan::pxf::read_pxf_str(include_str!(
            "../../../tests/fixtures/pxf/synthetic-small.epj"
        ))
        .unwrap();
        file.interruptions = vec![
            point(0, long),
            point(1, long),
            point(0, "L1"),
            point(0, "N1"),
            point(0, "=B2+CAB2+W9_W2/PE"),
            // On a page that is not drawn: not named at all.
            point(5, "=99X99+Z/Y"),
        ];
        let drawn: BTreeSet<usize> = [0, 1].into();
        let shown = shown_point_names(&file, &drawn);
        // Short names stay, and no generated name takes one of them.
        assert_eq!(shown["L1"], "L1");
        assert_eq!(shown["N1"], "N1");
        assert_eq!(
            shown[long], "N2",
            "N1 is a real name, so the first free one"
        );
        assert_eq!(shown["=B2+CAB2+W9_W2/PE"], "N3");
        assert_eq!(shown.len(), 4);
        // Both ends of the long name show the same short one.
        let ends: BTreeSet<&String> = file.interruptions[..2]
            .iter()
            .map(|p| &shown[&p.text])
            .collect();
        assert_eq!(ends.len(), 1);
    }

    #[test]
    fn a_quarter_turn_moves_points_and_the_way_they_face() {
        // 90 degrees counter-clockwise, y up: (1, 0) -> (0, 1), right -> up.
        let p = LibPort {
            index: 0,
            x: 2.0,
            y: 0.0,
            dirs: RIGHT,
        };
        let t = turn_port(&p, [0.0, -1.0, 1.0, 0.0]);
        assert_eq!((t.x, t.y, t.dirs), (0.0, 2.0, UP));
        assert_eq!(turn_port(&p, [1.0, 0.0, 0.0, 1.0]), p);
        // A mirror in x turns a point facing right to face left.
        let m = turn_port(&p, [-1.0, 0.0, 0.0, 1.0]);
        assert_eq!((m.x, m.dirs), (-2.0, LEFT));
    }

    #[test]
    fn turned_artwork_follows_the_same_matrix_in_screen_coordinates() {
        use novo_symbols::Stroke;
        let line = Graphic::Line {
            from: [0.0, 0.0],
            to: [2.0, 0.0],
            stroke: Stroke::Solid,
        };
        // The quarter turn above: a line along +x (right) must end up pointing
        // up, which is -y on the screen.
        let out = turn_art(&[line], [0.0, -1.0, 1.0, 0.0]);
        let Graphic::Line { to, .. } = &out[0] else {
            panic!("a line")
        };
        assert!((to[0]).abs() < 1e-9 && (to[1] + 2.0).abs() < 1e-9, "{to:?}");
    }
}
