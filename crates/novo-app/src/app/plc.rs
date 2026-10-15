//! PLC and I/O commands (docs/design/plc.md §4): the command field names
//! them, the app runs them through novo-plc. Each is one undo step.

use std::collections::BTreeSet;

use novo_core::{DeviceTag, Endpoint, Placement, Point};
use novo_editor::catalog::model_pins;
use novo_editor::naming::next_device_tag;
use novo_plc::commands::{NewCardLayout, add_card, apply_renumber, renumber_plan, to_plc};
use novo_plc::{CardLayout, ChannelType, PlcIndex, cards};

use super::{Level, NovoApp};

/// How far right of its signal a channel is drawn, in millimetres.
const CHANNEL_OFFSET: f64 = 50.0;

/// The library key of the symbol that draws a channel of `kind`.
fn channel_symbol(kind: ChannelType) -> &'static str {
    match kind {
        ChannelType::DigitalInput => "iec/plc/digital_input",
        ChannelType::DigitalOutput => "iec/plc/digital_output",
        ChannelType::RelayOutput => "iec/plc/relay_output",
        ChannelType::AnalogInput => "iec/plc/analog_input",
        ChannelType::AnalogOutput => "iec/plc/analog_output",
    }
}

impl NovoApp {
    /// `add card DI16 DQ8`: a PLC card with that layout, the next free `-K`
    /// tag on the page and the next free addresses of the station.
    pub(super) fn plc_add_card(&mut self, argument: Option<&str>) {
        let Some(page) = self.current_page else {
            return;
        };
        let text = argument.map(str::trim).unwrap_or_default();
        if text.is_empty() {
            self.message(
                Level::Info,
                "Add card needs its channels, for example: add card DI16 DQ8",
            );
            return;
        }
        let layout: CardLayout = match text.parse() {
            Ok(layout) => layout,
            Err(e) => {
                self.message(Level::Warning, format!("Not a card layout: {e}"));
                return;
            }
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Add PLC card", |tx| {
            let tag: DeviceTag = next_device_tag(tx.project(), page, "K");
            let card = add_card(tx, tag.clone(), "", NewCardLayout::Typed(layout), "", None)?;
            Ok((card, tag))
        });
        match result {
            Ok(((_, tag), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Added card {tag} ({text})"));
            }
            Err(e) => self.message(Level::Error, format!("Could not add the card: {e}")),
        }
    }

    /// `to plc`: with a contact, sensor or coil selected, draws the next
    /// free channel of the right type beside it and connects it.
    pub(super) fn plc_to_plc(&mut self) {
        let Some(symbol) = self.selected_symbol() else {
            self.message(
                Level::Info,
                "Select a contact, sensor or coil to send to the PLC",
            );
            return;
        };
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let library = view.editor.catalog.library();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let project = doc.project();
        let Some(signal) = project.symbol(symbol) else {
            return;
        };
        let kind = library
            .get(&signal.symbol.0)
            .and_then(|s| novo_plc::commands::signal_channel_type(&s.function));
        let Some(kind) = kind else {
            self.message(Level::Info, "To PLC works on a contact, a sensor or a coil");
            return;
        };
        let index = PlcIndex::build(project, library);
        let Some(&card) = index.cards_with_free(kind).first() else {
            self.message(
                Level::Info,
                format!(
                    "No PLC card has a free {} channel. Add one with: add card",
                    kind.code()
                ),
            );
            return;
        };
        // The pin that nothing is wired to yet, from the far end.
        let pin = (0..signal.pins.len())
            .rev()
            .filter_map(|i| u16::try_from(i).ok())
            .find(|&i| project.wires_at(Endpoint::pin(symbol, i)).next().is_none());
        let Some(pin) = pin else {
            self.message(Level::Info, "Every pin of that symbol is already wired");
            return;
        };
        let key = channel_symbol(kind);
        let Some(channel) = library.get(key) else {
            return;
        };
        let pins = model_pins(channel);
        let origin = signal.placement.origin;
        let placement = Placement::at(Point::new(origin.x + CHANNEL_OFFSET, origin.y));
        let tag = project.device(card).map(|d| d.tag.to_string());
        let result = doc.transact("Send signal to PLC", |tx| {
            to_plc(
                tx,
                library,
                card,
                Endpoint::pin(symbol, pin),
                kind,
                key,
                placement,
                pins,
            )
        });
        match result {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                let tag = tag.unwrap_or_default();
                self.message(
                    Level::Info,
                    format!("Sent to {tag}: next free {} channel", kind.code()),
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not send it to the PLC: {e}")),
        }
    }

    /// `renumber io`: packs every station's cards back to back in slot
    /// order, so addresses have no gaps.
    pub(super) fn plc_renumber_io(&mut self) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let stations: BTreeSet<String> = cards(doc.project())
            .into_iter()
            .map(|c| c.station)
            .collect();
        if stations.is_empty() {
            self.message(Level::Info, "The project has no PLC cards");
            return;
        }
        let result = doc.transact("Renumber I/O", |tx| {
            let mut moved = 0;
            let mut skipped = 0;
            for station in &stations {
                let plan = renumber_plan(tx.project(), station);
                moved += plan.moves.len();
                skipped += plan.skipped.len();
                apply_renumber(tx, &plan)?;
            }
            Ok((moved, skipped))
        });
        match result {
            Ok(((moved, skipped), changes)) => {
                self.changed_outside_canvas(&changes);
                let text = match (moved, skipped) {
                    (0, 0) => "I/O addresses are already in order".to_owned(),
                    (m, 0) => format!("Renumbered I/O: {m} start address(es) moved"),
                    (m, s) => format!(
                        "Renumbered I/O: {m} start address(es) moved; {s} card(s) with a wrong layout were left alone"
                    ),
                };
                self.message(Level::Info, text);
            }
            Err(e) => self.message(Level::Error, format!("Could not renumber I/O: {e}")),
        }
    }
}
