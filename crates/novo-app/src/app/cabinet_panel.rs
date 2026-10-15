//! A section for the Cabinet inspector: the panel-building data of a placed
//! device (power loss, mounting type and the four clearances), editable in
//! place. The inspector (`cabinet_inspector.rs`) calls [`show`] inside its
//! device block and passes the edit on as `Ask::SetPanelValue`; [`command`]
//! turns it into the validated one-undo-step command, through
//! `novo_parts::PropertySheet`.

use eframe::egui;
use novo_core::{AnyId, Device, DeviceId, Project};
use novo_parts::{PanelData, keys};
use novo_theme::widgets::section_label;

/// The fields in the order they are shown: key, label, unit.
const NUMBERS: [(&str, &str, &str); 5] = [
    (keys::POWER_LOSS_W, "Power loss", "W"),
    (keys::CLEARANCE_ABOVE_MM, "Space above", "mm"),
    (keys::CLEARANCE_BELOW_MM, "Space below", "mm"),
    (keys::CLEARANCE_LEFT_MM, "Space left", "mm"),
    (keys::CLEARANCE_RIGHT_MM, "Space right", "mm"),
];

/// Text buffers of the inspector, one per field (the numbers, then the
/// mounting type).
#[derive(Default)]
pub(super) struct PanelFields {
    text: [String; 5],
}

/// One edit asked for: the device, the property key and the typed text.
pub(super) struct PanelEdit {
    pub device: u64,
    pub key: &'static str,
    pub text: String,
}

fn shown(value: Option<f64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// The panel-building values a device has, its own over its part's.
pub(super) fn panel_data(project: &Project, device: &Device) -> PanelData {
    let part = device
        .parts
        .first()
        .and_then(|p| project.part(*p))
        .map(|p| &p.properties);
    PanelData::of_device(&device.properties, part)
}

/// Draws the section for a placed device and returns the edit the user just
/// committed (Enter or leaving the field), if any.
pub(super) fn show(
    ui: &mut egui::Ui,
    fields: &mut PanelFields,
    project: &Project,
    id: u64,
) -> Option<PanelEdit> {
    let device = project.get::<Device>(DeviceId::from(id))?;
    let data = panel_data(project, device);
    let current = [
        shown(data.power_loss_w),
        shown(data.clearances.above_mm),
        shown(data.clearances.below_mm),
        shown(data.clearances.left_mm),
        shown(data.clearances.right_mm),
    ];
    let mut edit = None;
    let mut picked: Option<String> = None;
    ui.add_space(6.0);
    section_label(ui, "PANEL DATA");
    if current.iter().all(String::is_empty) && data.mounting.is_none() {
        ui.weak("No data from the part. Type it here, or pick a part with a data sheet.");
    } else {
        ui.weak("A value here overrides the part's.");
    }
    ui.weak("Space is measured as the part is drawn, before rotating.");
    ui.add_space(6.0);
    for (i, (key, label, unit)) in NUMBERS.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(*label);
            let typed = fields.text[i].trim();
            let bad = !typed.is_empty() && novo_parts::parse_number(typed).is_none_or(|v| v < 0.0);
            let mut field = egui::TextEdit::singleline(&mut fields.text[i])
                .id_salt(("cabinet_panel_field", id, *key))
                .hint_text("not set")
                .desired_width(64.0);
            if bad {
                field = field.text_color(ui.visuals().error_fg_color);
            }
            let response = ui.add(field);
            ui.weak(*unit);
            if bad {
                response.clone().on_hover_text("A number of 0 or more");
            }
            if !response.has_focus() && !response.lost_focus() && !bad {
                fields.text[i] = current[i].clone();
            }
            if response.lost_focus() && fields.text[i].trim() != current[i] {
                edit = Some(PanelEdit {
                    device: id,
                    key,
                    text: fields.text[i].clone(),
                });
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Mounting");
        let mounting = data.mounting;
        egui::ComboBox::from_id_salt(("cabinet_panel_mounting", id))
            .selected_text(mounting.map_or("not set", |m| m.label()))
            .show_ui(ui, |ui| {
                if ui.selectable_label(mounting.is_none(), "not set").clicked() {
                    picked = Some(String::new());
                }
                for m in novo_parts::Mounting::ALL {
                    if ui
                        .selectable_label(mounting == Some(m), m.label())
                        .clicked()
                    {
                        picked = Some(m.as_str().to_owned());
                    }
                }
            });
    });
    let mounting = data.mounting.map_or("", |m| m.as_str());
    if let Some(text) = picked
        && text != mounting
    {
        edit = Some(PanelEdit {
            device: id,
            key: keys::MOUNTING,
            text,
        });
    }
    edit
}

/// The command for an edit, checked like the property sheet checks it.
pub(super) fn command(
    project: &Project,
    edit: &PanelEdit,
) -> Result<Option<novo_core::Command>, novo_parts::EditError> {
    let target = AnyId::from(DeviceId::from(edit.device));
    novo_parts::PropertySheet::build(project, &[target]).edit(
        project,
        &novo_parts::RowKey::PartData(edit.key.into()),
        &edit.text,
    )
}
