//! Contacts of the NFPA pack are classified like their IEC twins.

use novo_checks::ContactKind;
use novo_core::SymbolKey;
use novo_symbols::{Library, Pack};

#[test]
fn every_nfpa_symbol_is_classified_like_its_iec_twin() {
    let iec = Library::builtin().unwrap();
    let nfpa = Library::builtin_pack(Pack::Nfpa).unwrap();
    for symbol in nfpa.iter() {
        let Some(twin) = Pack::counterpart(&symbol.id, Pack::Iec).and_then(|id| iec.get(&id))
        else {
            continue;
        };
        assert_eq!(
            ContactKind::of_key(&SymbolKey(symbol.id.clone())),
            ContactKind::of_key(&SymbolKey(twin.id.clone())),
            "{}",
            symbol.id
        );
    }
}
