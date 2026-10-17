//! "Detect connection points" in the Parts window (lane PINS-UI).
//!
//! Reads the part's stored STEP model for wire openings and pins, matches
//! them to the catalog's terminals and shows how sure each match is:
//! Measured (the model's openings equal the terminals on that side),
//! Partial (more openings than terminals) or Estimated (spread evenly, not
//! read from the model). Saving writes the positions into the user's
//! catalog; wire routing then ends each wire on the terminal itself.
//!
//! Where it is: Parts window > select a part with a STEP model > 3D model >
//! Detect connection points.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use eframe::egui;
use novo_interop::part3d::pins::{Fix, PinMapping, apply_to_part, map_terminals, scan_step};
use novo_parts::{Catalog, CatalogPart, PartKey};

/// Longest a scan may take before it is given up.
const SCAN_LIMIT: Duration = Duration::from_secs(20);

/// What a scan found, matched to a part.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Detection {
    pub mapping: PinMapping,
    /// Wire openings and pins the model has, before matching.
    pub openings: usize,
    pub pins: usize,
}

/// Reads `bytes` (a STEP model) and matches its connection points to the
/// terminals of `part`.
pub(crate) fn detect(
    bytes: &[u8],
    part: &CatalogPart,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
) -> Result<Detection, String> {
    let size = part
        .dimensions
        .map(|d| [d.width_mm, d.height_mm, d.depth_mm]);
    let scan = scan_step(bytes, size, cancel, deadline).map_err(|e| e.to_string())?;
    Ok(Detection {
        mapping: map_terminals(&part.terminals, &scan),
        openings: scan.openings().count(),
        pins: scan.pins().count(),
    })
}

/// One line of the result list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PinRow {
    pub name: String,
    pub position: String,
    pub how: &'static str,
}

pub(crate) fn fix_label(fix: Fix) -> &'static str {
    match fix {
        Fix::Measured => "Measured",
        Fix::Partial => "Partial",
        Fix::Estimated => "Estimated",
    }
}

/// The list of terminals with their position and how it was found, then the
/// terminals left without one.
pub(crate) fn rows(d: &Detection) -> Vec<PinRow> {
    let mut out: Vec<PinRow> = d
        .mapping
        .fixes
        .iter()
        .map(|f| PinRow {
            name: f.name.clone(),
            position: format!(
                "{:.1} / {:.1} / {:.1} mm",
                f.position_mm[0], f.position_mm[1], f.position_mm[2]
            ),
            how: fix_label(f.fix),
        })
        .collect();
    out.extend(d.mapping.unplaced.iter().map(|n| PinRow {
        name: n.clone(),
        position: "no side given".into(),
        how: "Not placed",
    }));
    out
}

/// A one-line summary: counts per kind of fix.
pub(crate) fn summary(d: &Detection) -> String {
    let m = &d.mapping;
    format!(
        "{} openings and {} pins in the model. {} measured, {} partial, {} estimated, {} not placed.",
        d.openings,
        d.pins,
        m.count(Fix::Measured),
        m.count(Fix::Partial),
        m.count(Fix::Estimated),
        m.unplaced.len()
    )
}

/// Writes the detection into `part`; estimated points only when asked.
/// Returns how many terminals got a position.
pub(crate) fn apply(part: &mut CatalogPart, d: &Detection, with_estimates: bool) -> usize {
    apply_to_part(part, &d.mapping, with_estimates)
}

/// Saves `part` into the user catalog file at `path` (created when absent).
pub(crate) fn save_to_user_catalog(path: &Path, part: &CatalogPart) -> Result<(), String> {
    let mut user = if path.exists() {
        Catalog::load(path).map_err(|e| e.to_string())?
    } else {
        Catalog::new()
    };
    user.upsert(part.clone());
    user.save(path).map_err(|e| e.to_string())
}

enum Entry {
    Running(Receiver<Result<Detection, String>>),
    Done(Result<Detection, String>),
}

/// State of the block for the selected part.
#[derive(Default)]
pub(crate) struct PinsState {
    current: Option<(PartKey, Entry)>,
    /// Also save the estimated points.
    pub with_estimates: bool,
    /// Result of the last save, shown under the list.
    pub note: Option<String>,
}

impl PinsState {
    /// Starts a scan of the STEP file at `step` for `part` in the background.
    pub(crate) fn start(&mut self, ctx: &egui::Context, part: &CatalogPart, step: PathBuf) {
        let (tx, rx) = channel();
        let part_copy = part.clone();
        let wake = ctx.clone();
        std::thread::spawn(move || {
            let result = std::fs::read(&step)
                .map_err(|e| format!("The STEP file cannot be read: {e}"))
                .and_then(|bytes| {
                    detect(
                        &bytes,
                        &part_copy,
                        &AtomicBool::new(false),
                        Some(Instant::now() + SCAN_LIMIT),
                    )
                });
            let _ = tx.send(result);
            wake.request_repaint();
        });
        self.note = None;
        self.current = Some((part.key(), Entry::Running(rx)));
    }

    fn poll(&mut self) {
        if let Some((_, entry @ Entry::Running(_))) = self.current.as_mut() {
            let Entry::Running(rx) = entry else { return };
            if let Ok(done) = rx.try_recv() {
                *entry = Entry::Done(done);
            }
        }
    }

    /// The finished detection for `key`, if one is showing.
    pub(crate) fn detection(&self, key: &PartKey) -> Option<&Detection> {
        match self.current.as_ref()? {
            (k, Entry::Done(Ok(d))) if k == key => Some(d),
            _ => None,
        }
    }
}

/// What the block asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PinsAction {
    Detect,
    Save,
}

/// The block under "3D model" in the part detail. `has_step` is whether the
/// part has a stored STEP model.
pub(crate) fn block(
    ui: &mut egui::Ui,
    state: &mut PinsState,
    part: &CatalogPart,
    has_step: bool,
) -> Option<PinsAction> {
    state.poll();
    let key = part.key();
    let mut action = None;
    ui.add_space(8.0);
    ui.strong("Connection points");
    let known = part
        .terminals
        .iter()
        .filter(|t| t.position_mm.is_some())
        .count();
    ui.label(format!(
        "{} of {} terminals have a position. Wires end on them when they do.",
        known,
        part.terminals.len()
    ));
    let running = matches!(&state.current, Some((k, Entry::Running(_))) if *k == key);
    ui.add_enabled_ui(has_step && !part.terminals.is_empty() && !running, |ui| {
        let b = ui.small_button("Detect connection points").on_hover_text(
            "Reads the wire openings and pins from the 3D model and matches them to the terminals",
        );
        if b.clicked() {
            action = Some(PinsAction::Detect);
        }
    });
    if !has_step {
        ui.weak("Import the 3D model (STEP) first.");
    } else if part.terminals.is_empty() {
        ui.weak("The part lists no terminals to place.");
    }
    if running {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Reading the model...");
        });
    }
    match &state.current {
        Some((k, Entry::Done(Err(why)))) if *k == key => {
            ui.colored_label(ui.visuals().warn_fg_color, why);
        }
        Some((k, Entry::Done(Ok(d)))) if *k == key => {
            ui.weak(summary(d));
            egui::Grid::new("parts_pins_grid")
                .num_columns(3)
                .show(ui, |ui| {
                    for r in rows(d) {
                        ui.label(r.name);
                        ui.label(r.position);
                        ui.label(r.how);
                        ui.end_row();
                    }
                });
            ui.checkbox(
                &mut state.with_estimates,
                "Also save estimated points (spread evenly, not read from the model)",
            );
            if ui.small_button("Save to my catalog").clicked() {
                action = Some(PinsAction::Save);
            }
        }
        _ => {}
    }
    if let Some(note) = &state.note {
        ui.weak(note);
    }
    action
}
