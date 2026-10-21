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
//! Inferred: the artwork. A placement whose function a KoperCAD symbol of
//! the same pin count stands for is drawn with that symbol (its pins land
//! where the symbol has them, not where EPLAN had them); anything else is a
//! labelled box with the pins where EPLAN had them. Wire routes are plain
//! right-angle runs, not EPLAN's lines. Interruption points face any way.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Direction, Pin, Point, Rotation};
use novo_symbols::{Library, Pack, Symbol};

use super::library::{DOWN, LEFT, LibPort, Match, RIGHT, UP};
use super::model::{DeviceRole, PageClass, PxfFile};
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
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DrawnWire {
    pub page: usize,
    pub from: (usize, u16),
    pub to: (usize, u16),
    pub route: Vec<Point>,
    /// The route is EPLAN's: its runs and nodes, not a guess between the pins.
    pub followed: bool,
}

/// What the drawing step did, for the report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrawStats {
    pub pages: usize,
    pub library_symbols: usize,
    pub box_symbols: usize,
    /// Placements the embedded library could not explain: not drawn.
    pub unplaced: usize,
    /// Placements drawn although the library only loosely matched.
    pub loose: usize,
    /// Function names of the loosely matched placements, with counts.
    pub loose_functions: BTreeMap<String, usize>,
    pub interruptions: usize,
    pub nets: usize,
    pub wires: usize,
    /// Wires drawn along EPLAN's own runs and nodes, not a guessed route.
    pub wires_followed: usize,
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
                let origin = Point::new(at.x - pins[c0].offset.x, at.y - pins[c0].offset.y);
                let mut pins = pins;
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
                box_label = box_caption(&function, d.type_text.get("en_US"), &d.part_number);
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
            page_pos.insert(
                pin,
                Point::new(origin.x + kp.offset.x, origin.y + kp.offset.y),
            );
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
        if legs.len() >= 3 {
            // A T or cross joint: EPLAN draws a dot there.
            out.junctions.push((page, Point::new(x, PAGE_HEIGHT - y)));
        }
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

    let pos_of = |placed: &[Placed], k: &PinKey| -> Option<(usize, u16, usize, Point)> {
        let &(si, pin, page) = by_key.get(k)?;
        Some((si, pin, page, *placed[si].page_pos.get(&pin)?))
    };
    for net in &nets.nets {
        let mut per_page: BTreeMap<usize, Vec<(usize, u16, Point)>> = BTreeMap::new();
        for k in &net.pins {
            if let Some((si, pin, page, at)) = pos_of(&placed, k) {
                per_page.entry(page).or_default().push((si, pin, at));
            }
        }
        for (page, pts) in per_page {
            out.wires.extend(spanning_wires(page, &pts, &placed));
        }
    }
    follow_runs(&mut out.wires, &nets, &graph, &by_key);
    out.stats.wires = out.wires.len();
    out.stats.wires_followed = out.wires.iter().filter(|w| w.followed).count();
    out.keys = by_key
        .iter()
        .map(|(k, &(si, pin, _))| (k.clone(), (si, pin)))
        .collect();
    out.recon = nets;
    out.symbols = placed.into_iter().map(|p| p.symbol).collect();
    out
}

/// A minimum spanning tree over `pts` by right-angle distance. Pins that a
/// symbol already joins inside (a terminal's two sides) start joined.
/// Nets with more points than this are joined as a chain, not by every pair.
const MAX_PAIRWISE: usize = 3000;

fn spanning_wires(page: usize, pts: &[(usize, u16, Point)], placed: &[Placed]) -> Vec<DrawnWire> {
    let n = pts.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            if pts[i].0 == pts[j].0 {
                let b = &placed[pts[i].0].symbol.bridges;
                if b.iter()
                    .any(|g| g.contains(&pts[i].1) && g.contains(&pts[j].1))
                {
                    let (a, c) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = c;
                }
            }
        }
    }
    let mut edges: Vec<(f64, usize, usize)> = Vec::new();
    if n <= MAX_PAIRWISE {
        for i in 0..n {
            for j in i + 1..n {
                if pts[i].0 == pts[j].0 && pts[i].1 == pts[j].1 {
                    continue;
                }
                let d = (pts[i].2.x - pts[j].2.x).abs() + (pts[i].2.y - pts[j].2.y).abs();
                edges.push((d, i, j));
            }
        }
    } else {
        // A net this big (a damaged or hostile file) would need n squared
        // edges: chain the points in reading order instead.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            pts[a]
                .2
                .x
                .total_cmp(&pts[b].2.x)
                .then(pts[a].2.y.total_cmp(&pts[b].2.y))
        });
        for w in order.windows(2) {
            let (i, j) = (w[0], w[1]);
            if pts[i].0 == pts[j].0 && pts[i].1 == pts[j].1 {
                continue;
            }
            let d = (pts[i].2.x - pts[j].2.x).abs() + (pts[i].2.y - pts[j].2.y).abs();
            edges.push((d, i, j));
        }
    }
    edges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut wires = Vec::new();
    for (_, i, j) in edges {
        let (a, b) = (find(&mut parent, i), find(&mut parent, j));
        if a == b {
            continue;
        }
        parent[a] = b;
        wires.push(DrawnWire {
            page,
            from: (pts[i].0, pts[i].1),
            to: (pts[j].0, pts[j].1),
            route: route(
                pts[i].2,
                pts[j].2,
                placed[pts[i].0].symbol.pins[pts[i].1 as usize].direction,
            ),
            followed: false,
        });
    }
    wires
}

/// Replaces the guessed route of each wire by the one EPLAN implies: the
/// straight runs between the two pins through nodes only (a run never passes
/// through another symbol's pin). Wires with no such path keep their guess.
fn follow_runs(
    wires: &mut [DrawnWire],
    nets: &ReconNets,
    graph: &ConnGraph,
    by_key: &BTreeMap<PinKey, (usize, u16, usize)>,
) {
    let at: BTreeMap<&PinKey, (bool, Point)> = graph
        .points
        .iter()
        .map(|p| (&p.key, (p.node, Point::new(p.at.x, PAGE_HEIGHT - p.at.y))))
        .collect();
    let mut next: BTreeMap<&PinKey, Vec<&PinKey>> = BTreeMap::new();
    for (a, b) in &nets.runs {
        next.entry(a).or_default().push(b);
        next.entry(b).or_default().push(a);
    }
    let key_of: BTreeMap<(usize, u16), &PinKey> = by_key
        .iter()
        .rev()
        .map(|(k, &(si, pin, _))| ((si, pin), k))
        .collect();
    for w in wires.iter_mut() {
        let (Some(&from), Some(&to)) = (key_of.get(&w.from), key_of.get(&w.to)) else {
            continue;
        };
        // Breadth first, so the fewest corners win; only nodes are passed.
        let mut came: BTreeMap<&PinKey, &PinKey> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::from([from]);
        let mut seen: BTreeSet<&PinKey> = BTreeSet::from([from]);
        let mut found = false;
        while let Some(k) = queue.pop_front() {
            if k == to {
                found = true;
                break;
            }
            if k != from && !at.get(k).is_some_and(|(node, _)| *node) {
                continue;
            }
            for &m in next.get(k).into_iter().flatten() {
                if seen.insert(m) {
                    came.insert(m, k);
                    queue.push_back(m);
                }
            }
        }
        if !found {
            continue;
        }
        let mut path = vec![to];
        while let Some(&prev) = came.get(path[path.len() - 1]) {
            path.push(prev);
        }
        path.reverse();
        let mut pts: Vec<Point> = Vec::new();
        for k in &path {
            if let Some(&(_, p)) = at.get(k)
                && pts
                    .last()
                    .is_none_or(|l| (l.x - p.x).abs() > 1e-6 || (l.y - p.y).abs() > 1e-6)
            {
                pts.push(p);
            }
        }
        // Keep only the corners; the ends are the pins themselves.
        let mut corners: Vec<Point> = Vec::new();
        for i in 1..pts.len().saturating_sub(1) {
            let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
            let straight = ((a.x - b.x).abs() < 1e-6 && (b.x - c.x).abs() < 1e-6)
                || ((a.y - b.y).abs() < 1e-6 && (b.y - c.y).abs() < 1e-6);
            if !straight {
                corners.push(b);
            }
        }
        w.route = corners;
        w.followed = true;
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
    fn a_wire_follows_the_runs_through_the_nodes_of_the_file() {
        // A pin facing right, a corner node, a pin facing down (y up in the
        // file): the wire turns at the node, not at a guessed corner.
        let mut graph = ConnGraph::default();
        graph
            .points
            .push(ConnPoint::pin("a", 0, 0.0, 0.0, Dir::Right));
        graph
            .points
            .push(ConnPoint::node("n", 0, 10.0, 0.0, &[Dir::Left, Dir::Up]));
        graph
            .points
            .push(ConnPoint::pin("b", 0, 10.0, 20.0, Dir::Down));
        let nets = rebuild_nets(&graph);
        // each run is seen from both of its ends
        assert!(nets.runs.len() >= 2);
        let by_key: BTreeMap<PinKey, (usize, u16, usize)> = [
            (PinKey("a".into()), (0, 0, 0)),
            (PinKey("b".into()), (1, 0, 0)),
        ]
        .into();
        let mut wires = vec![DrawnWire {
            page: 0,
            from: (0, 0),
            to: (1, 0),
            route: vec![Point::new(0.0, 0.0)],
            followed: false,
        }];
        follow_runs(&mut wires, &nets, &graph, &by_key);
        assert!(wires[0].followed);
        assert_eq!(wires[0].route, vec![Point::new(10.0, PAGE_HEIGHT)]);
        // No path through nodes: the guess stays.
        wires[0].followed = false;
        wires[0].route = vec![Point::new(1.0, 1.0)];
        let none = ReconNets::default();
        follow_runs(&mut wires, &none, &graph, &by_key);
        assert!(!wires[0].followed);
        assert_eq!(wires[0].route, vec![Point::new(1.0, 1.0)]);
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
}
