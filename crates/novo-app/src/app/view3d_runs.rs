//! Cable and wire runs in the 3D view, read as a whole: the path of the
//! selected wire (or every core of a cable) lit up along the ducts it lies
//! in, the ducts outlined in their fill colour, the cabinet entry point as a
//! marker, and a card that says how the length was found, how full each duct
//! is and what is not checked.
//!
//! Where the user finds it: click a wire in the 3D view (or a row of View >
//! 3D > Wire runs) and the Run card opens at the left; hover a wire for the
//! short version. A run drawn to the cabinet entry point shows where the
//! entry is; the outside part is a stated length and is never drawn.
//!
//! The lengths come from the wire list's own measurement
//! (`novo_reports::wire_cut_jobs`), so the card, the cut list and the wire
//! list never disagree. The geometry is the routing engine's.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use glam::Vec3;
use novo_core::{Project, StructureId, WireId, natural_cmp};
use novo_routing::Point;
use novo_view3d::scene::{ItemId, ItemKind};
use novo_view3d::{Overlay, OverlayLines, Scene};

use super::cabinet_routes::Tone;
use super::view3d_wires::WireFacts;

/// Basis texts that are not a measurement of the finished wire: the card
/// says in words what is still missing.
const WEAK_BASES: [&str; 4] = ["estimate", "free air", "routed, no depth", "door, no gap"];

/// How the length of one wire was found, as the wire list states it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Basis {
    /// The wire list's words: `routed`, `estimate`, `cable length` ...
    pub text: String,
    /// The length of the wire list and the cut list, whole millimetres.
    pub length_mm: f64,
    /// One sentence on what the figure is and what it leaves out.
    pub why: &'static str,
    /// The cable the wire is a core of.
    pub cable: Option<CableFacts>,
}

impl Basis {
    /// Not a measurement of the finished wire: an estimate, a straight line
    /// or a length that leaves something out.
    pub(super) fn is_weak(&self) -> bool {
        WEAK_BASES.contains(&self.text.as_str())
    }
}

/// A cable that carries the wire.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CableFacts {
    pub tag: String,
    /// Cores that are wires of the project.
    pub cores_used: usize,
    /// Cores the cable has.
    pub cores_total: usize,
    /// The cable as bought, mm.
    pub length_mm: f64,
    /// The length was stated for the cable; else it is its longest core.
    pub stated: bool,
    /// Every wire of the project that is a core of it.
    pub wires: Vec<u64>,
}

impl CableFacts {
    pub(super) fn line(&self) -> String {
        let how = if self.stated {
            "as bought, stated"
        } else {
            "longest core, no length stated"
        };
        format!(
            "Cable {}: {} of {} cores wired, {:.2} m {how}",
            self.tag,
            self.cores_used,
            self.cores_total,
            self.length_mm / 1000.0
        )
    }
}

/// A wire drawn to the cabinet entry point.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EntryFacts {
    pub at: Option<Point>,
    /// The wire outside the entry the designer stated, mm.
    pub outside_mm: Option<f64>,
}

impl EntryFacts {
    pub(super) fn line(&self) -> String {
        let at = self
            .at
            .map(|p| format!(" at x {:.0}, y {:.0} mm", p.x, p.y))
            .unwrap_or_default();
        match self.outside_mm {
            Some(m) => format!(
                "Drawn to the cabinet entry{at}; outside it {:.2} m as stated, not drawn",
                m / 1000.0
            ),
            None => format!("Drawn to the cabinet entry{at}; the part outside is not stated"),
        }
    }
}

/// A wire with no path in the picture.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Undrawn {
    pub id: u64,
    pub name: String,
    pub text: String,
    pub length_mm: f64,
    pub why: &'static str,
}

/// What makes a length an estimate although the wire is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gap {
    None,
    /// Drawn to the entry and nothing is stated for the part outside.
    OutsideNotStated,
    /// The stated cable length is shorter than the route inside the cabinet.
    CableTooShort,
}

/// The sentence for a basis text.
fn why(text: &str, gap: Gap) -> &'static str {
    match text {
        "routed" => "Measured along the ducts from terminal to terminal, loops included.",
        "routed, no depth" => {
            "Routed, but a device depth is unknown, so it is a little short. Add the depth to the part."
        }
        "free air" => {
            "No duct reaches both ends: a straight line. Place a duct or move the device."
        }
        "door, no gap" => {
            "The part on the plate is routed; the gap to the door and the hinge loop are not in it."
        }
        "cable length" => {
            "The cable's stated length: the whole cable as bought, stripped ends included."
        }
        "to entry + stated outside" => {
            "Routed to the cabinet entry, plus the length outside that you stated."
        }
        "estimate" if gap == Gap::CableTooShort => {
            "The stated cable length is shorter than the route inside the cabinet, so it cannot be right: the inside route is shown as an estimate. Check the cable length."
        }
        "estimate" if gap == Gap::OutsideNotStated => {
            "Drawn to the entry, but the wire outside it is not stated, so the length stays an estimate. State it in Cabinet > Routes."
        }
        _ => {
            "Not routed: a planning figure from the drawing. Place the devices (and set the cabinet entry for wires that leave) to make it exact."
        }
    }
}

/// The length and basis of every wire that is a connection, and the wires
/// that are not in `drawn` (no path in the picture). Runs the wire list's
/// measurement, which routes the project again, so call it when a card or the
/// list needs it, not on every rebuild.
pub(super) fn bases(
    project: &Project,
    drawn: &BTreeMap<u64, WireFacts>,
) -> (BTreeMap<u64, Basis>, Vec<Undrawn>) {
    let jobs = novo_reports::wire_cut_jobs(project);
    let runs = novo_reports::cable_runs(project, &jobs);
    let mut cables: BTreeMap<novo_core::CableId, CableFacts> = BTreeMap::new();
    for run in &runs {
        let Some(cable) = project.cable(run.cable) else {
            continue;
        };
        let wires = jobs
            .iter()
            .filter(|j| j.cable == Some(run.cable))
            .flat_map(|j| j.wires.iter().map(|w| w.raw()))
            .collect();
        cables.insert(
            run.cable,
            CableFacts {
                tag: run.tag.clone(),
                cores_used: run.cores,
                cores_total: cable.cores.len(),
                length_mm: run.length_mm,
                stated: cable.length_m.is_some_and(|m| m.is_finite() && m > 0.0),
                wires,
            },
        );
    }
    let mut out = BTreeMap::new();
    let mut undrawn = Vec::new();
    for job in &jobs {
        for wire in &job.wires {
            let id = wire.raw();
            let cable = job.cable.and_then(|c| cables.get(&c));
            let is_drawn = drawn.get(&id);
            let gap = if is_drawn.is_some() && cable.is_some_and(|c| c.stated) {
                Gap::CableTooShort
            } else if is_drawn
                .and_then(|f| f.entry.as_ref())
                .is_some_and(|e| e.outside_mm.is_none())
            {
                Gap::OutsideNotStated
            } else {
                Gap::None
            };
            let basis = Basis {
                text: job.basis.clone(),
                length_mm: job.length_mm,
                why: why(&job.basis, gap),
                cable: job.cable.and_then(|c| cables.get(&c).cloned()),
            };
            if !drawn.contains_key(&id) {
                undrawn.push(Undrawn {
                    id,
                    name: wire_name(project, *wire),
                    text: basis.text.clone(),
                    length_mm: basis.length_mm,
                    why: basis.why,
                });
            }
            out.insert(id, basis);
        }
    }
    undrawn.sort_by(|a, b| natural_cmp(&a.name, &b.name).then(a.id.cmp(&b.id)));
    (out, undrawn)
}

fn wire_name(project: &Project, id: WireId) -> String {
    let Some(w) = project.wire(id) else {
        return format!("#{}", id.raw());
    };
    if let Some(n) = w.number.as_deref().filter(|n| !n.is_empty()) {
        return n.to_owned();
    }
    let pin = |e: &novo_core::Endpoint| {
        e.as_pin()
            .and_then(|p| project.pin_designation(p))
            .unwrap_or_default()
    };
    format!("{} → {}", pin(&w.from), pin(&w.to))
}

/// How long the project must stay unchanged before the basis is read, in
/// seconds: a drag or a run of nudges changes the revision every frame, and
/// the read routes the whole project again.
const SETTLE: f64 = 0.25;

/// The revision last seen and since when it has been unchanged.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct Settle {
    revision: u64,
    since: f64,
}

impl Settle {
    /// Whether `revision` has been unchanged for [`SETTLE`] seconds at `now`;
    /// notes it when it is new.
    fn settled(&mut self, revision: u64, now: f64) -> bool {
        if self.revision != revision {
            *self = Settle {
                revision,
                since: now,
            };
        }
        now - self.since >= SETTLE
    }
}

/// Reads how the length of every wire was found, once per project revision
/// (it routes the project again, so it waits until a card or the list needs
/// it and until the project has stopped changing), and puts it on the drawn
/// wires' facts. Until then the card shows the routed length without a basis.
#[allow(clippy::too_many_arguments)] // two caches, the project and the clock
pub(super) fn ensure(
    ctx: &egui::Context,
    now: f64,
    settle: &mut Settle,
    facts: &mut Option<(StructureId, u64, BTreeMap<u64, WireFacts>)>,
    undrawn: &mut Option<(StructureId, u64, Vec<Undrawn>)>,
    project: &Project,
    id: StructureId,
    revision: u64,
) {
    let Some((s, r, map)) = facts.as_mut() else {
        return;
    };
    if *s != id || *r != revision {
        return;
    }
    if matches!(undrawn, Some((s, r, _)) if *s == id && *r == revision) {
        return;
    }
    if !settle.settled(revision, now) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(SETTLE));
        return;
    }
    let (found, rest) = bases(project, map);
    for (wire, f) in map.iter_mut() {
        f.basis = found.get(wire).cloned();
    }
    *undrawn = Some((id, revision, rest));
}

/// The wires of the selection that have a path in the picture.
pub(super) fn picked<'a>(
    facts: &'a BTreeMap<u64, WireFacts>,
    selection: &BTreeSet<ItemId>,
) -> Vec<&'a WireFacts> {
    selection
        .iter()
        .filter(|i| i.kind == ItemKind::Wire)
        .filter_map(|i| facts.get(&i.raw))
        .collect()
}

/// The ducts of the selection (rail ids).
pub(super) fn ducts_picked(selection: &BTreeSet<ItemId>) -> Vec<u64> {
    selection
        .iter()
        .filter(|i| i.kind == ItemKind::Duct)
        .map(|i| i.raw)
        .collect()
}

/// A picked duct and the runs that lie in it: the reverse of the Run card.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DuctRuns<'a> {
    pub rail: u64,
    /// How full it is; `None` when no drawn wire lies in it (its place and
    /// fill are read from the wires that do).
    pub fill: Option<&'a super::view3d_wires::DuctUse>,
    /// The wires in it, in list order.
    pub wires: Vec<&'a WireFacts>,
}

/// The picked ducts with their runs, from the facts of the plate.
pub(super) fn duct_runs<'a>(
    facts: &'a BTreeMap<u64, WireFacts>,
    selection: &BTreeSet<ItemId>,
) -> Vec<DuctRuns<'a>> {
    ducts_picked(selection)
        .into_iter()
        .map(|rail| {
            let wires: Vec<&WireFacts> = super::view3d_wires::sorted(facts)
                .into_iter()
                .filter(|f| f.duct_use.iter().any(|d| d.rail == rail))
                .collect();
            let fill = wires
                .iter()
                .flat_map(|f| f.duct_use.iter())
                .find(|d| d.rail == rail);
            DuctRuns { rail, fill, wires }
        })
        .collect()
}

/// The cable every picked wire is a core of, when they are all of one.
pub(super) fn common_cable<'a>(picked: &[&'a WireFacts]) -> Option<&'a CableFacts> {
    let first = picked.first()?.basis.as_ref()?.cable.as_ref()?;
    picked
        .iter()
        .all(|f| {
            f.basis
                .as_ref()
                .and_then(|b| b.cable.as_ref())
                .is_some_and(|c| c.tag == first.tag)
        })
        .then_some(first)
}

// ---- the lit path, the outlined ducts and the entry marker ------------------

/// Colour of the lit path (sRGB, straight alpha): warm white, so it reads on
/// every wire colour and on the dark plate.
const PATH: [u8; 4] = [255, 232, 170, 255];
/// The cabinet entry marker: the copper accent.
const ENTRY: [u8; 4] = [232, 150, 80, 255];

fn tone_colour(t: Tone) -> [u8; 4] {
    let p = crate::theme::ui_palette();
    let c = match t {
        Tone::Idle | Tone::NoSize => p.text_secondary,
        Tone::Ok => p.success,
        Tone::Busy => p.warning,
        Tone::Over => p.error,
    };
    [c.r(), c.g(), c.b(), 255]
}

/// The overlays of the runs: the entry marker when the plate has an entry
/// point, and for the picked wires their path and the outline of each duct
/// they lie in, in the duct's fill colour.
pub(super) fn overlays(
    scene: &Scene,
    facts: &BTreeMap<u64, WireFacts>,
    selection: &BTreeSet<ItemId>,
    entry: Option<Point>,
) -> Vec<Overlay> {
    let mut out = Vec::new();
    if let Some(p) = entry {
        out.push(Overlay::Lines(entry_marker(scene, p)));
    }
    let mut chosen = picked(facts, selection);
    // A picked duct lights every run in it and outlines itself by its fill.
    let in_ducts = duct_runs(facts, selection);
    for d in &in_ducts {
        for w in &d.wires {
            if !chosen.iter().any(|c| c.id == w.id) {
                chosen.push(w);
            }
        }
    }
    if chosen.is_empty() {
        return out;
    }
    let mut path = OverlayLines {
        width_px: 4.0,
        order: 21,
        ..OverlayLines::default()
    };
    for f in &chosen {
        let Some(wire) = scene.wires.iter().find(|w| w.id.raw == f.id) else {
            continue;
        };
        path.starts.push(path.positions.len() as u32);
        for p in &wire.points {
            path.positions.push(*p);
            path.colors.push(PATH);
        }
    }
    let mut ducts = OverlayLines {
        width_px: 2.0,
        order: 20,
        ..OverlayLines::default()
    };
    let mut seen = BTreeSet::new();
    for f in &chosen {
        for d in &f.duct_use {
            if !seen.insert(d.rail) {
                continue;
            }
            let Some(item) = scene
                .items
                .iter()
                .find(|i| i.id.kind == ItemKind::Duct && i.id.raw == d.rail)
            else {
                continue;
            };
            box_edges(&mut ducts, scene, item, tone_colour(d.tone));
        }
    }
    if !ducts.positions.is_empty() {
        out.push(Overlay::Lines(ducts));
    }
    let tight: Vec<(Point, f64)> = chosen
        .iter()
        .filter_map(|f| f.checks.as_ref())
        .flat_map(|c| c.corners.iter().map(|k| (k.at, k.top_mm)))
        .collect();
    if !tight.is_empty() {
        out.push(Overlay::Lines(corner_marks(scene, &tight)));
    }
    if !path.positions.is_empty() {
        out.push(Overlay::Lines(path));
    }
    out
}

/// The twelve edges of an item's box as polylines.
fn box_edges(lines: &mut OverlayLines, scene: &Scene, item: &novo_view3d::Item, colour: [u8; 4]) {
    let to_world = scene.world_matrix(item);
    let (hx, hy) = (item.size.x * 0.5, item.size.y * 0.5);
    let corner = |x: f32, y: f32, z: f32| to_world.transform_point3(Vec3::new(x, y, z));
    let z = item.size.z;
    let ring = |lines: &mut OverlayLines, z: f32| {
        lines.starts.push(lines.positions.len() as u32);
        for (x, y) in [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy), (-hx, -hy)] {
            lines.positions.push(corner(x, y, z));
            lines.colors.push(colour);
        }
    };
    ring(lines, 0.0);
    ring(lines, z);
    for (x, y) in [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)] {
        lines.starts.push(lines.positions.len() as u32);
        for zz in [0.0, z] {
            lines.positions.push(corner(x, y, zz));
            lines.colors.push(colour);
        }
    }
}

/// A diamond with a cross on the plate and a short post out of it: the
/// gland, where the wires leave the cabinet.
fn entry_marker(scene: &Scene, at: Point) -> OverlayLines {
    let r = 14.0;
    let c = scene.frame.world(at.x, at.y, 1.0);
    let mut lines = OverlayLines {
        width_px: 2.5,
        order: 22,
        ..OverlayLines::default()
    };
    let mut poly = |pts: &[Vec3]| {
        lines.starts.push(lines.positions.len() as u32);
        for p in pts {
            lines.positions.push(*p);
            lines.colors.push(ENTRY);
        }
    };
    let (dx, dy) = (Vec3::X * r, Vec3::Y * r);
    poly(&[c + dx, c + dy, c - dx, c - dy, c + dx]);
    poly(&[c - dx, c + dx]);
    poly(&[c - dy, c + dy]);
    poly(&[c, c + Vec3::Z * 40.0]);
    lines
}

/// A red cross over each corner the wire cannot take, held above the ducts
/// that meet there (a mark on the plate would sit inside them, out of sight).
fn corner_marks(scene: &Scene, at: &[(Point, f64)]) -> OverlayLines {
    let r = 16.0;
    let red = tone_colour(Tone::Over);
    let mut lines = OverlayLines {
        width_px: 3.0,
        order: 23,
        ..OverlayLines::default()
    };
    for (p, top) in at {
        let c = scene.frame.world(p.x, p.y, *top + 4.0);
        for (a, b) in [
            (Vec3::X + Vec3::Y, -Vec3::X - Vec3::Y),
            (Vec3::X - Vec3::Y, Vec3::Y - Vec3::X),
        ] {
            lines.starts.push(lines.positions.len() as u32);
            for q in [c + a * r, c + b * r] {
                lines.positions.push(q);
                lines.colors.push(red);
            }
        }
    }
    lines
}

// ---- the Run card ----------------------------------------------------------

/// What the Run card asked for this frame.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Asked {
    /// Select every core of the cable (wire ids).
    pub select_cores: Option<Vec<u64>>,
    pub clear: bool,
}

fn tone_text(t: Tone) -> egui::Color32 {
    let p = crate::theme::ui_palette();
    match t {
        Tone::Idle | Tone::NoSize => p.text_secondary,
        Tone::Ok => p.success,
        Tone::Busy => p.warning,
        Tone::Over => p.error,
    }
}

/// The lines of the card for one wire, as (tone, text): the card draws them,
/// and tests read them.
pub(super) fn card_lines(f: &WireFacts) -> Vec<(Tone, String)> {
    let mut out = Vec::new();
    match &f.basis {
        Some(b) => {
            // A stated cable length is the designer's figure, not a result:
            // neutral, not the green of a routed length.
            let tone = if b.is_weak() {
                Tone::Busy
            } else if b.text == "cable length" {
                Tone::Idle
            } else {
                Tone::Ok
            };
            out.push((tone, format!("{:.0} mm, {}", b.length_mm, b.text)));
            out.push((Tone::Idle, b.why.to_owned()));
        }
        None => out.push((Tone::Idle, format!("{:.0} mm", f.length_mm))),
    }
    if let Some(b) = &f.basis
        && b.length_mm.round() != f.length_mm.round()
    {
        out.push((
            Tone::Idle,
            format!("Inside the cabinet: {:.0} mm", f.length_mm),
        ));
    }
    let path = if f.free_air {
        "Path: straight line, no duct reaches both ends".to_owned()
    } else if f.duct_use.is_empty() {
        "Path: along the plate, no duct".to_owned()
    } else {
        format!(
            "Path: {} duct{}, {} bend{}",
            f.duct_use.len(),
            if f.duct_use.len() == 1 { "" } else { "s" },
            f.bends,
            if f.bends == 1 { "" } else { "s" }
        )
    };
    out.push((if f.free_air { Tone::Over } else { Tone::Idle }, path));
    for d in &f.duct_use {
        out.push((d.tone, d.text()));
    }
    if !f.duct_use.is_empty() {
        out.push((
            Tone::Idle,
            "Fill is a planning value: the wire cross-sections added up against the share the rules allow".to_owned(),
        ));
    }
    if let Some(e) = &f.entry {
        out.push((Tone::Idle, e.line()));
    }
    if let Some(c) = f.basis.as_ref().and_then(|b| b.cable.as_ref()) {
        out.push((Tone::Idle, c.line()));
    }
    out.extend(f.problems.iter().cloned());
    if f.flat {
        out.push((
            Tone::Busy,
            "A device depth is unknown: the length is a little short".to_owned(),
        ));
    }
    match &f.checks {
        Some(c) => out.extend(c.lines()),
        None => out.push((
            Tone::Idle,
            "Not checked: cable bend radius, clearance to hot parts".to_owned(),
        )),
    }
    out
}

/// The Run card for the picked wires, as the body of the Wire runs panel.
fn card_body(ui: &mut egui::Ui, chosen: &[&WireFacts]) -> Asked {
    let mut asked = Asked::default();
    let Some(first) = chosen.first() else {
        return asked;
    };
    ui.horizontal(|ui| {
        ui.strong(if chosen.len() == 1 {
            format!("Run {}", first.name())
        } else {
            format!("{} runs", chosen.len())
        });
        if ui
            .small_button("Unpick")
            .on_hover_text("Let go of this run; the list stays")
            .clicked()
        {
            asked.clear = true;
        }
    });
    if chosen.len() == 1 {
        ui.weak(format!("{} → {}", first.from, first.to));
        for (tone, text) in card_lines(first) {
            let rich = egui::RichText::new(text).color(tone_text(tone));
            ui.add(egui::Label::new(rich).wrap());
        }
        if let Some(c) = first.basis.as_ref().and_then(|b| b.cable.as_ref())
            && ui
                .small_button("Select all cores of the cable")
                .on_hover_text("Light up every wire of this cable")
                .clicked()
        {
            asked.select_cores = Some(c.wires.clone());
        }
    } else {
        let total: f64 = chosen.iter().map(|f| f.length_mm).sum();
        ui.label(format!(
            "{} inside the cabinet",
            super::view3d_wires::metres(total)
        ));
        if let Some(c) = common_cable(chosen) {
            ui.label(c.line());
        }
        let weak = chosen
            .iter()
            .filter(|f| f.basis.as_ref().is_some_and(Basis::is_weak))
            .count();
        if weak > 0 {
            ui.colored_label(
                tone_text(Tone::Busy),
                format!("{weak} of them are estimates or leave something out"),
            );
        }
        let mut ducts: BTreeMap<u64, &super::view3d_wires::DuctUse> = BTreeMap::new();
        for d in chosen.iter().flat_map(|f| &f.duct_use) {
            ducts.insert(d.rail, d);
        }
        for d in ducts.values() {
            ui.colored_label(tone_text(d.tone), d.text());
        }
        ui.weak("Click one wire for its path and length basis");
    }
    asked
}

/// The card of a picked duct: how full it is and the runs in it, each one
/// click from its own Run card.
fn duct_body(ui: &mut egui::Ui, d: &DuctRuns<'_>) -> Asked {
    let mut asked = Asked::default();
    ui.horizontal(|ui| {
        ui.strong(match d.fill {
            Some(u) => u.place.clone(),
            None => format!("Duct {}", d.rail),
        });
        if ui
            .small_button("Unpick")
            .on_hover_text("Let go of this duct")
            .clicked()
        {
            asked.clear = true;
        }
    });
    match d.fill {
        Some(u) => {
            // The place is the heading already.
            let text = u.text();
            let text = text
                .strip_prefix(&format!("{}: ", u.place))
                .unwrap_or(&text)
                .to_owned();
            ui.colored_label(tone_text(u.tone), text);
        }
        None => {
            ui.weak("No drawn wire lies in this duct");
        }
    }
    if d.wires.is_empty() {
        return asked;
    }
    let total: f64 = d.wires.iter().map(|f| f.length_mm).sum();
    ui.weak(format!(
        "{} runs, {} inside the cabinet",
        d.wires.len(),
        super::view3d_wires::metres(total)
    ));
    if ui
        .small_button(format!("Select its {} runs", d.wires.len()))
        .on_hover_text("Pick every run in this duct to read them together")
        .clicked()
    {
        asked.select_cores = Some(d.wires.iter().map(|f| f.id).collect());
    }
    egui::ScrollArea::vertical()
        .id_salt(("view3d_duct_runs", d.rail))
        .auto_shrink([false, true])
        .max_height(140.0)
        .show(ui, |ui| {
            for f in &d.wires {
                let text = format!("{}   {:.0} mm", f.name(), f.length_mm);
                if ui
                    .selectable_label(false, text)
                    .on_hover_text(f.lines().join("\n"))
                    .clicked()
                {
                    asked.select_cores = Some(vec![f.id]);
                }
            }
        });
    asked
}

/// What the Wire runs panel asked for this frame.
#[derive(Debug, Default, PartialEq)]
pub(super) struct PanelAsked {
    pub list: super::view3d_wires::Asked,
    pub card: Asked,
}

/// What the panel lists: the wires of the plate and those without a path.
pub(super) struct ListInput<'a> {
    pub all: &'a BTreeMap<u64, WireFacts>,
    pub undrawn: &'a [Undrawn],
}

/// Start the panel this far below the view's top, under the view cube.
const UNDER_CUBE: f32 = 12.0 + super::view3d_heat::CUBE_SIZE + 8.0;
/// Room kept under the panel for the status bar and the title bar, in points.
const BELOW_PANEL: f32 = 100.0;
/// Width of the panel, in points.
const PANEL_WIDTH: f32 = 300.0;

/// Where the panel starts: under the view cube on the right, under the Heat
/// panel when that is open (it hangs from the same corner); on a window too
/// short for both, on the left under the toolbar, where the Heat panel is not.
fn placement(ctx: &egui::Context, view: egui::Rect, heat_open: bool) -> (super::panel::Side, f32) {
    let heat = heat_open
        .then(|| ctx.memory(|m| m.area_rect(super::panel::id("view3d_heat_panel"))))
        .flatten()
        .map(|r| r.bottom() - view.top() + 8.0);
    match heat {
        Some(b) if view.height() - b > 220.0 => (super::panel::Side::Right, b.max(UNDER_CUBE)),
        Some(_) => (super::panel::Side::Left, super::view3d_controls::BELOW),
        None => (super::panel::Side::Right, UNDER_CUBE),
    }
}

/// The Wire runs panel: the list when it is open, and the Run card of the
/// picked wires below it. One panel on the right under the view cube, placed
/// from the canvas like the other floating panels (it follows the canvas and
/// can be dragged); the Inspector and the placement list live on the left.
pub(super) fn panel(
    ctx: &egui::Context,
    view: egui::Rect,
    heat_open: bool,
    list: Option<ListInput<'_>>,
    chosen: &[&WireFacts],
    ducts: &[DuctRuns<'_>],
    selection: &BTreeSet<ItemId>,
) -> Option<PanelAsked> {
    if list.is_none() && chosen.is_empty() && ducts.is_empty() {
        return None;
    }
    let title = match (&list, chosen, ducts) {
        (Some(_), _, _) => "Wire runs".to_owned(),
        (None, [one], _) => format!("Run {}", one.name()),
        (None, [], [_, ..]) => "Duct runs".to_owned(),
        _ => "Runs".to_owned(),
    };
    let mut asked = PanelAsked::default();
    let height = view.height();
    let (side, top) = placement(ctx, view, heat_open);
    // What is left of the canvas under the panel's title bar; the body scrolls
    // when the list and the card together are taller.
    let room = (height - top - BELOW_PANEL).max(120.0);
    super::panel::Panel::new("view3d_wire_runs", title, view, side, PANEL_WIDTH)
        .top(top)
        .height(room.min(760.0))
        .above_card()
        .show(ctx, |ui| {
            // The list on top so another run is always one click away; the card of
            // the picked run below it. The panel's own scroll bar takes over when
            // together they are taller than the room.
            if let Some(l) = &list {
                asked.list = super::view3d_wires::list_body(
                    ui,
                    height,
                    !chosen.is_empty(),
                    l.all,
                    l.undrawn,
                    selection,
                );
                if !chosen.is_empty() || !ducts.is_empty() {
                    ui.separator();
                }
            }
            for d in ducts {
                let a = duct_body(ui, d);
                if a.select_cores.is_some() {
                    asked.card.select_cores = a.select_cores;
                }
                asked.card.clear |= a.clear;
                ui.separator();
            }
            if !chosen.is_empty() {
                let a = card_body(ui, chosen);
                if a.select_cores.is_some() {
                    asked.card.select_cores = a.select_cores;
                }
                asked.card.clear |= a.clear;
            }
        });
    Some(asked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view3d_wires::DuctUse;

    fn facts(id: u64) -> WireFacts {
        WireFacts {
            id,
            number: format!("W{id}"),
            from: "-F1:2".into(),
            to: "-K1:1".into(),
            colour: None,
            section_mm2: Some(2.5),
            length_mm: 640.0,
            bends: 3,
            ducts: 2,
            free_air: false,
            flat: false,
            duct_use: vec![
                DuctUse {
                    rail: 4,
                    place: "Duct at y 250 mm".into(),
                    ratio: Some(0.4),
                    share_pct: Some(20.0),
                    limit_pct: 50.0,
                    wires: 3,
                    tone: Tone::Ok,
                },
                DuctUse {
                    rail: 5,
                    place: "Duct at y 300 mm".into(),
                    ratio: Some(1.2),
                    share_pct: Some(60.0),
                    limit_pct: 50.0,
                    wires: 9,
                    tone: Tone::Over,
                },
            ],
            problems: vec![(
                Tone::Over,
                "Duct at y 300 mm carries 120 % of what it may: move wires out of it".into(),
            )],
            entry: None,
            basis: None,
            checks: None,
        }
    }

    fn basis(text: &str, mm: f64) -> Basis {
        Basis {
            text: text.into(),
            length_mm: mm,
            why: why(text, Gap::None),
            cable: None,
        }
    }

    #[test]
    fn a_routed_length_is_called_measured_and_an_estimate_is_not() {
        let mut f = facts(1);
        f.basis = Some(basis("routed", 640.0));
        let lines = card_lines(&f);
        assert_eq!(lines[0].1, "640 mm, routed");
        assert_eq!(lines[0].0, Tone::Ok);
        f.basis = Some(basis("estimate", 900.0));
        let lines = card_lines(&f);
        assert_eq!(lines[0].1, "900 mm, estimate");
        assert_eq!(lines[0].0, Tone::Busy);
        assert!(lines[1].1.starts_with("Not routed"));
    }

    #[test]
    fn a_cable_length_differs_from_the_inside_route_and_both_are_given() {
        let mut f = facts(1);
        f.basis = Some(basis("cable length", 8000.0));
        let lines = card_lines(&f);
        assert!(lines.iter().any(|(_, t)| t == "Inside the cabinet: 640 mm"));
    }

    #[test]
    fn every_duct_is_listed_with_its_tone_and_the_overfull_one_is_a_problem() {
        let f = facts(1);
        let lines = card_lines(&f);
        assert!(lines.contains(&(Tone::Idle, "Path: 2 ducts, 3 bends".to_owned())));
        assert!(
            lines.iter().any(|(t, s)| *t == Tone::Ok
                && s.starts_with("Duct at y 250 mm: 20 % full of 50 % allowed"))
        );
        assert!(
            lines
                .iter()
                .any(|(t, s)| *t == Tone::Over && s.starts_with("Duct at y 300 mm"))
        );
        assert!(
            lines
                .iter()
                .any(|(t, s)| *t == Tone::Over && s.contains("move wires out"))
        );
        // The hover card repeats only the ducts that need attention.
        let hover = f.lines().join("\n");
        assert!(hover.contains("y 300 mm") && !hover.contains("y 250 mm"));
    }

    #[test]
    fn the_card_always_says_what_is_not_checked() {
        let lines = card_lines(&facts(1));
        assert!(
            lines
                .last()
                .unwrap()
                .1
                .starts_with("Not checked: cable bend radius")
        );
    }

    #[test]
    fn an_entry_run_says_whether_the_outside_part_is_stated() {
        let e = EntryFacts {
            at: Some(Point::new(400.0, 560.0)),
            outside_mm: None,
        };
        assert_eq!(
            e.line(),
            "Drawn to the cabinet entry at x 400, y 560 mm; the part outside is not stated"
        );
        let e = EntryFacts {
            outside_mm: Some(3000.0),
            ..e
        };
        assert!(e.line().contains("outside it 3.00 m as stated, not drawn"));
        assert!(why("estimate", Gap::OutsideNotStated).contains("State it in Cabinet > Routes"));
        assert!(!why("estimate", Gap::None).contains("State it"));
    }

    #[test]
    fn the_basis_waits_until_the_project_stops_changing() {
        let mut s = Settle::default();
        assert!(!s.settled(5, 10.0), "a new revision starts the clock");
        assert!(!s.settled(5, 10.2));
        assert!(s.settled(5, 10.3));
        assert!(!s.settled(6, 10.4), "an edit restarts it");
        assert!(s.settled(6, 10.7));
    }

    #[test]
    fn a_cable_shorter_than_the_route_is_not_called_not_routed() {
        let w = why("estimate", Gap::CableTooShort);
        assert!(w.contains("shorter than the route inside the cabinet"));
        assert!(!w.starts_with("Not routed"));
        assert!(why("estimate", Gap::None).starts_with("Not routed"));
    }

    #[test]
    fn a_stated_cable_length_is_neutral_and_fill_says_it_is_a_planning_value() {
        let mut f = facts(1);
        f.basis = Some(basis("cable length", 8000.0));
        let lines = card_lines(&f);
        assert_eq!(lines[0].0, Tone::Idle);
        assert!(
            lines
                .iter()
                .any(|(_, t)| t.starts_with("Fill is a planning value"))
        );
        f.duct_use.clear();
        assert!(
            !card_lines(&f)
                .iter()
                .any(|(_, t)| t.starts_with("Fill is a planning"))
        );
    }

    #[test]
    fn weak_bases_are_the_ones_that_leave_something_out() {
        for t in ["estimate", "free air", "routed, no depth", "door, no gap"] {
            assert!(basis(t, 1.0).is_weak(), "{t}");
        }
        for t in ["routed", "cable length", "to entry + stated outside"] {
            assert!(!basis(t, 1.0).is_weak(), "{t}");
        }
    }

    #[test]
    fn a_selection_of_cores_of_one_cable_names_the_cable() {
        let cable = CableFacts {
            tag: "-W1".into(),
            cores_used: 3,
            cores_total: 4,
            length_mm: 8000.0,
            stated: true,
            wires: vec![1, 2, 3],
        };
        let mut a = facts(1);
        let mut b = facts(2);
        for f in [&mut a, &mut b] {
            let mut x = basis("cable length", 8000.0);
            x.cable = Some(cable.clone());
            f.basis = Some(x);
        }
        let got = common_cable(&[&a, &b]).expect("one cable");
        assert_eq!(
            got.line(),
            "Cable -W1: 3 of 4 cores wired, 8.00 m as bought, stated"
        );
        b.basis.as_mut().unwrap().cable = None;
        assert!(common_cable(&[&a, &b]).is_none());
    }
}
