use std::collections::HashSet;
use std::path::Path;

use novo_symbols::{Library, SvgOptions, Symbol, to_svg, validate};

fn builtin() -> Library {
    Library::builtin().unwrap_or_else(|e| panic!("built-in library failed to load: {e}"))
}

#[test]
fn builtin_library_loads_and_validates() {
    let lib = builtin();
    assert!(lib.len() >= 120, "only {} symbols", lib.len());
    for symbol in lib.iter() {
        assert_eq!(validate(symbol), vec![], "{}", symbol.id);
    }
}

#[test]
fn directory_loader_matches_builtin() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("symbols");
    let from_dir = Library::load_dir(&dir).unwrap();
    let builtin = builtin();
    assert_eq!(from_dir.len(), builtin.len());
    for symbol in builtin.iter() {
        assert_eq!(from_dir.get(&symbol.id), Some(symbol));
    }
}

#[test]
fn core_set_is_present() {
    let lib = builtin();
    for id in [
        "iec/contacts/no",
        "iec/contacts/nc",
        "iec/contacts/changeover",
        "iec/coils/relay",
        "iec/contactors/main_3p",
        "iec/protection/fuse",
        "iec/protection/mcb_1p",
        "iec/protection/mcb_3p",
        "iec/protection/motor_protection_breaker",
        "iec/protection/overload_relay",
        "iec/contacts/thermal_nc",
        "iec/operators/pushbutton_no",
        "iec/operators/pushbutton_nc",
        "iec/operators/emergency_stop",
        "iec/motors/motor_3ph",
        "iec/signalling/lamp",
        "iec/plc/digital_input",
        "iec/plc/digital_output",
        "iec/terminals/terminal",
        "iec/terminals/terminal_pe",
    ] {
        assert!(lib.get(id).is_some(), "missing {id}");
    }
}

#[test]
fn coil_and_contacts_use_en_50005_numbers() {
    let lib = builtin();
    let ids = |id: &str| -> Vec<String> {
        lib.get(id)
            .unwrap()
            .connections
            .iter()
            .map(|c| c.id.clone())
            .collect()
    };
    assert_eq!(ids("iec/coils/relay"), ["A1", "A2"]);
    assert_eq!(ids("iec/contacts/no"), ["13", "14"]);
    assert_eq!(ids("iec/contacts/nc"), ["21", "22"]);
    assert_eq!(ids("iec/contacts/thermal_nc"), ["95", "96"]);
    assert_eq!(
        ids("iec/contactors/main_3p"),
        ["1", "2", "3", "4", "5", "6"]
    );
    assert_eq!(ids("iec/motors/motor_3ph"), ["U1", "V1", "W1", "PE"]);
}

#[test]
fn every_connection_lies_on_a_wire_axis_through_its_direction() {
    // A wire leaves each point in its direction; nothing in the symbol may
    // sit on the first grid step of that path, or the wire would overlap
    // the artwork.
    for symbol in builtin().iter() {
        let (min, max) = symbol.bounds();
        for c in &symbol.connections {
            let v = c.dir.vector();
            let outside = [c.at[0] + v[0] * 0.01, c.at[1] + v[1] * 0.01];
            let inside_x = outside[0] > min[0] && outside[0] < max[0];
            let inside_y = outside[1] > min[1] && outside[1] < max[1];
            let leaves = match c.dir {
                novo_symbols::Direction::Up => outside[1] < min[1] || !inside_x,
                novo_symbols::Direction::Down => outside[1] > max[1] || !inside_x,
                novo_symbols::Direction::Left => outside[0] < min[0] || !inside_y,
                novo_symbols::Direction::Right => outside[0] > max[0] || !inside_y,
            };
            assert!(
                leaves,
                "{}: connection {} does not face out of the symbol",
                symbol.id, c.id
            );
        }
    }
}

#[test]
fn function_lookup_matches_prefixes() {
    let lib = builtin();
    let contacts: HashSet<_> = lib
        .by_function("contact.no")
        .map(|s| s.id.as_str())
        .collect();
    assert!(contacts.contains("iec/contacts/no"));
    assert!(contacts.contains("iec/operators/pushbutton_no"));
    assert!(!contacts.contains("iec/contacts/nc"));
    assert!(lib.by_function("contact.n").next().is_none());
}

#[test]
fn round_trips_through_toml() {
    for symbol in builtin().iter() {
        let text = toml::to_string(symbol).unwrap();
        let back = Symbol::from_toml(&text).unwrap();
        assert_eq!(&back, symbol);
    }
}

#[test]
fn renders_svg() {
    let lib = builtin();
    let svg = to_svg(
        lib.get("iec/motors/motor_3ph").unwrap(),
        &SvgOptions::default(),
    );
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">M</text>"));
    assert!(svg.contains(">U1</text>"));
}

#[test]
fn rejects_bad_symbols() {
    let text = r#"
format = 1
id = "iec/contacts/bad"
name = "Bad"
standard = "IEC 60617"
category = "relays"
function = "contact.no"
connections = [
  { id = "1", at = [0, 0], dir = "up" },
  { id = "1", at = [1, 16], dir = "down" },
]
anchors = []
graphics = [ { kind = "line", from = [0, 0], to = [0, 5] } ]

[behavior]
kind = "teleport"
poles = [["1", "9"]]
"#;
    let symbol = Symbol::from_toml(text).unwrap();
    let messages: Vec<_> = validate(&symbol).into_iter().map(|i| i.message).collect();
    let has = |needle: &str| messages.iter().any(|m| m.contains(needle));
    assert!(has("category"), "{messages:?}");
    assert!(has("defined twice"), "{messages:?}");
    assert!(has("not on the 4 mm grid"), "{messages:?}");
    assert!(has("not the end of a line"), "{messages:?}");
    assert!(has("no 'tag' anchor"), "{messages:?}");
    assert!(has("unknown behavior kind"), "{messages:?}");
    assert!(has("unknown connection '9'"), "{messages:?}");
}

#[test]
fn rejects_unknown_fields() {
    let text = r#"
format = 1
id = "iec/contacts/x"
name = "X"
standard = "IEC 60617"
category = "contacts"
function = "contact.no"
colour = "red"
connections = []
graphics = []
[behavior]
kind = "none"
"#;
    assert!(Symbol::from_toml(text).is_err());
    let text = text.replace("colour = \"red\"\n", "").replace(
        "graphics = []",
        "graphics = [ { kind = \"line\", from = [0, 0], to = [0, 1], width = 2 } ]",
    );
    assert!(Symbol::from_toml(&text).is_err());
}

#[test]
fn bridges_join_terminal_sides_but_not_contacts() {
    let lib = builtin();
    assert_eq!(
        lib.get("iec/terminals/terminal").unwrap().bridges(),
        vec![vec![0, 1]]
    );
    assert_eq!(
        lib.get("iec/terminals/terminal_2level").unwrap().bridges(),
        vec![vec![0, 1], vec![2, 3]]
    );
    assert!(lib.get("iec/contacts/no").unwrap().bridges().is_empty());
    assert!(
        lib.get("iec/connectivity/interruption_point")
            .unwrap()
            .bridges()
            .is_empty()
    );
}
