//! The PLC address style (IEC 61131, Siemens, Rockwell, plain) of the
//! project: how every channel address is written. It changes text only, never
//! which channel is which. Where the user finds it: the Add PLC card dialog
//! (Draw > PLC > Add PLC card) and the header of the I/O list in the Reports
//! workspace.

use eframe::egui;
use novo_plc::AddressStyle;

use super::{Level, NovoApp};

/// What each style looks like, for the picker's hover text.
pub(crate) fn example(style: AddressStyle) -> &'static str {
    match style {
        AddressStyle::Iec => "digital %IX4.3, analog %IW6",
        AddressStyle::Siemens => "digital %I4.3, analog %IW6",
        AddressStyle::Rockwell => {
            "Local:3:I.Data.3, Local:3:I.Ch1Data (slot and channel; the card needs a slot)"
        }
        AddressStyle::Plain => "digital I4.3, analog IW6",
    }
}

/// The combo box for a style. Returns the style picked, when one is.
pub(crate) fn picker(ui: &mut egui::Ui, id: &str, current: AddressStyle) -> Option<AddressStyle> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for style in AddressStyle::ALL {
                if ui
                    .selectable_label(style == current, style.label())
                    .on_hover_text(example(style))
                    .clicked()
                {
                    picked = Some(style);
                }
            }
        })
        .response
        .on_hover_text(format!(
            "How PLC addresses are written in this project ({}). Text only: no channel moves. \
             Changing it is one undo step.",
            example(current)
        ));
    picked
}

impl NovoApp {
    /// The project's address style.
    pub(crate) fn plc_address_style(&self) -> AddressStyle {
        self.backend
            .document()
            .map(|d| novo_plc::address_style(d.project()))
            .unwrap_or_default()
    }

    /// Writes the project's address style, as one undo step. Does nothing when
    /// it is the style already.
    pub(crate) fn set_plc_address_style(&mut self, style: AddressStyle) -> bool {
        if self.plc_address_style() == style {
            return false;
        }
        let Some(doc) = self.backend.document_mut() else {
            return false;
        };
        let result = doc.transact("Change PLC address style", |tx| {
            tx.set_info(|i| {
                i.properties
                    .insert(novo_plc::card::ADDRESS_STYLE.to_owned(), style.key().into());
            })
        });
        match result {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(
                    Level::Info,
                    format!("PLC addresses are now written {}", style.label()),
                );
                true
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not change the address style: {e}"),
                );
                false
            }
        }
    }
}
