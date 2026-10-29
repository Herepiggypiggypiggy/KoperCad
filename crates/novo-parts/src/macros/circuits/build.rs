//! Turns a [`CircuitSpec`] into a macro file: the circuit is drawn in a
//! scratch project through the same commands a user's drawing takes, then
//! captured with `define_block`. The file is checked while it is built, so a
//! circuit with a pin that is neither wired nor listed as open, a wrong pin
//! name or a mounting view that does not fit never gets as far as the
//! library.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    BlockParameter, DeviceId, Endpoint, JunctionId, Placement, Point, Record, Rotation, SymbolId,
    SymbolRole, Value,
};
use novo_symbols::Library;

use super::mounting::{MountItem, MountPlate, MountRail, MountingView};
use super::spec::{CircuitSpec, Hop, Node, ValueSpec, parse_chain};
use super::{CHAPTERS, CircuitError};
use crate::macros::MacroFile;
use crate::macros::starter::{Sheet, capture};

type Pin = (SymbolId, u16);

fn value(v: &ValueSpec) -> Value {
    match v {
        ValueSpec::Bool(b) => Value::Bool(*b),
        ValueSpec::Int(i) => Value::Int(*i),
        ValueSpec::Float(f) => Value::Float(*f),
        ValueSpec::Text(t) => Value::Text(t.clone()),
    }
}

/// What the symbol library says about the symbols the circuit uses: the
/// connection ids of each, in pin order.
struct Pins {
    by_symbol: BTreeMap<String, (String, Vec<String>)>,
}

impl Pins {
    fn of(spec: &CircuitSpec, lib: &Library) -> Result<Self, CircuitError> {
        let mut by_symbol = BTreeMap::new();
        for s in &spec.symbols {
            let Some(sym) = lib.get(&s.key) else {
                return Err(CircuitError::at(
                    spec,
                    format!(
                        "symbol {} uses {}, which the library does not have",
                        s.id, s.key
                    ),
                ));
            };
            let ids = sym.connections.iter().map(|c| c.id.clone()).collect();
            if by_symbol
                .insert(s.id.clone(), (s.key.clone(), ids))
                .is_some()
            {
                return Err(CircuitError::at(
                    spec,
                    format!("symbol id {} is used twice", s.id),
                ));
            }
        }
        Ok(Pins { by_symbol })
    }

    fn index(&self, spec: &CircuitSpec, symbol: &str, pin: &str) -> Result<u16, CircuitError> {
        let (key, ids) = self
            .by_symbol
            .get(symbol)
            .ok_or_else(|| CircuitError::at(spec, format!("no symbol {symbol}")))?;
        ids.iter()
            .position(|i| i == pin)
            .map(|i| i as u16)
            .ok_or_else(|| {
                CircuitError::at(
                    spec,
                    format!(
                        "{symbol} ({key}) has no pin {pin}; it has {}",
                        ids.join(", ")
                    ),
                )
            })
    }
}

/// An endpoint of a wire, resolved against the symbol library but not yet
/// drawn.
#[derive(Clone, Copy)]
enum End<'a> {
    Pin(&'a str, u16),
    Junction(&'a str),
}

/// A wire: its two ends and the corners between them.
type PlannedWire<'a> = (End<'a>, End<'a>, Vec<(f64, f64)>);

struct Plan<'a> {
    wires: Vec<PlannedWire<'a>>,
    buses: Vec<(f64, Vec<(&'a str, u16)>)>,
    /// Every pin the file says is open, as (symbol, pin index).
    open: BTreeSet<(&'a str, u16)>,
}

fn hop_ends<'a>(
    spec: &CircuitSpec,
    pins: &Pins,
    junctions: &BTreeSet<&str>,
    hop: &Hop<'a>,
    outgoing: bool,
) -> Result<Vec<End<'a>>, CircuitError> {
    match hop {
        Hop::Junction(id) if junctions.contains(id) => Ok(vec![End::Junction(id)]),
        Hop::Junction(id) => Err(CircuitError::at(
            spec,
            format!("'{id}' in a chain is neither symbol.pin nor a junction"),
        )),
        Hop::Pins {
            symbol,
            incoming,
            outgoing: out,
        } => (if outgoing { out } else { incoming })
            .iter()
            .map(|n| Ok(End::Pin(symbol, pins.index(spec, symbol, n)?)))
            .collect(),
    }
}

fn plan<'a>(spec: &'a CircuitSpec, pins: &Pins) -> Result<Plan<'a>, CircuitError> {
    let junctions: BTreeSet<&str> = spec.junctions.iter().map(|j| j.id.as_str()).collect();
    let end = |text: &'a str| -> Result<End<'a>, CircuitError> {
        match Node::parse(text) {
            Node::Pin { symbol, pin } => Ok(End::Pin(symbol, pins.index(spec, symbol, pin)?)),
            Node::Junction(id) if junctions.contains(id) => Ok(End::Junction(id)),
            Node::Junction(id) => Err(CircuitError::at(
                spec,
                format!("'{id}' is neither symbol.pin nor a junction"),
            )),
        }
    };
    let mut wires = Vec::new();
    for w in &spec.wires {
        let via = w.via.iter().map(|p| (p[0], p[1])).collect();
        wires.push((end(&w.from)?, end(&w.to)?, via));
    }
    for c in &spec.chains {
        let hops = parse_chain(&c.path);
        for pair in hops.windows(2) {
            let la = hop_ends(spec, pins, &junctions, &pair[0], true)?;
            let lb = hop_ends(spec, pins, &junctions, &pair[1], false)?;
            if la.len() != lb.len() && la.len() != 1 && lb.len() != 1 {
                return Err(CircuitError::at(
                    spec,
                    format!("chain '{}' joins {} pins to {}", c.path, la.len(), lb.len()),
                ));
            }
            for i in 0..la.len().max(lb.len()) {
                wires.push((la[i % la.len()], lb[i % lb.len()], Vec::new()));
            }
        }
    }
    let mut buses = Vec::new();
    for b in &spec.buses {
        let mut taps = Vec::new();
        for t in &b.taps {
            match end(t)? {
                End::Pin(s, p) => taps.push((s, p)),
                End::Junction(_) => {
                    return Err(CircuitError::at(
                        spec,
                        format!("bus tap '{t}' is not a pin"),
                    ));
                }
            }
        }
        buses.push((b.y, taps));
    }
    let mut open = BTreeSet::new();
    for o in &spec.open {
        match parse_chain(o).as_slice() {
            [
                Hop::Pins {
                    symbol,
                    incoming: names,
                    ..
                },
            ] => {
                for n in names {
                    let idx = pins.index(spec, symbol, n)?;
                    let sym = spec
                        .symbols
                        .iter()
                        .find(|s| s.id == *symbol)
                        .expect("indexed");
                    open.insert((sym.id.as_str(), idx));
                }
            }
            _ => {
                return Err(CircuitError::at(
                    spec,
                    format!("open point '{o}' is not symbol.pin[,pin]"),
                ));
            }
        }
    }
    Ok(Plan { wires, buses, open })
}

/// Draws and captures one circuit.
pub fn build(spec: &CircuitSpec) -> Result<MacroFile, CircuitError> {
    if !CHAPTERS.iter().any(|c| c.slug == spec.chapter) {
        return Err(CircuitError::at(
            spec,
            format!(
                "chapter '{}' is not one of the library's chapters",
                spec.chapter
            ),
        ));
    }
    let lib = Library::builtin().expect("the built-in symbols load");
    let pins = Pins::of(spec, &lib)?;
    let plan = plan(spec, &pins)?;

    let mut tags = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for d in &spec.devices {
        if !ids.insert(d.id.as_str()) {
            return Err(CircuitError::at(
                spec,
                format!("device id {} is used twice", d.id),
            ));
        }
        if novo_core::DeviceTag::parse(&d.tag).is_err() || !tags.insert(d.tag.as_str()) {
            return Err(CircuitError::at(
                spec,
                format!("device tag '{}' is not valid or is used twice", d.tag),
            ));
        }
    }
    for s in &spec.symbols {
        if let Some(d) = &s.device
            && !ids.contains(d.as_str())
        {
            return Err(CircuitError::at(
                spec,
                format!("symbol {} names no device {d}", s.id),
            ));
        }
        if !matches!(s.rotate, 0 | 90 | 180 | 270) {
            return Err(CircuitError::at(
                spec,
                format!("symbol {} turns by {}, not a quarter turn", s.id, s.rotate),
            ));
        }
        if !s.numbers.is_empty() && s.numbers.len() > pins.by_symbol[&s.id].1.len() {
            return Err(CircuitError::at(
                spec,
                format!("symbol {} has too many numbers", s.id),
            ));
        }
    }
    // A device's main symbol is the one marked `main = true`, else the first
    // symbol listed for it that is not marked `main = false`.
    let mut main_of: BTreeMap<&str, usize> = BTreeMap::new();
    let mut marked: BTreeSet<&str> = BTreeSet::new();
    for (i, s) in spec.symbols.iter().enumerate() {
        let Some(d) = s.device.as_deref() else {
            continue;
        };
        if s.main == Some(true) {
            if !marked.insert(d) {
                return Err(CircuitError::at(
                    spec,
                    format!("device {d} has two main symbols"),
                ));
            }
            main_of.insert(d, i);
        } else if s.main.is_none() && !marked.contains(d) {
            main_of.entry(d).or_insert(i);
        }
    }
    for d in &spec.devices {
        let has_symbol = spec
            .symbols
            .iter()
            .any(|s| s.device.as_deref() == Some(d.id.as_str()));
        if !has_symbol {
            return Err(CircuitError::at(
                spec,
                format!("device {} has no symbol", d.id),
            ));
        }
        if !main_of.contains_key(d.id.as_str()) {
            return Err(CircuitError::at(
                spec,
                format!("device {} has no main symbol", d.id),
            ));
        }
    }

    let problems: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let parameters: Vec<BlockParameter> = spec
        .parameters
        .iter()
        .map(|p| BlockParameter {
            name: p.name.clone(),
            default: value(&p.default),
            description: p.description.clone(),
        })
        .collect();
    let tag_list: Vec<&str> = spec.tags.iter().map(String::as_str).collect();

    let mut file = capture(&spec.name, &spec.description, parameters, &tag_list, |s| {
        draw(spec, &plan, &main_of, &problems, s)
    })
    .map_err(|e| CircuitError::at(spec, format!("drawing failed: {e}")))?;
    let problems = problems.into_inner();
    if !problems.is_empty() {
        return Err(CircuitError::at(spec, problems.join("; ")));
    }
    if !spec.notes.is_empty() {
        file.extra.insert("notes".into(), spec.notes.clone().into());
    }
    if let Some(m) = mounting(spec, &file)? {
        file.set_mounting(Some(&m));
    }
    Ok(file)
}

fn draw(
    spec: &CircuitSpec,
    plan: &Plan<'_>,
    main_of: &BTreeMap<&str, usize>,
    problems: &RefCell<Vec<String>>,
    s: &mut Sheet<'_, '_>,
) -> novo_core::Result<()> {
    let mut devices: BTreeMap<&str, DeviceId> = BTreeMap::new();
    for d in &spec.devices {
        let id = s.device(&d.tag, &d.description)?;
        for (key, v) in &d.props {
            s.tx.update::<novo_core::Device>(id, |dev| {
                dev.properties.insert(key.clone(), value(v));
            })?;
        }
        devices.insert(&d.id, id);
    }
    let mut symbols: BTreeMap<&str, SymbolId> = BTreeMap::new();
    for (i, sym) in spec.symbols.iter().enumerate() {
        let [x, y] = sym.at;
        let role = match &sym.device {
            Some(d) => SymbolRole::Device {
                device: devices[d.as_str()],
                main: main_of.get(d.as_str()) == Some(&i),
            },
            None => SymbolRole::Unassigned,
        };
        let placement = Placement {
            origin: Point::new(x, y),
            rotation: match sym.rotate {
                90 => Rotation::R90,
                180 => Rotation::R180,
                270 => Rotation::R270,
                _ => Rotation::R0,
            },
            mirrored: sym.mirror,
        };
        let id = s.place(&sym.key, placement, role)?;
        if !sym.numbers.is_empty() {
            let names: Vec<&str> = sym.numbers.iter().map(String::as_str).collect();
            s.number(id, &names)?;
        }
        symbols.insert(&sym.id, id);
    }
    let mut junctions: BTreeMap<&str, JunctionId> = BTreeMap::new();
    for j in &spec.junctions {
        junctions.insert(&j.id, s.junction(j.at[0], j.at[1])?);
    }
    let endpoint = |e: End<'_>| -> Endpoint {
        match e {
            End::Pin(sym, pin) => Endpoint::pin(symbols[sym], pin),
            End::Junction(j) => Endpoint::junction(junctions[j]),
        }
    };
    for (a, b, via) in &plan.wires {
        if via.is_empty() {
            s.tx.connect(endpoint(*a), endpoint(*b))?;
        } else {
            s.wire_via(endpoint(*a), endpoint(*b), via)?;
        }
    }
    for (y, taps) in &plan.buses {
        let taps: Vec<Pin> = taps.iter().map(|(sym, p)| (symbols[sym], *p)).collect();
        s.bus(*y, &taps)?;
    }

    // Every pin is wired, or listed as open and not wired.
    let mut wired: BTreeSet<Pin> = BTreeSet::new();
    for w in s.tx.project().wires() {
        for e in [w.from, w.to] {
            if let Endpoint::Pin { symbol, pin } = e {
                wired.insert((symbol, pin));
            }
        }
    }
    let mut found = Vec::new();
    for sym in &spec.symbols {
        let id = symbols[sym.id.as_str()];
        let Some(instance) = s.tx.project().symbol(id) else {
            continue;
        };
        for (i, pin) in instance.pins.iter().enumerate() {
            let listed = plan.open.contains(&(sym.id.as_str(), i as u16));
            let is_wired = wired.contains(&(id, i as u16));
            match (is_wired, listed, pin.optional) {
                (false, false, false) => {
                    found.push(format!("{}.{} is not wired", sym.id, pin.name))
                }
                (true, true, _) => {
                    found.push(format!("{}.{} is wired but listed open", sym.id, pin.name))
                }
                _ => {}
            }
        }
    }
    problems.borrow_mut().extend(found);
    Ok(())
}

fn mounting(spec: &CircuitSpec, file: &MacroFile) -> Result<Option<MountingView>, CircuitError> {
    let wants = spec.devices.iter().any(|d| d.mount.is_some());
    if !wants {
        return Ok(None);
    }
    let template: BTreeMap<String, u64> = file
        .definition
        .members
        .iter()
        .filter_map(|m| match m {
            Record::Device(d) => Some((d.tag.to_string(), d.id.raw())),
            _ => None,
        })
        .collect();
    let Some(plate) = &spec.plate else {
        return Err(CircuitError::at(
            spec,
            "a mounting view needs a [plate]".into(),
        ));
    };
    let mut items = Vec::new();
    for d in &spec.devices {
        let Some(m) = &d.mount else { continue };
        items.push(MountItem {
            device: template[&d.tag],
            tag: d.tag.clone(),
            description: d.description.clone(),
            kind: m.kind,
            size: m.size,
            at: m.at,
            rail: m.rail.clone(),
            place: m.place.clone(),
            note: m.note.clone(),
        });
    }
    let view = MountingView {
        plate: MountPlate {
            width: plate.width,
            height: plate.height,
        },
        rails: spec
            .rails
            .iter()
            .map(|r| MountRail {
                id: r.id.clone(),
                x: r.x,
                y: r.y,
                length: r.length,
            })
            .collect(),
        items,
    };
    let problems = view.problems();
    if problems.is_empty() {
        Ok(Some(view))
    } else {
        Err(CircuitError::at(
            spec,
            format!("mounting view: {}", problems.join("; ")),
        ))
    }
}
