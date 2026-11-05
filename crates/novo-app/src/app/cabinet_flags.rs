//! Problem triangles on the Cabinet plate (docs/design/problem-triangles.md,
//! lane T1).
//!
//! The physical checks (devices that overlap, leave the plate, sit too close,
//! overfill a rail, lie under a duct, ...) are Issues problems and reach the
//! flag store through `flags.rs`. This module turns the flags that belong to
//! one plate into triangles on its devices and its frame, and it adds the
//! flags the Issues engine does not raise: the broken Place rules and the
//! wiring openings a device covers. The Cabinet keeps no list of its own: its
//! count, its hover text, its table and its inspector all read the store.

use eframe::egui;
use novo_checks::ProblemKind;
use novo_core::{Project, StructureId};
use novo_flags::{
    Action, Anchor, Detail, Flag, FlagDraft, FlagId, FlagStore, Life, RollUp, Severity, Source,
};
use novo_layout::check::{Problem as RuleProblem, Violation};
use novo_layout::model::load;
use novo_layout::rules::RuleSet;
use novo_layout::state::CabinetLayout;

/// The Issues kinds that are about what lies on a mounting plate.
pub(super) const PLATE_KINDS: [ProblemKind; 14] = [
    ProblemKind::DeviceCollision,
    ProblemKind::DeviceOutsidePlate,
    ProblemKind::DevicesTooClose,
    ProblemKind::RailOverflow,
    ProblemKind::DeviceOffRail,
    ProblemKind::FieldDeviceInCabinet,
    ProblemKind::DuctOverlap,
    ProblemKind::OverfullDuct,
    ProblemKind::UnsizedDuct,
    ProblemKind::DeviceTooDeep,
    ProblemKind::DoorGapTight,
    ProblemKind::DepthUnknown,
    ProblemKind::PlateTooBig,
    ProblemKind::DoorSwing,
];

/// True for a flag that is about a plate: one of the kinds above, or one the
/// Cabinet raised itself (Place rules, wiring openings).
pub(super) fn is_plate_flag(flag: &Flag) -> bool {
    match &flag.source {
        Source::Layout => true,
        Source::Issues(key) => PLATE_KINDS.iter().any(|k| k.key() == key),
        _ => false,
    }
}

/// Where on the plate a flag is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Spot {
    /// The top right corner of a placed device.
    Device(u64),
    /// The corner of the plate frame.
    Frame,
}

/// The spots of `layout` (the plate of `structure`) a flag lands on.
fn spots(flag: &Flag, layout: &CabinetLayout, structure: StructureId) -> Vec<Spot> {
    let mut out = Vec::new();
    for anchor in &flag.anchors {
        let spot = match anchor {
            Anchor::Device(raw) => raw
                .parse::<u64>()
                .ok()
                .filter(|id| {
                    layout.placements.iter().any(|p| p.id == *id)
                        && !layout.off_plate.contains_key(id)
                })
                .map(Spot::Device),
            Anchor::Plate(raw, _) if *raw == structure.raw().to_string() => Some(Spot::Frame),
            _ => None,
        };
        if let Some(spot) = spot
            && !out.contains(&spot)
        {
            out.push(spot);
        }
    }
    out
}

/// The flags of this plate that are not waived, worst first.
pub(super) fn plate_flags<'a>(
    store: &'a FlagStore,
    layout: &CabinetLayout,
    structure: StructureId,
) -> Vec<&'a Flag> {
    store
        .list(false)
        .into_iter()
        .filter(|f| is_plate_flag(f) && !spots(f, layout, structure).is_empty())
        .collect()
}

/// Red and yellow count of this plate: what the Cabinet pill shows, and
/// the same flags the top bar counts.
pub(super) fn roll_up(store: &FlagStore, layout: &CabinetLayout, structure: StructureId) -> RollUp {
    let mut roll = RollUp::default();
    for f in plate_flags(store, layout, structure) {
        match f.severity {
            Severity::Red => roll.red += 1,
            Severity::Yellow => roll.yellow += 1,
        }
    }
    roll
}

/// The flags that sit on one placed device, for its hover text, the table row
/// and the inspector: `(red, unchecked, title, detail)`.
pub(super) fn device_flags(
    store: &FlagStore,
    layout: &CabinetLayout,
    structure: StructureId,
    device: u64,
) -> Vec<DeviceFlag> {
    plate_flags(store, layout, structure)
        .into_iter()
        .filter(|f| spots(f, layout, structure).contains(&Spot::Device(device)))
        .map(|f| DeviceFlag {
            id: f.id,
            red: f.severity == Severity::Red,
            unchecked: f.unchecked,
            title: f.title.clone(),
            detail: f.detail.0.join(" "),
        })
        .collect()
}

/// One flag as the table, the hover text and the inspector show it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DeviceFlag {
    pub(super) id: FlagId,
    pub(super) red: bool,
    pub(super) unchecked: bool,
    pub(super) title: String,
    pub(super) detail: String,
}

/// The triangles of one spot: one triangle, a count badge when several flags
/// share it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Cluster {
    pub(super) spot: Spot,
    /// Worst first.
    pub(super) ids: Vec<FlagId>,
    pub(super) red: bool,
    pub(super) unchecked: bool,
    /// One line per flag, for the hover text.
    pub(super) text: String,
    /// "Error: Devices overlap" for screen readers.
    pub(super) name: String,
}

/// The clusters to draw on this plate.
pub(super) fn clusters(
    store: &FlagStore,
    layout: &CabinetLayout,
    structure: StructureId,
) -> Vec<Cluster> {
    let mut by_spot: Vec<(Spot, Vec<&Flag>)> = Vec::new();
    for flag in plate_flags(store, layout, structure) {
        for spot in spots(flag, layout, structure) {
            match by_spot.iter_mut().find(|(s, _)| *s == spot) {
                Some((_, v)) => v.push(flag),
                None => by_spot.push((spot, vec![flag])),
            }
        }
    }
    by_spot.sort_by_key(|(s, _)| *s);
    by_spot
        .into_iter()
        .map(|(spot, flags)| {
            let red = flags.iter().any(|f| f.severity == Severity::Red);
            let unchecked = !red && flags.iter().all(|f| f.unchecked);
            let text = flags
                .iter()
                .map(|f| {
                    let detail = f.detail.0.join(" ");
                    if detail.is_empty() {
                        f.title.clone()
                    } else {
                        format!("{}: {detail}", f.title)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let first = flags[0].accessible_name();
            let name = if flags.len() > 1 {
                format!("{first} and {} more", flags.len() - 1)
            } else {
                first
            };
            Cluster {
                spot,
                ids: flags.iter().map(|f| f.id).collect(),
                red,
                unchecked,
                text,
                name,
            }
        })
        .collect()
}

/// Side of the triangle (design: 14 pt) and of its hit area.
const GLYPH: f32 = 14.0;
const HIT: f32 = 24.0;

/// What drawing the triangles found out.
#[derive(Default)]
pub(super) struct Drawn {
    /// A triangle was clicked (or Enter on it): the flag to open and where.
    pub(super) clicked: Option<(FlagId, egui::Pos2)>,
    /// The pointer is over a triangle: the device's own tooltip stays quiet.
    pub(super) hovered: bool,
    /// Screen centres of the triangles, for tests and lessons.
    pub(super) centres: Vec<(Spot, egui::Pos2)>,
}

/// Paints the triangles over the plate (`rect` is the canvas widget's screen
/// rectangle). `open` is the flag whose popover is up: a second click on the
/// same triangle steps to the next flag of its cluster.
pub(super) fn draw(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    view: &novo_canvas::Viewport,
    layout: &CabinetLayout,
    clusters: &[Cluster],
    open: Option<FlagId>,
) -> Drawn {
    let to_screen = |x: f64, y: f64| {
        let s = view.world_to_screen(novo_canvas::vec2(x as f32, y as f32));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    let pal = *novo_theme::Palette::of(ui);
    let painter = ui.painter_at(rect.expand(2.0));
    let mut out = Drawn::default();
    for c in clusters {
        let at = match c.spot {
            Spot::Device(id) => {
                let Some(p) = layout.placements.iter().find(|p| p.id == id) else {
                    continue;
                };
                let f = p.footprint();
                to_screen(f.x + f.w, f.y) + egui::vec2(-GLYPH * 0.7, GLYPH * 0.7)
            }
            Spot::Frame => to_screen(0.0, 0.0) + egui::vec2(-GLYPH * 0.9, -GLYPH * 0.9),
        };
        // A corner that scrolled out of the pane stays at its edge.
        let at = egui::pos2(
            at.x.clamp(rect.min.x + GLYPH * 0.6, rect.max.x - GLYPH * 0.6),
            at.y.clamp(rect.min.y + GLYPH * 0.6, rect.max.y - GLYPH * 0.6),
        );
        let hit = egui::Rect::from_center_size(at, egui::vec2(HIT, HIT));
        let key = c.ids.first().map_or(0, |i| i.0);
        let response = ui.interact(
            hit,
            egui::Id::new(("plate_flag", key, c.spot)),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, c.name.clone())
        });
        novo_theme::widgets::flag_triangle(&painter, at, GLYPH, c.red, c.unchecked, &pal);
        if c.ids.len() > 1 {
            let colour = if c.red { pal.error } else { pal.warning };
            let badge = at + egui::vec2(GLYPH * 0.6, -GLYPH * 0.55);
            painter.circle(badge, 6.0, pal.raised, egui::Stroke::new(1.0, colour));
            painter.text(
                badge,
                egui::Align2::CENTER_CENTER,
                c.ids.len().min(9).to_string(),
                egui::FontId::proportional(9.0),
                pal.text_primary,
            );
        }
        if response.has_focus() {
            painter.rect_stroke(
                hit.shrink(2.0),
                3.0,
                egui::Stroke::new(1.5, pal.accent),
                egui::StrokeKind::Inside,
            );
        }
        if response.hovered() {
            out.hovered = true;
            response.clone().on_hover_text(c.text.clone());
        }
        if response.clicked() {
            let next = open
                .and_then(|o| c.ids.iter().position(|i| *i == o))
                .map_or(0, |p| (p + 1) % c.ids.len());
            out.clicked = Some((c.ids[next], at));
        }
        out.centres.push((c.spot, at));
    }
    out
}

// ---------------------------------------------------------------------------
// Flags the Issues engine does not raise

/// A device is kept off the plate's flag list by its tag only in words, never
/// in the flag's identity: ids hash the anchors and this key.
fn draft(
    structure: StructureId,
    rule: String,
    anchors: Vec<Anchor>,
    title: &str,
    detail: String,
) -> FlagDraft {
    FlagDraft {
        severity: Severity::Yellow,
        unchecked: false,
        source: Source::Layout,
        rule,
        anchors,
        title: title.to_owned(),
        detail: Detail::new([detail]),
        actions: vec![Action::Locate, Action::Copy],
        waiver_key: None,
        life: Life::Live,
    }
    .with_plate(structure)
}

trait WithPlate {
    fn with_plate(self, structure: StructureId) -> Self;
}

impl WithPlate for FlagDraft {
    /// Two plates may hold the same rule problem; the plate keeps them apart.
    fn with_plate(mut self, structure: StructureId) -> Self {
        self.rule = format!("{}@{}", self.rule, structure.raw());
        self
    }
}

/// The flags of the Place rules and of the wiring openings, for every plate
/// of the project.
pub(super) fn layout_drafts(project: &Project, rules: &RuleSet) -> Vec<FlagDraft> {
    let tag = |id: u64| {
        project
            .device(novo_core::DeviceId::from(id))
            .map_or_else(|| "A device".to_owned(), |d| d.tag.to_string())
    };
    let mut out = Vec::new();
    for s in project.structures().filter(|s| s.plate.is_some()) {
        let Some(layout) = load(project, s.id) else {
            continue;
        };
        let device = |id: u64| Anchor::Device(id.to_string());
        for c in &layout.report().opening_conflicts {
            out.push(draft(
                s.id,
                format!("opening.{}.{}", c.opening, c.device),
                vec![device(c.device)],
                "Covers a wiring opening",
                c.message(&tag),
            ));
        }
        for (i, text) in layout.opening_problems.iter().enumerate() {
            out.push(draft(
                s.id,
                format!("opening.property.{i}"),
                vec![Anchor::Plate(s.id.raw().to_string(), [0, 0])],
                "Wiring opening unclear",
                text.clone(),
            ));
        }
        for v in super::cabinet_rules::judge(project, s.id, rules).violations {
            out.push(rule_draft(s.id, &v, rules, &tag));
        }
    }
    out
}

fn rule_draft(
    structure: StructureId,
    v: &Violation,
    rules: &RuleSet,
    tag: &dyn Fn(u64) -> String,
) -> FlagDraft {
    let (title, key) = match &v.problem {
        RuleProblem::WrongZone {
            device,
            against,
            hot,
            ..
        } => (
            if *hot {
                "Hot device in the wrong zone"
            } else {
                "Device in the wrong zone"
            },
            format!("zone.{device}.{against}"),
        ),
        RuleProblem::Clearance {
            device,
            side,
            neighbour,
            ..
        } => (
            "Too little room around it",
            format!("clearance.{device}.{}.{neighbour}", side.as_str()),
        ),
        RuleProblem::Separation { power, signal, .. } => {
            ("Ducts too close", format!("separation.{power}.{signal}"))
        }
    };
    let devices = v.devices();
    let anchors = if devices.is_empty() {
        vec![Anchor::Plate(structure.raw().to_string(), [0, 0])]
    } else {
        devices
            .into_iter()
            .map(|d| Anchor::Device(d.to_string()))
            .collect()
    };
    draft(
        structure,
        format!("place.{key}"),
        anchors,
        title,
        v.describe(rules, tag),
    )
}

/// The devices and plate a plate flag names, for Locate: `(structure raw id
/// if the flag names a plate, device ids)`.
pub(super) fn targets(flag: &Flag) -> (Option<u64>, Vec<u64>) {
    let mut plate = None;
    let mut devices = Vec::new();
    for a in &flag.anchors {
        match a {
            Anchor::Plate(raw, _) => plate = raw.parse().ok().or(plate),
            Anchor::Device(raw) => devices.extend(raw.parse::<u64>().ok()),
            _ => {}
        }
    }
    (plate, devices)
}

/// The plate a placed device sits on.
pub(super) fn plate_of(project: &Project, device: u64) -> Option<StructureId> {
    project
        .structures()
        .filter(|s| s.plate.is_some())
        .find(|s| load(project, s.id).is_some_and(|l| l.placements.iter().any(|p| p.id == device)))
        .map(|s| s.id)
}
