//! PLC and I/O commands (docs/design/plc.md §4): the command field names
//! them, the app runs them through novo-plc. Each is one undo step.

use std::collections::BTreeSet;

use eframe::egui;
use novo_core::{Endpoint, Placement, Point};
use novo_editor::catalog::model_pins;
use novo_plc::commands::{RenumberPlan, apply_renumber, renumber_plan, to_plc};
use novo_plc::{CardLayout, ChannelType, PlcIndex, Space, cards};

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
        if self.current_page.is_none() {
            return;
        }
        let text = argument.map(str::trim).unwrap_or_default();
        if text.is_empty() {
            self.open_plc_card_dialog();
            return;
        }
        let layout: CardLayout = match text.parse() {
            Ok(layout) => layout,
            Err(e) => {
                self.message(Level::Warning, format!("Not a card layout: {e}"));
                return;
            }
        };
        self.plc_add_card_with(layout, "", None, [None, None]);
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
        let key = view.editor.catalog.in_pack(channel_symbol(kind));
        let key = key.as_str();
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

    /// `renumber io`: plans every station's cards packed back to back in
    /// slot order and opens the preview; Enter applies it, Esc cancels
    /// (docs/design/plc.md §3).
    pub(super) fn plc_renumber_io(&mut self) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let stations: BTreeSet<String> = cards(project).into_iter().map(|c| c.station).collect();
        if stations.is_empty() {
            self.message(Level::Info, "The project has no PLC cards");
            return;
        }
        let plans: Vec<RenumberPlan> = stations
            .iter()
            .map(|station| renumber_plan(project, station))
            .filter(|plan| !plan.is_empty())
            .collect();
        if plans.is_empty() {
            self.message(Level::Info, "I/O addresses are already in order");
            return;
        }
        self.wb.renumber = Some(plans);
    }

    /// The renumber preview: every address that moves, and the cards left
    /// alone. Enter applies, Esc or Cancel leaves the project untouched.
    pub(super) fn renumber_dialog(&mut self, ctx: &egui::Context) {
        let Some(plans) = &self.wb.renumber else {
            return;
        };
        let Some(doc) = self.backend.document() else {
            self.wb.renumber = None;
            return;
        };
        let project = doc.project();
        let tag_of = |id| {
            project
                .device(id)
                .map(|d| d.tag.to_string())
                .unwrap_or_default()
        };
        let address = |space: Space, start: Option<u32>| match start {
            Some(n) => format!("%{}{n}", space.letter()),
            None => "none".to_owned(),
        };
        let modal = egui::Modal::new(egui::Id::new("renumber_io")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading("Renumber I/O");
            ui.add_space(4.0);
            for plan in plans {
                for m in &plan.moves {
                    ui.label(format!(
                        "{}  {} → {}",
                        tag_of(m.card),
                        address(m.space, m.from),
                        address(m.space, Some(m.to))
                    ));
                }
                for &card in &plan.skipped {
                    ui.label(format!(
                        "{}  left alone: its layout is not valid",
                        tag_of(card)
                    ));
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let apply = ui.button("Renumber (Enter)").clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                (apply, cancel)
            })
            .inner
        });
        let (apply, cancel) = modal.inner;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if apply || enter {
            if let Some(plans) = self.wb.renumber.take() {
                self.apply_renumber_plans(&plans);
            }
        } else if cancel || modal.should_close() {
            self.wb.renumber = None;
        }
    }

    /// The address texts under imported channels follow the cards: after an
    /// edit of a device (a card added, moved, deleted) or of the project info
    /// (the address style), stale ones are brought up to date in the same undo
    /// step. Does nothing, and adds no step, when none is stale.
    pub(super) fn sync_plc_address_texts(&mut self, changes: &novo_core::ChangeSet) {
        let touches_cards = changes.info
            || changes
                .added
                .iter()
                .chain(&changes.removed)
                .chain(&changes.modified)
                .any(|id| matches!(id, novo_core::AnyId::Device(_)));
        if !touches_cards {
            return;
        }
        let library = self
            .canvas
            .schematic_ref()
            .map(|view| view.editor.catalog.library())
            .unwrap_or_else(|| novo_symbols::Library::shared(novo_symbols::Pack::Iec));
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        if novo_plc::stale_address_texts(doc.project(), library).is_empty() {
            return;
        }
        if let Ok((_, extra)) = doc.amend_last("Update PLC address texts", |tx| {
            novo_plc::sync_address_texts(tx, library).map(|_| ())
        }) {
            self.changed_outside_canvas(&extra);
        }
    }

    fn apply_renumber_plans(&mut self, plans: &[RenumberPlan]) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let library = self
            .canvas
            .schematic_ref()
            .map(|view| view.editor.catalog.library());
        let result = doc.transact("Renumber I/O", |tx| {
            plans.iter().try_for_each(|plan| apply_renumber(tx, plan))?;
            // The address texts drawn by the I/O list import follow the cards.
            if let Some(library) = library {
                novo_plc::sync_address_texts(tx, library)?;
            }
            Ok(())
        });
        match result {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                let moved: usize = plans.iter().map(|p| p.moves.len()).sum();
                self.message(
                    Level::Info,
                    format!("Renumbered I/O: {moved} start address(es) moved"),
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not renumber I/O: {e}")),
        }
    }
}
