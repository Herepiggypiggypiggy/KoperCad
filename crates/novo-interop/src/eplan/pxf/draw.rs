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

use novo_core::{Direction, Pin, Point};
use novo_symbols::{Library, Pack, Symbol};

use super::library::{DOWN, LEFT, LibPort, Match, RIGHT, UP};
use super::model::{DeviceRole, PageClass, PxfFile};
use crate::eplan::nets::{
    ConnGraph, ConnPoint, Dir, Interruption, NetIssue, PinKey, ReconNets, rebuild_nets,
};
use crate::eplan::symbols::{EplanSymbol, SymbolMapper, Target};

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
    pub interruptions: usize,
    pub nets: usize,
    pub wires: usize,
    /// Function names drawn as boxes, with counts.
    pub box_functions: BTreeMap<String, usize>,
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

/// KoperCAD's y axis points down, so up and down swap.
fn direction_of(bits: u8) -> Option<Direction> {
    dirs_of(bits).first().map(|d| match d {
        Dir::Up => Direction::Down,
        Dir::Down => Direction::Up,
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
        if quality > Match::SameFamily {
            out.stats.loose += 1;
        }
        let function = file
            .library
            .function(key3)
            .map(|f| f.name.get("en_US").to_string())
            .unwrap_or_default();
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
                box_label = function.clone();
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
    for (ii, ip) in file.interruptions.iter().enumerate() {
        let (Some(page), Some((x, y))) = (ip.page, ip.position) else {
            continue;
        };
        if !drawn_pages.contains(&page) {
            continue;
        }
        let name = {
            let t = ip.tag.full();
            if t.is_empty() { ip.text.clone() } else { t }
        };
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
                role: Role::Interruption(name),
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
    out.stats.wires = out.wires.len();
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
    for i in 0..n {
        for j in i + 1..n {
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
        });
    }
    wires
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
    if candidates.is_empty() && !terminal && !function.is_empty() {
        let r = mapper.map(&EplanSymbol {
            key: None,
            function: Some(function.to_string()),
            letters: Some(letters.to_string()),
            pins: if named {
                pin_names.to_vec()
            } else {
                Vec::new()
            },
        });
        if let (Target::Library(id), crate::eplan::symbols::Basis::Name) = (&r.target, r.basis)
            && let Some(s) = lib.get(id)
        {
            candidates.push(s);
        }
    }
    for s in candidates {
        if s.connections.len() != ports.len() {
            continue;
        }
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
                    (0..ports.len()).collect()
                }
            } else {
                (0..ports.len()).collect()
            }
        } else {
            (0..ports.len()).collect()
        };
        let distinct: BTreeSet<usize> = conns.iter().copied().collect();
        if distinct.len() == ports.len() {
            return (Some(s), conns);
        }
    }
    (None, Vec::new())
}
