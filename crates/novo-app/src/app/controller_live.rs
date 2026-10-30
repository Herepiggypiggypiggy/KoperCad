//! The virtual controller in the live view (lane C2): a radio, pendant or joystick whose signals
//! close the dry contacts of a receiver drawn in the schematic, so the circuit logic reacts as on
//! the real crane. Plan: `plans/controller-live-view.md`; engine: `novo-controller`; design:
//! `docs/design/controller-live.md`.
//!
//! This file is the whole non-UI side and the only place that knows the controller feature:
//! the instances and bindings, their storage in the project, how a binding target leads to a
//! drawn pole, and the per-frame step that turns signals into poles. The live view calls
//! [`ControllerLive::drive`] before it solves (everything that solves, lessons and tests too),
//! and the card ([`super::controller_card`]) edits the state. Nothing here is a model change:
//! the data lives in the project's add-on data block (`controllers` / `project`), so it saves,
//! copies and undoes with the project and stays in the file when the add-on is not installed.
//! Projects written before the block existed hold it in the property `controllers.project`;
//! that is read as a fallback and replaced by the block at the next write.
//!
//! What a signal does to the drawing, by kind of target:
//! - a pin of a device (`-A1:3`): the switched pole that pin belongs to closes while the signal
//!   is high (a receiver's dry contacts, or any contact symbol that no coil drives);
//! - a hand device (`-S1`): its poles go to the opposite of their rest state while the signal is
//!   high, as when the button is held (an inverted binding gives the E-stop chain this way);
//! - a PLC input (`%I0.1`): the bit is held for the running PLC program;
//! - a machine port (`hoist/=A+B-M1@up`): the level goes to the machine loop of a running hoist or
//!   drive run as a command (lane L5b, [`super::controller_machine`]).

use std::collections::{BTreeMap, BTreeSet};

use novo_controller::{
    BindingProblem, BindingTable, ControllerSet, MachineCatalog, MachinePath, Profile,
    ProfileLibrary, ProjectControllers, Severity, SignalKey, Target,
    import::{Options, Table, WordList, library, propose},
};
use novo_core::{Project, SymbolId, Value};
use novo_machine::commands::{Commands, Port, PortClass};
use novo_sim::PoleState;
use novo_sim::pins::{PoleRef, force_pole, operate_poles, pole_of_pin, poles_of_device};

use super::plc_run::PlcRun;

/// The add-on data block that holds the controllers (instances, bindings, the profiles in use).
pub(super) const ADDON: &str = "controllers";
pub(super) const BLOCK: &str = "project";
/// Version of the text in the block.
pub(super) const BLOCK_FORMAT: u32 = 1;
/// Where projects from before the add-on data block kept the same text.
pub(super) const LEGACY_PROPERTY: &str = "controllers.project";

/// Where a profile came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Source {
    /// Ships with the program (generic stand-ins, never a vendor product).
    Shipped,
    /// The person's own, in the profile folder.
    Folder(String),
}

/// A profile the person can add an instance of.
pub(super) struct ProfileEntry {
    pub(super) profile: Profile,
    pub(super) source: Source,
}

/// What a binding target leads to in this drawing.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Resolved {
    Pole(PoleRef),
    Hand(Vec<PoleRef>),
    Plc(String),
    /// A port of a module of the machine model; the machine loop reads it.
    Machine(MachineRef),
    /// Why this target reaches nothing, in plain words.
    Broken(String),
}

/// A machine port a target reaches.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MachineRef {
    /// The module instance: the device tag of its motor.
    pub(super) module: String,
    pub(super) port: Port,
    /// `hoist/=A+B-M1@up`, the spelling the catalog gives.
    pub(super) path: String,
}

/// One line in the binding checks: what is wrong and for which signal.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Finding {
    pub(super) severity: Severity,
    pub(super) key: Option<SignalKey>,
    pub(super) text: String,
}

/// Targets resolved against one revision of the project.
struct ResolvedTargets {
    revision: u64,
    table_generation: u64,
    by_target: BTreeMap<Target, Resolved>,
    rest: PoleState,
}

/// Everything the live view keeps about its controllers.
#[derive(Default)]
pub(crate) struct ControllerLive {
    pub(super) set: ControllerSet,
    pub(super) table: BindingTable,
    /// The property text last read from or written to the project; `None` before the first read.
    synced: Option<String>,
    /// Set when the property text could not be read: nothing is written over it until the
    /// person discards it.
    pub(super) damaged: Option<String>,
    /// What went wrong when the data was restored (a profile that changed, a missing one).
    pub(super) notes: Vec<String>,
    library: Option<(Vec<ProfileEntry>, Vec<String>)>,
    resolved: Option<ResolvedTargets>,
    table_generation: u64,
    /// The machine model of the drawing, for the revision it was built for.
    catalog: Option<(u64, MachineCatalog)>,
    /// The levels of the machine ports the bound signals call for now.
    commands: Commands,
    /// Poles and PLC bits this module held last frame, to let go of the ones no longer bound.
    applied: BTreeSet<PoleRef>,
    plc_applied: BTreeSet<String>,
    driven: BTreeSet<SymbolId>,
    /// Whole milliseconds of the clock are fed to the engine; the rest waits for the next frame.
    frac_ms: f64,
    /// The card is open.
    pub(super) open: bool,
    /// The instance the card shows.
    pub(super) selected: Option<String>,
    /// Instances shown in a floating panel of their own, so several can be operated at once.
    pub(super) popped: BTreeSet<String>,
    /// The keyboard drives the selected instance (off until asked, so the canvas keys stay free).
    pub(super) keys: bool,
    /// A click keeps a button or the stick held until clicked again (several motions at once).
    pub(super) sticky: bool,
    /// The structure changed since it was last written to the project.
    pub(super) dirty: bool,
    /// Findings of the last check, kept for the revision and table they were worked out for.
    findings: Option<(u64, u64, Vec<Finding>)>,
    /// Spring controls held away from rest, by instance and control.
    holds: BTreeMap<(String, String), Hold>,
    /// The last thing a control refused, in words (an interlock, an E-stop that needs a reset).
    pub(super) refused: Option<String>,
    /// The gamepad: device, maps, mappers, monitor (lane C5, see `controller_pad`).
    pub(super) pad: super::controller_pad::PadHost,
}

/// What holds a spring control away from rest: the pointer and a key, each at a position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Hold {
    mouse: Option<String>,
    key: Option<String>,
    pad: Option<String>,
}

/// Who is holding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoldBy {
    Mouse,
    Key,
    /// A gamepad or joystick (lane C5), through the map of the instance.
    Pad,
}

/// Colours offered for instances; new instances take them in turn.
pub(super) const PRESET_COLOURS: [(&str, [u8; 3]); 6] = [
    ("Blue", [0x4c, 0x9a, 0xff]),
    ("Orange", [0xf5, 0x9e, 0x42]),
    ("Green", [0x4c, 0xc2, 0x7a]),
    ("Pink", [0xe0, 0x6c, 0xb4]),
    ("Yellow", [0xe8, 0xd0, 0x4a]),
    ("Violet", [0x9b, 0x7c, 0xf0]),
];

/// `#rrggbb` to its bytes.
pub(super) fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let t = text.strip_prefix('#')?;
    if t.len() != 6 || !t.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// The text the project holds for the controllers, empty for none.
pub(super) fn stored(project: &Project) -> &str {
    if let Some(block) = project.info().addon_data.get(ADDON, BLOCK) {
        return block.text.as_str();
    }
    match project.info().properties.get(LEGACY_PROPERTY) {
        Some(Value::Text(t)) => t.as_str(),
        _ => "",
    }
}

/// Generic profiles that ship with the program and the person's own from the profile folder.
fn load_library() -> (Vec<ProfileEntry>, Vec<String>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (id, _) in library::list() {
        let profile = library::build(id).and_then(|p| Profile::load(&p.profile.to_toml()).ok());
        match profile {
            Some(profile) => out.push(ProfileEntry {
                profile,
                source: Source::Shipped,
            }),
            None => problems.push(format!("The shipped profile {id} could not be built")),
        }
    }
    if let Some(dir) = super::controller_import::controllers_dir()
        && let Ok(entries) = std::fs::read_dir(&dir)
    {
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".controller.toml"))
            })
            .collect();
        files.sort();
        for path in files {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| Profile::load(&t).map_err(|e| e.to_string()))
            {
                Ok(profile) => {
                    // A profile of the folder with a shipped id replaces the shipped one.
                    out.retain(|e| e.profile.controller.id != profile.controller.id);
                    out.push(ProfileEntry {
                        profile,
                        source: Source::Folder(name),
                    });
                }
                Err(e) => problems.push(format!("{name}: {e}")),
            }
        }
    }
    (out, problems)
}

impl ControllerLive {
    /// The profiles to choose from, read once per session (see [`Self::reload_library`]).
    pub(super) fn profiles(&mut self) -> &[ProfileEntry] {
        &self.library.get_or_insert_with(load_library).0
    }

    /// Problems found while reading the profile folder.
    pub(super) fn library_problems(&self) -> &[String] {
        self.library.as_ref().map_or(&[], |l| l.1.as_slice())
    }

    /// Reads the profile folder again (after the person added a profile with the wizard).
    pub(super) fn reload_library(&mut self) {
        self.library = Some(load_library());
    }

    fn profile_library(&mut self) -> ProfileLibrary {
        let mut lib = ProfileLibrary::new();
        for e in self.profiles() {
            lib.insert(e.profile.clone());
        }
        lib
    }

    /// Takes the project's data when it differs from what is held (a file just opened, an undo).
    /// The held data is the truth for what the person is working on until it is written back.
    pub(super) fn sync(&mut self, project: &Project) {
        let text = stored(project);
        if self.synced.as_deref() == Some(text) {
            return;
        }
        self.synced = Some(text.to_owned());
        self.dirty = false;
        self.damaged = None;
        self.notes.clear();
        self.findings = None;
        self.resolved = None;
        self.table_generation += 1;
        if text.is_empty() {
            self.set = ControllerSet::new();
            self.table = BindingTable::new();
            self.pad.set_maps(Default::default());
            self.release();
            return;
        }
        let library = self.profile_library();
        let mut maps = Default::default();
        let restored = ProjectControllers::from_text(text)
            .map_err(|e| format!("The controller data in this project could not be read: {e}"))
            .and_then(|data| {
                maps = data.gamepads.clone();
                data.restore(&library).map_err(|e| {
                    format!("The controllers of this project could not be restored: {e}")
                })
            });
        match restored {
            Ok((set, table, notes)) => {
                self.set = set;
                self.table = table;
                self.notes = notes;
                self.pad.set_maps(maps);
            }
            Err(e) => {
                self.set = ControllerSet::new();
                self.table = BindingTable::new();
                self.pad.set_maps(Default::default());
                self.damaged = Some(e);
            }
        }
        if self
            .selected
            .as_ref()
            .is_none_or(|s| self.set.instance(s).is_none())
        {
            self.selected = self.set.instances().first().map(|i| i.id.clone());
        }
        self.release();
    }

    /// Forget which poles were held (the next step sets what is bound and lets go of the rest).
    fn release(&mut self) {
        self.applied.clear();
        self.plc_applied.clear();
        self.driven.clear();
    }

    /// The text to store in the project, or empty for nothing. Controls are stored at rest: a
    /// saved project does not reopen with an E-stop pressed or a rocker held.
    pub(super) fn snapshot(&self) -> Result<String, String> {
        let mut data = ProjectControllers::capture(&self.set, &self.table)?;
        data.gamepads = self
            .pad
            .maps
            .iter()
            .filter(|(id, m)| !m.is_empty() && self.set.instance(id).is_some())
            .map(|(id, m)| (id.clone(), m.clone()))
            .collect();
        for record in &mut data.instances {
            if let Some(i) = self.set.instance(&record.id)
                && let Ok(fresh) = novo_controller::Controller::new(i.controller.profile().clone())
            {
                record.state = fresh.state().clone();
            }
        }
        if data.is_empty() {
            Ok(String::new())
        } else {
            data.to_text()
        }
    }

    /// The text to write when the structure changed, `None` when the project already holds it or
    /// the stored data is damaged and must be discarded first.
    pub(super) fn pending_write(&mut self) -> Option<Result<String, String>> {
        if !self.dirty || self.damaged.is_some() {
            return None;
        }
        match self.snapshot() {
            Ok(text) if self.synced.as_deref() == Some(text.as_str()) => {
                self.dirty = false;
                None
            }
            other => Some(other),
        }
    }

    /// The project now holds `text` (after the write): do not reload it.
    pub(super) fn written(&mut self, text: &str) {
        self.synced = Some(text.to_owned());
        self.dirty = false;
    }

    /// The structure changed: bindings are resolved again and the project gets a new copy.
    pub(super) fn changed(&mut self) {
        self.dirty = true;
        self.table_generation += 1;
        self.findings = None;
    }

    /// Adds an instance of `profile` named after it, with an id that is free.
    pub(super) fn add_instance(&mut self, profile: &Profile) -> Result<String, String> {
        let base = novo_controller::import::slug(&profile.controller.id);
        let base = if base.is_empty() {
            "controller".to_owned()
        } else {
            base
        };
        let mut n = 1;
        let id = loop {
            let candidate = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            if self.set.instance(&candidate).is_none() {
                break candidate;
            }
            n += 1;
        };
        let label = if n == 1 {
            profile.controller.name.clone()
        } else {
            format!("{} {n}", profile.controller.name)
        };
        self.set
            .add_instance(&id, &label, profile)
            .map_err(|e| e.to_string())?;
        self.selected = Some(id.clone());
        self.changed();
        Ok(id)
    }

    /// Removes an instance and every binding of its signals.
    pub(super) fn remove_instance(&mut self, id: &str) {
        if self.set.remove_instance(id).is_err() {
            return;
        }
        self.table.bindings.retain(|b| b.key.controller != id);
        self.popped.remove(id);
        self.holds.retain(|(who, _), _| who != id);
        self.pad.forget(id);
        if self.selected.as_deref() == Some(id) {
            self.selected = self.set.instances().first().map(|i| i.id.clone());
        }
        self.changed();
    }

    /// A new instance of the same profile, selected, with bindings of its own (none copied).
    pub(super) fn duplicate_instance(&mut self, from: &str) -> Result<String, String> {
        let label = self
            .set
            .instance(from)
            .map(|i| format!("{} copy", i.label))
            .ok_or_else(|| format!("No controller {from}"))?;
        let mut n = 2;
        let id = loop {
            let candidate = format!("{from}-{n}");
            if self.set.instance(&candidate).is_none() {
                break candidate;
            }
            n += 1;
        };
        self.set
            .duplicate_instance(from, &id, &label)
            .map_err(|e| e.to_string())?;
        self.selected = Some(id.clone());
        self.changed();
        Ok(id)
    }

    /// Renames what the person sees; the id stays, so bindings and scripts keep working.
    pub(super) fn rename_instance(&mut self, id: &str, label: &str) {
        let label = label.trim();
        if !label.is_empty() && self.set.rename(id, label).is_ok() {
            self.changed();
        }
    }

    pub(super) fn set_colour(&mut self, id: &str, colour: Option<&str>) {
        if self.set.set_colour(id, colour).is_ok() {
            self.changed();
        }
    }

    pub(super) fn set_target_group(&mut self, id: &str, group: &str) {
        let group = group.trim();
        let group = (!group.is_empty()).then_some(group);
        if self.set.set_target_group(id, group).is_ok() {
            self.changed();
        }
    }

    /// The colour of an instance: its own, or one of the presets by position in the list.
    pub(super) fn colour_of(&self, id: &str) -> [u8; 3] {
        let index = self.set.instances().iter().position(|i| i.id == id);
        let own = self
            .set
            .instance(id)
            .and_then(|i| i.colour.as_deref())
            .and_then(parse_hex);
        own.unwrap_or_else(|| {
            let (_, rgb) = PRESET_COLOURS[index.unwrap_or(0) % PRESET_COLOURS.len()];
            rgb
        })
    }

    /// Shows the instance in a panel of its own (or back in the card). The keyboard stays with
    /// the selected instance; pressing in a panel does not move it.
    pub(super) fn set_popped(&mut self, id: &str, on: bool) {
        if on && self.set.instance(id).is_some() {
            self.popped.insert(id.to_owned());
        } else {
            self.popped.remove(id);
        }
    }

    /// Targets this instance shares with another instance, for the owner lines in its panel.
    pub(super) fn shared_targets(&self, id: &str) -> Vec<novo_controller::TargetOwnership> {
        self.table
            .ownership(&self.set)
            .into_iter()
            .filter(|o| o.controllers.len() > 1 && o.controllers.iter().any(|c| c == id))
            .collect()
    }

    /// Discards data the project holds that could not be read.
    pub(super) fn discard_damaged(&mut self) {
        self.damaged = None;
        self.set = ControllerSet::new();
        self.table = BindingTable::new();
        self.dirty = true;
        self.table_generation += 1;
    }

    // ---- pressing ----

    /// The pointer or a key holds `control` of `instance` at `position`, or lets go (`None`).
    /// [`Self::apply_holds`] moves the engine.
    pub(super) fn hold(
        &mut self,
        instance: &str,
        control: &str,
        by: HoldBy,
        position: Option<&str>,
    ) {
        let key = (instance.to_owned(), control.to_owned());
        if position.is_none() && !self.holds.contains_key(&key) {
            return;
        }
        let entry = self.holds.entry(key).or_default();
        let slot = match by {
            HoldBy::Mouse => &mut entry.mouse,
            HoldBy::Key => &mut entry.key,
            HoldBy::Pad => &mut entry.pad,
        };
        *slot = position.map(str::to_owned);
    }

    /// Applies one action to an instance and keeps what the engine refused, in words.
    pub(super) fn act(&mut self, instance: &str, action: novo_controller::Action) {
        use novo_controller::Applied;
        self.refused = match self.set.act(instance, &action) {
            Ok(Applied::Blocked(b)) => Some(block_words(&b)),
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };
    }

    /// Moves every held control to the position that holds it and lets the others go.
    pub(super) fn apply_holds(&mut self) {
        use novo_controller::{Action, Applied};
        let keys: Vec<(String, String)> = self.holds.keys().cloned().collect();
        for key in keys {
            let Some(hold) = self.holds.get(&key).cloned() else {
                continue;
            };
            let wanted = hold.pad.clone().or(hold.key.clone()).or(hold.mouse.clone());
            let Some((at, rest)) = self.set.instance(&key.0).and_then(|i| {
                let at = i.controller.position(&key.1)?.to_owned();
                let rest = i.controller.profile().control(&key.1)?.rest_position();
                Some((at, rest))
            }) else {
                self.holds.remove(&key);
                continue;
            };
            let action = match &wanted {
                Some(p) if *p != at => Some(Action::SetPosition {
                    control: key.1.clone(),
                    position: p.clone(),
                }),
                None if at != rest => Some(Action::Release {
                    control: key.1.clone(),
                }),
                _ => None,
            };
            if let Some(action) = action {
                match self.set.act(&key.0, &action) {
                    Ok(Applied::Blocked(b)) => self.refused = Some(block_words(&b)),
                    Ok(_) => {}
                    Err(e) => self.refused = Some(e.to_string()),
                }
            }
            if wanted.is_none() {
                self.holds.remove(&key);
            }
        }
    }

    /// Puts every instance back to its start state (controls at rest, link up, powered).
    pub(super) fn reset_state(&mut self) {
        let mut records = self.set.to_records();
        let mut lib = ProfileLibrary::new();
        for i in self.set.instances() {
            lib.insert(i.controller.profile().clone());
            if let Some(r) = records.iter_mut().find(|r| r.id == i.id)
                && let Ok(fresh) = novo_controller::Controller::new(i.controller.profile().clone())
            {
                r.state = fresh.state().clone();
            }
        }
        if let Ok(set) = ControllerSet::from_records(&records, &lib) {
            self.set = set;
        }
        self.holds.clear();
        self.refused = None;
    }

    // ---- the per-frame step ----

    fn advance(&mut self, dt: f64) {
        self.frac_ms += dt.clamp(0.0, 1.0) * 1000.0;
        let whole = self.frac_ms.floor();
        self.frac_ms -= whole;
        if whole >= 1.0 {
            self.set.advance(whole as u64);
        }
    }

    /// Whether a receiver self-test or anything else needs the clock to run without input.
    pub(super) fn wants_frames(&self) -> bool {
        self.set
            .instances()
            .iter()
            .any(|i| i.controller.in_self_test())
    }

    /// Symbols a controller closes or opens now; their badges are not for clicking.
    pub(super) fn driven(&self) -> &BTreeSet<SymbolId> {
        &self.driven
    }

    /// Moves time on and sets the poles and PLC bits the bound signals call for, in `poles`
    /// (the live view's hand-set poles) and for the PLC program. Poles that were bound last frame
    /// and are not now go back to their rest state.
    pub(super) fn drive(
        &mut self,
        project: &Project,
        dt: f64,
        poles: &mut PoleState,
        plc: &mut PlcRun,
    ) {
        self.sync(project);
        if self.set.instances().is_empty() && self.applied.is_empty() && self.plc_applied.is_empty()
        {
            self.commands = Commands::new();
            return;
        }
        self.advance(dt);
        self.resolve_targets(project);
        let levels = self.table.resolve(&self.set);
        let Some(resolved) = &self.resolved else {
            return;
        };
        let mut now_poles = BTreeSet::new();
        let mut now_plc = BTreeSet::new();
        let mut commands = Commands::new();
        for (target, what) in &resolved.by_target {
            let on = levels.get(target).copied().unwrap_or(0.0) != 0.0;
            match what {
                Resolved::Pole(pole) => {
                    force_pole(poles, *pole, on);
                    now_poles.insert(*pole);
                }
                Resolved::Hand(list) => {
                    operate_poles(poles, &resolved.rest, list, on);
                    now_poles.extend(list.iter().copied());
                }
                Resolved::Plc(address) => {
                    plc.override_input(address, Some(on));
                    now_plc.insert(address.clone());
                }
                Resolved::Machine(r) => {
                    // Two spellings of one port combine as the checks warn: a chain or enable
                    // port needs every one high, a command any one.
                    let level = match (commands.level(&r.module, r.port), r.port.class()) {
                        (Some(before), PortClass::Command) => before || on,
                        (Some(before), _) => before && on,
                        (None, _) => on,
                    };
                    commands.set(&r.module, r.port, level);
                }
                Resolved::Broken(_) => {}
            }
        }
        self.commands = commands;
        for gone in self.applied.difference(&now_poles) {
            poles.set(
                gone.symbol,
                gone.group,
                resolved.rest.is_closed(gone.symbol, gone.group),
            );
        }
        for gone in self.plc_applied.difference(&now_plc) {
            plc.override_input(gone, None);
        }
        self.driven = now_poles.iter().map(|p| p.symbol).collect();
        self.applied = now_poles;
        self.plc_applied = now_plc;
    }

    /// Works out what each bound target is in this drawing; kept until the drawing or the table
    /// changes.
    fn resolve_targets(&mut self, project: &Project) {
        if self.resolved.as_ref().is_some_and(|r| {
            r.revision == project.revision() && r.table_generation == self.table_generation
        }) {
            return;
        }
        let mut by_target = BTreeMap::new();
        let wants_machine = self
            .table
            .bindings
            .iter()
            .any(|b| matches!(b.target, Target::ModulePath(_)));
        let catalog = wants_machine.then(|| self.catalog(project).clone());
        for b in &self.table.bindings {
            by_target
                .entry(b.target.clone())
                .or_insert_with(|| resolve_target_in(project, &b.target, catalog.as_ref()));
        }
        self.resolved = Some(ResolvedTargets {
            revision: project.revision(),
            table_generation: self.table_generation,
            by_target,
            rest: novo_sim::rest_state(project),
        });
    }

    /// What a target leads to, for the binding table (resolved against the current drawing).
    pub(super) fn resolution(&mut self, project: &Project, target: &Target) -> Resolved {
        self.resolve_targets(project);
        if let Some(found) = self
            .resolved
            .as_ref()
            .and_then(|r| r.by_target.get(target).cloned())
        {
            return found;
        }
        resolve_target(project, target)
    }

    /// The machine model of the drawing (hoists, drive hoists, winches, travel axes with their
    /// mechanical data), kept for the revision of the project.
    pub(super) fn catalog(&mut self, project: &Project) -> &MachineCatalog {
        let stale = self
            .catalog
            .as_ref()
            .is_none_or(|(rev, _)| *rev != project.revision());
        if stale {
            self.catalog = Some((
                project.revision(),
                super::controller_machine::catalog(project),
            ));
        }
        self.catalog.as_ref().map(|(_, c)| c).expect("set above")
    }

    /// The machine ports the bound signals command now.
    pub(super) fn machine_commands(&self) -> &Commands {
        &self.commands
    }

    /// Which signals are bound to ports of which module, for the run windows and the table:
    /// module instance to `(port, signal, high)`.
    pub(super) fn machine_drivers(&self) -> BTreeMap<String, Vec<(Port, SignalKey, bool)>> {
        let mut out: BTreeMap<String, Vec<(Port, SignalKey, bool)>> = BTreeMap::new();
        let Some(resolved) = &self.resolved else {
            return out;
        };
        for b in &self.table.bindings {
            if let Some(Resolved::Machine(r)) = resolved.by_target.get(&b.target) {
                let high = self.set.level(&b.key).is_some_and(|l| l != 0.0) != b.invert;
                out.entry(r.module.clone())
                    .or_default()
                    .push((r.port, b.key.clone(), high));
            }
        }
        out
    }

    // ---- checks ----

    /// The binding checks of the engine plus what only the drawing can say: a target that leads
    /// nowhere, two targets that close the same contact, a contact a coil owns.
    pub(super) fn findings(&mut self, project: &Project) -> Vec<Finding> {
        self.resolve_targets(project);
        if let Some((rev, generation, list)) = &self.findings
            && *rev == project.revision()
            && *generation == self.table_generation
        {
            return list.clone();
        }
        let mut out: Vec<Finding> = Vec::new();
        // A control panel has many signals a drawing does not use; one line per instance, not one
        // per signal.
        let mut idle: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let wants_machine = self
            .table
            .bindings
            .iter()
            .any(|b| matches!(b.target, Target::ModulePath(_)));
        let catalog = wants_machine.then(|| self.catalog(project).clone());
        for f in self.table.check_with(&self.set, catalog.as_ref()) {
            if f.problem == BindingProblem::Unbound {
                idle.entry(f.key.controller.clone())
                    .or_default()
                    .push(f.key.signal.clone());
                continue;
            }
            out.push(Finding {
                severity: f.severity,
                key: Some(f.key),
                text: f.message,
            });
        }
        for (controller, signals) in idle {
            let shown: Vec<&str> = signals.iter().take(4).map(String::as_str).collect();
            let more = signals.len().saturating_sub(shown.len());
            out.push(Finding {
                severity: Severity::Warning,
                key: None,
                text: format!(
                    "{controller}: {} signals drive nothing ({}{})",
                    signals.len(),
                    shown.join(", "),
                    if more > 0 {
                        format!(" and {more} more")
                    } else {
                        String::new()
                    }
                ),
            });
        }
        if let Some(resolved) = &self.resolved {
            let mut by_pole: BTreeMap<PoleRef, Vec<&Target>> = BTreeMap::new();
            for b in &self.table.bindings {
                match resolved.by_target.get(&b.target) {
                    // A machine target is explained by the machine checks above.
                    Some(Resolved::Broken(_)) if matches!(b.target, Target::ModulePath(_)) => {}
                    Some(Resolved::Broken(why)) => out.push(Finding {
                        severity: Severity::Error,
                        key: Some(b.key.clone()),
                        text: format!("{} -> {}: {why}", b.key, b.target),
                    }),
                    Some(Resolved::Pole(p)) => by_pole.entry(*p).or_default().push(&b.target),
                    Some(Resolved::Hand(list)) => {
                        for p in list {
                            by_pole.entry(*p).or_default().push(&b.target);
                        }
                    }
                    Some(Resolved::Plc(a)) if analog_word(a) => out.push(Finding {
                        severity: Severity::Warning,
                        key: Some(b.key.clone()),
                        text: format!(
                            "{}: a PLC word input is not driven yet; only input bits are",
                            b.key
                        ),
                    }),
                    _ => {}
                }
                if let Some(Resolved::Pole(p)) = resolved.by_target.get(&b.target)
                    && coil_owns(project, *p)
                {
                    out.push(Finding {
                        severity: Severity::Warning,
                        key: Some(b.key.clone()),
                        text: format!(
                            "{} -> {}: a coil drives this contact, so the controller cannot close it",
                            b.key, b.target
                        ),
                    });
                }
            }
            for (_, targets) in by_pole
                .iter()
                .filter(|(_, t)| t.iter().collect::<BTreeSet<_>>().len() > 1)
            {
                let names: Vec<String> = targets.iter().map(|t| t.to_string()).collect();
                out.push(Finding {
                    severity: Severity::Warning,
                    key: None,
                    text: format!("{} close the same contact", names.join(" and ")),
                });
            }
        }
        self.findings = Some((project.revision(), self.table_generation, out.clone()));
        out
    }

    // ---- binding ----

    /// Binds every signal of `instance` whose profile pin name is a pin of `device` (a tag text
    /// such as `-A1`), when that pin leads to exactly one contact not yet bound to another signal.
    /// Returns how many were bound and the pin names that found no contact.
    pub(super) fn auto_bind(
        &mut self,
        project: &Project,
        instance: &str,
        device: &str,
    ) -> (usize, Vec<String>) {
        let Some(inst) = self.set.instance(instance) else {
            return (0, Vec::new());
        };
        let signals: Vec<(String, Option<String>)> = inst
            .controller
            .profile()
            .signals
            .iter()
            .map(|s| (s.name.clone(), s.pin.clone()))
            .collect();
        let mut taken: BTreeSet<PoleRef> = BTreeSet::new();
        for b in &self.table.bindings {
            if let Resolved::Pole(p) = resolve_target(project, &b.target) {
                taken.insert(p);
            }
        }
        let (mut bound, mut missed) = (0, Vec::new());
        for (name, pin) in signals {
            let key = SignalKey::new(instance, &name);
            if self.table.targets_of(&key).next().is_some() {
                continue;
            }
            let Some(pin) = pin.filter(|p| !p.is_empty()) else {
                continue;
            };
            let designation = format!("{}:{pin}", device.trim());
            match pole_of_pin(project, &designation) {
                Ok(pole) if taken.insert(pole) => {
                    self.table.bind(key, Target::Pin(designation));
                    bound += 1;
                }
                _ => missed.push(pin),
            }
        }
        if bound > 0 {
            self.changed();
        }
        (bound, missed)
    }

    /// The bindings of `instance` to the pins of `device`, as the card lists them.
    pub(super) fn bind(&mut self, key: SignalKey, target: Target, invert: bool) {
        if self.table.bind_with(key, target, invert) {
            self.changed();
        }
    }

    pub(super) fn unbind(&mut self, key: &SignalKey) {
        if self.table.targets_of(key).next().is_some() {
            self.table.unbind(key);
            self.changed();
        }
    }
}

/// Why the engine refused a change, as the card says it.
fn block_words(block: &novo_controller::Block) -> String {
    use novo_controller::Block;
    match block {
        Block::LinkDown => "Refused: the radio link is down or the receiver is off".to_owned(),
        Block::NeedsReset => "Refused: the emergency stop only releases with its Reset".to_owned(),
        Block::Requires(c) => format!("Refused: needs {c}"),
        Block::Inhibited(c) => format!("Refused: held at rest by {c}"),
        Block::Interlock { with } => format!("Refused: {with} is already down"),
    }
}

fn analog_word(address: &str) -> bool {
    novo_sim::program::Address::parse(address).is_some_and(|a| !a.is_bit())
}

/// A contact whose device has a coil follows that coil, whatever the controller says.
fn coil_owns(project: &Project, pole: PoleRef) -> bool {
    let Some(device) = project.symbol(pole.symbol).and_then(|s| s.device()) else {
        return false;
    };
    project
        .symbols_of_device(device)
        .iter()
        .any(|s| novo_sim::is_coil(s))
}

/// What `target` is in `project`.
pub(super) fn resolve_target(project: &Project, target: &Target) -> Resolved {
    if matches!(target, Target::ModulePath(_)) {
        let catalog = super::controller_machine::catalog(project);
        return resolve_target_in(project, target, Some(&catalog));
    }
    resolve_target_in(project, target, None)
}

/// [`resolve_target`] with the machine model already worked out.
fn resolve_target_in(
    project: &Project,
    target: &Target,
    catalog: Option<&MachineCatalog>,
) -> Resolved {
    match target {
        Target::Pin(designation) => match pole_of_pin(project, designation) {
            Ok(pole) => Resolved::Pole(pole),
            Err(e) => Resolved::Broken(e.to_string()),
        },
        Target::Symbol(tag) => match poles_of_device(project, tag) {
            Ok(list) => Resolved::Hand(list),
            Err(e) => Resolved::Broken(e.to_string()),
        },
        Target::PlcAddress(address) => match novo_sim::program::Address::parse(address) {
            // A word input is kept and reported by the checks: it would need a scaling nobody
            // has chosen yet.
            Some(a) if a.space == novo_sim::program::Space::Input => Resolved::Plc(address.clone()),
            _ => Resolved::Broken(format!("{address} is not a PLC input address")),
        },
        Target::ModulePath(text) => resolve_machine(text, catalog),
    }
}

/// A machine target against the machine model: the module and port it reaches, or why it reaches
/// none.
fn resolve_machine(text: &str, catalog: Option<&MachineCatalog>) -> Resolved {
    let path = match MachinePath::parse(text) {
        Ok(p) => p,
        Err(e) => return Resolved::Broken(e.to_string()),
    };
    let Some(catalog) = catalog else {
        return Resolved::Broken("the machine model is not known".to_owned());
    };
    let Some(module) = catalog.module(&path) else {
        return Resolved::Broken(format!(
            "no module {} in the machine model of this drawing",
            path.module()
        ));
    };
    let Some(port_name) = path.port() else {
        return Resolved::Broken(format!(
            "{} names no port (add @up, @down, ...)",
            module.path
        ));
    };
    let Some(port) = module.port(port_name).and_then(|_| Port::parse(port_name)) else {
        return Resolved::Broken(format!("{} has no port {port_name}", module.path));
    };
    let id = module.path.rsplit('/').next().unwrap_or(&module.path);
    Resolved::Machine(MachineRef {
        module: id.to_owned(),
        port,
        path: format!("{}@{port_name}", module.path),
    })
}

/// A profile for the shipped generic radio, the same one the import wizard builds. Used by tests
/// and by the QA scenarios through the card's "Add".
#[allow(dead_code)]
pub(super) fn generic_profile(id: &str) -> Option<Profile> {
    library::build(id).and_then(|p| Profile::load(&p.profile.to_toml()).ok())
}

/// A profile from an I/O table the same way the wizard reads one (tests, scripts).
#[allow(dead_code)]
pub(super) fn profile_from_table(text: &str, id: &str, name: &str) -> Option<Profile> {
    let proposal = propose(
        &Table::from_text(text),
        &WordList::builtin(),
        &Options {
            id: id.to_owned(),
            name: name.to_owned(),
            ..Options::default()
        },
    );
    Profile::load(&proposal.profile.to_toml()).ok()
}

/// Every output contact of a receiver drawn in the project, as `tag:pin`: one pin per switched
/// pole of a `controllers/` symbol (the shared common is left out).
pub(super) fn receiver_outputs(project: &Project) -> Vec<String> {
    let mut out = Vec::new();
    for symbol in project.symbols() {
        if !symbol.symbol.0.contains("/controllers/") || symbol.pass_through.is_empty() {
            continue;
        }
        let Some(device) = symbol.device().and_then(|d| project.device(d)) else {
            continue;
        };
        let mut uses: BTreeMap<u16, usize> = BTreeMap::new();
        for pin in symbol.pass_through.iter().flatten() {
            *uses.entry(*pin).or_default() += 1;
        }
        for group in &symbol.pass_through {
            let pin = group
                .iter()
                .find(|p| uses.get(p).copied().unwrap_or(0) == 1)
                .or(group.first());
            if let Some(name) = pin.and_then(|p| symbol.pins.get(usize::from(*p))) {
                out.push(format!("{}:{}", device.tag, name.name));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The tags of the receiver devices drawn in the project.
pub(super) fn receivers(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = project
        .symbols()
        .filter(|s| s.symbol.0.contains("/controllers/outputs_"))
        .filter_map(|s| s.device().and_then(|d| project.device(d)))
        .map(|d| d.tag.to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The tags of the hand devices (push buttons, switches, E-stops) drawn in the project.
pub(super) fn hand_devices(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = project
        .symbols()
        .filter(|s| s.symbol.0.contains("/operators/") && !s.pass_through.is_empty())
        .filter_map(|s| s.device().and_then(|d| project.device(d)))
        .map(|d| d.tag.to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}
