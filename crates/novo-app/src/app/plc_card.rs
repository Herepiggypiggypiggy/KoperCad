//! Draw > PLC > Add PLC card (a small layout dialog) and Draw > PLC > PLC
//! overview (a read-only list of cards, channels and addresses).
//!
//! Both sit on `novo-plc`: the dialog builds a [`CardLayout`] and calls the
//! same `add_card` the command field's `add card DI16 DQ8` uses; the
//! overview reads [`PlcIndex`] and shows what it holds.

use std::collections::BTreeMap;

use eframe::egui;
use novo_core::{DeviceTag, Project, Value};
use novo_editor::naming::next_device_tag;
use novo_plc::card::start_key;
use novo_plc::commands::{NewCardLayout, add_card};
use novo_plc::{CardLayout, ChannelType, PlcIndex, Space, cards, next_start};

use super::{Level, NovoApp};

/// The Add PLC card dialog's fields.
#[derive(Debug, Clone, Default)]
pub(crate) struct PlcCardForm {
    /// Channel counts in the order of [`ChannelType::ALL`].
    pub counts: [u16; 5],
    pub station: String,
    /// Slot in the station; empty for none.
    pub slot: String,
    /// First input byte; empty for the next free one.
    pub start_input: String,
    /// First output byte; empty for the next free one.
    pub start_output: String,
    pub problem: Option<String>,
}

/// A few common cards to start from.
const PRESETS: &[(&str, [u16; 5])] = &[
    ("DI16", [16, 0, 0, 0, 0]),
    ("DQ16", [0, 16, 0, 0, 0]),
    ("DI16 DQ8", [16, 8, 0, 0, 0]),
    ("DI8 RO8", [8, 0, 8, 0, 0]),
    ("AI4", [0, 0, 0, 4, 0]),
    ("AQ2", [0, 0, 0, 0, 2]),
];

/// What a checked form asks for: layout, slot, chosen starts.
type Checked = (CardLayout, Option<u16>, [Option<u32>; 2]);

impl PlcCardForm {
    /// The layout the counts make, `DI16 DQ8`.
    pub(crate) fn layout_text(&self) -> String {
        ChannelType::ALL
            .iter()
            .zip(self.counts)
            .filter(|(_, n)| *n > 0)
            .map(|(k, n)| format!("{}{n}", k.code()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn start_of(text: &str, what: &str) -> Result<Option<u32>, String> {
        let t = text.trim();
        if t.is_empty() {
            return Ok(None);
        }
        t.parse::<u32>()
            .map(Some)
            .map_err(|_| format!("{what} start must be a whole number of bytes"))
    }

    /// Checks the form against the project: the layout parses, the slot
    /// is a number, and a chosen start is on a word for analog channels and
    /// does not overlap another card of the station. Returns the layout and
    /// the starts to set.
    pub(crate) fn check(&self, project: &Project) -> Result<Checked, String> {
        let layout: CardLayout = self.layout_text().parse().map_err(|e| format!("{e}"))?;
        let slot = match self.slot.trim() {
            "" => None,
            s => Some(s.parse::<u16>().map_err(|_| "the slot must be a number")?),
        };
        let starts = [
            Self::start_of(&self.start_input, "Input")?,
            Self::start_of(&self.start_output, "Output")?,
        ];
        let station = self.station.trim();
        for space in Space::ALL {
            let Some(start) = starts[space as usize] else {
                continue;
            };
            let bytes = layout.bytes(space);
            if bytes == 0 {
                return Err(format!(
                    "the card has no {} channels, so it has no start there",
                    if space == Space::Input {
                        "input"
                    } else {
                        "output"
                    }
                ));
            }
            if layout.has_analog(space) && start % 2 != 0 {
                return Err("a card with analog channels starts on an even byte".into());
            }
            let end = start + bytes;
            for c in cards(project).iter().filter(|c| c.station == station) {
                if let Some(r) = c.range(space)
                    && start < r.end
                    && r.start < end
                {
                    let tag = project
                        .device(c.device)
                        .map(|d| d.tag.to_string())
                        .unwrap_or_default();
                    return Err(format!(
                        "bytes {start} to {} overlap {tag} ({} to {})",
                        end - 1,
                        r.start,
                        r.end - 1
                    ));
                }
            }
        }
        Ok((layout, slot, starts))
    }
}

impl NovoApp {
    /// Opens the Add PLC card dialog.
    pub(super) fn open_plc_card_dialog(&mut self) {
        if self.current_page.is_none() {
            self.message(Level::Info, "Open a page to add a PLC card to");
            return;
        }
        self.wb.plc_card = Some(PlcCardForm {
            counts: [16, 8, 0, 0, 0],
            ..PlcCardForm::default()
        });
    }

    /// Adds a card with `layout` on the current page: the next free `-K`
    /// tag, the station and slot given, and the chosen start addresses
    /// where the person set one (the next free ones otherwise).
    pub(crate) fn plc_add_card_with(
        &mut self,
        layout: CardLayout,
        station: &str,
        slot: Option<u16>,
        starts: [Option<u32>; 2],
    ) -> bool {
        let Some(page) = self.current_page else {
            return false;
        };
        let text = layout_string(&layout);
        let Some(doc) = self.backend.document_mut() else {
            return false;
        };
        let result = doc.transact("Add PLC card", |tx| {
            let tag: DeviceTag = next_device_tag(tx.project(), page, "K");
            let card = add_card(
                tx,
                tag.clone(),
                "",
                NewCardLayout::Typed(layout),
                station,
                slot,
            )?;
            for space in Space::ALL {
                if let Some(start) = starts[space as usize] {
                    tx.set_property(card, start_key(space), Some(Value::Int(i64::from(start))))?;
                }
            }
            Ok((card, tag))
        });
        match result {
            Ok(((_, tag), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Added card {tag} ({text})"));
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not add the card: {e}"));
                false
            }
        }
    }

    /// Applies the dialog; the dialog stays open with the reason when the
    /// form is wrong.
    pub(crate) fn confirm_plc_card(&mut self) -> bool {
        let Some(form) = self.wb.plc_card.clone() else {
            return false;
        };
        let Some(doc) = self.backend.document() else {
            return false;
        };
        match form.check(doc.project()) {
            Ok((layout, slot, starts)) => {
                self.wb.plc_card = None;
                self.plc_add_card_with(layout, form.station.trim(), slot, starts)
            }
            Err(e) => {
                if let Some(f) = self.wb.plc_card.as_mut() {
                    f.problem = Some(e);
                }
                false
            }
        }
    }

    pub(super) fn plc_card_dialog(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.wb.plc_card else {
            return;
        };
        let (mut go, mut cancel) = (false, false);
        // What the next free start would be, shown as the box's hint.
        let hints: [String; 2] = match (
            self.backend.document(),
            form.layout_text().parse::<CardLayout>(),
        ) {
            (Some(doc), Ok(layout)) => Space::ALL.map(|space| {
                next_start(doc.project(), form.station.trim(), &layout, space, None)
                    .map(|n| format!("next free: {n}"))
                    .unwrap_or_else(|| "no channels".to_owned())
            }),
            _ => [String::new(), String::new()],
        };
        let modal = egui::Modal::new(egui::Id::new("plc_card")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Add PLC card");
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Start from");
                for (name, counts) in PRESETS {
                    if ui.small_button(*name).clicked() {
                        form.counts = *counts;
                    }
                }
            });
            ui.add_space(4.0);
            egui::Grid::new("plc_card_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for (kind, count) in ChannelType::ALL.iter().zip(form.counts.iter_mut()) {
                        ui.label(match kind {
                            ChannelType::DigitalInput => "Digital inputs (DI)",
                            ChannelType::DigitalOutput => "Digital outputs (DQ)",
                            ChannelType::RelayOutput => "Relay outputs (RO)",
                            ChannelType::AnalogInput => "Analog inputs (AI)",
                            ChannelType::AnalogOutput => "Analog outputs (AQ)",
                        });
                        ui.add(egui::DragValue::new(count).range(0..=1024));
                        ui.end_row();
                    }
                    ui.label("Station");
                    ui.text_edit_singleline(&mut form.station);
                    ui.end_row();
                    ui.label("Slot");
                    ui.add(egui::TextEdit::singleline(&mut form.slot).desired_width(60.0));
                    ui.end_row();
                    ui.label("First input byte");
                    ui.add(
                        egui::TextEdit::singleline(&mut form.start_input)
                            .hint_text(hints[0].as_str())
                            .desired_width(120.0),
                    );
                    ui.end_row();
                    ui.label("First output byte");
                    ui.add(
                        egui::TextEdit::singleline(&mut form.start_output)
                            .hint_text(hints[1].as_str())
                            .desired_width(120.0),
                    );
                    ui.end_row();
                });
            ui.add_space(4.0);
            let layout = form.layout_text();
            ui.weak(if layout.is_empty() {
                "Layout: no channels yet".to_owned()
            } else {
                format!("Layout: {layout}. Left empty, the starts follow the cards already there.")
            });
            if let Some(p) = &form.problem {
                ui.colored_label(novo_theme::tokens::WARNING, p);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                go = ui.button("Add card").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if go {
            self.confirm_plc_card();
        } else if cancel || modal.should_close() {
            self.wb.plc_card = None;
        }
    }
}

fn layout_string(layout: &CardLayout) -> String {
    layout
        .groups
        .iter()
        .map(|g| format!("{}{}", g.kind.code(), g.count))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One row of the overview: a card with its ranges.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CardRow {
    pub tag: String,
    pub station: String,
    pub slot: Option<u16>,
    pub layout: String,
    pub input: Option<String>,
    pub output: Option<String>,
    /// `DI 0  %IX0.0` and whether it is drawn on a page.
    pub channels: Vec<(String, bool)>,
    pub problem: Option<String>,
}

/// The overview's rows and the index's issues, from the project.
pub(crate) fn overview(
    project: &Project,
    functions: &dyn novo_plc::index::Functions,
) -> (Vec<CardRow>, Vec<String>) {
    let index = PlcIndex::build(project, functions);
    let style = index.style;
    let range = |r: Option<std::ops::Range<u32>>, letter: char| {
        r.map(|r| format!("%{letter}B{} to {}", r.start, r.end - 1))
    };
    let rows = index
        .cards()
        .iter()
        .map(|card| {
            let layout = card.channels();
            let mut channels = Vec::new();
            for kind in ChannelType::ALL {
                for n in 0..layout.count(kind) {
                    let address = card
                        .address_in(kind, n, style)
                        .map(|a| a.format(style))
                        .unwrap_or_else(|| "no address".to_owned());
                    let drawn = !index.drawn(card.device, kind, n).is_empty();
                    channels.push((format!("{} {n}  {address}", kind.code()), drawn));
                }
            }
            CardRow {
                tag: project
                    .device(card.device)
                    .map(|d| d.tag.to_string())
                    .unwrap_or_default(),
                station: card.station.clone(),
                slot: card.slot,
                layout: layout_string(&layout),
                input: range(card.range(Space::Input), 'I'),
                output: range(card.range(Space::Output), 'Q'),
                channels,
                problem: card.layout.as_ref().err().map(ToString::to_string),
            }
        })
        .collect();
    let issues = index.issues().iter().map(|i| i.message(project)).collect();
    (rows, issues)
}

impl NovoApp {
    pub(super) fn plc_overview_window(&mut self, ctx: &egui::Context) {
        if !self.wb.plc_overview {
            return;
        }
        let Some(doc) = self.backend.document() else {
            self.wb.plc_overview = false;
            return;
        };
        let library = match self.canvas.schematic_ref() {
            Some(view) => view.editor.catalog.library(),
            None => novo_symbols::Library::shared(novo_symbols::Pack::Iec),
        };
        let (rows, issues) = overview(doc.project(), library);
        let mut open = true;
        let (mut add, mut renumber) = (false, false);
        egui::Window::new("PLC overview")
            .id(egui::Id::new("plc_overview"))
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| {
                if rows.is_empty() {
                    ui.label("The project has no PLC cards yet.");
                }
                ui.horizontal(|ui| {
                    add = ui.button("Add PLC card...").clicked();
                    renumber = ui
                        .add_enabled(!rows.is_empty(), egui::Button::new("Renumber I/O..."))
                        .clicked();
                });
                ui.add_space(4.0);
                let mut by_station: BTreeMap<&str, Vec<&CardRow>> = BTreeMap::new();
                for r in &rows {
                    by_station.entry(r.station.as_str()).or_default().push(r);
                }
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for (station, cards) in by_station {
                            ui.strong(if station.is_empty() {
                                "Station (unnamed)".to_owned()
                            } else {
                                format!("Station {station}")
                            });
                            for r in cards {
                                let title = format!(
                                    "{}  {}{}",
                                    r.tag,
                                    if r.layout.is_empty() {
                                        "no channels"
                                    } else {
                                        &r.layout
                                    },
                                    r.slot.map(|s| format!("  slot {s}")).unwrap_or_default()
                                );
                                egui::CollapsingHeader::new(title)
                                    .id_salt(("plc_overview_card", &r.tag))
                                    .show(ui, |ui| {
                                        ui.weak(format!(
                                            "Inputs: {}   Outputs: {}",
                                            r.input.as_deref().unwrap_or("none"),
                                            r.output.as_deref().unwrap_or("none")
                                        ));
                                        if let Some(p) = &r.problem {
                                            ui.colored_label(novo_theme::tokens::WARNING, p);
                                        }
                                        for (text, drawn) in &r.channels {
                                            ui.label(format!(
                                                "{text}{}",
                                                if *drawn { "   drawn" } else { "" }
                                            ));
                                        }
                                    });
                            }
                        }
                        if !issues.is_empty() {
                            ui.add_space(6.0);
                            ui.strong("Problems");
                            for i in &issues {
                                ui.colored_label(novo_theme::tokens::WARNING, i);
                            }
                        }
                    });
            });
        if !open {
            self.wb.plc_overview = false;
        }
        if add {
            self.open_plc_card_dialog();
        }
        if renumber {
            self.plc_renumber_io();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counts_make_a_layout_text() {
        let f = PlcCardForm {
            counts: [16, 8, 0, 2, 0],
            ..PlcCardForm::default()
        };
        assert_eq!(f.layout_text(), "DI16 DQ8 AI2");
        assert_eq!(PlcCardForm::default().layout_text(), "");
    }

    #[test]
    fn an_empty_form_is_refused_and_a_bad_slot_is_named() {
        let project = Project::new("t");
        assert!(PlcCardForm::default().check(&project).is_err());
        let f = PlcCardForm {
            counts: [4, 0, 0, 0, 0],
            slot: "x".into(),
            ..PlcCardForm::default()
        };
        assert!(f.check(&project).unwrap_err().contains("slot"));
    }

    #[test]
    fn a_start_with_no_channels_there_or_odd_for_analog_is_refused() {
        let project = Project::new("t");
        let f = PlcCardForm {
            counts: [4, 0, 0, 0, 0],
            start_output: "8".into(),
            ..PlcCardForm::default()
        };
        assert!(f.check(&project).unwrap_err().contains("no output"));
        let f = PlcCardForm {
            counts: [0, 0, 0, 2, 0],
            start_input: "3".into(),
            ..PlcCardForm::default()
        };
        assert!(f.check(&project).unwrap_err().contains("even"));
    }
}
