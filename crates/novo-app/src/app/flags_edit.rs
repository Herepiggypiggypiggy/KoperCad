//! Refused edits as transient triangles (problem triangles lane T7,
//! docs/design/problem-triangles.md section 7).
//!
//! An edit that is refused ("Rename: that tag is taken", "Could not paste") used
//! to raise a red or yellow toast. It is a flag now: a triangle on the
//! selection (or, with nothing selected, on the canvas corner where the tool in
//! hand is), with the reason in the popover. It goes with the next input. The
//! tool's own refusal (the editor notice) uses the same triangle and goes with
//! the notice. Both stay in the message log; neither is a toast.
//!
//! This module also resolves the anchors the schematic could not draw before:
//! a spot on a page, a device, a terminal and a cable.

use eframe::egui;
use novo_canvas::{Aabb, MarkTarget, vec2};
use novo_core::{AnyId, CableId, DeviceId, PageId, Project, SymbolId, TerminalId, WireId};
use novo_flags::{Action, Anchor, Detail, Flag, FlagDraft, FlagId, Life, Severity, Source};

use super::{Level, NovoApp};

/// The control anchor of the tool in hand (the canvas corner).
pub(super) const TOOL_CONTROL: &str = "tool";

/// The rule key of a refused edit.
const RULE: &str = "edit.refused";

/// The most triangles one refusal draws (a big selection stays readable).
const MOST_ANCHORS: usize = 3;

/// The transient flags the app holds.
#[derive(Default)]
pub(super) struct EditFlags {
    /// Frames drawn so far (counted by [`NovoApp::expire_refusal`]).
    frame: u64,
    /// A refused edit, and the frame it was raised in. Goes with the next input.
    transient: Option<(FlagId, u64)>,
    /// The refusal of the tool in hand, and the text it was built from.
    tool: Option<(FlagId, String)>,
}

impl EditFlags {
    /// The ids of the flags that are drawn at the canvas corner.
    fn corner(&self, store: &novo_flags::FlagStore) -> Vec<FlagId> {
        [self.transient.map(|t| t.0), self.tool.as_ref().map(|t| t.0)]
            .into_iter()
            .flatten()
            .filter(|id| {
                store
                    .get(*id)
                    .is_some_and(|f| f.anchors.contains(&Anchor::Control(TOOL_CONTROL.into())))
            })
            .collect()
    }
}

/// Kinds of the cabinet, routing and EMC lanes: their device anchors belong on
/// the plate and in 3D, not on the schematic symbols of the device.
fn is_physical(source: &Source) -> bool {
    use novo_checks::ProblemKind as K;
    const PHYSICAL: [K; 19] = [
        K::DeviceCollision,
        K::DeviceOffRail,
        K::DeviceOutsidePlate,
        K::DeviceTooDeep,
        K::DevicesTooClose,
        K::DoorGapTight,
        K::DoorSwing,
        K::DuctOverlap,
        K::FieldDeviceInCabinet,
        K::OverfullDuct,
        K::PlateTooBig,
        K::RailOverflow,
        K::UnplacedDevice,
        K::UnsizedDuct,
        K::WireThroughDevice,
        K::WireWithoutDuct,
        K::DepthUnknown,
        K::GuessedWire,
        K::Emc,
    ];
    matches!(source, Source::Issues(key) if PHYSICAL.iter().any(|k| k.key() == key))
}

/// Where the triangles of `flag` go on `page`: one per anchor that is drawn
/// there, each object once (two anchors on one symbol are one triangle).
pub(super) fn mark_targets(project: &Project, flag: &Flag, page: PageId) -> Vec<MarkTarget> {
    let physical = is_physical(&flag.source);
    let mut out: Vec<MarkTarget> = Vec::new();
    for anchor in &flag.anchors {
        let target = match anchor {
            Anchor::Pin(raw, pin)
                if super::flags_wiring::pin_area(project, raw, *pin)
                    .is_some_and(|(at, _)| at == page) =>
            {
                super::flags_wiring::pin_area(project, raw, *pin)
                    .map(|(_, area)| MarkTarget::Area(area))
            }
            Anchor::Symbol(raw) | Anchor::Pin(raw, _) => raw
                .parse()
                .ok()
                .map(SymbolId)
                .and_then(|s| project.symbol(s))
                .filter(|s| s.page == page)
                .map(|s| item(AnyId::Symbol(s.id))),
            Anchor::Wire(raw) => raw
                .parse()
                .ok()
                .map(WireId)
                .and_then(|w| project.wire(w))
                .filter(|w| w.page == page)
                .map(|w| item(AnyId::Wire(w.id))),
            Anchor::Device(raw) if !physical => raw.parse().ok().and_then(|d| {
                project
                    .symbols_of_device(DeviceId(d))
                    .into_iter()
                    .find(|s| s.page == page)
                    .map(|s| item(AnyId::Symbol(s.id)))
            }),
            Anchor::Terminal(raw) if !physical => raw.parse().ok().and_then(|t| {
                project
                    .symbols_of_terminal(TerminalId(t))
                    .into_iter()
                    .find(|s| s.page == page)
                    .map(|s| item(AnyId::Symbol(s.id)))
            }),
            Anchor::Cable(raw) if !physical => raw.parse().ok().and_then(|c| {
                let cable = CableId(c);
                project
                    .wires()
                    .find(|w| w.page == page && project.cable_of_wire(w.id) == Some(cable))
                    .map(|w| item(AnyId::Wire(w.id)))
            }),
            Anchor::Page(raw, [x, y]) if raw.parse().ok().map(PageId) == Some(page) => {
                let at = vec2(*x as f32, *y as f32);
                Some(MarkTarget::Area(Aabb::new(at, at + vec2(1.0, 1.0))))
            }
            _ => None,
        };
        if let Some(target) = target
            && !out.contains(&target)
        {
            out.push(target);
        }
    }
    out
}

fn item(id: AnyId) -> MarkTarget {
    MarkTarget::Item(novo_canvas::ItemId(id.raw()))
}

/// The title and the detail line of a refusal text: "Could not change it: the
/// tag is taken" is a title and a reason; any other text is the reason under a
/// short generic title.
pub(super) fn split_refusal(level: Level, text: &str) -> (String, String) {
    let fallback = match level {
        Level::Error => "Could not do that",
        _ => "Not done",
    };
    if let Some((head, tail)) = text.split_once(": ")
        && head.chars().count() <= novo_flags::TITLE_MAX
        && !tail.trim().is_empty()
        && head.chars().all(|c| c != '"')
    {
        return (head.to_owned(), tail.trim().to_owned());
    }
    (fallback.to_owned(), text.trim().to_owned())
}

fn draft(level: Level, text: &str, anchors: Vec<Anchor>) -> FlagDraft {
    // The engines quote with backticks; the popover quotes with plain marks.
    let (title, detail) = split_refusal(level, &text.replace('`', "\""));
    FlagDraft {
        severity: if level == Level::Error {
            Severity::Red
        } else {
            Severity::Yellow
        },
        unchecked: false,
        source: Source::Edit,
        rule: RULE.to_owned(),
        anchors,
        title,
        detail: Detail::new([detail]),
        actions: vec![Action::Copy],
        waiver_key: None,
        life: Life::Until,
    }
}

impl NovoApp {
    /// A refused edit: a triangle on the selection (the canvas corner when
    /// nothing is selected) that goes with the next input. Replaces the
    /// warning and error toasts of the edits. The text stays in the log.
    pub(super) fn refuse(&mut self, level: Level, text: impl Into<String>) {
        let anchors = self.refusal_anchors();
        self.refuse_at(level, text.into(), anchors);
    }

    /// [`Self::refuse`] with a title of its own, for a reason that has no
    /// "Head: reason" shape.
    pub(super) fn refuse_titled(&mut self, level: Level, title: &str, reason: impl Into<String>) {
        let anchors = self.refusal_anchors();
        self.refuse_at(level, format!("{title}: {}", reason.into()), anchors);
    }

    /// [`Self::refuse`] on the tool in hand (the canvas corner): for a refusal
    /// from a dialog or a menu, which has no selection to point at.
    pub(super) fn refuse_tool(&mut self, level: Level, text: impl Into<String>) {
        let anchors = vec![Anchor::Control(TOOL_CONTROL.into())];
        self.refuse_at(level, text.into(), anchors);
    }

    fn refuse_at(&mut self, level: Level, text: String, anchors: Vec<Anchor>) {
        // The log keeps it (and the person's Messages list); no toast.
        let shown = self.log_only(level, text);
        let edit = &mut self.wb.flags.edit;
        let born = edit.frame;
        if let Some((old, _)) = edit.transient.take() {
            self.wb.flags.store.clear(old);
        }
        let id = self.wb.flags.store.set(draft(level, &shown, anchors));
        self.wb.flags.edit.transient = Some((id, born));
    }

    /// The flag of the refusal being shown, for tests.
    #[cfg(test)]
    pub(super) fn refusal_flag(&self) -> Option<&novo_flags::Flag> {
        let (id, _) = self.wb.flags.edit.transient?;
        self.wb.flags.store.get(id)
    }

    /// What a refusal points at: the selected symbols and wires, else the tool.
    fn refusal_anchors(&self) -> Vec<Anchor> {
        let mut anchors: Vec<Anchor> = self
            .selection()
            .into_iter()
            .filter_map(|id| match id {
                AnyId::Symbol(_) => Some(Anchor::Symbol(id.raw().to_string())),
                AnyId::Wire(_) => Some(Anchor::Wire(id.raw().to_string())),
                _ => None,
            })
            .take(MOST_ANCHORS)
            .collect();
        if anchors.is_empty() {
            anchors.push(Anchor::Control(TOOL_CONTROL.into()));
        }
        anchors
    }

    /// Keeps the flag of the tool in hand in step with the editor's notice:
    /// raised when a refusal appears, gone when it does.
    pub(super) fn sync_tool_refusal(&mut self, refused: bool, text: &str) {
        let edit = &mut self.wb.flags.edit;
        if !refused {
            if let Some((id, _)) = edit.tool.take() {
                self.wb.flags.store.clear(id);
            }
            return;
        }
        if edit.tool.as_ref().is_some_and(|(_, t)| t == text) {
            return;
        }
        if let Some((id, _)) = edit.tool.take() {
            self.wb.flags.store.clear(id);
        }
        let anchors = vec![Anchor::Control(TOOL_CONTROL.into())];
        let id = self
            .wb
            .flags
            .store
            .set(draft(Level::Warning, text, anchors));
        self.wb.flags.edit.tool = Some((id, text.to_owned()));
    }

    /// The refused edit goes with the next key or click, unless that click is
    /// on its triangle (which opens the popover; the popover keeps it). A click
    /// counts on release, when the triangle has already seen it.
    pub(super) fn expire_refusal(&mut self, ctx: &egui::Context) {
        self.wb.flags.edit.frame += 1;
        let Some((id, born)) = self.wb.flags.edit.transient else {
            return;
        };
        if self.wb.flags.edit.frame <= born + 1
            || self.wb.flags.open.is_some_and(|(open, _)| open == id)
        {
            return;
        }
        let input = ctx.input(|i| {
            i.pointer.any_released()
                || i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)
                    )
                })
        });
        if input {
            self.wb.flags.edit.transient = None;
            self.wb.flags.store.clear(id);
        }
    }

    /// The triangles of the refusals that point at the tool, at the top left
    /// of the canvas where the long-refusal card used to be. A click opens the
    /// popover.
    pub(super) fn flag_corner(&mut self, ctx: &egui::Context) {
        let Some((_, canvas)) = self.wb.canvas_area else {
            return;
        };
        let ids = self.wb.flags.edit.corner(&self.wb.flags.store);
        let pal = crate::theme::ui_palette();
        for (n, id) in ids.into_iter().enumerate() {
            let Some(flag) = self.wb.flags.store.get(id).cloned() else {
                continue;
            };
            let pos = canvas.left_top() + egui::vec2(12.0 + 28.0 * n as f32, 12.0);
            let size = egui::vec2(24.0, 24.0);
            let response = egui::Area::new(egui::Id::new("flag_corner").with(n))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                    novo_theme::widgets::flag_triangle(
                        ui.painter(),
                        rect.center(),
                        novo_canvas::marks::GLYPH_SIZE,
                        flag.severity == Severity::Red,
                        flag.unchecked,
                        pal,
                    );
                    response
                })
                .inner;
            let response = response.on_hover_text(format!(
                "{}: {}",
                flag.accessible_name(),
                flag.detail.0.join(" ")
            ));
            if response.clicked() {
                self.wb.flags.open = Some((id, response.interact_pointer_pos().unwrap_or(pos)));
            }
        }
    }
}
