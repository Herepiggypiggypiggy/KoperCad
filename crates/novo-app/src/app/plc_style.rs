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

/// The line under the I/O list header when Rockwell addresses read `?` for the
/// slot: names the cards with no slot. `None` when every address has one.
pub(crate) fn missing_slot_note(output: &novo_reports::ReportOutput) -> Option<String> {
    use novo_reports::Cell;
    let mut cards: Vec<String> = Vec::new();
    for section in &output.sections {
        let card = section.columns.iter().position(|c| c.key == "card");
        for row in &section.rows {
            let unslotted = row
                .cells
                .iter()
                .any(|c| matches!(c, Cell::Text(t) if t.starts_with("Local:?:")));
            if !unslotted {
                continue;
            }
            let name = match card.and_then(|i| row.cells.get(i)) {
                Some(Cell::Text(t)) if !t.is_empty() => t.clone(),
                _ => String::from("a card"),
            };
            if !cards.contains(&name) {
                cards.push(name);
            }
        }
    }
    if cards.is_empty() {
        return None;
    }
    Some(format!(
        "Slot missing on {}: Rockwell addresses read the card's slot (Local:<slot>:...), shown as \
         ? until the card has one.",
        cards.join(", ")
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use novo_reports::{Cell, Column, ReportOutput, Row, Section};

    fn output(rows: &[(&str, &str)]) -> ReportOutput {
        let mut section = Section::new(
            "plc",
            "PLC",
            vec![
                Column::new("address", "Address", 10.0),
                Column::new("card", "Card", 10.0),
            ],
        );
        for (address, card) in rows {
            section.rows.push(Row::new(
                vec![
                    Cell::Text((*address).to_owned()),
                    Cell::Text((*card).to_owned()),
                ],
                Vec::new(),
            ));
        }
        ReportOutput {
            report: "io".to_owned(),
            title: "I/O list".to_owned(),
            revision: 0,
            sections: vec![section],
        }
    }

    #[test]
    fn rockwell_addresses_with_no_slot_name_their_card() {
        let none = output(&[("Local:3:I.Data.0", "-A1"), ("%IX0.0", "-A2")]);
        assert_eq!(missing_slot_note(&none), None);
        let some = output(&[
            ("Local:?:I.Data.0", "-A1"),
            ("Local:?:O.Data.0", "-A1"),
            ("Local:?:I.Data.0", "-A3"),
        ]);
        let note = missing_slot_note(&some).unwrap();
        assert!(note.starts_with("Slot missing on -A1, -A3:"), "{note}");
    }
}
