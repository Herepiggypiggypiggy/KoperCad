//! Text conflict baseline (docs/TEXT-PLACEMENT.md, stage 0): how many labels
//! cover other labels, symbols or wires on the Gate 1 motor starter with the
//! real IEC artwork. The numbers are a ratchet: a stage that improves
//! placement lowers `BASELINE`, and nothing may raise it.

use novo_canvas::model::{ModelStyle, PageScene};
use novo_canvas::text_audit::audit;
use novo_core::Document;
use novo_symbols::Library;
use novo_tests::motor_starter;

/// (text on text, text on symbol, text on wire) summed over both pages, as
/// measured at the start of stage 0.
const BASELINE: (usize, usize, usize) = (0, 1, 1);

#[test]
fn motor_starter_text_conflicts_do_not_grow() {
    let library = Library::builtin().unwrap();
    let mut doc = Document::default();
    let m = motor_starter(&mut doc).unwrap();
    let p = doc.project();
    let style = ModelStyle::default();
    let art = novo_editor::art::LibraryArt::new(&library, style.symbol);
    let mut sum = (0, 0, 0);
    for page in [m.main_page, m.control_page] {
        let a = audit(PageScene::new(p, page, &art).scene());
        assert!(a.labels > 0);
        for line in a.describe() {
            eprintln!("page {page:?}: {line}");
        }
        sum.0 += a.text_on_text.len();
        sum.1 += a.text_on_ink.len();
        sum.2 += a.text_on_wire.len();
    }
    eprintln!("text conflicts (text, symbol, wire): {sum:?}");
    assert!(sum.0 <= BASELINE.0 && sum.1 <= BASELINE.1 && sum.2 <= BASELINE.2);
}
