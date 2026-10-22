//! Safety, PLC and network symbols: pins, functions, letters and search
//! words (findings RM-04 and RM-20 of the real-machine build).

use novo_symbols::{Library, letter_code};

fn lib() -> Library {
    Library::builtin().unwrap()
}

fn pins(id: &str) -> Vec<String> {
    lib()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .connections
        .iter()
        .map(|c| c.id.clone())
        .collect()
}

#[test]
fn safety_relay_has_supply_channels_reset_and_outputs() {
    assert_eq!(
        pins("iec/safety/relay"),
        [
            "A1", "S34", "S11", "S12", "S21", "S22", "A2", "13", "14", "23", "24", "33", "34",
            "41", "42"
        ]
    );
    assert_eq!(
        pins("iec/safety/relay_expander"),
        [
            "A1", "Y1", "Y2", "A2", "13", "14", "23", "24", "33", "34", "43", "44"
        ]
    );
}

#[test]
fn two_channel_devices_have_two_contacts_and_use_en_50005_numbers() {
    assert_eq!(
        pins("iec/safety/emergency_stop_2ch"),
        ["11", "12", "21", "22"]
    );
    assert_eq!(pins("iec/safety/door_switch_nc"), ["11", "12", "21", "22"]);
    assert_eq!(pins("iec/safety/door_switch_no"), ["33", "34"]);
    let lib = lib();
    for id in ["iec/safety/emergency_stop_2ch", "iec/safety/door_switch_nc"] {
        let s = lib.get(id).unwrap();
        assert_eq!(s.behavior.kind, "contact", "{id}");
        assert_eq!(s.pass_through().paths().len(), 2, "{id}: two channels");
    }
}

#[test]
fn light_curtain_pair_and_plc_and_network_pins() {
    assert_eq!(pins("iec/safety/light_curtain_tx"), ["L+", "M"]);
    assert_eq!(
        pins("iec/safety/light_curtain_rx"),
        ["L+", "M", "RES", "OSSD1", "OSSD2"]
    );
    assert_eq!(
        pins("iec/plc/cpu"),
        ["L+", "M", "PE", "P1", "P2", "A", "B", "SH"]
    );
    assert_eq!(pins("iec/plc/remote_io"), ["L+", "M", "PE", "P1", "P2"]);
    assert_eq!(
        pins("iec/network/ethernet_switch_5"),
        ["P1", "P2", "P3", "P4", "P5", "L+", "M"]
    );
    assert_eq!(pins("iec/network/ethernet_switch_8").len(), 10);
    assert_eq!(pins("iec/network/port_ethernet"), ["P1"]);
    assert_eq!(pins("iec/network/port_ethernet_2p"), ["P1", "P2"]);
    assert_eq!(pins("iec/network/port_rs485"), ["A", "B", "SH"]);
    assert_eq!(pins("iec/network/port_can"), ["CH", "CL", "CG"]);
    assert_eq!(pins("iec/network/bus_terminator"), ["A", "B"]);
}

#[test]
fn protective_earth_pins_carry_the_pe_role() {
    let lib = lib();
    for id in ["iec/plc/cpu", "iec/plc/remote_io"] {
        let pe = lib
            .get(id)
            .unwrap()
            .connections
            .iter()
            .find(|c| c.id == "PE")
            .unwrap();
        assert_eq!(pe.role.as_deref(), Some("pe"), "{id}");
    }
}

#[test]
fn function_types_and_letters() {
    let lib = lib();
    let ids = |f: &str| -> Vec<String> { lib.by_function(f).map(|s| s.id.clone()).collect() };
    assert_eq!(ids("safety.relay").len(), 2);
    assert_eq!(ids("safety.lightcurtain").len(), 2);
    assert_eq!(ids("network.switch").len(), 2);
    assert_eq!(ids("network.port").len(), 4);
    assert_eq!(ids("plc.cpu"), ["iec/plc/cpu"]);
    // The safety contacts are also found as ordinary break and make contacts.
    assert!(ids("contact.nc.estop").contains(&"iec/safety/emergency_stop_2ch".to_string()));
    assert!(ids("contact.nc.interlock").contains(&"iec/safety/door_switch_nc".to_string()));
    assert_eq!(letter_code("safety.relay"), Some("K"));
    assert_eq!(letter_code("safety.lightcurtain.rx"), Some("B"));
    assert_eq!(letter_code("contact.nc.interlock"), Some("S"));
    assert_eq!(letter_code("plc.cpu"), Some("K"));
    assert_eq!(letter_code("network.switch.ethernet"), Some("A"));
    assert_eq!(letter_code("network.terminator"), Some("R"));
}

#[test]
fn quick_insert_aliases_work() {
    let lib = lib();
    for (alias, id) in [
        ("sr", "iec/safety/relay"),
        ("estop2", "iec/safety/emergency_stop_2ch"),
        ("lcrx", "iec/safety/light_curtain_rx"),
        ("cpu", "iec/plc/cpu"),
        ("sw5", "iec/network/ethernet_switch_5"),
        ("rs485", "iec/network/port_rs485"),
    ] {
        assert_eq!(
            lib.by_alias(alias).map(|s| s.id.as_str()),
            Some(id),
            "{alias}"
        );
    }
}
