//! The insert search works on the NFPA pack, including favourites and recents
//! saved with IEC ids.

use novo_symbols::{Library, Pack};
use novo_workflow::insert::{InsertSearch, mnemonic};

fn nfpa() -> Library {
    Library::builtin_pack(Pack::Nfpa).unwrap()
}

#[test]
fn default_favourites_resolve_to_the_nfpa_pack() {
    let lib = nfpa();
    let search = InsertSearch::default();
    assert_eq!(search.favourite(&lib, 1).unwrap().id, "nfpa/contacts/no");
    assert_eq!(
        search.favourite(&lib, 4).unwrap().id,
        "nfpa/contactors/main_3p"
    );
    let empty: Vec<_> = search
        .search(&lib, "")
        .iter()
        .map(|h| h.symbol.id.clone())
        .collect();
    assert_eq!(empty.len(), 9);
    assert!(empty.iter().all(|id| id.starts_with("nfpa/")));
}

#[test]
fn a_recent_iec_symbol_ranks_its_nfpa_twin_first() {
    let lib = nfpa();
    let mut search = InsertSearch::default();
    search.used("iec/signalling/lamp");
    let empty = search.search(&lib, "");
    assert_eq!(empty[0].symbol.id, "nfpa/signalling/lamp");
}

#[test]
fn mnemonics_find_nfpa_symbols() {
    let lib = nfpa();
    assert_eq!(mnemonic(&lib, "no").unwrap().id, "nfpa/contacts/no");
    assert_eq!(mnemonic(&lib, "mcb3").unwrap().id, "nfpa/protection/mcb_3p");
    let hits = InsertSearch::default().search(&lib, "pb");
    assert_eq!(hits[0].symbol.id, "nfpa/operators/pushbutton_no");
}
