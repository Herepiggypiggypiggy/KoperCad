//! Several controller instances per project.
//!
//! An instance is a profile plus its own control state, a label, a colour and an optional
//! target group (the crane, crab or module path it normally drives). The same profile can be
//! used by any number of instances; each has its own state and its own bindings.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::binding::{Binding, BindingTable, SignalKey};
use crate::engine::{Applied, ControlError, Controller, State};
use crate::profile::Profile;
use crate::sequence::{Action, Recorder, Sequence, TimedAction};
use crate::validate::Issue;

/// Profiles by id (shipped generic ones and the user's own).
#[derive(Debug, Clone, Default)]
pub struct ProfileLibrary {
    profiles: BTreeMap<String, Profile>,
}

impl ProfileLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace a profile. Returns the previous one with the same id.
    pub fn insert(&mut self, profile: Profile) -> Option<Profile> {
        self.profiles.insert(profile.controller.id.clone(), profile)
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.profiles.keys().map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetError {
    DuplicateId(String),
    /// Ids are used in `controller/signal` keys and in files: not empty, no `/`, no whitespace.
    InvalidId(String),
    /// A replay would take more samples than [`crate::sequence::MAX_SAMPLES`].
    TooManySamples,
    UnknownInstance(String),
    UnknownProfile(String),
    InvalidProfile(Vec<Issue>),
    Control(ControlError),
}

impl fmt::Display for SetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetError::DuplicateId(id) => write!(f, "controller instance `{id}` already exists"),
            SetError::InvalidId(id) => write!(f, "`{id}` is not a usable controller instance id"),
            SetError::TooManySamples => write!(
                f,
                "replay would take too many samples; raise sample_ms or shorten it"
            ),
            SetError::UnknownInstance(id) => write!(f, "no controller instance `{id}`"),
            SetError::UnknownProfile(id) => write!(f, "no controller profile `{id}`"),
            SetError::InvalidProfile(issues) => {
                write!(f, "profile has {} problem(s)", issues.len())
            }
            SetError::Control(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SetError {}

impl From<ControlError> for SetError {
    fn from(e: ControlError) -> Self {
        SetError::Control(e)
    }
}

/// Ids are used in `controller/signal` keys and in files: not empty, no `/`, no whitespace.
fn check_id(id: &str) -> Result<(), SetError> {
    if id.is_empty() || id.contains('/') || id.contains(char::is_whitespace) {
        Err(SetError::InvalidId(id.to_string()))
    } else {
        Ok(())
    }
}

/// One instance as saved in the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub colour: Option<String>,
    #[serde(default)]
    pub target_group: Option<String>,
    /// Id of the profile in the library.
    pub profile: String,
    pub state: State,
}

/// What the project stores: instances (with state), bindings and the profiles they use.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectControllers {
    #[serde(default, rename = "instance")]
    pub instances: Vec<InstanceRecord>,
    #[serde(default, rename = "binding")]
    pub bindings: Vec<Binding>,
    /// The profiles the instances use, as profile text by profile id, so a project opens on a PC
    /// whose profile folder lacks them (a profile is data, not a licence to a vendor's file).
    #[serde(
        default,
        rename = "profile",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub profiles: BTreeMap<String, String>,
}

impl ProjectControllers {
    /// The project data for `set` and `table`, with the text of every profile in use.
    pub fn capture(set: &ControllerSet, table: &BindingTable) -> Result<Self, String> {
        let mut profiles = BTreeMap::new();
        for i in set.instances() {
            let p = i.controller.profile();
            if !profiles.contains_key(&p.controller.id) {
                profiles.insert(p.controller.id.clone(), p.to_toml()?);
            }
        }
        Ok(ProjectControllers {
            instances: set.to_records(),
            bindings: table.bindings.clone(),
            profiles,
        })
    }

    /// True when there is nothing to store (no instances and no bindings).
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty() && self.bindings.is_empty()
    }

    /// The data as TOML text, the form the project file keeps it in.
    pub fn to_text(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_text(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Rebuild the set and the binding table. A profile saved in the project wins over the
    /// library's copy with the same id (the project keeps working as it was made); a note says
    /// when the two differ. Fails when an instance names a profile neither has.
    pub fn restore(
        &self,
        library: &ProfileLibrary,
    ) -> Result<(ControllerSet, BindingTable, Vec<String>), SetError> {
        let mut merged = library.clone();
        let mut notes = Vec::new();
        for (id, text) in &self.profiles {
            match Profile::parse(text) {
                Ok(p) => {
                    if library.get(id).is_some_and(|mine| mine != &p) {
                        notes.push(format!(
                            "Profile `{id}` in this project differs from the copy in your profile folder; the project's copy is used"
                        ));
                    }
                    merged.insert(p);
                }
                Err(e) => notes.push(format!(
                    "Profile `{id}` saved in the project is damaged: {e}"
                )),
            }
        }
        let set = ControllerSet::from_records(&self.instances, &merged)?;
        let table = BindingTable {
            bindings: self.bindings.clone(),
        };
        Ok((set, table, notes))
    }
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub id: String,
    pub label: String,
    pub colour: Option<String>,
    pub target_group: Option<String>,
    pub controller: Controller,
}

/// All instances of a project, in the order the user made them.
#[derive(Debug, Clone, Default)]
pub struct ControllerSet {
    instances: Vec<Instance>,
    time_ms: u64,
    recorder: Option<Recorder>,
}

impl ControllerSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }

    pub fn instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn time_ms(&self) -> u64 {
        self.time_ms
    }

    pub fn add_instance(
        &mut self,
        id: &str,
        label: &str,
        profile: &Profile,
    ) -> Result<(), SetError> {
        check_id(id)?;
        if self.instance(id).is_some() {
            return Err(SetError::DuplicateId(id.to_string()));
        }
        let mut controller = Controller::new(profile.clone()).map_err(SetError::InvalidProfile)?;
        controller.advance(self.time_ms);
        self.instances.push(Instance {
            id: id.to_string(),
            label: label.to_string(),
            colour: None,
            target_group: None,
            controller,
        });
        Ok(())
    }

    /// A new instance of the same profile in its start state (not a copy of the presses).
    pub fn duplicate_instance(
        &mut self,
        from: &str,
        new_id: &str,
        label: &str,
    ) -> Result<(), SetError> {
        let src = self
            .instance(from)
            .ok_or_else(|| SetError::UnknownInstance(from.to_string()))?;
        let (profile, colour, group) = (
            src.controller.profile().clone(),
            src.colour.clone(),
            src.target_group.clone(),
        );
        self.add_instance(new_id, label, &profile)?;
        let new = self.instance_mut(new_id)?;
        new.colour = colour;
        new.target_group = group;
        Ok(())
    }

    pub fn remove_instance(&mut self, id: &str) -> Result<(), SetError> {
        let before = self.instances.len();
        self.instances.retain(|i| i.id != id);
        if self.instances.len() == before {
            Err(SetError::UnknownInstance(id.to_string()))
        } else {
            Ok(())
        }
    }

    pub fn rename(&mut self, id: &str, label: &str) -> Result<(), SetError> {
        self.instance_mut(id)?.label = label.to_string();
        Ok(())
    }

    pub fn set_colour(&mut self, id: &str, colour: Option<&str>) -> Result<(), SetError> {
        self.instance_mut(id)?.colour = colour.map(str::to_string);
        Ok(())
    }

    pub fn set_target_group(&mut self, id: &str, group: Option<&str>) -> Result<(), SetError> {
        self.instance_mut(id)?.target_group = group.map(str::to_string);
        Ok(())
    }

    fn instance_mut(&mut self, id: &str) -> Result<&mut Instance, SetError> {
        self.instances
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| SetError::UnknownInstance(id.to_string()))
    }

    /// Apply one action to an instance (and record it when recording).
    pub fn act(&mut self, instance: &str, action: &Action) -> Result<Applied, SetError> {
        let t_ms = self.time_ms;
        let ctl = &mut self.instance_mut(instance)?.controller;
        let applied = match action {
            Action::Press { control } => ctl.press(control)?,
            Action::Release { control } => ctl.release(control)?,
            Action::SetPosition { control, position } => ctl.set_position(control, position)?,
            Action::Reset { control } => ctl.reset(control)?,
            Action::Link { ok } => ctl.set_link(*ok),
            Action::BatteryLow { low } => ctl.set_battery_low(*low),
            Action::Power { on } => ctl.power(*on),
        };
        if let Some(rec) = &mut self.recorder {
            rec.push(TimedAction {
                t_ms,
                instance: instance.to_string(),
                action: action.clone(),
            });
        }
        Ok(applied)
    }

    /// Let time pass for every instance (they share one clock).
    pub fn advance(&mut self, dt_ms: u64) {
        self.time_ms = self.time_ms.saturating_add(dt_ms);
        for i in &mut self.instances {
            i.controller.advance(dt_ms);
        }
    }

    pub fn start_recording(&mut self) {
        self.recorder = Some(Recorder::default());
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Stop and return what was pressed since `start_recording`, with the first event at 0 ms.
    pub fn stop_recording(&mut self, name: &str) -> Option<Sequence> {
        self.recorder.take().map(|r| r.finish(name))
    }

    /// Every signal of every instance, keyed `controller/signal`, in instance order.
    pub fn signals(&self) -> Vec<(SignalKey, f64)> {
        self.instances
            .iter()
            .flat_map(|i| {
                i.controller
                    .signals()
                    .into_iter()
                    .map(|s| (SignalKey::new(&i.id, &s.name), s.level))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn level(&self, key: &SignalKey) -> Option<f64> {
        self.instance(&key.controller)?
            .controller
            .level(&key.signal)
    }

    pub fn has_signal(&self, key: &SignalKey) -> bool {
        self.instance(&key.controller)
            .is_some_and(|i| i.controller.profile().signal(&key.signal).is_some())
    }

    /// Whether the instance's profile marks it as allowed to share targets.
    pub fn is_shared(&self, instance: &str) -> bool {
        self.instance(instance)
            .is_some_and(|i| i.controller.profile().controller.shared)
    }

    pub fn to_records(&self) -> Vec<InstanceRecord> {
        self.instances
            .iter()
            .map(|i| InstanceRecord {
                id: i.id.clone(),
                label: i.label.clone(),
                colour: i.colour.clone(),
                target_group: i.target_group.clone(),
                profile: i.controller.profile().controller.id.clone(),
                state: i.controller.state().clone(),
            })
            .collect()
    }

    /// Rebuild from saved records. Fails when a profile is missing from the library.
    pub fn from_records(
        records: &[InstanceRecord],
        library: &ProfileLibrary,
    ) -> Result<ControllerSet, SetError> {
        let mut set = ControllerSet::new();
        for r in records {
            check_id(&r.id)?;
            if set.instance(&r.id).is_some() {
                return Err(SetError::DuplicateId(r.id.clone()));
            }
            let profile = library
                .get(&r.profile)
                .ok_or_else(|| SetError::UnknownProfile(r.profile.clone()))?;
            let controller = Controller::restore(profile.clone(), r.state.clone())
                .map_err(SetError::InvalidProfile)?;
            set.time_ms = set.time_ms.max(controller.time_ms());
            set.instances.push(Instance {
                id: r.id.clone(),
                label: r.label.clone(),
                colour: r.colour.clone(),
                target_group: r.target_group.clone(),
                controller,
            });
        }
        // Instances saved at different moments catch up to the newest clock.
        let now = set.time_ms;
        for i in &mut set.instances {
            let behind = now - i.controller.time_ms();
            if behind > 0 {
                i.controller.advance(behind);
            }
        }
        Ok(set)
    }
}
