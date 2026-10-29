//! The simulation status chip: one row, in the top left corner of the Cabinet
//! (a card of its own) and as the first row of the 3D toolbar, that says
//! whether the heat simulation was run, how far it is, whether the result
//! still fits the cabinet, and offers the one action that fits (Run, Cancel,
//! Show heat). It reads
//! [`HeatState::sim_status`], the same value as the Cabinet strip and the 3D
//! Heat panel, so it is there with every section folded and the panel closed,
//! and the person never has to go back to the Cabinet to find out
//! (docs/design/visual-redesign-3d.md section 4).

use eframe::egui;

use super::heat::HeatState;
use super::heat_status::SimStatus;
use super::panel::{MARGIN, Panel, Side};

/// Room the Cabinet's chip card takes at the top left, gap included: panels
/// that start at the top of the Cabinet canvas on that side start below it.
pub(super) const TOP_ROOM: f32 = MARGIN + 48.0 + 6.0;

/// Below this canvas width the detail line moves into the tooltip.
const NARROW: f32 = 560.0;

/// What a chip's button asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ask {
    Run,
    Cancel,
    ShowHeat,
}

/// How the chip is coloured: words and a marker always say it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Quiet,
    Busy,
    Good,
    Warn,
    Bad,
}

/// What the chip says for one status.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Chip {
    pub marker: &'static str,
    pub words: String,
    pub detail: String,
    pub tone: Tone,
    /// The one button: its label and what it asks for.
    pub action: Option<(&'static str, Ask)>,
    /// A run goes: the bar, with its fraction (None: still setting up).
    pub progress: Option<Option<f32>>,
}

/// The chip for `status`. `heat_visible` says whether the Heat result is on
/// screen already (the "Show heat" button is then not offered).
pub(super) fn describe(status: &SimStatus, heat_visible: bool) -> Chip {
    match status {
        SimStatus::NotRun => Chip {
            marker: "▶",
            words: "Heat has not been run".into(),
            detail: "Shows temperatures and the hottest device".into(),
            tone: Tone::Quiet,
            action: Some(("Run heat", Ask::Run)),
            progress: None,
        },
        SimStatus::Running {
            fraction,
            label,
            stopping,
        } => Chip {
            marker: "●",
            words: if *stopping {
                "Heat is stopping".into()
            } else {
                "Heat is running".into()
            },
            detail: label.clone(),
            tone: Tone::Busy,
            action: if *stopping {
                None
            } else {
                Some(("Cancel run", Ask::Cancel))
            },
            progress: Some(*fraction),
        },
        SimStatus::Done { label, converged } => Chip {
            marker: "✔",
            words: "Heat is done".into(),
            detail: if *converged {
                label.clone()
            } else {
                format!("{label} (not converged)")
            },
            tone: if *converged { Tone::Good } else { Tone::Warn },
            action: (!heat_visible).then_some(("Show heat", Ask::ShowHeat)),
            progress: None,
        },
        SimStatus::OutOfDate => Chip {
            marker: "!",
            words: "Heat is out of date".into(),
            detail: "The cabinet changed since the last run".into(),
            tone: Tone::Warn,
            action: Some(("Run heat again", Ask::Run)),
            progress: None,
        },
        SimStatus::Failed(why) => Chip {
            marker: "×",
            words: "Heat failed".into(),
            detail: why.clone(),
            tone: Tone::Bad,
            action: Some(("Run heat again", Ask::Run)),
            progress: None,
        },
    }
}

/// One row of the chip: the marker, the words, the bar of a run, the detail
/// and the one button. It is the first row of the 3D toolbar and the content
/// of the Cabinet's chip. The action is applied to `state`; what was asked is
/// returned for a caller with more to do (only `ShowHeat`: showing the result
/// is the view's business, the state just switches heat on).
pub(super) fn row(
    ui: &mut egui::Ui,
    state: &mut HeatState,
    heat_visible: bool,
    reduce_motion: bool,
) -> Option<Ask> {
    let status = state.sim_status();
    let chip = describe(&status, heat_visible);
    let narrow = ui.max_rect().width() < NARROW;
    let pal = crate::theme::ui_palette();
    let colour = match chip.tone {
        Tone::Quiet | Tone::Busy => pal.text_secondary,
        Tone::Good => pal.success,
        Tone::Warn => pal.warning,
        Tone::Bad => pal.error,
    };
    let mut asked = None;
    let whole = ui
        .horizontal(|ui| {
            ui.label(egui::RichText::new(chip.marker).strong().color(colour));
            ui.label(egui::RichText::new(&chip.words).strong());
            if let Some(fraction) = chip.progress {
                let bar = egui::ProgressBar::new(fraction.unwrap_or(0.0)).desired_width(90.0);
                ui.add(
                    if fraction.is_none() || fraction.is_some_and(|f| f < 0.01) {
                        bar.animate(!reduce_motion)
                    } else {
                        bar.show_percentage()
                    },
                );
            }
            if !narrow {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&chip.detail)
                            .small()
                            .color(pal.text_secondary),
                    )
                    .truncate(),
                );
            }
            if let Some((label, ask)) = chip.action
                && ui.button(label).clicked()
            {
                asked = Some(ask);
            }
        })
        .response;
    // The whole chip says it in full on hover, also on a narrow canvas.
    whole.on_hover_text(format!("{}\n{}", chip.words, chip.detail));
    match asked {
        Some(Ask::Run) => state.request_run(),
        Some(Ask::Cancel) => state.cancel(),
        Some(Ask::ShowHeat) => state.on = true,
        None => {}
    }
    asked
}

/// The chip as a card of its own over `view` (the Cabinet canvas): the 3D
/// view has it as the first row of its toolbar instead, so it takes no room
/// from the panels under it.
pub(super) fn show(
    ctx: &egui::Context,
    view: egui::Rect,
    state: &mut HeatState,
    heat_visible: bool,
    reduce_motion: bool,
) -> Option<Ask> {
    let mut asked = None;
    Panel::new(
        "sim_status_chip",
        "Simulation status",
        view,
        Side::Left,
        520.0,
    )
    .bare()
    .fit()
    .top(MARGIN)
    .show(ctx, |ui| {
        asked = row(ui, state, heat_visible, reduce_motion);
    });
    asked
}
