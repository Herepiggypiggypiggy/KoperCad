//! An NFPA symbol is recognised the way its IEC counterpart is.

use novo_core::SymbolKey;
use novo_terminals::{is_earth_symbol, symbol_pins};

fn key(id: &str) -> SymbolKey {
    SymbolKey(id.into())
}

#[test]
fn earth_symbols_of_both_packs_earth_a_net() {
    for pack in ["iec", "nfpa"] {
        assert!(is_earth_symbol(&format!("{pack}/earth/earth")), "{pack}");
        assert!(
            is_earth_symbol(&format!("{pack}/earth/protective_earth")),
            "{pack}"
        );
        assert!(!is_earth_symbol(&format!("{pack}/earth/chassis")), "{pack}");
        assert!(
            !is_earth_symbol(&format!("{pack}/terminals/terminal")),
            "{pack}"
        );
    }
}

#[test]
fn terminal_pins_are_read_from_the_pack_of_the_key() {
    for pack in ["iec", "nfpa"] {
        for name in ["terminal", "terminal_pe", "terminal_n", "terminal_bridged"] {
            let pins = symbol_pins(&key(&format!("{pack}/terminals/{name}"))).unwrap();
            assert_eq!(pins.len(), 2, "{pack}/{name}");
        }
        assert!(symbol_pins(&key(&format!("{pack}/terminals/missing"))).is_err());
    }
}
