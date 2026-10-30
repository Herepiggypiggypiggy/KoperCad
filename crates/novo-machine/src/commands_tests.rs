//! Wiring of the drive-fed modules to their ports (see `commands`).

use std::collections::BTreeSet;

use crate::cl_winch_spec::closed_winch_from_properties;
use crate::commands::{CommandedCircuit, Commands, Port, Wiring};
use crate::drive_hoist_spec::drive_hoist_from_properties;
use crate::port::{CircuitPort, FakeCircuit};

fn get(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| {
        pairs
            .iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| (*v).to_owned())
    }
}

#[test]
fn a_virtual_tag_left_behind_by_an_unbound_port_reads_as_the_slot_did_before() {
    let mut inner = FakeCircuit::new();
    let circuit = CommandedCircuit::new(&mut inner, &[], &Commands::new());
    // an input with no device is off, a main contactor or safety channel with none is on
    assert!(!circuit.is_energised("~cmd:=M1:up"));
    assert!(circuit.is_energised("~on:=M1:permit"));
    assert!(circuit.knows_device("~on:=M1:permit"));
}

#[test]
fn a_drive_hoist_gets_its_inputs_from_the_ports() {
    let g = get(&[("dh_mass_kg", "200")]);
    let (mut m, _) = drive_hoist_from_properties("=M3", &g).unwrap().unwrap();
    let bound: BTreeSet<Port> = [Port::Up, Port::Fast, Port::Permit].into();
    let w = Wiring::drive_hoist(&mut m, &bound);
    for p in [Port::Up, Port::Down, Port::Fast, Port::Reset, Port::Permit] {
        assert!(w.has(p), "{p:?}");
    }
    assert_eq!(m.binding.up.as_deref(), Some("~cmd:=M3:up"));
    assert_eq!(m.binding.down, None, "an unbound port stays as drawn");
    assert_eq!(m.binding.speed_inputs[0].as_deref(), Some("~cmd:=M3:fast"));
    assert_eq!(m.binding.mains.as_deref(), Some("~on:=M3:permit"));
}

#[test]
fn a_closed_loop_winch_masks_its_safety_torque_off_with_the_permit() {
    let g = get(&[("cw_mass_kg", "200")]);
    let (mut m, _) = closed_winch_from_properties("=M4", &g).unwrap().unwrap();
    let bound: BTreeSet<Port> = [Port::Up, Port::Permit].into();
    let w = Wiring::closed_winch(&mut m, &bound);
    assert!(w.has(Port::Reset));
    assert_eq!(m.binding.sto[0].as_deref(), Some("~on:=M4:permit"));
    assert_eq!(m.binding.sto[1].as_deref(), Some("~on:=M4:permit"));
    let mut c = Commands::new();
    c.set("=M4", Port::Up, true);
    c.set("=M4", Port::Permit, false);
    let mut inner = FakeCircuit::new();
    let circuit = CommandedCircuit::new(&mut inner, std::slice::from_ref(&w), &c);
    assert!(!circuit.is_energised("~cmd:=M4:up"));
    assert!(!circuit.is_energised("~on:=M4:permit"));
    c.set("=M4", Port::Permit, true);
    let circuit = CommandedCircuit::new(&mut inner, &[w], &c);
    assert!(circuit.is_energised("~cmd:=M4:up"));
    assert!(
        circuit.is_energised("~on:=M4:permit"),
        "a closed chain lets the safety channels through"
    );
}
