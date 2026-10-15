//! The inspector (DESIGN.md §3): an overlay that appears with a selection
//! and slides away when it clears. It opens on the side away from the
//! selection so it never hides what was selected. Relations come first
//! (where the device is drawn: its main function and contacts), then the
//! part, then a search over every other property. The tag and the
//! description are edited in place, each edit one undo step.

use eframe::egui;
use novo_core::{AnyId, DeviceId, Project, SymbolRole, Value};
use novo_theme::widgets;
use novo_xref::{Place, XrefIndex};

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
}

/// One place the device is drawn: `NO 13-14 2.6`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Relation {
    pub row: String,
    pub place: Place,
    /// The main function (a contactor's coil).
    pub main: bool,
}

/// The inspector's fields while they are edited.
#[derive(Debug, Default)]
pub(super) struct InspectorState {
    device: Option<DeviceId>,
    tag: String,
    description: String,
    pub(super) filter: String,
}

/// What an inspector click or edit asks for.
enum InspectorAction {
    Go(Place),
    Rename(String),
    Describe(String),
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
                .map(|r| (r.row(), Place::of(r), r.main))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // The main function first (a contactor's coil), then the rest in
    // reading order.
    let mut relations: Vec<Relation> = relations
        .into_iter()
        .map(|(row, place, main)| Relation { row, place, main })
        .collect();
    relations.sort_by_key(|r| !r.main);
    let properties = d
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), value_text(v)))
        .collect();
    Some(DeviceInfo {
        device,
        tag: d.tag.to_string(),
        description: d.description.clone(),
        part,
        relations,
        properties,
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
        let Some(device) = device else {
            self.wb.inspector = InspectorState::default();
            return;
        };
        let info = {
            let index = xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref);
            let Some(doc) = self.backend.document() else {
                return;
            };
            device_info(doc.project(), index, device)
        };
        let Some(info) = info else {
            return;
        };
        let left = self.selection_under_inspector(canvas, &selection);

        // The fields follow the model unless they are being edited.
        let tag_id = egui::Id::new("inspector_tag");
        let description_id = egui::Id::new("inspector_description");
        let state = &mut self.wb.inspector;
        if state.device != Some(device) {
            *state = InspectorState {
                device: Some(device),
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
        let max_height = (canvas.height() - TOP - 72.0).max(120.0);
        let mut actions = Vec::new();
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
                            inspector_body(ui, &info, state, tag_id, description_id, &mut actions)
                        });
                });
            });

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
            }
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
        if tag != old.0 && !tag.is_empty() {
            let result = self.with_editor(|editor, doc| editor.rename(doc, device, &tag));
            if let Some(Err(reason)) = result {
                self.message(Level::Warning, reason);
            }
        }
        if description != old.1 {
            self.describe(device, description);
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
    state: &mut InspectorState,
    tag_id: egui::Id,
    description_id: egui::Id,
    actions: &mut Vec<InspectorAction>,
) {
    let tag = ui.add(
        egui::TextEdit::singleline(&mut state.tag)
            .id(tag_id)
            .font(egui::TextStyle::Heading)
            .desired_width(f32::INFINITY),
    );
    if committed(ui, &tag) && state.tag.trim() != info.tag {
        actions.push(InspectorAction::Rename(state.tag.trim().to_owned()));
    }
    let description = ui.add(
        egui::TextEdit::singleline(&mut state.description)
            .id(description_id)
            .hint_text("Description")
            .desired_width(f32::INFINITY),
    );
    if committed(ui, &description) && state.description != info.description {
        actions.push(InspectorAction::Describe(state.description.clone()));
    }

    ui.add_space(6.0);
    widgets::section_label(ui, "Relations");
    if info.relations.is_empty() {
        ui.weak("Not drawn yet");
    }
    for relation in &info.relations {
        let hover = if relation.main {
            "Main function · show on the drawing"
        } else {
            "Show on the drawing"
        };
        let text = egui::RichText::new(&relation.row);
        let text = if relation.main { text.strong() } else { text };
        if ui
            .selectable_label(false, text)
            .on_hover_text(hover)
            .clicked()
        {
            actions.push(InspectorAction::Go(relation.place));
        }
    }

    ui.add_space(6.0);
    widgets::section_label(ui, "Part");
    if info.part.is_empty() {
        ui.weak("No part");
    } else {
        ui.label(&info.part);
    }

    ui.add_space(6.0);
    widgets::section_label(ui, "Properties");
    if info.properties.is_empty() {
        ui.weak("None");
        return;
    }
    ui.add(
        egui::TextEdit::singleline(&mut state.filter)
            .hint_text("Search properties")
            .desired_width(f32::INFINITY),
    );
    let filter = state.filter.to_lowercase();
    for (key, value) in &info.properties {
        if filter.is_empty()
            || key.to_lowercase().contains(&filter)
            || value.to_lowercase().contains(&filter)
        {
            widgets::field(ui, key, value);
        }
    }
}

/// Enter, or leaving the field after a change, commits it.
fn committed(ui: &egui::Ui, response: &egui::Response) -> bool {
    response.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape))
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
        let info = device_info(&project, None, k1).unwrap();
        assert_eq!(info.tag, "-K1");
        assert_eq!(info.description, "Pump");
        assert!(info.part.is_empty() && info.relations.is_empty());
    }
}
