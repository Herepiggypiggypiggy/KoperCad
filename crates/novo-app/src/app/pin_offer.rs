//! The offer to name a part's terminals like the symbol's pins, made right
//! after a part is assigned to a device whose symbols are all of one device
//! class (coil A1/A2, motor U1/V1/W1, ...). One click renames the terminals
//! to the usual names of the class, as one undo step; the same repair is on
//! the Check > Issues "Pin names" row.
//!
//! Where the user finds it: assign a part to a device in the Inspector part
//! list, or to one selected device in the selection sheet; a small window
//! offers the rename when the terminals of the part differ from the usual
//! names. The offer is dropped when its device is gone (another project, an
//! undo of the placement).

use eframe::egui;
use novo_core::DeviceId;

use novo_core::Value;

use super::{Level, NovoApp};

/// The project property that switches the offer off (`off`); anything else,
/// or no property, means the offer is on.
pub(super) const PIN_OFFER_PROPERTY: &str = "pin.offer";

/// A rename waiting for the person's answer.
pub(super) struct PinOffer {
    device: DeviceId,
    /// The device tag, for the text.
    tag: String,
    fix: novo_checks::PinFix,
}

impl NovoApp {
    /// Whether this project wants the offer after a part is assigned.
    pub(super) fn pin_offer_wanted(&self) -> bool {
        !self.backend.document().is_some_and(|d| {
            matches!(
                d.project().info().properties.get(PIN_OFFER_PROPERTY),
                Some(Value::Text(t)) if t == "off"
            )
        })
    }

    /// Switches the offer on or off for this project, as one undo step.
    pub(super) fn set_pin_offer_wanted(&mut self, wanted: bool) {
        if wanted == self.pin_offer_wanted() {
            return;
        }
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Part terminal offer", |tx| {
            tx.set_info(|i| {
                if wanted {
                    i.properties.remove(PIN_OFFER_PROPERTY);
                } else {
                    i.properties
                        .insert(PIN_OFFER_PROPERTY.to_owned(), Value::Text("off".into()));
                }
            })
        });
        match result {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Warning, format!("Part terminal offer: {e}")),
        }
    }

    /// Looks at the part just assigned to `device` and, when its terminals are
    /// not the usual names of the class and a clear rename exists, opens the
    /// offer.
    pub(super) fn offer_part_terminals(&mut self, device: DeviceId) {
        if !self.pin_offer_wanted() {
            return;
        }
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let Some(fix) = novo_checks::part_terminal_fix(project, device) else {
            return;
        };
        let tag = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        self.wb.pin_offer = Some(PinOffer { device, tag, fix });
    }

    pub(super) fn pin_offer_dialog(&mut self, ctx: &egui::Context) {
        let Some(offer) = &self.wb.pin_offer else {
            return;
        };
        let alive = self
            .backend
            .document()
            .is_some_and(|d| d.project().device(offer.device).is_some());
        if !alive {
            self.wb.pin_offer = None;
            return;
        }
        let Some(offer) = &self.wb.pin_offer else {
            return;
        };
        let (mut rename, mut keep, mut never) = (false, false, false);
        let modal = super::panel::modal(
            ctx,
            "pin_offer",
            "Name the terminals like the pins",
            [500.0, 210.0],
            |ui| {
                ui.label(format!(
                    "The terminals of the part on {} are not named like the usual pins of its \
                 symbol, so they cannot be matched to them.",
                    offer.tag
                ));
                if let Some(note) = &offer.fix.note {
                    ui.add_space(4.0);
                    ui.weak(note);
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    rename = novo_theme::widgets::primary_button(ui, &offer.fix.label).clicked();
                    keep = novo_theme::widgets::ghost_button(ui, "Keep the part's names").clicked();
                    never = novo_theme::widgets::ghost_button(
                        ui,
                        "Keep, and do not ask again in this project",
                    )
                    .on_hover_text("Switch it on again in Project > Rules, Usual pin names.")
                    .clicked();
                });
            },
        );
        if rename {
            if let Some(offer) = self.wb.pin_offer.take() {
                match self.execute(offer.fix.command) {
                    Some(Ok(_)) => {
                        self.message(Level::Info, "Renamed the terminals to the usual names")
                    }
                    Some(Err(e)) => {
                        self.message(Level::Error, format!("Could not rename them: {e}"))
                    }
                    None => {}
                }
            }
        } else if never {
            self.wb.pin_offer = None;
            self.set_pin_offer_wanted(false);
            self.message(
                Level::Info,
                "Will not ask again in this project (Project > Rules, Usual pin names)",
            );
        } else if keep || modal.should_close() {
            self.wb.pin_offer = None;
        }
    }
}
