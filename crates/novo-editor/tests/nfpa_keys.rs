//! The editor works with an NFPA catalog the way it does with the IEC one.

use novo_editor::{Catalog, potential, row};
use novo_symbols::Pack;

#[test]
fn an_nfpa_catalog_has_favourites_and_a_potential_feed() {
    let catalog = Catalog::builtin_pack(Pack::Nfpa);
    assert_eq!(catalog.favourites().len(), 9);
    assert!(catalog.favourites().iter().all(|f| f.starts_with("nfpa/")));
    assert!(
        catalog
            .get(&catalog.in_pack(potential::FEED_SYMBOL))
            .is_some()
    );
    assert_eq!(catalog.in_pack("iec/contacts/no"), "nfpa/contacts/no");
    let iec = Catalog::builtin();
    assert_eq!(iec.in_pack("iec/contacts/no"), "iec/contacts/no");
}

#[test]
fn mnemonics_find_the_nfpa_symbol() {
    let catalog = Catalog::builtin_pack(Pack::Nfpa);
    for (word, id) in [
        ("no", "nfpa/contacts/no"),
        ("coil", "nfpa/coils/relay"),
        ("mcb3", "nfpa/protection/mcb_3p"),
    ] {
        let hits = catalog.search(word);
        assert_eq!(hits.first().map(|m| m.symbol.as_str()), Some(id), "{word}");
    }
}

#[test]
fn nfpa_one_pole_symbols_have_pole_families() {
    for id in [
        "nfpa/protection/mcb_1p",
        "nfpa/protection/fuse",
        "nfpa/contactors/main_1p",
    ] {
        let family = row::pole_family(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(family.len(), 4);
        assert!(family.iter().all(|f| f.starts_with("nfpa/")));
    }
}
