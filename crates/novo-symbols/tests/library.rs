use std::collections::HashSet;
use std::path::Path;

use novo_symbols::{Library, Pack, SvgOptions, Symbol, to_svg, validate};

fn builtin() -> Library {
    Library::builtin().unwrap_or_else(|e| panic!("built-in library failed to load: {e}"))
}

#[test]
fn builtin_library_loads_and_validates() {
    let lib = builtin();
    assert!(lib.len() >= 145, "only {} symbols", lib.len());
    for symbol in lib.iter() {
        assert_eq!(validate(symbol), vec![], "{}", symbol.id);
    }
}

#[test]
fn directory_loader_matches_builtin() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("symbols");
    let from_dir = Library::load_dir_pack(&dir, Pack::Iec).unwrap();
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
kind = "contact"
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
    assert!(has("needs 'normally'"), "{messages:?}");
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
    assert_eq!(
        lib.get("iec/connectivity/bridge_4").unwrap().bridges(),
        vec![vec![0, 1, 2, 3]]
    );
    assert!(lib.get("iec/contacts/no").unwrap().bridges().is_empty());
    assert!(
        lib.get("iec/connectivity/interruption_point")
            .unwrap()
            .bridges()
            .is_empty()
    );
}

fn behavior_messages(behavior: &str) -> Vec<String> {
    let text = format!(
        r#"
format = 1
id = "iec/contacts/x"
name = "X"
standard = "IEC 60617"
category = "contacts"
function = "contact.no"
connections = [
  {{ id = "1", at = [0, 0], dir = "up" }},
  {{ id = "2", at = [0, 16], dir = "down" }},
  {{ id = "3", at = [8, 0], dir = "up" }},
]
anchors = [ {{ role = "tag", at = [-6, 8], align = "right" }} ]
graphics = [
  {{ kind = "line", from = [0, 0], to = [0, 16] }},
  {{ kind = "line", from = [8, 0], to = [8, 4] }},
]
{behavior}
"#
    );
    let symbol = Symbol::from_toml(&text).unwrap();
    validate(&symbol).into_iter().map(|i| i.message).collect()
}

#[test]
fn rejects_malformed_poles() {
    let m = behavior_messages(
        "[behavior]\nkind = \"conductor\"\npoles = [[\"1\", 2], [\"1\", \"2\"], [\"2\"]]",
    );
    let has = |needle: &str| m.iter().any(|s| s.contains(needle));
    assert!(has("must be a list of lists"), "{m:?}");
    assert!(has("more than once"), "{m:?}");
    assert!(has("needs at least 2 connections"), "{m:?}");
}

#[test]
fn rejects_unknown_behavior_keys_and_values() {
    let m = behavior_messages(
        "[behavior]\nkind = \"contact\"\nnormaly = \"open\"\npoles = [[\"1\", \"2\"]]",
    );
    assert!(
        m.iter().any(|s| s.contains("no parameter 'normaly'")),
        "{m:?}"
    );
    assert!(m.iter().any(|s| s.contains("needs 'normally'")), "{m:?}");

    let m = behavior_messages(
        "[behavior]\nkind = \"contact\"\nnormally = \"sometimes\"\npoles = [[\"1\", \"2\"]]",
    );
    assert!(m.iter().any(|s| s.contains("behavior.normally")), "{m:?}");

    let m = behavior_messages("[behavior]\nkind = \"teleport\"");
    assert!(
        m.iter().any(|s| s.contains("unknown behavior kind")),
        "{m:?}"
    );
}

#[test]
fn changeover_poles_have_three_connections() {
    let ok = behavior_messages(
        "[behavior]\nkind = \"contact\"\nnormally = \"changeover\"\npoles = [[\"2\", \"1\", \"3\"]]",
    );
    assert_eq!(ok, Vec::<String>::new());
}

#[test]
fn top_level_bridges_join_connections_of_any_kind() {
    let lib = builtin();
    let vfd = lib.get("iec/drives/frequency_converter").unwrap();
    let pe = vfd.connections.iter().position(|c| c.id == "PE").unwrap();
    let pe2 = vfd.connections.iter().position(|c| c.id == "PE2").unwrap();
    assert_eq!(vfd.bridges(), vec![vec![pe, pe2]]);

    let text = "bridges = [[\"1\", \"9\"], [\"1\", \"2\"]]\n[behavior]\nkind = \"none\"";
    let m = behavior_messages(text);
    assert!(
        m.iter().any(|s| s.contains("unknown connection '9'")),
        "{m:?}"
    );
    assert!(
        m.iter().any(|s| s.contains("more than one bridges group")),
        "{m:?}"
    );
}

#[test]
fn rejects_non_finite_sizes() {
    let text = r#"
format = 1
id = "iec/contacts/x"
name = "X"
standard = "IEC 60617"
category = "contacts"
function = "x"
connections = []
graphics = [ { kind = "rect", at = [0, 0], size = [inf, 1] } ]
[behavior]
kind = "none"
"#;
    let symbol = Symbol::from_toml(text).unwrap();
    assert!(!validate(&symbol).is_empty());
}

#[test]
fn library_rejects_duplicates_and_lenient_load_keeps_good_symbols() {
    let good = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("symbols/iec/contacts/no.toml"),
    )
    .unwrap();
    let bad = good.replace("kind = \"contact\"", "kind = \"teleport\"");
    let dup = Library::from_sources([
        ("iec/contacts/no".to_string(), good.as_str()),
        ("iec/contacts/no".to_string(), good.as_str()),
    ]);
    assert!(matches!(
        dup,
        Err(novo_symbols::LoadError::Duplicate { .. })
    ));

    let other = good.replace("iec/contacts/no", "iec/contacts/other");
    let (lib, problems) = Library::from_sources_lenient([
        ("iec/contacts/other".to_string(), other.as_str()),
        ("iec/contacts/no".to_string(), bad.as_str()),
    ]);
    assert_eq!(lib.len(), 1);
    assert!(lib.get("iec/contacts/other").is_some());
    assert_eq!(problems.len(), 1);
}

#[test]
fn every_behavior_kind_has_a_schema() {
    for kind in novo_symbols::BEHAVIOR_KINDS {
        assert!(novo_symbols::behavior_keys(kind).is_some(), "{kind}");
    }
}

#[test]
fn overload_main_circuit_is_permanently_connected() {
    let lib = builtin();
    let ol = lib.get("iec/protection/overload_relay").unwrap();
    assert_eq!(ol.bridges(), vec![vec![0, 1], vec![2, 3], vec![4, 5]]);
    assert_eq!(ol.pass_through().paths(), ol.bridges().as_slice());
}

#[test]
fn switching_devices_pass_potentials_but_loads_do_not() {
    let lib = builtin();
    for id in [
        "iec/contactors/main_3p",
        "iec/protection/motor_protection_breaker",
        "iec/protection/mcb_3p",
        "iec/protection/fuse_3p",
        "iec/switches/main_switch_3p",
    ] {
        let s = lib.get(id).unwrap();
        assert!(s.bridges().is_empty(), "{id} must not be bridged");
        assert_eq!(
            s.pass_through().paths(),
            [vec![0, 1], vec![2, 3], vec![4, 5]],
            "{id}"
        );
    }
    for id in [
        "iec/coils/relay",
        "iec/signalling/lamp",
        "iec/motors/motor_3ph",
    ] {
        assert!(
            lib.get(id).unwrap().pass_through().paths().is_empty(),
            "{id}"
        );
    }
}

#[test]
fn series_elements_that_always_conduct_are_bridged() {
    let lib = builtin();
    let pairs = |id: &str| -> Vec<Vec<String>> {
        let s = lib.get(id).unwrap();
        s.bridges()
            .iter()
            .map(|g| g.iter().map(|&i| s.connections[i].id.clone()).collect())
            .collect()
    };
    assert_eq!(pairs("iec/measuring/current_transformer"), [["P1", "P2"]]);
    assert_eq!(pairs("iec/measuring/ammeter"), [["1", "2"]]);
    assert!(pairs("iec/measuring/voltmeter").is_empty());
    assert_eq!(pairs("iec/transformers/reactor_3ph").len(), 3);
    assert_eq!(pairs("iec/transformers/emc_filter").len(), 4);
}

#[test]
fn changeover_passes_through_from_the_common_side_only() {
    let lib = builtin();
    let co = lib.get("iec/contacts/changeover").unwrap();
    // connections: 11 (common), 12 (break), 14 (make)
    assert_eq!(co.pass_through().paths(), [vec![0, 1], vec![0, 2]]);
}

#[test]
fn palette_aliases_find_common_symbols() {
    let lib = builtin();
    for (alias, id) in [
        ("no", "iec/contacts/no"),
        ("nc", "iec/contacts/nc"),
        ("coil", "iec/coils/relay"),
        ("mcb3", "iec/protection/mcb_3p"),
        ("motor3", "iec/motors/motor_3ph"),
        ("x", "iec/terminals/terminal_end"),
        ("term", "iec/terminals/terminal"),
        ("br2", "iec/connectivity/bridge_2"),
    ] {
        assert_eq!(
            lib.by_alias(alias).map(|s| s.id.as_str()),
            Some(id),
            "{alias}"
        );
    }
    assert_eq!(
        lib.by_alias(" MCB3 ").map(|s| s.id.as_str()),
        Some("iec/protection/mcb_3p")
    );
    assert!(lib.by_alias("nothing").is_none());

    let favorites: Vec<_> = lib.favorites().map(|s| s.id.as_str()).collect();
    assert_eq!(favorites.len(), 20, "{favorites:?}");
    assert!(favorites.contains(&"iec/terminals/terminal_end"));
    assert_eq!(lib.by_function("accessory.terminal").count(), 2);
}

#[test]
fn aliases_are_checked_and_unique_per_library() {
    let no = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("symbols/iec/contacts/no.toml"),
    )
    .unwrap();
    let nc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("symbols/iec/contacts/nc.toml"),
    )
    .unwrap()
    .replace("aliases = [\"nc\"]", "aliases = [\"no\"]");
    let clash = Library::from_sources([
        ("iec/contacts/no".to_string(), no.as_str()),
        ("iec/contacts/nc".to_string(), nc.as_str()),
    ]);
    assert!(
        matches!(&clash, Err(novo_symbols::LoadError::DuplicateAlias { alias, .. }) if alias == "no"),
        "{clash:?}"
    );

    for bad in [
        "\"NO\"",
        "\"\"",
        "\"make contact\"",
        "\"averyveryverylongalias\"",
    ] {
        let text = no.replace("aliases = [\"no\"]", &format!("aliases = [{bad}]"));
        let symbol = Symbol::from_toml(&text).unwrap();
        assert!(
            validate(&symbol)
                .iter()
                .any(|i| i.message.contains("alias")),
            "{bad}"
        );
    }
    let twice = no.replace("aliases = [\"no\"]", "aliases = [\"no\", \"no\"]");
    let symbol = Symbol::from_toml(&twice).unwrap();
    assert!(
        validate(&symbol)
            .iter()
            .any(|i| i.message.contains("twice"))
    );
}

/// Every clamp of a terminal strip is drawn the same way, so a strip of 400
/// terminals of one type looks uniform whatever variant each one is.
#[test]
fn terminal_clamps_share_one_drawing() {
    use novo_symbols::Graphic;
    let lib = builtin();
    let terminals: Vec<_> = lib.by_function("terminal").collect();
    assert!(terminals.len() >= 6);
    for symbol in terminals {
        let clamps: Vec<_> = symbol
            .graphics
            .iter()
            .filter_map(|g| match g {
                Graphic::Circle { center, r, .. } => Some((*center, *r)),
                _ => None,
            })
            .collect();
        assert!(!clamps.is_empty(), "{} has no clamp", symbol.id);
        for (center, r) in clamps {
            assert_eq!(r, 1.2, "{}", symbol.id);
            // A one-ended terminal has its clamp at the end of the one wire.
            if let [only] = symbol.connections.as_slice() {
                assert!(
                    only.at[0] == center[0] && only.at[1] < center[1],
                    "{}: the clamp at {center:?} is not at the end of the wire",
                    symbol.id
                );
                continue;
            }
            // Each clamp sits on a conductor, between two of its connections.
            let xs: Vec<_> = symbol
                .connections
                .iter()
                .filter(|c| c.at[0] == center[0])
                .collect();
            assert!(
                xs.len() >= 2,
                "{}: clamp at {center:?} is off the conductor",
                symbol.id
            );
            assert!(
                xs.iter().any(|c| c.at[1] < center[1]) && xs.iter().any(|c| c.at[1] > center[1]),
                "{}: clamp at {center:?} is not between its connections",
                symbol.id
            );
        }
        let pitch: Vec<_> = symbol.connections.iter().map(|c| c.at[1]).collect();
        assert!(
            pitch.iter().all(|&y| y == 0.0 || y == 8.0 || y == 16.0),
            "{}: {pitch:?}",
            symbol.id
        );
    }
}

#[test]
fn changeover_switch_has_an_off_position_and_strips_have_end_pieces() {
    let lib = builtin();
    let switch = lib.get("iec/switches/changeover_switch_3p").unwrap();
    assert_eq!(
        switch
            .behavior
            .params
            .get("center_off")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(
        behavior_messages("[behavior]\nkind = \"contact\"\nnormally = \"open\"\ncenter_off = true\npoles = [[\"1\", \"2\"]]")
            .iter()
            .any(|m| m.contains("center_off only applies"))
    );
    for (alias, id) in [
        ("term", "iec/terminals/terminal"),
        ("endplate", "iec/terminals/end_plate"),
        ("NO", "iec/contacts/no"),
        ("endstop", "iec/terminals/end_bracket"),
    ] {
        assert_eq!(
            lib.by_alias(alias).map(|s| s.id.as_str()),
            Some(id),
            "{alias}"
        );
    }
}
