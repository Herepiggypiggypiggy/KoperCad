//! The simulation status chip, docked in the status bar: one small button,
//! in the Cabinet and 3D workspaces, that says whether the heat simulation
//! was run, how far it is, whether the result still fits the cabinet, and
//! carries the one action that fits (Run, Cancel, Show heat) on click. It
//! reads [`HeatState::sim_status`], the same value as the Cabinet strip and
//! the 3D Heat panel. It used to float over the Cabinet canvas and sit as the
//! first row of the 3D toolbar; nothing floats any more
//! (docs/design/visual-redesign-3d.md section 4, dock/chip-inventory.md).

use eframe::egui;

use super::heat::HeatState;
use super::heat_status::SimStatus;

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
    /// The label on the status bar: marker and a few words.
    pub short: String,
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
            short: "Heat not run".into(),
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
            short: match (stopping, fraction) {
                (true, _) => "Heat stopping".into(),
                (false, Some(f)) => format!("Heat {:.0}%", f * 100.0),
                (false, None) => "Heat starting".into(),
            },
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
            short: "Heat done".into(),
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
            short: "Heat out of date".into(),
            words: "Heat is out of date".into(),
            detail: "The cabinet changed since the last run".into(),
            tone: Tone::Warn,
            action: Some(("Run heat again", Ask::Run)),
            progress: None,
        },
        SimStatus::Failed(why) => Chip {
            marker: "×",
            short: "Heat failed".into(),
            words: "Heat failed".into(),
            detail: why.clone(),
            tone: Tone::Bad,
            action: Some(("Run heat again", Ask::Run)),
            progress: None,
        },
    }
}

/// The chip as one small button in the status bar, for the status as it is
/// now. What was clicked is returned; [`apply`] does it.
pub(super) fn bar_slot(ui: &mut egui::Ui, status: &SimStatus, heat_visible: bool) -> Option<Ask> {
    let chip = describe(status, heat_visible);
    let pal = crate::theme::ui_palette();
    let colour = match chip.tone {
        Tone::Quiet | Tone::Busy => pal.text_primary,
        Tone::Good => pal.success,
        Tone::Warn => pal.warning,
        Tone::Bad => pal.error,
    };
    let mut hover = format!("{}\n{}", chip.words, chip.detail);
    if let Some((label, _)) = chip.action {
        hover.push_str(&format!("\nClick: {label}"));
    }
    let button = egui::Button::new(
        egui::RichText::new(format!("{} {}", chip.marker, chip.short)).color(colour),
    )
    .fill(pal.control)
    .small();
    let response = ui.add(button).on_hover_text(hover);
    chip.action.filter(|_| response.clicked()).map(|(_, a)| a)
}

/// Does what a click on the chip asked for.
pub(super) fn apply(state: &mut HeatState, ask: Ask) {
    match ask {
        Ask::Run => state.request_run(),
        Ask::Cancel => state.cancel(),
        Ask::ShowHeat => state.on = true,
    }
}

/// The label the slot shows, for the status bar's room count.
pub(super) fn slot_label(status: &SimStatus, heat_visible: bool) -> String {
    let chip = describe(status, heat_visible);
    format!("{} {}", chip.marker, chip.short)
}
