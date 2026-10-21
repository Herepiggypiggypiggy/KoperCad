//! A fresh I/O list import is clean: the review dialog promises pages, and the
//! Issues panel must not answer with warnings for the field side it leaves open.

use novo_checks::{Severity, check};
use novo_core::Document;
use novo_plc::generate::{Options, apply, plan, read_list};
use novo_symbols::{Library, Pack};

const LIST: &str = "tag,description,signal type,address,station\n\
    S1,Start button,DI,E0.0,Pump\n\
    S2,Stop button,DI,E0.1,Pump\n\
    K1,Pump contactor,DO,A0.0,Pump\n\
    H1,Running lamp,DO,A0.1,Pump\n\
    B1,Level sensor,AI,EW64,Pump\n";

#[test]
fn a_fresh_import_shows_no_warnings_and_no_errors() {
    let library = Library::shared(Pack::Iec);
    let list = read_list(LIST.as_bytes()).unwrap();
    let mut doc = Document::default();
    let plan = plan(doc.project(), &list, &Options::default());
    doc.transact("Import I/O list", |tx| apply(tx, library, &list, &plan))
        .unwrap();
    let problems = check(doc.project());
    let loud: Vec<_> = problems
        .iter()
        .filter(|p| p.severity != Severity::Info)
        .map(|p| p.message.clone())
        .collect();
    assert!(loud.is_empty(), "{loud:#?}");
}
