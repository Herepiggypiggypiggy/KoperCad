//! The Pages list at a thousand pages with a wire to nearly every symbol
//! (the Windows hardware test, HPR-04): filtering, editing and deleting a
//! page. Debug builds run a fifth of the project; the budgets allow for it.

use std::time::Instant;

use eframe::egui;
use novo_core::{Document, Endpoint, Pin, Placement, Point, PotentialKind, Result, SymbolRole, Tx};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn build(tx: &mut Tx<'_>, count: usize, pages: usize) -> Result<()> {
    let potential = tx.add_potential("L1", PotentialKind::Line)?;
    let pages: Vec<_> = (1..=pages)
        .map(|n| tx.add_page(format!("{n}")))
        .collect::<Result<_>>()?;
    let per_page = count.div_ceil(pages.len());
    let strip = tx.add_terminal_strip(novo_core::DeviceTag::parse("-X1").expect("tag"))?;
    let mut previous = None;
    for n in 0..count {
        let page = pages[n / per_page];
        let k = n % per_page;
        let (x, y) = (20.0 + (k % 80) as f64 * 12.0, 20.0 + (k / 80) as f64 * 14.0);
        let symbol = if n % 5 == 0 {
            tx.place_symbol(
                page,
                "iec/connectivity/potential_feed",
                Placement::at(Point::new(x, y)),
                vec![Pin::new("1", 0.0, 8.0)],
                SymbolRole::PotentialFeed { potential },
            )?
        } else {
            let terminal = tx.add_terminal(strip, format!("{n}"))?;
            tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(x, y)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                SymbolRole::Terminal {
                    terminals: vec![terminal],
                },
            )?
        };
        if k > 0
            && let Some(prev) = previous
        {
            tx.connect(Endpoint::pin(prev, 0), Endpoint::pin(symbol, 0))?;
        }
        previous = Some(symbol);
    }
    Ok(())
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn a_thousand_pages_filter_edit_and_delete_quickly() {
    let pages = if cfg!(debug_assertions) { 200 } else { 1000 };
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    let mut big = Document::default();
    big.transact("Dense", |tx| build(tx, pages * 50, pages))
        .unwrap();
    let path = dir.path().join("p.kopercad");
    novo_storage::ProjectFile::create(&path, big.project()).expect("save");
    drop(big);
    d.app.open_path(&path);
    let size = egui::vec2(1366.0, 768.0);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    d.app.settings.outline_pinned = Some(true);
    d.frame_sized(size, Vec::new());
    let t = Instant::now();
    for _ in 0..5 {
        d.frame_sized(size, Vec::new());
    }
    eprintln!("idle frame: {:.1} ms", ms(t) / 5.0);
    d.app.page_filter = "1".into();
    d.frame_sized(size, Vec::new());
    let cached = d
        .app
        .ol
        .net_wires
        .as_ref()
        .map(|(_, _, g)| std::rc::Rc::as_ptr(g));
    assert!(cached.is_some(), "the filtered wire rows were worked out");
    let t = Instant::now();
    for _ in 0..5 {
        d.frame_sized(size, Vec::new());
    }
    assert_eq!(
        d.app
            .ol
            .net_wires
            .as_ref()
            .map(|(_, _, g)| std::rc::Rc::as_ptr(g)),
        cached,
        "the filtered wire rows are kept between frames"
    );
    let filter = ms(t) / 5.0;
    eprintln!("frame with the filter \"1\": {filter:.1} ms");
    d.app.page_filter.clear();
    d.frame_sized(size, Vec::new());
    let order: Vec<_> = d
        .doc()
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .collect();
    let t = Instant::now();
    d.app.finish_rename(super::outline::Rename {
        of: super::outline::RenameOf::Page(order[10]),
        text: "Edited".into(),
    });
    d.frame_sized(size, Vec::new());
    let edit = ms(t);
    assert!(d.app.ol.rect.is_some(), "the Pages list was drawn");
    assert!(
        !d.app
            .wb
            .reports
            .live(d.app.backend.document().unwrap().project())
            .is_current(d.app.backend.document().unwrap().project(), "toc"),
        "the edit did not wait for the contents page"
    );
    eprintln!("edit a page and draw: {edit:.1} ms");
    // Once the edits pause the rows are made again.
    std::thread::sleep(std::time::Duration::from_millis(1600));
    d.frame_sized(size, Vec::new());
    assert!(
        d.app.page_rows().iter().any(|r| r.label.contains("Edited")),
        "the settled rows hold the new title"
    );
    let t = Instant::now();
    d.app.delete_page(order[20]);
    d.frame_sized(size, Vec::new());
    let delete = ms(t);
    eprintln!("delete a page and draw: {delete:.1} ms");
    // Wall time on a loaded CI machine proves nothing, so the work is counted.
    // The timings above are for the log: before the fix they were about 280,
    // 2100 and 2200 ms in a debug build at 200 pages.
    let _ = (filter, edit, delete);
    assert!(
        !d.app
            .wb
            .reports
            .live(d.app.backend.document().unwrap().project())
            .is_current(d.app.backend.document().unwrap().project(), "toc"),
        "the delete did not wait for the contents page"
    );
}
