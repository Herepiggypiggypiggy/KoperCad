//! The inspector (DESIGN.md §3): an overlay that appears with a selection
//! and slides away when it clears. It opens on the side away from the
//! selection so it never hides what was selected. Relations come first
//! (where the device is drawn: its main function and contacts), then the
//! part, then a search over every other property. The tag and the
//! description are edited in place, each edit one undo step.

use eframe::egui;
use novo_canvas::model::SymbolArt;
use novo_core::{AnyId, DeviceId, DeviceKind, DeviceTag, Project, SymbolRole, Value};
use novo_theme::{icons, widgets};
use novo_xref::{Place, XrefIndex};

use super::part_picker::{Candidate, PartPicker, device_candidates, picker_ui};
use super::symbol_preview::{self, SymbolPreview, symbol_preview};
use super::xref::xref_of;
use super::{Level, NovoApp};

/// The inspector's width.
pub(super) const INSPECTOR_WIDTH: f32 = 276.0;
/// Its distance from the canvas edges; the top clears the outline toggle
/// and the HUD.
const MARGIN: f32 = 12.0;
const TOP: f32 = 40.0;

/// What the inspector shows for one device.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DeviceInfo {
    pub device: DeviceId,
    pub tag: String,
    pub description: String,
    /// `Manufacturer order-number`, empty without a part.
    pub part: String,
    /// Where the device is drawn: its main function, then the rest in
    /// reading order.
    pub relations: Vec<Relation>,
    pub properties: Vec<(String, String)>,
    /// For a property the device sets differently from its part: the
    /// part's own value, which the reset arrow goes back to.
    pub part_values: Vec<(String, String)>,
    /// The set time in seconds, for a device with a timer contact.
    pub delay: Option<f64>,
    /// For a plug or socket: the tag of its other half (`plug.mate`),
    /// empty when none is named. `None` for any other device.
    pub mate: Option<String>,
    /// The calculation inputs ([`DEVICE_INPUTS`]) as the checks read them:
    /// the device's own value, else its part's. `(value, from the part)`.
    pub calc_inputs: Vec<(String, bool)>,
}

/// The numbers a device states for the calculations: property key, label,
/// unit and what a typical entry looks like. `trip_curve` is a letter.
pub(super) const DEVICE_INPUTS: [(&str, &str, &str, &str); 8] = [
    ("rated_current_a", "Rated current", "A", "16"),
    ("breaking_capacity_ka", "Breaking capacity", "kA", "10"),
    ("trip_curve", "Trip curve", "B C D", "C"),
    ("sccr_ka", "SCCR", "kA", "10"),
    (
        "let_through_i2t",
        "Let-through I\u{b2}t",
        "A\u{b2}s",
        "30000",
    ),
    ("setting_min_a", "Relay setting min", "A", "6"),
    ("setting_max_a", "Relay setting max", "A", "10"),
    ("ac3_current_a", "AC-3 current", "A", "25"),
];

/// Reads an entry for a calculation input: empty clears it, `trip_curve`
/// takes B, C or D, every other input a positive number.
pub(super) fn parse_device_input(key: &str, text: &str) -> Result<Option<Value>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if key == "trip_curve" {
        return match text.to_ascii_uppercase().as_str() {
            c @ ("B" | "C" | "D") => Ok(Some(Value::Text(c.to_owned()))),
            _ => Err(format!("\"{text}\" is not a trip curve: use B, C or D")),
        };
    }
    match text.replace(',', ".").parse::<f64>() {
        Ok(v) if v.is_finite() && v > 0.0 => Ok(Some(Value::Float(v))),
        _ => Err(format!("\"{text}\" is not a positive number")),
    }
}

/// One place the device is drawn: `NO 13-14 2.6`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Relation {
    pub row: String,
    pub place: Place,
    /// The main function (a contactor's coil).
    pub main: bool,
    /// What it is, `Coil`, `NO contact`, `Main contacts`.
    pub what: String,
    /// Its connection designations, `13-14`; empty for a coil.
    pub pins: String,
    /// Where: `Page 3 Control circuit, column 4`.
    pub page: String,
    /// A picture of the symbol as drawn there.
    pub preview: Option<SymbolPreview>,
}

/// The inspector's fields while they are edited.
#[derive(Debug, Default)]
pub(super) struct InspectorState {
    device: Option<DeviceId>,
    tag: String,
    description: String,
    delay: String,
    mate: String,
    pub(super) filter: String,
    /// The pin: a pinned inspector stays where it is with nothing selected
    /// and shows the page (DESIGN.md §3).
    pub(super) pinned: bool,
    /// The Part row's search list.
    picker: PartPicker,
    /// What was shown last, drawn while the panel slides away.
    leaving: Option<(DeviceInfo, bool)>,
}

/// What an inspector click or edit asks for.
enum InspectorAction {
    Go(Place),
    Rename(String),
    Describe(String),
    SetDelay(f64),
    /// The tag of a plug's other half; empty clears it.
    SetMate(String),
    /// The reset arrow of an overridden property: back to the part's value.
    ResetProperty(String),
    Pin(bool),
    /// A part picked in the Part row.
    AssignPart(novo_parts::PartKey),
    /// A calculation input edited: property key and the text typed.
    SetInput(&'static str, String),
}

/// The one device the selection draws, if it draws exactly one.
pub(super) fn selected_device(project: &Project, selection: &[AnyId]) -> Option<DeviceId> {
    let mut devices = selection.iter().filter_map(|id| match id {
        AnyId::Device(d) => Some(*d),
        AnyId::Symbol(s) => match project.symbol(*s)?.role {
            SymbolRole::Device { device, .. } => Some(device),
            _ => None,
        },
        _ => None,
    });
    let first = devices.next()?;
    devices.all(|d| d == first).then_some(first)
}

pub(super) fn device_info(
    project: &Project,
    index: Option<&XrefIndex>,
    art: Option<&dyn SymbolArt>,
    device: DeviceId,
) -> Option<DeviceInfo> {
    let d = project.device(device)?;
    let part = d
        .parts
        .first()
        .and_then(|&p| project.part(p))
        .map(|p| format!("{} {}", p.manufacturer, p.order_number))
        .unwrap_or_default();
    let relations: Vec<_> = index
        .map(|index| {
            index
                .device_references(device)
                .into_iter()
                .map(|r| {
                    let title = project
                        .page(r.location.page)
                        .map(|p| p.title.as_str())
                        .unwrap_or_default();
                    let at = page_line(&r.location, title);
                    let what = match (r.main, r.kind.label()) {
                        (true, _) => "Coil".to_owned(),
                        (false, "main") => "Main contacts".to_owned(),
                        (false, "") => "Symbol".to_owned(),
                        (false, k) => format!("{k} contact"),
                    };
                    let preview = art.and_then(|art| symbol_preview(project, art, r.symbol));
                    (
                        r.row(),
                        Place::of(r),
                        r.main,
                        what,
                        r.pins.clone(),
                        at,
                        preview,
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // The main function first (a contactor's coil), then the rest in
    // reading order.
    let mut relations: Vec<Relation> = relations
        .into_iter()
        .map(|(row, place, main, what, pins, page, preview)| Relation {
            row,
            place,
            main,
            what,
            pins,
            page,
            preview,
        })
        .collect();
    relations.sort_by_key(|r| !r.main);
    let properties = d
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), value_text(v)))
        .collect();
    let part_values = d
        .parts
        .first()
        .and_then(|&p| project.part(p))
        .map(|p| {
            d.properties
                .iter()
                .filter_map(|(k, v)| {
                    let own = p.properties.get(k)?;
                    (own != v).then(|| (k.clone(), value_text(own)))
                })
                .collect()
        })
        .unwrap_or_default();
    let part_props = d.parts.first().and_then(|&p| project.part(p));
    let calc_inputs = DEVICE_INPUTS
        .iter()
        .map(|(key, ..)| match d.properties.get(*key) {
            Some(v) => (value_text(v), false),
            None => part_props
                .and_then(|p| p.properties.get(*key))
                .map_or((String::new(), false), |v| (value_text(v), true)),
        })
        .collect();
    let timed = project
        .symbols()
        .filter(|s| s.device() == Some(device))
        .any(novo_sim::is_timer_contact);
    let delay = timed.then(|| novo_sim::delay_seconds(project, device));
    Some(DeviceInfo {
        device,
        tag: d.tag.to_string(),
        description: d.description.clone(),
        part,
        relations,
        properties,
        part_values,
        delay,
        mate: (d.kind == DeviceKind::Connector).then(|| match d.properties.get("plug.mate") {
            Some(Value::Text(t)) => t.clone(),
            _ => String::new(),
        }),
        calc_inputs,
    })
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(t) => t.clone(),
    }
}

impl NovoApp {
    /// The inspector over the canvas, while the selection draws one device.
    pub(super) fn inspector(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.is_open() {
            return;
        }
        let selection = self.selection();
        let device = self
            .backend
            .document()
            .and_then(|doc| selected_device(doc.project(), &selection));
        // An edit not yet committed (the field still has the cursor) is kept
        // when the selection moves off the device, or clears.
        if let Some(editing) = self.wb.inspector.device
            && device != Some(editing)
        {
            self.commit_inspector_edit(editing);
        }
        let slide = ctx.animate_bool_with_time(
            egui::Id::new("inspector_slide"),
            device.is_some() || self.wb.inspector.pinned,
            0.15,
        );
        if device.is_none() && self.wire_panel(ctx, canvas) {
            return;
        }
        let Some(device) = device else {
            let pinned = self.wb.inspector.pinned;
            let leaving = self.wb.inspector.leaving.take();
            self.wb.inspector = InspectorState {
                pinned,
                ..InspectorState::default()
            };
            if pinned {
                self.page_inspector(ctx, canvas);
            } else if slide > 0.0
                && let Some((info, left)) = leaving
            {
                // The panel slides away with what it last showed.
                let mut state = InspectorState::default();
                let mut actions = Vec::new();
                self.inspector_area(
                    ctx,
                    canvas,
                    left,
                    slide,
                    (&info, &[]),
                    &mut state,
                    &mut actions,
                );
                self.wb.inspector.leaving = Some((info, left));
            }
            return;
        };
        if self.wb.preview_art.is_none()
            && let Some(library) = self.library()
        {
            let art =
                novo_editor::art::LibraryArt::new(library, crate::theme::model_style().symbol);
            self.wb.preview_art = Some(art);
        }
        let info = {
            let index = xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref);
            let Some(doc) = self.backend.document() else {
                return;
            };
            let art = self.wb.preview_art.as_ref().map(|a| a as &dyn SymbolArt);
            device_info(doc.project(), index, art, device)
        };
        let Some(info) = info else {
            return;
        };
        let left = self.selection_under_inspector(canvas, &selection);
        if self.wb.inspector.picker.open
            && let Some(problem) = self.wb.part_catalog.ensure()
        {
            self.message(Level::Error, problem);
        }
        let candidates = match (
            self.wb.part_catalog.get(),
            self.backend.document(),
            self.wb.inspector.picker.open,
        ) {
            (Some(catalog), Some(doc), true) => {
                // The match reads the circuit (coil voltage), so any edit,
                // undo or redo makes the rows stale.
                let scope = format!(
                    "{device:?}{}/{}/{:?}/{}",
                    doc.project()
                        .device(device)
                        .map_or(String::new(), |d| d.tag.to_string()),
                    doc.undo_depth(),
                    doc.undo_label(),
                    doc.can_redo()
                );
                self.wb.inspector.picker.rows(&scope, |query| {
                    device_candidates(catalog, doc.project(), device, query)
                })
            }
            _ => Vec::new(),
        };

        // The fields follow the model unless they are being edited.
        let tag_id = egui::Id::new("inspector_tag");
        let description_id = egui::Id::new("inspector_description");
        let state = &mut self.wb.inspector;
        if state.device != Some(device) {
            *state = InspectorState {
                device: Some(device),
                pinned: state.pinned,
                ..InspectorState::default()
            };
        }
        let focused = ctx.memory(|m| m.focused());
        if focused != Some(tag_id) {
            state.tag.clone_from(&info.tag);
        }
        if focused != Some(description_id) {
            state.description.clone_from(&info.description);
        }
        if focused != Some(egui::Id::new("inspector_delay")) {
            state.delay = info.delay.map(|d| d.to_string()).unwrap_or_default();
        }

        if focused != Some(egui::Id::new("inspector_mate")) {
            state.mate = info.mate.clone().unwrap_or_default();
        }

        let mut actions = Vec::new();
        let state = &mut self.wb.inspector;
        state.leaving = Some((info.clone(), left));
        let mut shown = std::mem::take(state);
        self.inspector_area(
            ctx,
            canvas,
            left,
            slide,
            (&info, &candidates),
            &mut shown,
            &mut actions,
        );
        self.wb.inspector = shown;

        for action in actions {
            match action {
                InspectorAction::Go(place) => self.jump(place, true),
                InspectorAction::Rename(tag) => {
                    let result = self.with_editor(|editor, doc| editor.rename(doc, device, &tag));
                    if let Some(Err(reason)) = result {
                        self.message(Level::Warning, reason);
                    }
                }
                InspectorAction::Describe(text) => self.describe(device, text),
                InspectorAction::SetDelay(seconds) => self.set_delay(device, seconds),
                InspectorAction::SetMate(tag) => self.set_mate(device, &tag),
                InspectorAction::ResetProperty(key) => self.reset_property(device, &key),
                InspectorAction::Pin(on) => self.wb.inspector.pinned = on,
                InspectorAction::AssignPart(key) => self.assign_device_part(device, &key),
                InspectorAction::SetInput(key, text) => self.set_device_input(device, key, &text),
            }
        }
    }

    /// Draws the inspector panel `slide` of the way in (1 is fully in, a
    /// pinned panel never slides), on the side away from the selection.
    #[allow(clippy::too_many_arguments)]
    fn inspector_area(
        &self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        left: bool,
        slide: f32,
        (info, candidates): (&DeviceInfo, &[Candidate]),
        state: &mut InspectorState,
        actions: &mut Vec<InspectorAction>,
    ) {
        let (pivot, at) = if left {
            (
                egui::Align2::LEFT_TOP,
                canvas.left_top() + egui::vec2(MARGIN, TOP),
            )
        } else {
            (
                egui::Align2::RIGHT_TOP,
                canvas.right_top() + egui::vec2(-MARGIN, TOP),
            )
        };
        let away = if state.pinned {
            0.0
        } else {
            (1.0 - slide) * (INSPECTOR_WIDTH + MARGIN)
        };
        let at = at + egui::vec2(if left { -away } else { away }, 0.0);
        let max_height = (canvas.height() - TOP - 72.0).max(120.0);
        let tag_id = egui::Id::new("inspector_tag");
        let description_id = egui::Id::new("inspector_description");
        egui::Area::new(egui::Id::new("inspector"))
            .order(egui::Order::Middle)
            .pivot(pivot)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(INSPECTOR_WIDTH);
                    egui::ScrollArea::vertical()
                        .max_height(max_height)
                        .show(ui, |ui| {
                            inspector_body(
                                ui,
                                info,
                                candidates,
                                state,
                                tag_id,
                                description_id,
                                actions,
                            )
                        });
                });
            });
    }

    /// The pinned inspector with nothing selected: it shows the page.
    fn page_inspector(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let page = self.current_page().and_then(|id| {
            let project = self.backend.document()?.project();
            let page = project.page(id)?;
            Some((page.name.clone(), page.title.clone()))
        });
        let mut unpin = false;
        egui::Area::new(egui::Id::new("inspector"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(canvas.right_top() + egui::vec2(-MARGIN, TOP))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(INSPECTOR_WIDTH);
                    ui.horizontal(|ui| {
                        let heading = match &page {
                            Some((name, _)) => format!("Page {name}"),
                            None => "No page open".to_owned(),
                        };
                        ui.add(egui::Label::new(egui::RichText::new(heading).heading()).truncate());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_toggle(
                                ui,
                                icons::action::PIN,
                                true,
                                "Unpin: the inspector slides away with the selection",
                            )
                            .clicked()
                            {
                                unpin = true;
                            }
                        });
                    });
                    if let Some((_, title)) = &page
                        && !title.is_empty()
                    {
                        ui.label(title);
                    }
                    ui.add_space(6.0);
                    ui.weak("Nothing selected");
                });
            });
        if unpin {
            self.wb.inspector.pinned = false;
        }
    }

    /// Writes a calculation input on the device: one undo step.
    pub(super) fn set_device_input(&mut self, device: DeviceId, key: &str, text: &str) {
        let value = match parse_device_input(key, text) {
            Ok(v) => v,
            Err(why) => {
                self.message(Level::Warning, why);
                return;
            }
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().device(device) else {
            return;
        };
        let label = format!("Set {key} of {}", old.tag);
        match doc.transact(label, |tx| tx.set_property(device, key, value)) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// The reset arrow of an overridden property: the device goes back to
    /// its part's value, as one undo step.
    fn reset_property(&mut self, device: DeviceId, key: &str) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().device(device) else {
            return;
        };
        let label = format!("Reset {key} of {}", old.tag);
        match doc.transact(label, |tx| tx.set_property(device, key, None)) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// Commits the tag and description being edited for `device`, if they
    /// differ from the model: the fields only differ while they are edited.
    fn commit_inspector_edit(&mut self, device: DeviceId) {
        let InspectorState {
            tag, description, ..
        } = std::mem::take(&mut self.wb.inspector);
        let Some(old) = self
            .backend
            .document()
            .and_then(|d| d.project().device(device))
            .map(|d| (d.tag.to_string(), d.description.clone()))
        else {
            return;
        };
        if tag != old.0 {
            let result = self.with_editor(|editor, doc| editor.rename(doc, device, &tag));
            if let Some(Err(reason)) = result {
                self.message(Level::Warning, reason);
            }
        }
        if description != old.1 {
            self.describe(device, description);
        }
    }

    /// Gives a device the catalog part `key`, as one undo step.
    pub(super) fn assign_device_part(&mut self, device: DeviceId, key: &novo_parts::PartKey) {
        let Some((command, title)) = self.backend.document().and_then(|d| {
            let catalog = self.wb.part_catalog.get()?;
            let command =
                super::part_picker::assign_device_command(d.project(), catalog, device, key)?;
            let part = catalog.get(key)?;
            Some((
                command,
                format!("{} {}", part.manufacturer, part.order_number),
            ))
        }) else {
            return;
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(Level::Info, format!("Part set: {title}")),
            Some(Err(e)) => self.message(Level::Error, format!("Could not set the part: {e}")),
            None => {}
        }
    }

    /// Sets the time of a coil or timer device, as one undo step.
    pub(super) fn set_delay(&mut self, device: DeviceId, seconds: f64) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().device(device) else {
            return;
        };
        let label = format!("Set the time of {}", old.tag);
        let value = Value::Text(format!("{seconds} s"));
        match doc.transact(label, |tx| tx.set_property(device, "delay", Some(value))) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// Names the other half of a plug or socket (`plug.mate`), as one undo
    /// step; empty clears it. Design checks compare the wiring of the two
    /// halves.
    pub(super) fn set_mate(&mut self, device: DeviceId, tag: &str) {
        let tag = tag.trim();
        let value = if tag.is_empty() {
            None
        } else if DeviceTag::parse(tag).is_ok() {
            Some(Value::Text(tag.to_owned()))
        } else {
            self.message(Level::Warning, format!("\"{tag}\" is not a device tag"));
            return;
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().device(device) else {
            return;
        };
        let label = format!("Set the other half of {}", old.tag);
        match doc.transact(label, |tx| tx.set_property(device, "plug.mate", value)) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// Sets a device's description, as one undo step.
    pub(super) fn describe(&mut self, device: DeviceId, text: String) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().device(device) else {
            return;
        };
        if old.description == text {
            return;
        }
        let label = format!("Describe {}", old.tag);
        match doc.transact(label, |tx| {
            tx.update::<novo_core::Device>(device, |d| d.description = text)
        }) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// Whether the selection sits where the inspector would open on the
    /// right, so it opens on the left instead.
    fn selection_under_inspector(&self, canvas: egui::Rect, selection: &[AnyId]) -> bool {
        let (Some(view), Some(doc)) = (self.canvas.schematic_ref(), self.backend.document()) else {
            return false;
        };
        let right = egui::Rect::from_min_max(
            egui::pos2(
                canvas.right() - INSPECTOR_WIDTH - 2.0 * MARGIN,
                canvas.top(),
            ),
            canvas.right_bottom(),
        );
        let viewport = &view.canvas.viewport;
        selection.iter().any(|id| {
            let AnyId::Symbol(s) = id else {
                return false;
            };
            doc.project().symbol(*s).is_some_and(|s| {
                let o = s.placement.origin;
                let p = viewport.world_to_screen(novo_canvas::Vec2::new(o.x as f32, o.y as f32));
                right.contains(canvas.min + egui::vec2(p.x, p.y))
            })
        })
    }
}

fn inspector_body(
    ui: &mut egui::Ui,
    info: &DeviceInfo,
    candidates: &[Candidate],
    state: &mut InspectorState,
    tag_id: egui::Id,
    description_id: egui::Id,
    actions: &mut Vec<InspectorAction>,
) {
    // Header: the tag is the title, the pin an icon at the right.
    let tag_height = 24.0;
    ui.horizontal(|ui| {
        let pin_width = 26.0;
        let tag = ui.add_sized(
            [
                ui.available_width() - pin_width - ui.spacing().item_spacing.x,
                tag_height,
            ],
            egui::TextEdit::singleline(&mut state.tag)
                .id(tag_id)
                .font(egui::TextStyle::Heading),
        );
        if committed(ui, &tag) && state.tag.trim() != info.tag {
            actions.push(InspectorAction::Rename(state.tag.trim().to_owned()));
        }
        let tip = if state.pinned {
            "Unpin: the inspector slides away with the selection"
        } else {
            "Pin: keep the inspector open, showing the page when nothing is selected"
        };
        if widgets::icon_toggle(ui, icons::action::PIN, state.pinned, tip).clicked() {
            actions.push(InspectorAction::Pin(!state.pinned));
        }
    });
    let description = ui.add(
        egui::TextEdit::singleline(&mut state.description)
            .id(description_id)
            .hint_text("Description")
            .desired_width(f32::INFINITY),
    );
    if committed(ui, &description) && state.description != info.description {
        actions.push(InspectorAction::Describe(state.description.clone()));
    }

    // Behaviour fields that only some devices have come first: they are what
    // the person usually came to change.
    if info.delay.is_some() {
        ui.add_space(6.0);
        let delay = labelled_edit(ui, "Time", "s", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.delay)
                    .id(egui::Id::new("inspector_delay"))
                    .hint_text("Seconds")
                    .desired_width(f32::INFINITY),
            )
        });
        if committed(ui, &delay)
            && let Ok(seconds) = state.delay.trim().parse::<f64>()
            && seconds >= 0.0
            && Some(seconds) != info.delay
        {
            actions.push(InspectorAction::SetDelay(seconds));
        }
    }
    if info.mate.is_some() {
        ui.add_space(6.0);
        let mate = labelled_edit(ui, "Other half", "", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.mate)
                    .id(egui::Id::new("inspector_mate"))
                    .hint_text("Tag of the plug or socket")
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text("Design checks report a pin wired on one half and not the other.")
        });
        if committed(ui, &mate) && Some(state.mate.trim()) != info.mate.as_deref() {
            actions.push(InspectorAction::SetMate(state.mate.clone()));
        }
    }

    // Part: one line, name left, the action right.
    ui.add_space(6.0);
    widgets::section_label(ui, "Part");
    ui.horizontal(|ui| {
        let label = match (state.picker.open, info.part.is_empty()) {
            (true, _) => "Close",
            (false, true) => "Choose...",
            (false, false) => "Change...",
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::ghost_button(ui, label)
                .on_hover_text(
                    "Pick this device's part from the catalog (Parts window: Ctrl+K, Parts)",
                )
                .clicked()
            {
                state.picker.toggle();
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if info.part.is_empty() {
                    ui.weak("No part assigned");
                } else {
                    ui.add(egui::Label::new(&info.part).truncate())
                        .on_hover_text(&info.part);
                }
            });
        });
    });
    if state.picker.open
        && let Some(key) = picker_ui(
            ui,
            &mut state.picker,
            candidates,
            "No part in the catalog matches. Open the Parts window to import one.",
        )
    {
        actions.push(InspectorAction::AssignPart(key));
        state.picker.close();
    }

    widgets::collapsing_section(
        ui,
        "inspector_relations",
        "Relations",
        &info.relations.len().to_string(),
        true,
        |ui| {
            if info.relations.is_empty() {
                ui.weak("Not drawn yet");
            }
            for relation in &info.relations {
                if relation_card(ui, relation).clicked() {
                    actions.push(InspectorAction::Go(relation.place));
                }
            }
        },
    );

    calc_inputs_section(ui, info, actions);

    // The rest of the device's properties are the advanced part: closed
    // until opened, and absent when there are none.
    if info.properties.is_empty() {
        return;
    }
    let many = info.properties.len() > 6;
    widgets::collapsing_section(
        ui,
        "inspector_properties",
        "All properties",
        &info.properties.len().to_string(),
        !many,
        |ui| {
            if many {
                ui.add(
                    egui::TextEdit::singleline(&mut state.filter)
                        .hint_text("Search properties")
                        .desired_width(f32::INFINITY),
                );
            }
            let filter = state.filter.to_lowercase();
            for (key, value) in &info.properties {
                if filter.is_empty()
                    || key.to_lowercase().contains(&filter)
                    || value.to_lowercase().contains(&filter)
                {
                    let part_value = info
                        .part_values
                        .iter()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v);
                    ui.horizontal(|ui| {
                        widgets::field(ui, key, value);
                        if let Some(part_value) = part_value
                            && widgets::icon_button(
                                ui,
                                icons::action::RESET,
                                &format!("Back to the part's value: {part_value}"),
                            )
                            .clicked()
                        {
                            actions.push(InspectorAction::ResetProperty(key.clone()));
                        }
                    });
                }
            }
        },
    );
}

/// A label in a fixed column, the field filling the rest of the row and an
/// optional unit after it; returns the field's response.
fn labelled_edit(
    ui: &mut egui::Ui,
    label: &str,
    unit: &str,
    field: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, 22.0],
            egui::Label::new(egui::RichText::new(label).color(ui.visuals().weak_text_color())),
        );
        if !unit.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(unit);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), field)
                    .inner
            })
            .inner
        } else {
            field(ui)
        }
    })
    .inner
}

/// Width of the label column of the inspector's rows.
const LABEL_WIDTH: f32 = 104.0;

/// Where a symbol is drawn, for a relation card: `Page 3 · Control circuit
/// · column 4`, with the page structure in front when it has one.
fn page_line(location: &novo_core::Location, title: &str) -> String {
    let mut parts = vec![format!(
        "Page {}{}",
        structure_prefix(location),
        location.page_name
    )];
    if !title.trim().is_empty() {
        parts.push(title.trim().to_owned());
    }
    parts.push(format!("column {}", location.column));
    parts.join(" · ")
}

fn structure_prefix(location: &novo_core::Location) -> String {
    let s = location.structure.to_string();
    if s.is_empty() { s } else { format!("{s}/") }
}

/// Height of a relation's picture and of its card.
const CARD_HEIGHT: f32 = 56.0;

/// One place the device is drawn: a picture of the symbol as drawn there,
/// what it is with its tag and pins, and the page. The whole card is the
/// click target that jumps there.
fn relation_card(ui: &mut egui::Ui, relation: &Relation) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, CARD_HEIGHT), egui::Sense::click());
    let response = response.on_hover_text(if relation.main {
        "Main function · show on the drawing"
    } else {
        "Show on the drawing"
    });
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let picture = egui::Rect::from_min_size(rect.min, egui::vec2(CARD_HEIGHT, CARD_HEIGHT));
    match &relation.preview {
        Some(preview) => symbol_preview::paint(ui, picture, preview, relation.main),
        None => {
            ui.painter().rect_stroke(
                picture,
                4.0,
                ui.visuals().widgets.noninteractive.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
    }
    let x = picture.right() + 8.0;
    let room = rect.right() - x;
    let visuals = ui.visuals();
    let painter = ui.painter_at(rect);
    let ellipsis = widgets::paint_text_ellipsis;
    let mut y = rect.top() + 6.0;
    let title = ellipsis(
        &painter,
        egui::pos2(x, y + 7.0),
        &relation.what,
        egui::FontId::proportional(13.0),
        visuals.strong_text_color(),
        room,
    );
    y = title.bottom() + 2.0;
    if !relation.pins.is_empty() {
        let pins = ellipsis(
            &painter,
            egui::pos2(x, y + 7.0),
            &relation.pins,
            egui::FontId::monospace(12.0),
            visuals.text_color(),
            room,
        );
        y = pins.bottom() + 2.0;
    }
    ellipsis(
        &painter,
        egui::pos2(x, y + 6.0),
        &relation.page,
        egui::FontId::proportional(11.0),
        visuals.weak_text_color(),
        room,
    );
    response
}

/// Enter, or leaving the field after a change, commits it.
fn committed(ui: &egui::Ui, response: &egui::Response) -> bool {
    response.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape))
}

/// The "Calculation inputs" rows: the numbers the cable, short-circuit and
/// starter calculations read from the device. A value the part supplies is
/// shown dimmed with a hint; typing overrides it on this device only.
fn calc_inputs_section(ui: &mut egui::Ui, info: &DeviceInfo, actions: &mut Vec<InspectorAction>) {
    let set = info
        .calc_inputs
        .iter()
        .filter(|(v, _)| !v.is_empty())
        .count();
    let summary = if set == 0 {
        String::new()
    } else {
        format!("{set} set")
    };
    widgets::collapsing_section(
        ui,
        "inspector_calc_inputs",
        "Calculation inputs",
        &summary,
        set > 0,
        |ui| {
            for ((key, label, unit, hint), (shown, from_part)) in
                DEVICE_INPUTS.iter().zip(&info.calc_inputs)
            {
                let id = egui::Id::new(("inspector_input", *key));
                let focused = ui.memory(|m| m.has_focus(id));
                let mut text = if focused {
                    ui.data(|d| d.get_temp::<String>(id))
                        .unwrap_or_else(|| shown.clone())
                } else {
                    shown.clone()
                };
                let r = labelled_edit(ui, label, unit, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .id(id)
                            .hint_text(*hint)
                            .desired_width(f32::INFINITY),
                    )
                    .on_hover_text(if *from_part {
                        format!("{key}: from the part. Type to override on this device.")
                    } else {
                        (*key).to_owned()
                    })
                });
                if r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, text.clone()));
                }
                if r.lost_focus() {
                    ui.data_mut(|d| d.remove::<String>(id));
                    if text.trim() != shown.trim() {
                        actions.push(InspectorAction::SetInput(key, text.clone()));
                    }
                }
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Blank is unknown: the checks stay silent, nothing is guessed. Results: Check > Calculations.",
                    )
                    .small()
                    .weak(),
                )
                .wrap(),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::DeviceTag;

    #[test]
    fn one_device_however_many_of_its_symbols() {
        let mut project = Project::new("Plant");
        let ((k1, k2), _) = project
            .transact("Add", |tx| {
                Ok((
                    tx.add_device(DeviceTag::parse("-K1").unwrap(), "Pump")?,
                    tx.add_device(DeviceTag::parse("-K2").unwrap(), "")?,
                ))
            })
            .unwrap();
        assert_eq!(selected_device(&project, &[]), None);
        assert_eq!(selected_device(&project, &[AnyId::Device(k1)]), Some(k1));
        assert_eq!(
            selected_device(&project, &[AnyId::Device(k1), AnyId::Device(k1)]),
            Some(k1)
        );
        assert_eq!(
            selected_device(&project, &[AnyId::Device(k1), AnyId::Device(k2)]),
            None
        );
        let info = device_info(&project, None, None, k1).unwrap();
        assert_eq!(info.tag, "-K1");
        assert_eq!(info.description, "Pump");
        assert!(info.part.is_empty() && info.relations.is_empty());
        assert_eq!(info.delay, None, "no coil, no time field");
        assert_eq!(info.mate, None, "only plugs name another half");
    }

    #[test]
    fn a_plug_names_its_other_half() {
        let mut project = Project::new("Plant");
        let ((plug, other), _) = project
            .transact("setup", |tx| {
                let plug = tx.add_device_of_kind(
                    DeviceTag::parse("-XM1").unwrap(),
                    "",
                    DeviceKind::Connector,
                )?;
                let other = tx.add_device_of_kind(
                    DeviceTag::parse("-XF1").unwrap(),
                    "",
                    DeviceKind::Connector,
                )?;
                tx.set_property(plug, "plug.mate", Some(Value::Text("-XF1".into())))?;
                Ok((plug, other))
            })
            .unwrap();
        assert_eq!(
            device_info(&project, None, None, plug)
                .unwrap()
                .mate
                .as_deref(),
            Some("-XF1")
        );
        assert_eq!(
            device_info(&project, None, None, other)
                .unwrap()
                .mate
                .as_deref(),
            Some("")
        );
    }

    #[test]
    fn a_property_set_differently_from_its_part_offers_the_parts_value() {
        let mut project = Project::new("Plant");
        let ((k1, part), _) = project
            .transact("Add", |tx| {
                let k1 = tx.add_device(DeviceTag::parse("-Q1").unwrap(), "")?;
                let part = tx.add_part("ABB", "S201")?;
                tx.assign_part(k1, part)?;
                tx.set_property(part, "rated_current", Some(Value::Text("16 A".into())))?;
                tx.set_property(part, "poles", Some(Value::Text("1".into())))?;
                tx.set_property(k1, "rated_current", Some(Value::Text("10 A".into())))?;
                tx.set_property(k1, "poles", Some(Value::Text("1".into())))?;
                tx.set_property(k1, "note", Some(Value::Text("by hand".into())))?;
                Ok((k1, part))
            })
            .unwrap();
        let info = device_info(&project, None, None, k1).unwrap();
        // Only the property that differs is an override; one that equals the
        // part's, or that the part does not have, is not.
        assert_eq!(
            info.part_values,
            vec![("rated_current".to_owned(), "16 A".to_owned())]
        );
        // The reset arrow removes the device's own value.
        project
            .transact("Reset", |tx| tx.set_property(k1, "rated_current", None))
            .unwrap();
        let info = device_info(&project, None, None, k1).unwrap();
        assert!(info.part_values.is_empty());
        assert!(project.part(part).is_some());
    }

    #[test]
    fn relations_describe_each_place_with_a_picture_of_its_symbol() {
        use novo_core::{Document, Pin, Placement, Point};
        let mut doc = Document::default();
        let k1 = doc
            .transact("Draw", |tx| {
                let p1 = tx.add_page("1")?;
                let p2 = tx.add_page("2")?;
                let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
                tx.place_symbol(
                    p2,
                    "iec/contacts/no",
                    Placement::at(Point::new(140.0, 100.0)),
                    vec![Pin::new("13", 0.0, -8.0), Pin::new("14", 0.0, 8.0)],
                    SymbolRole::Device {
                        device: k1,
                        main: false,
                    },
                )?;
                tx.place_symbol(
                    p1,
                    "iec/coils/relay",
                    Placement::at(Point::new(190.0, 100.0)),
                    vec![Pin::new("A1", 0.0, -8.0), Pin::new("A2", 0.0, 8.0)],
                    SymbolRole::Device {
                        device: k1,
                        main: true,
                    },
                )?;
                Ok(k1)
            })
            .unwrap()
            .0;
        let library = novo_symbols::Library::shared(novo_symbols::Pack::Iec);
        let index = XrefIndex::build(doc.project(), library);
        let art = novo_editor::art::LibraryArt::new(
            library,
            novo_canvas::Stroke::world(0.35, novo_canvas::Color::BLACK),
        );
        let info = device_info(doc.project(), Some(&index), Some(&art), k1).unwrap();
        assert_eq!(info.relations.len(), 2);
        let (coil, contact) = (&info.relations[0], &info.relations[1]);
        assert!(coil.main && coil.what == "Coil");
        assert_eq!(contact.what, "NO contact");
        assert_eq!(contact.pins, "13-14");
        assert_eq!(contact.page, "Page 2 · column 4");
        for r in &info.relations {
            let preview = r.preview.as_ref().expect("a picture of every symbol");
            assert!(!preview.shapes.is_empty() && preview.pins.len() == 2);
        }
    }

    #[test]
    fn page_line_names_structure_title_and_column() {
        use novo_core::{DeviceTag, Location};
        let at = |structure: &str, column| Location {
            page: novo_core::PageId(1),
            page_name: "3".into(),
            structure: DeviceTag::parse(structure).unwrap_or_default(),
            column,
        };
        assert_eq!(page_line(&at("", 4), ""), "Page 3 · column 4");
        assert_eq!(
            page_line(&at("=A2", 1), " Control "),
            "Page =A2/3 · Control · column 1"
        );
    }
}
