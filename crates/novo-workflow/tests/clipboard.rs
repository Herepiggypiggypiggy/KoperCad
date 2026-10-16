//! Copy, cut and paste: tags, connectivity, other projects and undo.

use novo_core::*;
use novo_workflow::clipboard::{Clip, Parsed, paste};
use novo_workflow::duplicate::TagMode;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

struct Drawn {
    doc: Document,
    page: PageId,
    items: Vec<AnyId>,
    wire: WireId,
}

/// Page `=A1` with breaker -Q1 wired to motor -M1 through a junction, a
/// numbered wire, a label text, and a coil -K1 with a contact on the page.
fn drawn(structure: &str) -> Drawn {
    let mut doc = Document::default();
    let ((page, items, wire), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.update::<Page>(page, |p| p.structure = tag(structure))?;
            let q = tx.add_device(tag(&format!("{structure}-Q1")), "Breaker")?;
            let m = tx.add_device(tag(&format!("{structure}-M1")), "Motor")?;
            let k = tx.add_device(tag(&format!("{structure}-K1")), "Contactor")?;
            let mut sym = |x: f64, d: DeviceId, main: bool| {
                tx.place_symbol(
                    page,
                    "x",
                    Placement::at(Point::new(x, 20.0)),
                    pins(),
                    SymbolRole::Device { device: d, main },
                )
            };
            let (q1, m1) = (sym(10.0, q, true)?, sym(50.0, m, true)?);
            let coil = sym(70.0, k, true)?;
            let contact = sym(90.0, k, false)?;
            let j = tx.insert(|id| Junction {
                id,
                page,
                position: Point::new(30.0, 30.0),
            })?;
            let w1 = tx.connect(Endpoint::pin(q1, 1), Endpoint::Junction { junction: j })?;
            tx.update::<Wire>(w1, |w| w.number = Some("W1".into()))?;
            tx.connect(Endpoint::Junction { junction: j }, Endpoint::pin(m1, 0))?;
            let label = tx.insert(|id| Text {
                id,
                page,
                attached_to: Some(q1.into()),
                content: TextContent::Literal {
                    text: "main breaker".into(),
                },
                placement: Default::default(),
                height_mm: 2.5,
                properties: Default::default(),
            })?;
            let _ = label;
            Ok((
                page,
                vec![
                    q1.into(),
                    m1.into(),
                    j.into(),
                    w1.into(),
                    coil.into(),
                    contact.into(),
                ],
                w1,
            ))
        })
        .unwrap();
    Drawn {
        doc,
        page,
        items,
        wire,
    }
}

fn tags(doc: &Document) -> Vec<String> {
    let mut t: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
    t.sort();
    t
}

fn no_duplicates(doc: &Document) {
    let all = tags(doc);
    let mut unique = all.clone();
    unique.dedup();
    assert_eq!(all, unique, "a tag is used twice");
}

#[test]
fn paste_takes_next_free_tags_and_never_a_duplicate() {
    let mut d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", false).unwrap();
    for n in 0..3 {
        d.doc
            .transact("Paste", |tx| {
                paste(
                    tx,
                    &clip,
                    d.page,
                    Point::new(0.0, 40.0 * (n + 1) as f64),
                    TagMode::NextFree,
                    "p",
                )
            })
            .unwrap();
    }
    no_duplicates(&d.doc);
    assert_eq!(d.doc.project().devices().count(), 12);
    assert!(tags(&d.doc).contains(&"=A1-Q4".to_owned()));
}

#[test]
fn a_coil_and_its_contact_stay_together() {
    let mut d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", false).unwrap();
    let (pasted, _) = d
        .doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                d.page,
                Point::new(0.0, 60.0),
                TagMode::NextFree,
                "p",
            )
        })
        .unwrap();
    let project = d.doc.project();
    let coil = pasted
        .copied
        .symbols
        .values()
        .filter_map(|&s| project.symbol(s));
    let devices: Vec<DeviceId> = coil.filter_map(|s| s.device()).collect();
    let k2 = project
        .devices()
        .find(|dev| dev.tag == tag("=A1-K2"))
        .unwrap()
        .id;
    assert_eq!(devices.iter().filter(|&&dev| dev == k2).count(), 2);
}

#[test]
fn wires_keep_their_connections_inside_the_paste_and_numbers_are_dropped() {
    let mut d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", false).unwrap();
    let (pasted, _) = d
        .doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                d.page,
                Point::new(0.0, 60.0),
                TagMode::NextFree,
                "p",
            )
        })
        .unwrap();
    assert_eq!(pasted.copied.wires.len(), 2);
    let project = d.doc.project();
    let new_wire = project.wire(pasted.copied.wires[&d.wire]).unwrap();
    assert!(new_wire.number.is_none());
    match (new_wire.from, new_wire.to) {
        (Endpoint::Pin { symbol, .. }, Endpoint::Junction { junction }) => {
            assert!(pasted.copied.symbols.values().any(|&s| s == symbol));
            assert!(pasted.copied.junctions.values().any(|&j| j == junction));
        }
        other => panic!("ends were not remapped: {other:?}"),
    }
    // The original wire still ends on the original pins.
    let old = project.wire(d.wire).unwrap();
    assert!(
        matches!(old.from, Endpoint::Pin { symbol, .. } if !pasted.copied.symbols.values().any(|&s| s == symbol))
    );
    // Attached texts came along.
    assert_eq!(pasted.copied.texts.len(), 1);
}

#[test]
fn a_lone_contact_stays_a_contact_of_its_device_in_the_same_project() {
    let mut d = drawn("=A1");
    let contact = *d.items.last().unwrap();
    let clip = Clip::capture(d.doc.project(), &[contact], "p", false).unwrap();
    let (pasted, _) = d
        .doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                d.page,
                Point::new(0.0, 60.0),
                TagMode::NextFree,
                "p",
            )
        })
        .unwrap();
    assert!(pasted.orphans.is_empty());
    assert_eq!(d.doc.project().devices().count(), 3);
}

#[test]
fn a_lone_contact_pasted_in_another_project_gets_a_device_of_its_own() {
    let d = drawn("=A1");
    let contact = *d.items.last().unwrap();
    let clip = Clip::capture(d.doc.project(), &[contact], "p", false).unwrap();
    let mut other = drawn("=A1");
    let before = other.doc.project().devices().count();
    let (pasted, _) = other
        .doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                other.page,
                Point::new(0.0, 60.0),
                TagMode::NextFree,
                "q",
            )
        })
        .unwrap();
    assert_eq!(pasted.orphans, vec![tag("=A1-K2")]);
    assert_eq!(other.doc.project().devices().count(), before + 1);
    no_duplicates(&other.doc);
}

#[test]
fn keep_if_free_keeps_tags_after_a_cut_and_renumbers_on_a_clash() {
    let mut d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", true).unwrap();
    // Cut: the originals are gone, so the tags are free.
    d.doc
        .transact("Cut", |tx| {
            for item in &d.items {
                if let AnyId::Symbol(s) = item {
                    tx.remove(AnyId::Symbol(*s))?;
                }
            }
            // Deleting the last symbol of a device removes the device.
            let devices: Vec<DeviceId> = tx.project().devices().map(|d| d.id).collect();
            for device in devices {
                tx.remove(AnyId::Device(device))?;
            }
            Ok(())
        })
        .unwrap();
    let (first, _) = d
        .doc
        .transact("Paste", |tx| {
            paste(tx, &clip, d.page, Point::ZERO, TagMode::KeepIfFree, "p")
        })
        .unwrap();
    assert!(first.renamed.is_empty());
    assert!(tags(&d.doc).contains(&"=A1-Q1".to_owned()));
    // A second paste of the same clip clashes and takes new numbers.
    let (second, _) = d
        .doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                d.page,
                Point::new(0.0, 40.0),
                TagMode::KeepIfFree,
                "p",
            )
        })
        .unwrap();
    assert_eq!(second.renamed.len(), 3);
    no_duplicates(&d.doc);
}

#[test]
fn a_page_of_another_structure_numbers_in_that_structure() {
    let d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", false).unwrap();
    let mut other = drawn("=A2");
    let (pasted, _) = other
        .doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                other.page,
                Point::new(0.0, 60.0),
                TagMode::NextFree,
                "q",
            )
        })
        .unwrap();
    assert!(pasted.renamed.contains(&(tag("=A1-Q1"), tag("=A2-Q2"))));
    no_duplicates(&other.doc);
}

#[test]
fn a_paste_is_one_undo_step() {
    let mut d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", false).unwrap();
    let before = d.doc.project().clone();
    d.doc
        .transact("Paste", |tx| {
            paste(
                tx,
                &clip,
                d.page,
                Point::new(0.0, 60.0),
                TagMode::NextFree,
                "p",
            )
        })
        .unwrap();
    assert!(d.doc.can_undo());
    d.doc.undo().unwrap();
    assert_eq!(d.doc.project().devices().count(), before.devices().count());
    assert_eq!(d.doc.project().symbols().count(), before.symbols().count());
    assert_eq!(d.doc.project().wires().count(), before.wires().count());
}

#[test]
fn the_clipboard_text_round_trips_and_other_text_is_ignored() {
    let d = drawn("=A1");
    let clip = Clip::capture(d.doc.project(), &d.items, "p", false).unwrap();
    let text = clip.to_text();
    assert_eq!(Clip::parse(&text), Parsed::Clip(Box::new(clip.clone())));
    assert_eq!(Clip::parse("hello"), Parsed::NotOurs);
    assert_eq!(Clip::parse("{\"a\":1}"), Parsed::NotOurs);
    let newer = text.replacen("\"version\":1", "\"version\":9", 1);
    assert_eq!(Clip::parse(&newer), Parsed::TooNew(9));
}

#[test]
fn terminals_land_on_the_strip_of_the_same_tag_and_potentials_are_matched() {
    let mut d = Document::default();
    let ((page, term_symbol), _) = d
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let strip = tx.add_terminal_strip(tag("-X1"))?;
            let t = tx.add_terminal(strip, "1")?;
            let s = tx.place_symbol(
                page,
                "t",
                Placement::at(Point::new(5.0, 5.0)),
                pins(),
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            Ok((page, s))
        })
        .unwrap();
    let clip = Clip::capture(d.project(), &[term_symbol.into()], "p", false).unwrap();
    // Same project: next number on the same strip.
    d.transact("Paste", |tx| {
        paste(
            tx,
            &clip,
            page,
            Point::new(20.0, 0.0),
            TagMode::NextFree,
            "p",
        )
    })
    .unwrap();
    assert_eq!(d.project().terminals().count(), 2);
    assert_eq!(
        d.project()
            .devices()
            .filter(|x| x.tag == tag("-X1"))
            .count(),
        1
    );
    // Another project: the strip is created, the name kept.
    let mut other = Document::default();
    let (page2, _) = other.transact("Page", |tx| tx.add_page("1")).unwrap();
    other
        .transact("Paste", |tx| {
            paste(tx, &clip, page2, Point::ZERO, TagMode::NextFree, "q")
        })
        .unwrap();
    let t = other.project().terminals().next().unwrap();
    assert_eq!(t.name, "1");
    assert_eq!(other.project().device(t.strip).unwrap().tag, tag("-X1"));
}
