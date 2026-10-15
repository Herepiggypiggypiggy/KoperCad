//! The packs agree with each other: a symbol and its counterpart differ only
//! in artwork (docs/design/symbol-packs.md).

use std::path::Path;

use novo_symbols::{Library, Pack, validate};

fn pack(pack: Pack) -> Library {
    Library::builtin_pack(pack).unwrap_or_else(|e| panic!("{} failed to load: {e}", pack.id()))
}

#[test]
fn every_pack_loads_and_ids_carry_the_pack_name() {
    for p in Pack::ALL {
        let lib = pack(p);
        assert!(
            lib.len() >= 100,
            "{} has only {} symbols",
            p.id(),
            lib.len()
        );
        for symbol in lib.iter() {
            assert_eq!(Pack::of(&symbol.id), Some(p), "{}", symbol.id);
            assert_eq!(symbol.standard, p.standard(), "{}", symbol.id);
            assert_eq!(validate(symbol), vec![], "{}", symbol.id);
        }
    }
}

#[test]
fn builtin_is_the_iec_pack() {
    assert_eq!(Library::builtin().unwrap().len(), pack(Pack::Iec).len());
}

#[test]
fn counterpart_swaps_the_pack_name() {
    assert_eq!(
        Pack::counterpart("iec/contacts/no", Pack::Nfpa).as_deref(),
        Some("nfpa/contacts/no")
    );
    assert_eq!(
        Pack::counterpart("nfpa/contacts/no", Pack::Iec).as_deref(),
        Some("iec/contacts/no")
    );
    assert_eq!(Pack::counterpart("acme/contacts/no", Pack::Iec), None);
}

#[test]
fn counterparts_agree_on_everything_but_artwork() {
    let iec = pack(Pack::Iec);
    let nfpa = pack(Pack::Nfpa);
    for symbol in nfpa.iter() {
        let Some(other) = Pack::counterpart(&symbol.id, Pack::Iec).and_then(|id| iec.get(&id))
        else {
            continue;
        };
        let id = &symbol.id;
        assert_eq!(
            symbol.function.split('.').next(),
            other.function.split('.').next(),
            "{id}"
        );
        assert_eq!(symbol.category, other.category, "{id}");
        assert_eq!(symbol.behavior, other.behavior, "{id}");
        assert_eq!(symbol.bridges, other.bridges, "{id}");
        let pins = |s: &novo_symbols::Symbol| {
            s.connections
                .iter()
                .map(|c| (c.id.clone(), c.role.clone(), c.optional))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pins(symbol),
            pins(other),
            "{id}: connection ids, order or roles differ"
        );
        assert_eq!(
            symbol.anchors.iter().map(|a| &a.role).collect::<Vec<_>>(),
            other.anchors.iter().map(|a| &a.role).collect::<Vec<_>>(),
            "{id}"
        );
    }
}

#[test]
fn nfpa_has_the_core_devices() {
    let nfpa = pack(Pack::Nfpa);
    for id in [
        "nfpa/contacts/no",
        "nfpa/contacts/nc",
        "nfpa/contacts/changeover",
        "nfpa/coils/relay",
        "nfpa/contactors/main_3p",
        "nfpa/operators/pushbutton_no",
        "nfpa/operators/pushbutton_nc",
        "nfpa/operators/emergency_stop",
        "nfpa/protection/fuse",
        "nfpa/protection/mcb_3p",
        "nfpa/protection/overload_relay",
        "nfpa/motors/motor_3ph",
        "nfpa/signalling/lamp",
        "nfpa/transformers/transformer_1ph",
        "nfpa/terminals/terminal",
        "nfpa/terminals/terminal_pe",
    ] {
        assert!(nfpa.get(id).is_some(), "{id} is missing");
    }
}

#[test]
fn aliases_work_in_both_packs() {
    for p in Pack::ALL {
        let lib = pack(p);
        for alias in ["no", "nc", "coil", "mcb3", "motor3", "ol"] {
            let symbol = lib
                .by_alias(alias)
                .unwrap_or_else(|| panic!("{alias} in {}", p.id()));
            assert_eq!(Pack::of(&symbol.id), Some(p));
        }
        assert!(lib.favorites().count() >= 15, "{}", p.id());
    }
}

#[test]
fn nfpa_contacts_are_bars_not_blades() {
    // Two bars across the conductor, 4 mm apart; the NC adds a slash.
    let nfpa = pack(Pack::Nfpa);
    let bars = |id: &str| {
        nfpa.get(id)
            .unwrap()
            .graphics
            .iter()
            .filter(|g| matches!(g, novo_symbols::Graphic::Line { from, to, .. } if from[1] == to[1] && (to[0] - from[0]).abs() > 5.0))
            .count()
    };
    assert_eq!(bars("nfpa/contacts/no"), 2);
    assert_eq!(bars("nfpa/contacts/nc"), 2);
    assert_eq!(nfpa.get("nfpa/contacts/nc").unwrap().graphics.len(), 5);
    assert_eq!(nfpa.get("nfpa/contacts/no").unwrap().graphics.len(), 4);
}

#[test]
fn directory_loader_reads_one_pack() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("symbols");
    for p in Pack::ALL {
        let from_dir = Library::load_dir_pack(&dir, p).unwrap();
        let builtin = pack(p);
        assert_eq!(from_dir.len(), builtin.len(), "{}", p.id());
        for symbol in builtin.iter() {
            assert_eq!(from_dir.get(&symbol.id), Some(symbol));
        }
    }
}

#[test]
fn every_nfpa_function_has_a_letter_code_like_its_counterpart() {
    let iec = pack(Pack::Iec);
    for symbol in pack(Pack::Nfpa).iter() {
        let other = Pack::counterpart(&symbol.id, Pack::Iec).and_then(|id| iec.get(&id));
        if let Some(other) = other {
            assert_eq!(
                novo_symbols::letter_code(&symbol.function),
                novo_symbols::letter_code(&other.function),
                "{}",
                symbol.id
            );
        }
    }
}

#[test]
fn path_of_ignores_the_pack() {
    assert_eq!(Pack::path_of("iec/contacts/no"), "contacts/no");
    assert_eq!(Pack::path_of("nfpa/contacts/no"), "contacts/no");
    assert_eq!(Pack::path_of("acme/contacts/no"), "acme/contacts/no");
    assert_eq!(Pack::path_of("no"), "no");
    assert!(Pack::same_device("iec/coils/relay", "nfpa/coils/relay"));
    assert!(!Pack::same_device(
        "iec/coils/relay",
        "nfpa/coils/contactor"
    ));
}

#[test]
fn a_library_knows_its_pack() {
    for p in Pack::ALL {
        assert_eq!(pack(p).pack(), Some(p));
    }
    assert_eq!(Library::default().pack(), None);
}
