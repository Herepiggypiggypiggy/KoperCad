//! Problem triangles, wiring lane (docs/design/problem-triangles.md, T5).
//!
//! Three things live here:
//!
//! * the wire-end checks (wire size at a terminal, terminal point full,
//!   ferrule) are anchored on the pin the wire lands on, so the triangle sits
//!   on the wire end instead of on the whole wire or symbol;
//! * a failed or empty export (Wire-it list, wire cut, wire production,
//!   wiring plan, workshop pack) is a flag on the job, kept until the same job
//!   succeeds, with Retry; the top bar shows the worst of them beside the
//!   Issues pill and the Wire-it strip shows its own on the Print button;
//! * an edit the wire panel refuses is a flag on the field it came from.

use std::collections::HashMap;

use eframe::egui;
use novo_checks::{Problem, ProblemKind};
use novo_core::{AnyId, Endpoint, Project, StructureId};
use novo_flags::{
    Action, Anchor, Detail, Flag, FlagDraft, FlagId, FlagStore, Life, Severity, Source,
};

use super::NovoApp;

/// What Retry runs again for a job flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum JobRetry {
    WireIt,
    WireCut,
    WireProduction,
    WiringDxf(StructureId),
    ProductionPage,
    Workshop,
}

/// Job keys: the anchor of the flag.
pub(super) mod job {
    pub(in crate::app) const WIRE_IT: &str = "wire_it.print";
    pub(in crate::app) const WIRE_CUT: &str = "wire_cut";
    pub(in crate::app) const WIRE_PRODUCTION: &str = "wire_production";
    pub(in crate::app) const WIRING_DXF: &str = "wiring_dxf";
    pub(in crate::app) const PRODUCTION_PAGE: &str = "wire_production_page";
    pub(in crate::app) const WORKSHOP: &str = "workshop";
}

/// Kinds that sit on the end of a wire at a connection point.
pub(super) fn is_wire_end_kind(kind: ProblemKind) -> bool {
    matches!(
        kind,
        ProblemKind::WireRange | ProblemKind::PointOccupied | ProblemKind::FerruleUse
    )
}

/// The pins a wire-end problem is about, as anchors. Empty when the model no
/// longer has what the problem names (the caller keeps its default anchors).
pub(super) fn pin_anchors(project: &Project, p: &Problem) -> Vec<Anchor> {
    if !is_wire_end_kind(p.kind) {
        return Vec::new();
    }
    let symbol = p.at.iter().find_map(|id| match id {
        AnyId::Symbol(s) => Some(*s),
        _ => None,
    });
    let Some(symbol) = symbol else {
        return Vec::new();
    };
    let wires: Vec<_> =
        p.at.iter()
            .filter_map(|id| match id {
                AnyId::Wire(w) => project.wire(*w),
                _ => None,
            })
            .collect();
    // How many of the named wires end on each pin of the symbol.
    let mut pins: std::collections::BTreeMap<u16, usize> = Default::default();
    for wire in &wires {
        for end in [wire.from, wire.to] {
            if let Endpoint::Pin { symbol: s, pin } = end
                && s == symbol
            {
                *pins.entry(pin).or_default() += 1;
            }
        }
    }
    let most = if p.kind == ProblemKind::PointOccupied {
        // The pin they share.
        pins.values().copied().max().unwrap_or(0).max(1)
    } else {
        1
    };
    let raw = AnyId::from(symbol).raw().to_string();
    pins.into_iter()
        .filter(|(_, n)| *n >= most)
        .map(|(pin, _)| Anchor::Pin(raw.clone(), u32::from(pin)))
        .collect()
}

/// The world box a glyph sits on for a pin: a point, so the glyph lands at
/// the wire end.
pub(super) fn pin_area(
    project: &Project,
    raw: &str,
    pin: u32,
) -> Option<(novo_core::PageId, novo_canvas::Aabb)> {
    use novo_canvas::{Aabb, Vec2};
    let symbol = project.symbol(novo_core::SymbolId(raw.parse().ok()?))?;
    let at = symbol.pin_position(u16::try_from(pin).ok()?)?;
    let (x, y) = (at.x as f32, at.y as f32);
    Some((
        symbol.page,
        Aabb::new(Vec2::new(x - 0.5, y - 0.5), Vec2::new(x + 0.5, y + 0.5)),
    ))
}

/// The leading triangle of a list row or beside a field: 20 pt, the glyph of
/// the flag's severity. Returns the response; the caller opens the popover.
pub(super) fn triangle_button(ui: &mut egui::Ui, flag: &Flag) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    novo_theme::widgets::flag_triangle(
        ui.painter(),
        rect.center(),
        15.0,
        flag.severity == Severity::Red,
        flag.unchecked,
        novo_theme::Palette::of(ui),
    );
    let name = flag.accessible_name();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    response.on_hover_text(name)
}

/// The popover is asked for at the triangle's right edge.
pub(super) fn ask_popover(open: &mut Option<(FlagId, egui::Pos2)>, id: FlagId, r: &egui::Response) {
    if r.clicked() {
        *open = Some((id, r.rect.right_bottom()));
    }
}

/// Where the flags of the wire panel's fields are kept between frames.
pub(super) type FieldFlags = HashMap<&'static str, FlagId>;

impl NovoApp {
    /// Raises (or renews) the flag of a job: red for a failure, yellow for
    /// "nothing to do". It stays until the same job succeeds or is dismissed.
    pub(super) fn wiring_job_flag(
        &mut self,
        job: &'static str,
        red: bool,
        title: &str,
        detail: &str,
        retry: JobRetry,
    ) {
        // The flag is the one every failed job gets (flags_jobs): the pill in the
        // top bar, with Retry, Copy details and Dismiss. Retry runs `retry`.
        self.wb
            .flags
            .retry
            .insert(super::flags_jobs::job_flag_id(job), retry);
        self.job_fail_red(job, title, red, detail);
    }

    /// The job went well: its flag goes.
    pub(super) fn job_clear(&mut self, job: &'static str) {
        self.job_ok(job);
    }

    /// Runs a job again from its popover.
    pub(super) fn retry_job(&mut self, retry: JobRetry) {
        match retry {
            JobRetry::WireIt => {
                self.ask_wire_it();
            }
            JobRetry::WireCut => {
                self.export_wire_cut();
            }
            JobRetry::WireProduction => {
                self.export_wire_production();
            }
            JobRetry::WiringDxf(s) => {
                self.export_wiring_dxf(s);
            }
            JobRetry::ProductionPage => self.add_wire_production_page(),
            JobRetry::Workshop => {
                self.export_workshop();
            }
        }
    }

    /// Raises the flag of a field the wire panel refused.
    pub(super) fn wire_field_flag(&mut self, field: &'static str, title: &str, detail: String) {
        let id = self.wb.flags.store.set(FlagDraft {
            severity: Severity::Yellow,
            unchecked: false,
            source: Source::Shell,
            rule: format!("wire_panel.{field}"),
            anchors: vec![Anchor::Control(format!("wire_panel.{field}"))],
            title: title.to_owned(),
            detail: Detail::new([detail]),
            actions: vec![Action::Copy],
            waiver_key: None,
            life: Life::Until,
        });
        if let Some(old) = self.wb.wire_panel.errors.insert(field, id)
            && old != id
        {
            self.wb.flags.store.clear(old);
        }
    }

    /// The field took its edit: its flag goes.
    pub(super) fn wire_field_clear(&mut self, field: &'static str) {
        if let Some(id) = self.wb.wire_panel.errors.remove(field) {
            self.wb.flags.store.clear(id);
        }
    }

    /// Every field flag goes (another wire is selected, or none).
    pub(super) fn wire_fields_clear(&mut self) {
        for (_, id) in std::mem::take(&mut self.wb.wire_panel.errors) {
            self.wb.flags.store.clear(id);
            if self.wb.flags.open.is_some_and(|(open, _)| open == id) {
                self.wb.flags.open = None;
            }
        }
    }
}

/// The flags of the wire panel's fields for one frame, and the triangle the
/// person clicked.
#[derive(Default)]
pub(super) struct Fields {
    flags: HashMap<&'static str, Flag>,
    pub(super) asked: Option<(FlagId, egui::Pos2)>,
}

impl Fields {
    pub(super) fn of(store: &FlagStore, errors: &FieldFlags) -> Self {
        Self {
            flags: errors
                .iter()
                .filter_map(|(field, id)| Some((*field, store.get(*id)?.clone())))
                .collect(),
            asked: None,
        }
    }

    /// The triangle beside `field`, if the panel refused its last edit.
    pub(super) fn mark(&mut self, ui: &mut egui::Ui, field: &str) {
        if let Some(flag) = self.flags.get(field) {
            let response = triangle_button(ui, flag);
            ask_popover(&mut self.asked, flag.id, &response);
        }
    }
}
