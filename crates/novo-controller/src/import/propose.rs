//! Turning table rows into a proposed controller profile and binding table, with a warning
//! for every row the guess is not sure about. Nothing is written here: the caller shows the
//! [`Proposal`] first.

use std::collections::BTreeMap;

use super::read::{Reading, Voltage, read_row};
use super::table::{Row, Table};
use super::words::{Dir, Role, WordList};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A guess worth knowing about.
    Info,
    /// A row that is ambiguous, skipped or probably wrong.
    Warning,
}

/// Something the person should look at before accepting the proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Line in the pasted text, 0 when the warning is about the table as a whole.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

/// One control of the proposed profile (a rocker, a button, the E-stop...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub id: String,
    pub label: String,
    /// `rocker2`, `rocker3`, `button`, `latch` or `estop`.
    pub kind: String,
    /// Written for rockers; the other kinds use the positions of their kind.
    pub positions: Vec<String>,
    /// Receiver-side control that drops when the radio link is lost (main/start).
    pub needs_link: bool,
    pub requires: Vec<String>,
    pub drops_when: Vec<String>,
}

impl Control {
    /// The positions the preview lists, with the defaults of the kind filled in.
    pub fn position_text(&self) -> String {
        if !self.positions.is_empty() {
            return self.positions.join(" / ");
        }
        match self.kind.as_str() {
            "estop" => "released / pressed",
            _ => "off / on",
        }
        .to_owned()
    }
}

/// A named output and the control positions that make it high.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub name: String,
    pub control: String,
    /// Positions that make it high (buttons, latches, the E-stop chain).
    pub when: Vec<String>,
    /// Rocker group and the contact number it closes; with `step2_includes_step1` of the
    /// profile this decides which positions make it high.
    pub group: Option<String>,
    pub step: Option<u8>,
    /// Not subject to the controller gate (main contact, E-stop chain, stop).
    pub ungated: bool,
    /// Terminal label of the table row this signal came from.
    pub pin: String,
}

/// Which pin carries which signal in this drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub signal: String,
    pub pin: String,
    /// What the pin is wired to, when the table came from a device in a drawing.
    pub target: String,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supply {
    pub ac: bool,
    pub volts: f64,
}

/// The profile the wizard proposes, in the structure of the controller profile file.
#[derive(Debug, Clone, PartialEq)]
pub struct ProposedProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub supply: Option<Supply>,
    pub common: Option<String>,
    /// A generic stand-in, not a specific product (the library sets it).
    pub generic: bool,
    /// A higher step keeps the lower contacts closed.
    pub step2_includes_step1: bool,
    /// Conditions every signal needs unless it is `ungated` (`start=on`).
    pub gate: Vec<String>,
    pub controls: Vec<Control>,
    pub signals: Vec<Signal>,
}

/// What became of one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The row became this signal.
    Signal(String),
    /// The row is the common pin.
    Common,
    /// The row was left out, with the reason.
    Skipped(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowOutcome {
    pub row: Row,
    pub outcome: Outcome,
}

/// Everything the wizard preview shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub profile: ProposedProfile,
    pub bindings: Vec<Binding>,
    pub rows: Vec<RowOutcome>,
    pub warnings: Vec<Warning>,
    /// How the table was read (delimiter, columns).
    pub notes: Vec<String>,
}

/// Choices the person can change in the wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub id: String,
    pub name: String,
    /// Step 2 keeps the step 1 contact closed, as on a real two-step switch.
    pub step2_includes_step1: bool,
    /// When the table has a main/start row, motions only work while it is on.
    pub gate_on_main: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            id: "imported-controller".to_owned(),
            name: "Imported controller".to_owned(),
            step2_includes_step1: true,
            gate_on_main: true,
        }
    }
}

impl Proposal {
    pub fn warning_count(&self) -> usize {
        self.warnings
            .iter()
            .filter(|w| w.severity == Severity::Warning)
            .count()
    }
}

/// A profile id and file name: lower-case letters, digits and hyphens. Never empty (`controller`
/// when the text has no letter or digit) and never a name Windows reserves for a device.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    let numbered = |prefix: &str| {
        out.strip_prefix(prefix)
            .is_some_and(|n| n.len() == 1 && n != "0" && n.chars().all(|c| c.is_ascii_digit()))
    };
    let reserved =
        matches!(out.as_str(), "con" | "prn" | "aux" | "nul") || numbered("com") || numbered("lpt");
    if out.is_empty() {
        "controller".to_owned()
    } else if reserved {
        format!("controller-{out}")
    } else {
        out
    }
}

fn upper_ident(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn capitalise(text: &str) -> String {
    let mut c = text.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// One row that makes a motion step signal.
struct MotionRow {
    row_index: usize,
    dir: Dir,
    step: u8,
}

/// Reads `table` with `words` and proposes a profile, bindings and warnings.
pub fn propose(table: &Table, words: &WordList, options: &Options) -> Proposal {
    let readings: Vec<Reading> = table.rows.iter().map(|r| read_row(r, words)).collect();
    let mut warnings: Vec<Warning> = Vec::new();
    let warn = |warnings: &mut Vec<Warning>, line, severity, message: String| {
        warnings.push(Warning {
            line,
            severity,
            message,
        });
    };
    let mut outcomes: Vec<Option<Outcome>> = vec![None; table.rows.len()];
    let skip = |i: usize, outcomes: &mut Vec<Option<Outcome>>, why: &str| {
        outcomes[i] = Some(Outcome::Skipped(why.to_owned()));
    };

    // Motions named anywhere in the table: a row without one can borrow the only one.
    let mut named: Vec<String> = Vec::new();
    for r in &readings {
        for m in &r.motions {
            if !named.contains(m) {
                named.push(m.clone());
            }
        }
    }
    let sole_motion = (named.len() == 1).then(|| named[0].clone());

    let mut motion_rows: BTreeMap<(String, Dir), Vec<MotionRow>> = BTreeMap::new();
    let mut motion_order: Vec<String> = Vec::new();
    let mut role_rows: Vec<(usize, Role)> = Vec::new();
    let mut common: Option<String> = None;
    let mut supply: Option<Voltage> = None;
    let mut kind: Option<String> = None;

    for (i, (row, r)) in table.rows.iter().zip(&readings).enumerate() {
        let line = row.line;
        if let Some(v) = r.voltage {
            match supply {
                None => supply = Some(v),
                Some(s) if (s.volts - v.volts).abs() > f64::EPSILON => warn(
                    &mut warnings,
                    line,
                    Severity::Warning,
                    format!(
                        "Another voltage ({} V) after {} V: the first was kept",
                        v.volts, s.volts
                    ),
                ),
                _ => {}
            }
        }
        if kind.is_none() {
            kind.clone_from(&r.kind);
        }
        if row.terminal.trim().is_empty() && row.function.trim().is_empty() {
            skip(i, &mut outcomes, "empty row");
            continue;
        }
        if r.bad_step_cell {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                format!(
                    "Step column “{}” is not a step number or a slow/fast word",
                    row.step.trim()
                ),
            );
        }
        if r.step_conflict {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                "The step column and the function text name different steps: the column was used"
                    .to_owned(),
            );
        }
        // A row naming both directions of a pair is a combined label (hoist/lower), not a pin.
        if r.dirs.len() > 1 && r.dirs.iter().any(|d| r.dirs.contains(&d.opposite())) {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                format!(
                    "“{}” names both directions: skipped. Put one direction per row",
                    row.function.trim()
                ),
            );
            skip(i, &mut outcomes, "names both directions");
            continue;
        }
        if r.dirs.len() > 1 {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                format!(
                    "“{}” names several directions: skipped",
                    row.function.trim()
                ),
            );
            skip(i, &mut outcomes, "names several directions");
            continue;
        }
        if r.motions.len() > 1 {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                format!(
                    "“{}” names several motions ({}): skipped",
                    row.function.trim(),
                    r.motions.join(", ")
                ),
            );
            skip(i, &mut outcomes, "names several motions");
            continue;
        }
        if let Some(dir) = r.dirs.first().copied() {
            let motion = match r.motions.first() {
                Some(m) => Some(m.clone()),
                None if named.is_empty() => {
                    warn(
                        &mut warnings,
                        line,
                        Severity::Info,
                        "No motion word in the table: filed under “motion”".to_owned(),
                    );
                    Some("motion".to_owned())
                }
                None => sole_motion.clone().inspect(|m| {
                    warn(
                        &mut warnings,
                        line,
                        Severity::Info,
                        format!("No motion word: assumed “{m}”, the only motion in the table"),
                    );
                }),
            };
            let Some(motion) = motion else {
                warn(
                    &mut warnings,
                    line,
                    Severity::Warning,
                    format!(
                        "“{}” has a direction but no motion and the table names several: skipped",
                        row.function.trim()
                    ),
                );
                skip(i, &mut outcomes, "motion unclear");
                continue;
            };
            let step = r.step.unwrap_or_else(|| {
                warn(
                    &mut warnings,
                    line,
                    Severity::Info,
                    "No step given: read as step 1".to_owned(),
                );
                1
            });
            if step > 3 {
                warn(
                    &mut warnings,
                    line,
                    Severity::Warning,
                    format!(
                        "Step {step}: the profile format has rockers with up to 3 steps, so “{}” is left out",
                        row.function.trim()
                    ),
                );
                skip(i, &mut outcomes, "more than 3 steps");
                continue;
            }
            if let Some(n) = &r.loose_number {
                warn(
                    &mut warnings,
                    line,
                    Severity::Warning,
                    format!(
                        "The number {n} in “{}” could be a step or a unit number: not used as a step",
                        row.function.trim()
                    ),
                );
            }
            if !motion_order.contains(&motion) {
                motion_order.push(motion.clone());
            }
            motion_rows
                .entry((motion, dir))
                .or_default()
                .push(MotionRow {
                    row_index: i,
                    dir,
                    step,
                });
            continue;
        }
        if let Some(role) = r.role {
            if role == Role::Common {
                match &common {
                    None => {
                        let label = if row.terminal.trim().is_empty() {
                            row.function.trim()
                        } else {
                            row.terminal.trim()
                        };
                        common = Some(label.to_owned());
                        outcomes[i] = Some(Outcome::Common);
                    }
                    Some(c) => {
                        warn(
                            &mut warnings,
                            line,
                            Severity::Warning,
                            format!("A second common pin; “{c}” was kept"),
                        );
                        skip(i, &mut outcomes, "second common");
                    }
                }
            } else if role == Role::Selector {
                warn(
                    &mut warnings,
                    line,
                    Severity::Warning,
                    format!(
                        "“{}” looks like a selector switch. Selectors are not guessed: add it in the profile editor",
                        row.function.trim()
                    ),
                );
                skip(i, &mut outcomes, "selector");
            } else {
                role_rows.push((i, role));
            }
            continue;
        }
        if !r.motions.is_empty() {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                format!(
                    "“{}” names a motion but no direction: skipped",
                    row.function.trim()
                ),
            );
            skip(i, &mut outcomes, "no direction");
        } else {
            warn(
                &mut warnings,
                line,
                Severity::Warning,
                format!(
                    "“{}” was not recognised: skipped. Add its words to a word list or write the control by hand",
                    row.function.trim()
                ),
            );
            skip(i, &mut outcomes, "not recognised");
        }
    }

    let mut controls: Vec<Control> = Vec::new();
    let mut signals: Vec<Signal> = Vec::new();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut add_binding = |row: &Row, signal: &str, warnings: &mut Vec<Warning>| {
        if row.terminal.trim().is_empty() {
            warnings.push(Warning {
                line: row.line,
                severity: Severity::Warning,
                message: format!("Signal {signal} has no terminal label, so it cannot be bound"),
            });
        }
        bindings.push(Binding {
            signal: signal.to_owned(),
            pin: row.terminal.trim().to_owned(),
            target: row.target.clone(),
            line: row.line,
        });
    };

    // Motions: one control per motion and direction pair.
    for motion in &motion_order {
        let pairs: Vec<Dir> = {
            let mut p: Vec<Dir> = motion_rows
                .keys()
                .filter(|(m, _)| m == motion)
                .map(|(_, d)| d.pair())
                .collect();
            p.dedup();
            p.sort();
            p.dedup();
            p
        };
        for pair in &pairs {
            let id = if pairs.len() > 1 {
                format!("{motion}-{}", pair.word())
            } else {
                motion.clone()
            };
            let mut positions = vec!["off".to_owned()];
            let mut dir_steps: Vec<(Dir, u8)> = Vec::new();
            for dir in [*pair, pair.opposite()] {
                if let Some(rows) = motion_rows.get(&(motion.clone(), dir)) {
                    let n = rows.iter().map(|m| m.step).max().unwrap_or(1);
                    dir_steps.push((dir, n));
                    positions.extend((1..=n).map(|s| format!("{}{s}", dir.word())));
                }
            }
            let top = dir_steps.iter().map(|d| d.1).max().unwrap_or(1);
            if dir_steps.len() == 1 {
                let (d, _) = dir_steps[0];
                let first = table.rows[motion_rows[&(motion.clone(), d)][0].row_index].line;
                warn(
                    &mut warnings,
                    first,
                    Severity::Warning,
                    format!(
                        "{} has {} but no {} row: one direction only",
                        capitalise(motion),
                        d.word(),
                        d.opposite().word()
                    ),
                );
            }
            if dir_steps.len() == 2 && dir_steps[0].1 != dir_steps[1].1 {
                let first =
                    table.rows[motion_rows[&(motion.clone(), dir_steps[0].0)][0].row_index].line;
                warn(
                    &mut warnings,
                    first,
                    Severity::Warning,
                    format!(
                        "{} has {} steps {} but {} steps {}",
                        capitalise(motion),
                        dir_steps[0].1,
                        dir_steps[0].0.word(),
                        dir_steps[1].1,
                        dir_steps[1].0.word()
                    ),
                );
            }
            let kind = match (dir_steps.len(), top) {
                (1, 1) => "button",
                (_, n) if n <= 2 => "rocker2",
                _ => "rocker3",
            };
            for (dir, n) in &dir_steps {
                let rows = &motion_rows[&(motion.clone(), *dir)];
                let mut seen: Vec<u8> = Vec::new();
                for m in rows {
                    let s = m.step;
                    let dup = seen.iter().filter(|x| **x == s).count();
                    seen.push(s);
                    let row = &table.rows[m.row_index];
                    let step_word = if top == 2 {
                        if s == 1 {
                            "SLOW".to_owned()
                        } else {
                            "FAST".to_owned()
                        }
                    } else {
                        s.to_string()
                    };
                    let mut name = format!("{}_{}_{step_word}", upper_ident(motion), m.dir.code());
                    if dup > 0 {
                        name = format!("{name}_{}", dup + 1);
                        warn(
                            &mut warnings,
                            row.line,
                            Severity::Warning,
                            format!(
                                "Another pin for {} {} step {s}: kept as its own signal {name}",
                                motion,
                                dir.word()
                            ),
                        );
                    }
                    outcomes[m.row_index] = Some(Outcome::Signal(name.clone()));
                    add_binding(row, &name, &mut warnings);
                    signals.push(Signal {
                        name,
                        control: id.clone(),
                        when: if kind == "button" {
                            vec!["on".to_owned()]
                        } else {
                            Vec::new()
                        },
                        group: (kind != "button").then(|| dir.word().to_owned()),
                        step: (kind != "button").then_some(s),
                        ungated: false,
                        pin: row.terminal.trim().to_owned(),
                    });
                }
                for s in 1..*n {
                    if !rows.iter().any(|m| m.step == s) {
                        warn(
                            &mut warnings,
                            0,
                            Severity::Warning,
                            format!(
                                "{} {} has step {n} but no step {s} pin",
                                capitalise(motion),
                                dir.word()
                            ),
                        );
                    }
                }
            }
            controls.push(Control {
                id,
                label: capitalise(motion),
                kind: kind.to_owned(),
                positions: if kind == "button" {
                    Vec::new()
                } else {
                    positions
                },
                needs_link: false,
                requires: Vec::new(),
                drops_when: Vec::new(),
            });
        }
    }

    let kind_guess = kind.clone();
    // Roles: E-stop, main, stop, horn, light. Several rows of one role are parallel contacts.
    for role in [Role::Estop, Role::Main, Role::Stop, Role::Horn, Role::Light] {
        let rows: Vec<usize> = role_rows
            .iter()
            .filter(|(_, r)| *r == role)
            .map(|(i, _)| *i)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let (id, label, kind, base, high, ungated) = match role {
            Role::Estop => (
                "estop",
                "Emergency stop",
                "estop",
                "ESTOP_OK",
                "released",
                true,
            ),
            Role::Main => ("start", "Main / start", "latch", "MAIN", "on", true),
            Role::Stop => ("stop", "Stop", "button", "STOP", "on", true),
            Role::Horn => ("horn", "Horn", "button", "HORN", "on", false),
            _ => ("light", "Light", "latch", "LIGHT", "on", false),
        };
        let has = |r: Role| role_rows.iter().any(|(_, x)| *x == r);
        let receiver = role == Role::Main;
        controls.push(Control {
            id: id.to_owned(),
            label: label.to_owned(),
            kind: kind.to_owned(),
            positions: Vec::new(),
            needs_link: receiver && kind_guess.as_deref() == Some("radio"),
            requires: if receiver && has(Role::Estop) {
                vec!["estop=released".to_owned()]
            } else {
                Vec::new()
            },
            drops_when: if receiver && has(Role::Stop) {
                vec!["stop=on".to_owned()]
            } else {
                Vec::new()
            },
        });
        for (n, i) in rows.iter().enumerate() {
            let name = if n == 0 {
                base.to_owned()
            } else {
                format!("{base}_{}", n + 1)
            };
            outcomes[*i] = Some(Outcome::Signal(name.clone()));
            add_binding(&table.rows[*i], &name, &mut warnings);
            signals.push(Signal {
                name,
                control: id.to_owned(),
                when: vec![high.to_owned()],
                group: None,
                step: None,
                ungated,
                pin: table.rows[*i].terminal.trim().to_owned(),
            });
        }
        match role {
            Role::Estop => warn(&mut warnings, table.rows[rows[0]].line, Severity::Info, "E-stop modelled as a normally closed chain: the signal is high while the mushroom is out. Confirm against the page".to_owned()),
            Role::Main if has(Role::Stop) => warn(&mut warnings, table.rows[rows[0]].line, Severity::Info, "Main/start is a latch that the stop button drops. Confirm against the page".to_owned()),
            _ => {}
        }
    }

    // Whole-table checks.
    if common.is_none() {
        warn(
            &mut warnings,
            0,
            Severity::Info,
            "No common (COM) row found: the common pin of the dry contacts is unknown".to_owned(),
        );
    }
    let mut pins: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for b in &bindings {
        if !b.pin.is_empty() {
            pins.entry(b.pin.as_str())
                .or_default()
                .push(b.signal.as_str());
        }
    }
    for (pin, sigs) in &pins {
        if sigs.len() > 1 {
            warn(
                &mut warnings,
                0,
                Severity::Warning,
                format!("Terminal {pin} is used for {}", sigs.join(" and ")),
            );
        }
    }
    if controls.is_empty() {
        warn(
            &mut warnings,
            0,
            Severity::Warning,
            "No control could be made from this table".to_owned(),
        );
    }
    let supply_out = supply.and_then(|v| match v.ac {
        Some(ac) => Some(Supply { ac, volts: v.volts }),
        None => {
            warn(&mut warnings, 0, Severity::Warning, format!("A supply of {} V is named but not whether it is AC or DC: set it in the profile", v.volts));
            None
        }
    });
    warnings.sort_by_key(|w| (w.line == 0, w.line));

    let gate = if options.gate_on_main && controls.iter().any(|c| c.id == "start") {
        warn(
            &mut warnings,
            0,
            Severity::Info,
            "Motions are gated on main/start: they only work after the start latch is on"
                .to_owned(),
        );
        vec!["start=on".to_owned()]
    } else {
        Vec::new()
    };
    let kind = kind.unwrap_or_else(|| {
        warn(
            &mut warnings,
            0,
            Severity::Info,
            "Controller kind not stated in the table: “custom”".to_owned(),
        );
        "custom".to_owned()
    });
    let rows = table
        .rows
        .iter()
        .cloned()
        .zip(outcomes)
        .map(|(row, outcome)| RowOutcome {
            row,
            outcome: outcome.unwrap_or_else(|| Outcome::Skipped("not used".to_owned())),
        })
        .collect();
    Proposal {
        profile: ProposedProfile {
            id: options.id.clone(),
            name: options.name.clone(),
            kind,
            supply: supply_out,
            common,
            generic: false,
            step2_includes_step1: options.step2_includes_step1,
            gate,
            controls,
            signals,
        },
        bindings,
        rows,
        warnings,
        notes: table.notes.clone(),
    }
}
