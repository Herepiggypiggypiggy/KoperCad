use novo_core::*;
use novo_revisions::{
    Located, NoteError, NoteFilter, NoteStatus, NoteTarget, ReviewNotes, locate_note,
    notes_appendix_lines, notes_pdf_markers,
};

fn pins() -> Vec<Pin> {
    vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)]
}

struct Drawn {
    doc: Document,
    p1: PageId,
    p2: PageId,
    k1: DeviceId,
    coil: SymbolId,
    wire: WireId,
}

fn draw() -> Drawn {
    let mut doc = Document::default();
    let ((p1, p2, k1, coil, wire), _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1").unwrap(), "Relay")?;
            let role = SymbolRole::Device {
                device: k1,
                main: true,
            };
            let coil = tx.place_symbol(
                p2,
                "coil",
                Placement::at(Point::new(50.0, 60.0)),
                pins(),
                role,
            )?;
            let lamp = tx.place_symbol(
                p2,
                "lamp",
                Placement::at(Point::new(90.0, 60.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let wire = tx.connect(Endpoint::pin(coil, 0), Endpoint::pin(lamp, 0))?;
            Ok((p1, p2, k1, coil, wire))
        })
        .unwrap();
    Drawn {
        doc,
        p1,
        p2,
        k1,
        coil,
        wire,
    }
}

const T0: &str = "2026-10-22T08:00:00Z";
const T1: &str = "2026-10-23T09:30:00Z";

#[test]
fn a_note_has_a_number_an_author_and_starts_open() {
    let mut notes = ReviewNotes::default();
    let a = notes
        .add(NoteTarget::Project, " TV ", T0, "  Check the PE  ")
        .unwrap();
    let b = notes.add(NoteTarget::Project, "KV", T0, "Second").unwrap();
    assert_eq!((a, b), (1, 2));
    let first = notes.get(a).unwrap();
    assert_eq!(first.text, "Check the PE");
    assert_eq!(first.author, "TV");
    assert_eq!(first.status, NoteStatus::Open);
    assert_eq!(first.name(), "N1");
    assert_eq!(first.date(), "2026-10-22");
    assert_eq!(
        notes.add(NoteTarget::Project, "TV", T0, "  "),
        Err(NoteError::Empty)
    );
}

#[test]
fn numbers_are_never_reused_after_a_delete_and_restore_keeps_the_place() {
    let mut notes = ReviewNotes::default();
    let a = notes.add(NoteTarget::Project, "TV", T0, "one").unwrap();
    let b = notes.add(NoteTarget::Project, "TV", T0, "two").unwrap();
    let removed = notes.remove(b).unwrap();
    let c = notes.add(NoteTarget::Project, "TV", T0, "three").unwrap();
    assert_eq!(c, 3, "N2 is not handed out again");
    notes.restore(removed);
    let ids: Vec<_> = notes.iter().map(|n| n.id).collect();
    assert_eq!(ids, [a, b, c]);
    assert_eq!(notes.remove(99), Err(NoteError::Unknown(99)));
}

#[test]
fn resolve_reopen_and_a_reply_to_a_resolved_note() {
    let mut notes = ReviewNotes::default();
    let id = notes.add(NoteTarget::Project, "TV", T0, "Fix").unwrap();
    notes.resolve(id, "KV", T1).unwrap();
    let n = notes.get(id).unwrap();
    assert_eq!(n.status, NoteStatus::Resolved);
    assert_eq!(n.resolved_by.as_deref(), Some("KV"));
    assert_eq!(notes.counts().open, 0);
    assert_eq!(notes.counts().resolved, 1);

    notes.reply(id, "TV", T1, "Not fixed yet").unwrap();
    let n = notes.get(id).unwrap();
    assert_eq!(n.status, NoteStatus::Open, "a reply opens it again");
    assert!(n.resolved_by.is_none() && n.resolved_at.is_none());
    assert_eq!(n.replies.len(), 1);

    notes.resolve(id, "KV", T1).unwrap();
    notes.reopen(id).unwrap();
    assert!(notes.get(id).unwrap().is_open());
    assert_eq!(notes.authors(), ["TV"]);
}

#[test]
fn the_notes_survive_json_and_an_empty_document_reads() {
    let d = draw();
    let mut notes = ReviewNotes::default();
    notes
        .add(NoteTarget::object(d.k1), "TV", T0, "Rating?")
        .unwrap();
    notes
        .add(
            NoteTarget::point(d.p2, Point::new(10.0, 20.0)),
            "KV",
            T0,
            "Here",
        )
        .unwrap();
    notes.reply(1, "KV", T1, "16 A").unwrap();
    notes.resolve(2, "TV", T1).unwrap();
    let back = ReviewNotes::from_json(&notes.to_json()).unwrap();
    assert_eq!(back, notes);
    assert!(ReviewNotes::from_json("{}").unwrap().is_empty());
    assert!(matches!(
        ReviewNotes::from_json("not json"),
        Err(NoteError::Unreadable(_))
    ));
}

#[test]
fn a_target_is_found_on_its_page_and_is_gone_when_deleted() {
    let mut d = draw();
    let spot = |p: &Project, t: NoteTarget| match locate_note(p, &t) {
        Located::Here(s) => s,
        Located::Gone => panic!("gone"),
    };
    let device = spot(d.doc.project(), NoteTarget::object(d.k1));
    assert_eq!(device.page, Some(d.p2));
    assert_eq!(device.symbol, Some(d.coil));
    assert_eq!(device.at, Some(Point::new(50.0, 60.0)));
    assert_eq!(device.what, "Device -K1");
    let wire = spot(d.doc.project(), NoteTarget::object(d.wire));
    assert_eq!(wire.page, Some(d.p2));
    assert_eq!(spot(d.doc.project(), NoteTarget::page(d.p1)).what, "Page 1");
    let project = spot(d.doc.project(), NoteTarget::Project);
    assert_eq!(project.page, None);

    d.doc
        .transact("Delete", |tx| {
            tx.remove(d.wire)?;
            tx.remove(d.p1)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        locate_note(d.doc.project(), &NoteTarget::object(d.wire)),
        Located::Gone
    );
    assert_eq!(
        locate_note(d.doc.project(), &NoteTarget::page(d.p1)),
        Located::Gone
    );
    assert!(matches!(
        locate_note(d.doc.project(), &NoteTarget::object(d.k1)),
        Located::Here(_)
    ));
}

#[test]
fn filters_combine_status_author_page_and_words() {
    let d = draw();
    let p = d.doc.project();
    let mut notes = ReviewNotes::default();
    notes
        .add(NoteTarget::object(d.k1), "TV", T0, "Rating too low")
        .unwrap();
    notes
        .add(NoteTarget::page(d.p1), "KV", T0, "Title block")
        .unwrap();
    notes
        .add(NoteTarget::Project, "KV", T0, "Revision text")
        .unwrap();
    notes.resolve(3, "TV", T1).unwrap();
    notes.reply(2, "TV", T1, "Done in the template").unwrap();

    let ids = |f: &NoteFilter| {
        notes
            .filtered(p, f)
            .iter()
            .map(|n| n.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(&NoteFilter::default()),
        [2, 1, 3],
        "page order, then project notes"
    );
    assert_eq!(ids(&NoteFilter::open()), [2, 1]);
    assert_eq!(
        ids(&NoteFilter {
            author: Some("TV".into()),
            ..Default::default()
        }),
        [2, 1],
        "a reply counts as taking part"
    );
    assert_eq!(
        ids(&NoteFilter {
            page: Some(d.p2),
            ..Default::default()
        }),
        [1]
    );
    assert_eq!(
        ids(&NoteFilter {
            text: "device -k1 RATING".into(),
            ..Default::default()
        }),
        [1],
        "words match the target's name too"
    );
    assert_eq!(
        ids(&NoteFilter {
            text: "template".into(),
            ..Default::default()
        }),
        [2]
    );
}

#[test]
fn resolve_all_touches_only_what_the_filter_shows() {
    let d = draw();
    let p = d.doc.project();
    let mut notes = ReviewNotes::default();
    notes.add(NoteTarget::page(d.p1), "TV", T0, "a").unwrap();
    notes.add(NoteTarget::page(d.p2), "TV", T0, "b").unwrap();
    let n = notes.resolve_all(
        p,
        &NoteFilter {
            page: Some(d.p1),
            ..NoteFilter::open()
        },
        "KV",
        T1,
    );
    assert_eq!(n, 1);
    assert!(!notes.get(1).unwrap().is_open());
    assert!(notes.get(2).unwrap().is_open());
}

#[test]
fn open_notes_on_a_page_are_what_the_canvas_marks() {
    let d = draw();
    let p = d.doc.project();
    let mut notes = ReviewNotes::default();
    notes
        .add(NoteTarget::object(d.coil), "TV", T0, "x")
        .unwrap();
    notes.add(NoteTarget::page(d.p2), "TV", T0, "y").unwrap();
    notes.add(NoteTarget::page(d.p1), "TV", T0, "z").unwrap();
    notes.resolve(2, "TV", T1).unwrap();
    let ids: Vec<_> = notes.open_on_page(p, d.p2).iter().map(|n| n.id).collect();
    assert_eq!(ids, [1]);
}

#[test]
fn pdf_markers_sit_at_the_point_beside_an_object_or_down_the_margin() {
    let d = draw();
    let p = d.doc.project();
    let mut notes = ReviewNotes::default();
    notes
        .add(NoteTarget::page(d.p2), "TV", T0, "page one")
        .unwrap();
    notes
        .add(NoteTarget::object(d.k1), "TV", T0, "device")
        .unwrap();
    notes
        .add(
            NoteTarget::point(d.p2, Point::new(120.0, 30.0)),
            "TV",
            T0,
            "point",
        )
        .unwrap();
    notes
        .add(NoteTarget::page(d.p2), "TV", T0, "page two")
        .unwrap();
    notes.add(NoteTarget::Project, "TV", T0, "no page").unwrap();
    notes.resolve(4, "KV", T1).unwrap();

    let all = notes_pdf_markers(p, &notes, &NoteFilter::default());
    let on = &all[&d.p2];
    assert_eq!(on.len(), 4, "the project note has no page to stand on");
    assert_eq!(on[0].note, 1);
    assert_eq!((on[0].at.x, on[0].at.y), (6.0, 12.0));
    assert_eq!((on[1].at.x, on[1].at.y), (56.0, 54.0));
    assert_eq!(on[2].at, Point::new(120.0, 30.0));
    assert_eq!(
        (on[3].at.x, on[3].at.y),
        (6.0, 21.0),
        "the second margin marker is below the first"
    );
    assert!(on[3].resolved);

    let open_only = notes_pdf_markers(p, &notes, &NoteFilter::open());
    assert_eq!(open_only[&d.p2].len(), 3);
}

#[test]
fn the_printed_list_names_place_author_replies_and_who_resolved() {
    let mut d = draw();
    let mut notes = ReviewNotes::default();
    notes
        .add(NoteTarget::object(d.k1), "TV", T0, "Rating?")
        .unwrap();
    notes.reply(1, "KV", T1, "16 A").unwrap();
    notes.resolve(1, "TV", T1).unwrap();
    notes
        .add(NoteTarget::object(d.wire), "KV", T0, "Number missing")
        .unwrap();
    d.doc.transact("Del", |tx| tx.remove(d.wire)).unwrap();

    let text: Vec<String> = notes_appendix_lines(d.doc.project(), &notes, &NoteFilter::default())
        .into_iter()
        .map(|l| l.text)
        .collect();
    assert_eq!(text[0], "2 notes: 1 open, 1 resolved");
    assert!(
        text.contains(&"N1  Device -K1, page 2  [Resolved]".to_owned()),
        "{text:?}"
    );
    assert!(text.contains(&"TV 2026-10-22: Rating?".to_owned()));
    assert!(text.contains(&"  > KV 2026-10-23: 16 A".to_owned()));
    assert!(text.contains(&"  Resolved by TV 2026-10-23".to_owned()));
    assert!(
        text.contains(&"N2  target deleted  [Open]".to_owned()),
        "{text:?}"
    );
    assert!(
        notes_appendix_lines(
            d.doc.project(),
            &ReviewNotes::default(),
            &NoteFilter::default()
        )
        .is_empty()
    );
}

#[test]
fn the_notes_table_has_a_row_per_note_with_replies_and_resolution() {
    use novo_revisions::notes_table;
    let d = draw();
    let mut notes = ReviewNotes::default();
    notes
        .add(NoteTarget::object(d.k1), "TV", T0, "Rating?")
        .unwrap();
    notes.reply(1, "KV", T1, "16 A").unwrap();
    notes.resolve(1, "TV", T1).unwrap();
    notes
        .add(NoteTarget::Project, "KV", T0, "Cover page")
        .unwrap();
    let out = notes_table(d.doc.project(), &notes, &NoteFilter::default());
    let section = out.section("notes").unwrap();
    assert_eq!(section.rows.len(), 2);
    let cells: Vec<String> = section.rows[0]
        .cells
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert_eq!(cells[0], "N1");
    assert_eq!(cells[1], "Resolved");
    assert_eq!(cells[2], "Device -K1, page 2");
    assert!(cells[5].contains("KV 2026-10-23: 16 A"), "{cells:?}");
    assert!(cells[5].contains("Resolved by TV 2026-10-23"), "{cells:?}");
    assert_eq!(
        notes_table(d.doc.project(), &notes, &NoteFilter::open()).row_count(),
        1
    );
}
