//! Machine-module bindings of the virtual controller (lane L5b): the modules and ports a drawing
//! offers, and the glue that lets a bound signal command the hoist and drive runs.
//!
//! A signal can be bound to a port of a module instance of the machine model, written
//! `hoist/=A+B-M1@up`. The modules are the motors of the drawing that carry mechanical data (the
//! property sheet sections "Mechanical load", "Travel drive", "Drive hoist" and "Closed-loop
//! winch"); the ports come from `novo_machine::commands`. While a run window is running, the
//! levels of the bound ports go in as commands: see that file for what a port does, and for what a
//! bound port replaces. Nothing here changes `novo-core`; the bindings are part of the controller
//! data the project already stores.
//!
//! It simulates wiring and logic, not a safety function: a permit port shows what a chain does to
//! the machine, it is no certified E-stop and says nothing about PL or SIL.

use std::collections::BTreeSet;

use novo_controller::{MachineCatalog, ModuleEntry, PortEntry, PortKind};
use novo_core::Project;
use novo_machine::cl_pair_loop::ClPairModule;
use novo_machine::cl_winch_loop::ClWinchModule;
use novo_machine::commands::{Commands, Port, PortClass, Wiring};
use novo_machine::drive_hoist_loop::DriveHoistModule;
use novo_machine::hoist_loop::HoistModule;
use novo_machine::travel_loop::TravelModule;

use super::crane_setup::CraneSetup;
use super::{drive_run, hoist_run};

fn kind_of(port: Port) -> PortKind {
    match port.class() {
        PortClass::Command => PortKind::Command,
        PortClass::Chain => PortKind::Chain,
        PortClass::Enable => PortKind::Enable,
    }
}

fn entry(prefix: &str, w: &Wiring) -> ModuleEntry {
    ModuleEntry {
        path: format!("{prefix}/{}", w.id),
        label: format!("{} {} ({})", prefix_word(prefix), w.id, w.kind.words()),
        ports: w
            .ports
            .iter()
            .map(|p| PortEntry {
                name: p.name().to_owned(),
                label: p.label().to_owned(),
                kind: kind_of(*p),
            })
            .collect(),
    }
}

fn prefix_word(prefix: &str) -> &'static str {
    match prefix {
        "bridge" => "Bridge",
        "trolley" => "Trolley",
        "travel" => "Travel axis",
        "winch" => "Winch",
        _ => "Hoist",
    }
}

/// The modules of the drawing's machine model and their ports. The travel axis the crane picture
/// uses for the bridge (or the trolley) is listed as `bridge/<tag>` (`trolley/<tag>`).
pub(super) fn catalog(project: &Project) -> MachineCatalog {
    let setup = CraneSetup::from_properties(&project.info().properties);
    let none = BTreeSet::new();
    let mut modules = Vec::new();
    for mut m in hoist_run::hoists_of(project).0 {
        modules.push(entry("hoist", &Wiring::hoist(&mut m, &none)));
    }
    for mut m in drive_run::drive_hoists_of(project).0 {
        modules.push(entry("hoist", &Wiring::drive_hoist(&mut m, &none)));
    }
    for mut m in drive_run::closed_winches_of(project).0 {
        modules.push(entry("winch", &Wiring::closed_winch(&mut m, &none)));
    }
    for mut m in drive_run::closed_pairs_of(project).0 {
        modules.push(entry("winch", &Wiring::closed_pair(&mut m, &none)));
    }
    for mut m in drive_run::axes_of(project).0 {
        let prefix = if setup.bridge_axis.as_deref() == Some(m.id.as_str()) {
            "bridge"
        } else if setup.trolley_axis.as_deref() == Some(m.id.as_str()) {
            "trolley"
        } else {
            "travel"
        };
        modules.push(entry(prefix, &Wiring::travel(&mut m, &none)));
    }
    modules.sort_by(|a, b| a.path.cmp(&b.path));
    MachineCatalog { modules }
}

/// The modules of the running loops, as the commands see them.
pub(super) struct Loops<'a> {
    pub hoists: &'a mut [HoistModule],
    pub drive_hoists: &'a mut [DriveHoistModule],
    pub winches: &'a mut [ClWinchModule],
    pub pairs: &'a mut [ClPairModule],
    pub axes: &'a mut [TravelModule],
}

/// The wiring of every module that has a bound port. A bound port that the module lacks is
/// ignored (the checks report it). Slots of the modules are filled with virtual tags for the bound
/// ports; unbound ports keep the drawn circuit.
pub(super) fn wirings(commands: &Commands, loops: Loops<'_>) -> Vec<Wiring> {
    let mut out = Vec::new();
    for m in loops.hoists.iter_mut() {
        let bound = commands.bound(&m.id);
        if !bound.is_empty() {
            out.push(Wiring::hoist(m, &bound));
        }
    }
    for m in loops.drive_hoists.iter_mut() {
        let bound = commands.bound(&m.id);
        if !bound.is_empty() {
            out.push(Wiring::drive_hoist(m, &bound));
        }
    }
    for m in loops.winches.iter_mut() {
        let bound = commands.bound(&m.id);
        if !bound.is_empty() {
            out.push(Wiring::closed_winch(m, &bound));
        }
    }
    for m in loops.pairs.iter_mut() {
        let bound = commands.bound(&m.id);
        if !bound.is_empty() {
            out.push(Wiring::closed_pair(m, &bound));
        }
    }
    for m in loops.axes.iter_mut() {
        let bound = commands.bound(&m.id);
        if !bound.is_empty() {
            out.push(Wiring::travel(m, &bound));
        }
    }
    out
}

/// One line per module a controller drives, for the run windows: "radio-1/H_UP_FAST up, ...".
pub(super) fn driver_line(drivers: &[(Port, novo_controller::SignalKey, bool)]) -> String {
    let parts: Vec<String> = drivers
        .iter()
        .map(|(port, key, high)| {
            format!(
                "{key} \u{2192} {}{}",
                port.name(),
                if *high { " (high)" } else { "" }
            )
        })
        .collect();
    parts.join(", ")
}
