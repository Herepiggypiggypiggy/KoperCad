//! Problem triangles for the simulation windows (lane T10 of
//! docs/design/problem-triangles.md): the notes of a drive, hoist or winch
//! run, the motor start result and the controller bindings become flags of
//! source `Sim`, anchored on the device that causes them or on the run.
//!
//! The adapters are pure (text and ids in, [`FlagDraft`]s out) so they can be
//! tested without a window; [`strip`] draws a list of flags as triangles with
//! a short title, and a click opens the shared popover (`flags.rs`).

use eframe::egui;
use novo_flags::{Action, Anchor, Detail, Flag, FlagDraft, FlagId, Life, Severity, Source};
use novo_sim::pins::PinProblem;

use super::{Level, NovoApp};

/// The windows that raise flags. The area is the run key of the flag and part
/// of its id, so the same device left out of two runs is two flags.
pub(super) const DRIVE: &str = "drive_run";
pub(super) const HOIST: &str = "hoist_run";
pub(super) const WINCH: &str = "winch";
pub(super) const MOTOR_START: &str = "motor_start";
pub(super) const CONTROLLER: &str = "controller";

/// How many triangles a strip shows before it folds the rest into "N more".
const STRIP_ROWS: usize = 5;

/// A short stable hash of a text, so two different reasons for the same
/// device are two flags.
fn tag_of(text: &str) -> String {
    format!("{:08x}", FlagId::of([text]).0 & 0xffff_ffff)
}

fn severity_of(level: Level) -> Option<Severity> {
    match level {
        Level::Error => Some(Severity::Red),
        Level::Warning => Some(Severity::Yellow),
        Level::Info => None,
    }
}

/// What a note of a run says, by its shape: `(rule, title, subject tag, detail)`.
/// The run builds its notes as "{tag} is left out: {why}", "{tag}: {finding}"
/// and a few plain sentences; the shape is the only thing they share.
fn classify(text: &str, level: Level) -> (&'static str, &'static str, Option<&str>) {
    if let Some((tag, _)) = text.split_once(" is left out: ") {
        return ("left_out", "Left out of the run", Some(tag.trim()));
    }
    if text.starts_with("No axis can run") || text.starts_with("No hoist can run") {
        return ("nothing_runs", "Nothing can run", None);
    }
    if let Some((tag, _)) = text.split_once(": ") {
        let tag = tag.trim();
        // A device tag has no spaces; a sentence with a colon does.
        if !tag.is_empty() && !tag.contains(' ') {
            return if level == Level::Error {
                ("run_problem", "Run problem", Some(tag))
            } else {
                ("binding", "Binding problem", Some(tag))
            };
        }
    }
    if level == Level::Error {
        ("cannot_start", "Run cannot start", None)
    } else {
        ("run_note", "Run note", None)
    }
}

/// The flags of a window's notes. `device_of` maps a tag text to the raw id of
/// the device with that tag, when the drawing has one.
pub(super) fn note_drafts(
    area: &str,
    notes: &[(Level, String)],
    device_of: &dyn Fn(&str) -> Option<String>,
) -> Vec<FlagDraft> {
    notes
        .iter()
        .filter_map(|(level, text)| {
            let severity = severity_of(*level)?;
            let (rule, title, tag) = classify(text, *level);
            let anchor = tag
                .and_then(device_of)
                .map_or_else(|| Anchor::Job(area.to_owned()), Anchor::Device);
            let mut actions = Vec::new();
            if matches!(anchor, Anchor::Device(_)) {
                actions.push(Action::Locate);
            }
            actions.push(Action::Copy);
            Some(FlagDraft {
                severity,
                unchecked: false,
                source: Source::Sim,
                rule: format!("{area}.{rule}.{}", tag_of(text)),
                anchors: vec![anchor],
                title: title.to_owned(),
                detail: Detail::new([text.clone()]),
                actions,
                waiver_key: None,
                life: Life::Run(area.to_owned()),
            })
        })
        .collect()
}

/// The flag of one finding of a scenario trace (generic hardware, so it sits
/// on the run, not on a device).
pub(super) fn finding_draft(area: &str, red: bool, text: &str) -> FlagDraft {
    FlagDraft {
        severity: if red { Severity::Red } else { Severity::Yellow },
        unchecked: false,
        source: Source::Sim,
        rule: format!("{area}.finding.{}", tag_of(text)),
        anchors: vec![Anchor::Job(area.to_owned())],
        title: if red {
            "Scenario problem"
        } else {
            "Scenario warning"
        }
        .to_owned(),
        detail: Detail::new([text.to_owned()]),
        actions: vec![Action::Copy],
        waiver_key: None,
        life: Life::Run(area.to_owned()),
    }
}

/// The flag of a failed run (the engine refused its numbers).
pub(super) fn failure_draft(area: &str, why: &str) -> FlagDraft {
    FlagDraft {
        severity: Severity::Red,
        unchecked: false,
        source: Source::Sim,
        rule: format!("{area}.failed"),
        anchors: vec![Anchor::Job(area.to_owned())],
        title: "Run failed".to_owned(),
        detail: Detail::new([why.to_owned()]),
        actions: vec![Action::Copy],
        waiver_key: None,
        life: Life::Run(area.to_owned()),
    }
}

/// Title and rule of a pin problem (novo-sim): what is wrong with the name a
/// signal is bound to.
fn pin_words(p: &PinProblem) -> (&'static str, &'static str) {
    match p {
        PinProblem::NotAPin(_) => ("not_a_pin", "Not a pin name"),
        PinProblem::NoDevice(_) => ("no_device", "No such device"),
        PinProblem::DuplicateDevice(_) => ("duplicate_tag", "Tag used twice"),
        PinProblem::NoPin { .. } => ("no_pin", "Device has no such pin"),
        PinProblem::NotSwitched { .. } => ("not_switched", "Pin is not a contact"),
        PinProblem::Shared { .. } => ("shared_pin", "Pin shared by contacts"),
        PinProblem::NoPoles(_) => ("no_poles", "Device has no contacts"),
    }
}

/// The device tag text a pin problem is about.
fn pin_device(p: &PinProblem) -> Option<&str> {
    match p {
        PinProblem::NoPin { device, .. }
        | PinProblem::NotSwitched { device, .. }
        | PinProblem::Shared { device, .. } => Some(device),
        PinProblem::NoPoles(t) | PinProblem::DuplicateDevice(t) => Some(t),
        PinProblem::NotAPin(_) | PinProblem::NoDevice(_) => None,
    }
}

/// The flag of a controller signal whose binding does not lead to a contact:
/// red on the signal row, and on the device when the drawing has it.
pub(super) fn pin_problem_draft(
    controller: &str,
    signal: &str,
    problem: &PinProblem,
    device_of: &dyn Fn(&str) -> Option<String>,
) -> FlagDraft {
    let (rule, title) = pin_words(problem);
    let mut anchors = vec![Anchor::Control(signal_key(controller, signal))];
    if let Some(device) = pin_device(problem).and_then(device_of) {
        anchors.push(Anchor::Device(device));
    }
    let locate = anchors.len() > 1;
    let mut actions = Vec::new();
    if locate {
        actions.push(Action::Locate);
    }
    actions.push(Action::Copy);
    FlagDraft {
        severity: Severity::Red,
        unchecked: false,
        source: Source::Sim,
        rule: format!("{CONTROLLER}.{rule}"),
        anchors,
        title: title.to_owned(),
        detail: Detail::new([format!("{signal}: {problem}")]),
        actions,
        waiver_key: None,
        life: Life::Run(CONTROLLER.to_owned()),
    }
}

/// The flag of a binding that is broken for a reason the pin engine does not
/// name (a PLC address, a machine port).
pub(super) fn binding_draft(controller: &str, signal: &str, why: &str) -> FlagDraft {
    FlagDraft {
        severity: Severity::Red,
        unchecked: false,
        source: Source::Sim,
        rule: format!("{CONTROLLER}.broken"),
        anchors: vec![Anchor::Control(signal_key(controller, signal))],
        title: "Binding does not resolve".to_owned(),
        detail: Detail::new([format!("{signal}: {why}")]),
        actions: vec![Action::Copy],
        waiver_key: None,
        life: Life::Run(CONTROLLER.to_owned()),
    }
}

/// The flag of a broken binding: typed by the pin engine when the target is a
/// pin or a hand device, else the engine's own sentence.
pub(super) fn broken_binding_draft(
    project: &novo_core::Project,
    controller: &str,
    signal: &str,
    target: &novo_controller::Target,
    why: &str,
) -> FlagDraft {
    use novo_controller::Target;
    use novo_sim::pins::{pole_of_pin, poles_of_device};
    let typed = match target {
        Target::Pin(d) => pole_of_pin(project, d).err(),
        Target::Symbol(t) => poles_of_device(project, t).err(),
        _ => None,
    };
    match typed {
        Some(p) => pin_problem_draft(controller, signal, &p, &|t| device_raw(project, t)),
        None => binding_draft(controller, signal, why),
    }
}

/// The control key of a signal row.
pub(super) fn signal_key(controller: &str, signal: &str) -> String {
    format!("signal:{controller}:{signal}")
}

/// The flag of the motor start result: the motor does not start (red) or is
/// not up to speed in the time simulated (yellow), or cannot be modelled.
pub(super) fn motor_start_draft(
    tag: &str,
    device: Option<String>,
    stalled: bool,
    slow_s: Option<f64>,
) -> Option<FlagDraft> {
    let (severity, rule, title, detail) = if stalled {
        (
            Severity::Red,
            "stalled",
            "Motor does not start",
            "The load needs more torque than the motor makes at standstill.".to_owned(),
        )
    } else {
        let s = slow_s?;
        (
            Severity::Yellow,
            "slow",
            "Not up to speed",
            format!("Not up to speed within {s:.0} s."),
        )
    };
    let mut anchors = vec![Anchor::Job(MOTOR_START.to_owned())];
    let mut actions = vec![Action::Copy];
    if let Some(d) = device {
        anchors = vec![Anchor::Device(d)];
        actions.insert(0, Action::Locate);
    }
    Some(FlagDraft {
        severity,
        unchecked: false,
        source: Source::Sim,
        rule: format!("{MOTOR_START}.{rule}.{}", tag_of(tag)),
        anchors,
        title: title.to_owned(),
        detail: Detail::new([format!("{tag}: {}", detail)]),
        actions,
        waiver_key: None,
        life: Life::Run(MOTOR_START.to_owned()),
    })
}

/// A motor that has no power entered cannot be modelled at all: yellow and
/// dashed, "could not be judged".
pub(super) fn motor_unmodelled_draft(
    tag: &str,
    device: Option<String>,
    why: Option<&str>,
) -> FlagDraft {
    let mut d = motor_start_draft(tag, device, true, None).unwrap_or_else(|| unreachable!());
    d.severity = Severity::Yellow;
    d.unchecked = true;
    d.rule = format!("{MOTOR_START}.unmodelled.{}", tag_of(tag));
    d.title = "Motor cannot be modelled".to_owned();
    d.detail = Detail::new([
        format!(
            "{tag}: {}",
            why.unwrap_or("this motor has no power entered.")
        ),
        "Give the motor a power in kW on its device.".to_owned(),
    ]);
    d
}

/// Draws `flags` as rows of a triangle and a short title. A click on a row
/// is returned with the spot to open the popover at; hovering reads the
/// first detail line. Rows past [`STRIP_ROWS`] fold into one "N more" line
/// the person can open.
pub(super) fn strip(ui: &mut egui::Ui, flags: &[Flag]) -> Option<(FlagId, egui::Pos2)> {
    let palette = novo_theme::Palette::of(ui);
    let fold = egui::Id::new(("sim_flag_strip", ui.id()));
    let all = ui.data(|d| d.get_temp::<bool>(fold)).unwrap_or(false);
    let shown = if all || flags.len() <= STRIP_ROWS + 1 {
        flags.len()
    } else {
        STRIP_ROWS
    };
    let mut clicked = None;
    for flag in &flags[..shown] {
        let response = ui
            .horizontal(|ui| {
                let (slot, tri) =
                    ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                novo_theme::widgets::flag_triangle(
                    ui.painter(),
                    slot.center(),
                    15.0,
                    flag.severity == Severity::Red,
                    flag.unchecked,
                    palette,
                );
                let label = ui.add(egui::Label::new(&flag.title).sense(egui::Sense::click()));
                tri.union(label)
            })
            .inner;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, flag.accessible_name())
        });
        let response = match flag.detail.0.first() {
            Some(line) => response.on_hover_text(line),
            None => response,
        };
        if response.clicked() {
            clicked = Some((flag.id, response.rect.left_bottom()));
        }
    }
    if shown < flags.len() {
        let more = format!("{} more", flags.len() - shown);
        if ui.link(more).clicked() {
            ui.data_mut(|d| d.insert_temp(fold, true));
        }
    }
    clicked
}

/// The raw device id for a tag text, when the drawing has exactly one.
pub(super) fn device_raw(project: &novo_core::Project, tag: &str) -> Option<String> {
    let tag = novo_core::DeviceTag::parse(tag.trim()).ok()?;
    match project.devices_by_tag(&tag).as_slice() {
        [one] => Some(one.raw().to_string()),
        _ => None,
    }
}

/// Replaces the flags of one window with `drafts` and rebuilds the `Sim`
/// source from all windows. Cheap: a few drafts.
pub(super) fn set_flags(
    flags: &mut super::flags::FlagUi,
    area: &'static str,
    drafts: Vec<FlagDraft>,
) {
    if flags.sim.get(area) == Some(&drafts) || (drafts.is_empty() && !flags.sim.contains_key(area))
    {
        return;
    }
    flags.sim.insert(area, drafts);
    let all: Vec<FlagDraft> = flags.sim.values().flatten().cloned().collect();
    flags.store.replace_source(&Source::Sim, all);
}

/// A window's notes as flags (the notes stay the window's own list).
pub(super) fn set_notes(
    flags: &mut super::flags::FlagUi,
    project: &novo_core::Project,
    area: &'static str,
    notes: Vec<(Level, String)>,
) -> Vec<(Level, String)> {
    let drafts = note_drafts(area, &notes, &|tag| device_raw(project, tag));
    set_flags(flags, area, drafts);
    notes
}

impl NovoApp {
    /// A window's notes as flags, for a place that holds no project borrow.
    pub(super) fn set_sim_notes(
        &mut self,
        area: &'static str,
        notes: Vec<(Level, String)>,
    ) -> Vec<(Level, String)> {
        match self.backend.document() {
            Some(doc) => set_notes(&mut self.wb.flags, doc.project(), area, notes),
            None => notes,
        }
    }

    /// The flags of one window, worst first.
    pub(super) fn sim_flags_of(&self, area: &str) -> Vec<Flag> {
        let mut out: Vec<Flag> = self
            .wb
            .flags
            .store
            .list(false)
            .into_iter()
            .filter(|f| f.source == Source::Sim && f.life == Life::Run(area.to_owned()))
            .cloned()
            .collect();
        out.sort_by_key(|f| f.severity.rank());
        out
    }

    /// Locate for a flag that is not a design check: the first device anchor,
    /// on its main page.
    pub(super) fn locate_flag(&mut self, flag: &Flag) {
        let device = flag.anchors.iter().find_map(|a| match a {
            Anchor::Device(raw) => raw.parse::<u64>().ok(),
            _ => None,
        });
        let Some(device) = device else {
            return;
        };
        match self.device_main(novo_core::DeviceId(device)) {
            Some(place) => self.jump(place, true),
            None => self.message(Level::Info, "That device is not drawn on a page"),
        }
    }
}
