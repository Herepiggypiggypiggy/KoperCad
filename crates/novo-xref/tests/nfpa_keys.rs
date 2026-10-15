//! Contacts of the NFPA pack are classified like their IEC twins, by key
//! and by function.

use novo_core::SymbolKey;
use novo_symbols::{Library, Pack};
use novo_xref::ContactKind;

#[test]
fn every_nfpa_contact_is_classified_like_its_iec_twin() {
    let iec = Library::builtin().unwrap();
    let nfpa = Library::builtin_pack(Pack::Nfpa).unwrap();
    let mut checked = 0;
    for symbol in nfpa.iter() {
        let Some(twin) = Pack::counterpart(&symbol.id, Pack::Iec).and_then(|id| iec.get(&id))
        else {
            continue;
        };
        let key = |s: &novo_symbols::Symbol| SymbolKey(s.id.clone());
        assert_eq!(
            ContactKind::classify(None, &key(symbol)),
            ContactKind::classify(None, &key(twin)),
            "{} by key",
            symbol.id
        );
        assert_eq!(
            ContactKind::classify(Some(&symbol.function), &key(symbol)),
            ContactKind::classify(Some(&twin.function), &key(twin)),
            "{} by function",
            symbol.id
        );
        checked += 1;
    }
    assert!(checked > 100);
}
