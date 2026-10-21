//! "Part from a datasheet" inside the Ask Claude window: the person picks a
//! PDF, agrees to send that file, Claude reads it, and the person checks every
//! value (with the page and words it came from) before the part is added.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui;
use novo_ai::api::{ApiError, Client};
use novo_ai::datasheet::{self, Draft, FieldValue, MAX_PDF_BYTES, Reading};
use novo_ai::usage::month_of;
use novo_core::{Command, Value};

use super::{Entry, NovoApp};
use crate::theme::ui_palette;

/// One line of the review table.
pub(super) struct Row {
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    keep: bool,
    text: String,
    /// Where in the datasheet it was read, or why it is empty.
    evidence: String,
}

pub(super) struct Review {
    file: String,
    rows: Vec<Row>,
    notes: String,
    refused: Vec<String>,
}

#[derive(Default)]
pub(super) enum Stage {
    #[default]
    Idle,
    /// A file is chosen; nothing has been sent yet.
    Confirm {
        name: String,
        bytes: Arc<Vec<u8>>,
    },
    Reading(Receiver<Result<Reading, ApiError>>),
    Review(Review),
    /// The last outcome, shown until the next datasheet.
    Note(String),
}

#[derive(Default)]
pub(super) struct State {
    pub(super) stage: Stage,
}

fn number_text(n: f64) -> String {
    let s = format!("{n:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn review_of(file: String, draft: Draft) -> Review {
    let mut rows: Vec<Row> = draft
        .fields
        .iter()
        .map(|f| Row {
            key: f.key,
            label: f.label,
            unit: f.unit,
            keep: true,
            text: match &f.value {
                FieldValue::Text(t) => t.clone(),
                FieldValue::Number(n) => number_text(*n),
            },
            evidence: match f.page {
                Some(p) => format!("page {p}: “{}”", f.quote),
                None => format!("“{}”", f.quote),
            },
        })
        .collect();
    // The part cannot be added without these two: the person types what is missing.
    for (pos, (key, label)) in [
        ("manufacturer", "Manufacturer"),
        ("order_number", "Order number"),
    ]
    .into_iter()
    .enumerate()
    {
        if !rows.iter().any(|r| r.key == key) {
            rows.insert(
                pos.min(rows.len()),
                Row {
                    key,
                    label,
                    unit: "",
                    keep: true,
                    text: String::new(),
                    evidence: "Not found in the datasheet: type it from the PDF.".into(),
                },
            );
        }
    }
    Review {
        file,
        rows,
        notes: draft.notes,
        refused: draft.refused,
    }
}

impl Review {
    fn text_of(&self, key: &str) -> String {
        self.rows
            .iter()
            .find(|r| r.key == key && r.keep)
            .map(|r| r.text.trim().to_string())
            .unwrap_or_default()
    }

    /// Why the part cannot be added yet, if it cannot.
    fn problem(&self) -> Option<String> {
        if self.text_of("manufacturer").is_empty() || self.text_of("order_number").is_empty() {
            return Some("Manufacturer and order number are needed.".into());
        }
        for r in self.rows.iter().filter(|r| r.keep && !r.unit.is_empty()) {
            match r.text.trim().replace(',', ".").parse::<f64>() {
                Ok(n) if n.is_finite() && n > 0.0 => {}
                _ => {
                    return Some(format!(
                        "{} must be a positive number, or untick it.",
                        r.label
                    ));
                }
            }
        }
        None
    }
}

impl NovoApp {
    /// Takes the answer of the background reading; called every frame.
    pub(super) fn datasheet_poll(&mut self) {
        let st = &mut self.wb.assistant;
        let Stage::Reading(rx) = &st.sheet.stage else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                st.sheet.stage = Stage::Note("Reading the datasheet stopped unexpectedly.".into());
                return;
            }
        };
        match result {
            Ok(reading) => {
                let month = month_of((st.now)());
                st.ledger.record(&month, &reading.usage, reading.cost_usd);
                st.session_cost += reading.cost_usd;
                if let Some(path) = &st.ledger_path {
                    let _ = st.ledger.save(path);
                }
                let file = st.datasheet_name.clone();
                st.sheet.stage = match reading.failure {
                    Some(f) => Stage::Note(f),
                    None => Stage::Review(review_of(file, reading.draft)),
                };
            }
            Err(e) => st.sheet.stage = Stage::Note(e.to_string()),
        }
    }

    pub(super) fn datasheet_start(&mut self, ctx: &egui::Context) {
        let Some((key, _)) = self.wb.assistant.key() else {
            self.wb.assistant.sheet.stage = Stage::Note("No API key yet.".into());
            return;
        };
        let settings = self.settings.ai.clone();
        let st = &mut self.wb.assistant;
        if !matches!(st.sheet.stage, Stage::Confirm { .. }) {
            return;
        }
        let Stage::Confirm { name, bytes } = std::mem::take(&mut st.sheet.stage) else {
            return;
        };
        st.datasheet_name = name.clone();
        let spent = st.spent();
        let factory = Arc::clone(&st.transport);
        let (tx, rx) = channel();
        st.sheet.stage = Stage::Reading(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut client = Client::new(factory(), key);
            let _ = tx.send(datasheet::read(
                &mut client,
                &settings,
                spent,
                &name,
                &bytes,
            ));
            ctx.request_repaint();
        });
    }

    fn datasheet_choose(&mut self) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Choose a datasheet")
            .add_filter("PDF", &["pdf"]);
        if let Some(dir) = dirs::document_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let name = path.file_name().map_or_else(
            || "datasheet.pdf".into(),
            |n| n.to_string_lossy().into_owned(),
        );
        if let Ok(meta) = std::fs::metadata(&path)
            && meta.len() > MAX_PDF_BYTES as u64
        {
            self.wb.assistant.sheet.stage = Stage::Note(format!(
                "{name} is larger than {} MB.",
                MAX_PDF_BYTES / 1024 / 1024
            ));
            return;
        }
        self.wb.assistant.sheet.stage = match std::fs::read(&path) {
            Ok(b) if b.len() > MAX_PDF_BYTES => Stage::Note(format!(
                "{name} is larger than {} MB.",
                MAX_PDF_BYTES / 1024 / 1024
            )),
            Ok(b) if !b.starts_with(b"%PDF") => Stage::Note(format!("{name} is not a PDF.")),
            Ok(b) => Stage::Confirm {
                name,
                bytes: Arc::new(b),
            },
            Err(e) => Stage::Note(format!("Could not read {name}: {e}")),
        };
    }

    /// Adds the checked part as one undo step. Returns the message to show.
    pub(super) fn datasheet_add(&mut self) -> Result<String, String> {
        let Stage::Review(review) = &self.wb.assistant.sheet.stage else {
            return Err("There is no datasheet to add.".into());
        };
        if let Some(p) = review.problem() {
            return Err(p);
        }
        let maker = review.text_of("manufacturer");
        let order = review.text_of("order_number");
        let type_number = review.text_of("type_number");
        let description = review.text_of("description");
        let props: Vec<(&'static str, f64)> = review
            .rows
            .iter()
            .filter(|r| r.keep && !r.unit.is_empty())
            .filter_map(|r| {
                r.text
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .map(|n| (r.key, n))
            })
            .collect();
        let doc = self
            .backend
            .document_mut()
            .ok_or_else(|| "No project is open.".to_string())?;
        let label = format!("Add part {maker} {order} from a datasheet");
        let (_, changes) = doc
            .transact(label, |tx| {
                let created = tx.execute(Command::AddPart {
                    manufacturer: maker.clone(),
                    order_number: order.clone(),
                    type_number,
                    description,
                })?;
                let id = created
                    .first()
                    .copied()
                    .ok_or_else(|| novo_core::ModelError::Format("no part was created".into()))?;
                for (key, n) in props {
                    tx.set_property(id, key, Some(Value::Float(n)))?;
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        self.canvas.document_changed(doc, &changes);
        self.backend.record(&changes);
        self.reports_heard(&changes);
        self.wb.assistant.sheet.stage =
            Stage::Note(format!("Added part {maker} {order}. Ctrl+Z undoes it."));
        Ok(format!("Added part {maker} {order}"))
    }

    /// Whether the part being reviewed is already in the project.
    pub(super) fn datasheet_duplicate(&self) -> bool {
        let Stage::Review(review) = &self.wb.assistant.sheet.stage else {
            return false;
        };
        let (maker, order) = (
            review.text_of("manufacturer"),
            review.text_of("order_number"),
        );
        if maker.is_empty() || order.is_empty() {
            return false;
        }
        self.document().is_some_and(|d| {
            d.project()
                .parts()
                .any(|p| p.manufacturer == maker && p.order_number == order)
        })
    }

    /// The section of the window. Nothing is sent before "Send datasheet".
    pub(super) fn datasheet_ui(&mut self, ui: &mut egui::Ui) {
        let pal = ui_palette();
        let busy = self.wb.assistant.running.is_some();
        let mut choose = false;
        let mut send = false;
        let mut cancel = false;
        let mut add = false;
        let mut note_clear = false;
        let mut stop = false;
        let spent = self.wb.assistant.spent();
        let duplicate = self.datasheet_duplicate();
        egui::CollapsingHeader::new("Part from a datasheet")
            .id_salt("claude_datasheet")
            .show(ui, |ui| match &mut self.wb.assistant.sheet.stage {
                Stage::Idle => {
                    ui.weak(
                        "Claude reads a PDF datasheet and fills in a part. You check every \
                         value before it is added.",
                    );
                    choose = ui
                        .add_enabled(!busy, egui::Button::new("Choose datasheet (PDF)…"))
                        .clicked();
                }
                Stage::Note(text) => {
                    ui.label(text.as_str());
                    if ui.small_button("OK").clicked() {
                        note_clear = true;
                    }
                }
                Stage::Confirm { name, bytes } => {
                    ui.label(
                        egui::RichText::new(format!(
                            "This sends the whole file “{name}” ({:.1} MB) to Anthropic with \
                             your API key. Nothing from your project is sent.",
                            bytes.len() as f64 / 1_048_576.0
                        ))
                        .color(pal.warning),
                    );
                    ui.weak(format!(
                        "A PDF is billed per page, as text and as pictures, so a long \
                         datasheet can cost a few dollars. Spent this month: ${spent:.2}."
                    ));
                    ui.horizontal(|ui| {
                        send = ui.button("Send datasheet").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                }
                Stage::Reading(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Claude is reading the datasheet…");
                        stop = ui.button("Stop").clicked();
                    });
                }
                Stage::Review(review) => {
                    ui.label(egui::RichText::new(format!("From “{}”", review.file)).strong());
                    ui.weak(
                        "Claude's reading, not the datasheet itself. Check each value against \
                         the PDF; untick what is wrong or not wanted.",
                    );
                    for r in &mut review.rows {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut r.keep, "");
                            ui.label(r.label);
                            ui.add_enabled(
                                r.keep,
                                egui::TextEdit::singleline(&mut r.text).desired_width(150.0),
                            );
                            ui.weak(r.unit);
                        });
                        ui.indent(("evidence", r.key), |ui| {
                            ui.weak(&r.evidence);
                        });
                    }
                    if !review.notes.is_empty() {
                        ui.label(format!("Note: {}", review.notes));
                    }
                    for line in &review.refused {
                        ui.colored_label(pal.warning, format!("Left out: {line}"));
                    }
                    if duplicate {
                        ui.colored_label(
                            pal.warning,
                            "A part with this manufacturer and order number is already in the \
                             project; adding it makes a second one.",
                        );
                    }
                    let problem = review.problem();
                    if let Some(p) = &problem {
                        ui.colored_label(pal.error, p);
                    }
                    ui.horizontal(|ui| {
                        add = ui
                            .add_enabled(
                                problem.is_none(),
                                egui::Button::new("Add part to project"),
                            )
                            .clicked();
                        cancel = ui.button("Discard").clicked();
                    });
                }
            });
        if choose {
            self.datasheet_choose();
        }
        if send {
            let ctx = ui.ctx().clone();
            self.datasheet_start(&ctx);
        }
        if stop {
            // The request may already be billed; its answer is dropped unread.
            self.wb.assistant.sheet.stage = Stage::Note(
                "Stopped. Anthropic may still bill the request, and its cost is not counted here."
                    .into(),
            );
        }
        if cancel || note_clear {
            self.wb.assistant.sheet.stage = Stage::Idle;
        }
        if add && let Err(e) = self.datasheet_add() {
            self.wb.assistant.transcript.push(Entry::Problem(e));
        }
    }
}
