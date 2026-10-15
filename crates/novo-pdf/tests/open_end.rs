//! An open wire end prints its ring: on the canvas the ring is a
//! screen-size handle, so the export draws it itself.

use lopdf::Document as Pdf;
use novo_core::{DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole};
use novo_pdf::{PdfOptions, export_pdf};

/// Bezier curve operators on the first page: the ring is a circle.
fn curves(doc: &Document) -> usize {
    let bytes = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let pdf = Pdf::load_mem(&bytes).unwrap();
    let id = pdf.get_pages()[&1];
    let content = String::from_utf8_lossy(&pdf.get_page_content(id)).into_owned();
    content
        .lines()
        .filter(|l| l.trim_end().ends_with(" c"))
        .count()
}

fn page_with_wire(open: bool) -> Document {
    let mut doc = Document::default();
    doc.transact("wire", |tx| {
        let page = tx.add_page("1")?;
        let strip = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
        let mut pins = Vec::new();
        for (i, x) in [100.0, 160.0].into_iter().enumerate() {
            let t = tx.add_terminal(strip, (i + 1).to_string())?;
            pins.push(tx.place_symbol(
                page,
                "iec/terminal",
                Placement::at(Point::new(x, 50.0)),
                vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                SymbolRole::Terminal { terminals: vec![t] },
            )?);
        }
        let to = if open {
            Endpoint::Free {
                at: Point::new(130.0, 20.0),
            }
        } else {
            Endpoint::pin(pins[1], 0)
        };
        tx.connect(Endpoint::pin(pins[0], 0), to)?;
        Ok(())
    })
    .unwrap();
    doc
}

#[test]
fn an_open_end_prints_a_ring() {
    // Each circle is four curves. The open wire has one dot (on its pin) and
    // the ring; the closed wire has a dot on each pin. Without the ring the
    // open page would have half the curves of the closed one.
    let open = curves(&page_with_wire(true));
    let closed = curves(&page_with_wire(false));
    assert_eq!(open, closed, "ring replaces the second dot");
}
