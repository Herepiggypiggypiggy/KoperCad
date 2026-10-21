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
    // Nor does the field side the import leaves open show up as a note at any
    // level: what is left are the "has no part" hints.
    let open: Vec<_> = problems
        .iter()
        .filter(|p| {
            let m = p.message.to_lowercase();
            m.contains("connect") || m.contains("open") || m.contains("free")
        })
        .map(|p| p.message.clone())
        .collect();
    assert!(open.is_empty(), "{open:#?}");
}

#[test]
fn the_io_list_report_shows_the_drawn_spares_as_spares() {
    use novo_reports::{IoList, Report};
    let library = Library::shared(Pack::Iec);
    let list = read_list(LIST.as_bytes()).unwrap();
    let mut doc = Document::default();
    let plan = plan(doc.project(), &list, &Options::default());
    doc.transact("Import I/O list", |tx| apply(tx, library, &list, &plan))
        .unwrap();
    let output = IoList.generate(doc.project());
    let section = &output.sections[0];
    let col = |key: &str| section.column(key).expect(key);
    let rows = section.text_rows();
    let (drawn, spare): (Vec<_>, Vec<_>) = rows.iter().partition(|r| !r[col("drawn")].is_empty());
    assert_eq!(drawn.len(), 5, "{rows:#?}");
    assert!(!spare.is_empty());
    // A spare symbol is on the page, but the list reads it as a spare: no pin,
    // no page, the word "Spare" in the signal column, and not once on a drawn row.
    for row in &spare {
        assert_eq!(row[col("signal")], "Spare", "{row:?}");
        assert!(row[col("pin")].is_empty(), "{row:?}");
    }
    assert!(
        drawn.iter().all(|r| r[col("signal")] != "Spare"),
        "{drawn:#?}"
    );
}
