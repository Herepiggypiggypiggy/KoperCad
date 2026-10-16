//! The tooltip over a device in the 3D view: tag, part, power loss and, once
//! a heat run exists, the device temperature. The text is built here from the
//! project and the last heat result so it can be tested without a window.
//!
//! Where the user finds it: hold the pointer over a device in the 3D view.

use eframe::egui;
use novo_core::{Device, DeviceId, Project};
use novo_thermal::DeviceResult;

/// What the tooltip says: a title and one line per fact.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Tip {
    pub title: String,
    pub lines: Vec<String>,
}

/// The tip for a device; `heat` is its row of the last heat result, if any.
pub(super) fn tip(
    project: &Project,
    device: DeviceId,
    heat: Option<(&DeviceResult, f64)>,
) -> Option<Tip> {
    let d = project.get::<Device>(device)?;
    let mut lines = Vec::new();
    if !d.description.is_empty() {
        lines.push(d.description.clone());
    }
    let parts: Vec<_> = d.parts.iter().filter_map(|p| project.part(*p)).collect();
    if parts.is_empty() {
        lines.push("No part assigned".to_owned());
    }
    for p in parts {
        lines.push(format!("Part: {} {}", p.manufacturer, p.order_number));
    }
    let sized = novo_layout::sizing_model::sizing_device(project, d);
    let known = novo_thermal::bridge::device_loss_w(project, d).is_some();
    let loss = sized.power_loss * sized.load_factor;
    lines.push(if known {
        format!("Power loss: {loss:.1} W")
    } else {
        format!("Power loss: {loss:.1} W (estimate, the part has no data)")
    });
    if let Some((r, ambient)) = heat {
        let mut text = format!(
            "Temperature: {:.0} °C surface, {:.0} °C air around (ambient {ambient:.0} °C)",
            r.surface_c, r.air_c
        );
        if let Some(margin) = r.margin_k {
            if margin < 0.0 {
                text.push_str(&format!(", {:.0} K over its limit", -margin));
            } else {
                text.push_str(&format!(", {margin:.0} K to its limit"));
            }
        }
        lines.push(text);
        lines.push("From the last heat run".to_owned());
    }
    Some(Tip {
        title: d.tag.to_string(),
        lines,
    })
}

/// Draws the tip next to the pointer.
pub(super) fn show(ctx: &egui::Context, tip: &Tip) {
    let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    egui::Area::new(egui::Id::new("view3d_device_tip"))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(pointer + egui::vec2(14.0, 18.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(300.0);
                ui.label(egui::RichText::new(&tip.title).strong());
                for line in &tip.lines {
                    ui.label(line);
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{DeviceTag, Value};

    fn project(loss: Option<f64>) -> (Project, DeviceId) {
        let mut project = Project::new("Plant");
        let (device, _) = project
            .transact("Add", |tx| {
                let d =
                    tx.add_device(DeviceTag::parse("=A1+CAB-K1").unwrap(), "Motor contactor")?;
                if let Some(w) = loss {
                    tx.set_property(d, "power_loss_w", Some(Value::Float(w)))?;
                }
                Ok(d)
            })
            .unwrap();
        (project, device)
    }

    fn result(surface: f64, margin: Option<f64>) -> DeviceResult {
        DeviceResult {
            id: 1,
            footprint: novo_layout::Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            loss_w: 5.0,
            surface_c: surface,
            air_c: 41.0,
            h_w_m2k: 8.0,
            plate_share: 0.3,
            surface_band_c: (surface - 2.0, surface + 2.0),
            air_band_c: (39.0, 43.0),
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            margin_k: margin,
            margin_pessimistic_k: margin,
            status: novo_thermal::Status::Ok,
        }
    }

    #[test]
    fn the_tip_names_the_device_its_part_state_and_its_loss() {
        let (project, device) = project(Some(12.0));
        let tip = tip(&project, device, None).unwrap();
        assert!(tip.title.contains("K1"), "{}", tip.title);
        let text = tip.lines.join("\n");
        assert!(text.contains("Motor contactor"));
        assert!(text.contains("No part assigned"));
        assert!(text.contains("Power loss: 12.0 W"), "{text}");
        assert!(!text.contains("Temperature"), "no heat run, no temperature");
    }

    #[test]
    fn a_heat_result_adds_the_temperature_and_the_margin() {
        let (project, device) = project(Some(12.0));
        let ok = tip(&project, device, Some((&result(55.0, Some(20.0)), 35.0))).unwrap();
        let text = ok.lines.join("\n");
        assert!(text.contains("55 °C surface"), "{text}");
        assert!(text.contains("20 K to its limit"));
        assert!(text.contains("last heat run"));
        let over = tip(&project, device, Some((&result(95.0, Some(-5.0)), 35.0))).unwrap();
        assert!(over.lines.join("\n").contains("5 K over its limit"));
    }

    #[test]
    fn a_device_that_is_gone_has_no_tip() {
        let (project, _) = project(None);
        assert!(tip(&project, DeviceId::from(9_999), None).is_none());
    }
}
