//! Texts and labels as records of their own.

use novo_core::*;

fn placement(x: f64, y: f64) -> TextPlacement {
    TextPlacement {
        offset: Point::new(x, y),
        ..TextPlacement::default()
    }
}

#[test]
fn labels_follow_their_symbol_and_show_live_values() {
    let mut doc = Document::default();
    let ((p1, p2, k1, coil, label, wire, number), _) = doc
        .transact("Coil with label", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            let coil = tx.place_symbol(
                p1,
                "iec/coils/relay",
                Placement::at(Point::new(100.0, 50.0)),
                vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)],
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            let label = tx.add_text(
                p1,
                TextContent::DeviceTag,
                placement(6.0, -2.0),
                Some(coil.into()),
            )?;
            let junction = tx.add_junction(p1, Point::new(100.0, 30.0))?;
            let wire = tx.connect(Endpoint::junction(junction), Endpoint::pin(coil, 0))?;
            tx.update::<Wire>(wire, |w| w.number = Some("101".into()))?;
            let number = tx.add_text(
                p1,
                TextContent::WireNumber,
                placement(2.0, 0.0),
                Some(wire.into()),
            )?;
            Ok((p1, p2, k1, coil, label, wire, number))
        })
        .unwrap();
    let project = doc.project();
    assert_eq!(project.text_string(label).as_deref(), Some("-K1"));
    assert_eq!(project.text_position(label), Some(Point::new(106.0, 48.0)));
    assert_eq!(project.text_string(number).as_deref(), Some("101"));
    // Wire labels sit at the middle of the first segment: (100,30)-(100,50).
    assert_eq!(project.text_position(number), Some(Point::new(102.0, 40.0)));
    assert_eq!(project.texts_of(coil), [label]);

    // The label moves with the symbol and shows a renamed tag.
    doc.transact("Move and rename", |tx| {
        tx.move_symbol(coil, Placement::at(Point::new(200.0, 50.0)))?;
        tx.set_tag(k1, DeviceTag::parse("-K7")?)
    })
    .unwrap();
    assert_eq!(
        doc.project().text_position(label),
        Some(Point::new(206.0, 48.0))
    );
    assert_eq!(doc.project().text_string(label).as_deref(), Some("-K7"));

    // The label moves and rotates on its own.
    let turned = TextPlacement {
        offset: Point::new(-4.0, 0.0),
        rotation: Rotation::R90,
        anchor: TextAnchor::MiddleRight,
    };
    doc.execute(Command::MoveText {
        text: label,
        placement: turned,
    })
    .unwrap();
    let project = doc.project();
    assert_eq!(project.text(label).unwrap().placement, turned);
    assert_eq!(
        project.symbol(coil).unwrap().placement.origin,
        Point::new(200.0, 50.0)
    );

    // A symbol cannot leave its labels on another page.
    let err = doc
        .transact("x", |tx| tx.update::<SymbolInstance>(coil, |s| s.page = p2))
        .unwrap_err();
    assert!(matches!(err, ModelError::WrongPage { .. }));

    // Deleting the symbol deletes its label; undo brings both back.
    doc.transact("Delete", |tx| tx.remove(coil)).unwrap();
    assert!(doc.project().text(label).is_none());
    assert!(doc.project().wire(wire).is_none());
    assert!(doc.project().text(number).is_none());
    doc.undo().unwrap();
    assert_eq!(doc.project().text_string(label).as_deref(), Some("-K7"));

    // Texts survive a save.
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert_eq!(loaded.text(label), doc.project().text(label));
    assert_eq!(loaded.page_contents(p1).texts.len(), 2);
}

#[test]
fn free_texts_and_refusals() {
    let mut doc = Document::default();
    let ((page, other, k1, symbol), _) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("1")?;
            let other = tx.add_page("2")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            let symbol = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(10.0, 10.0)),
                vec![],
                SymbolRole::Unassigned,
            )?;
            Ok((page, other, k1, symbol))
        })
        .unwrap();
    let created = doc
        .execute(Command::AddText {
            page,
            content: TextContent::Literal {
                text: "Main supply".into(),
            },
            placement: placement(20.0, 280.0),
            attached_to: None,
        })
        .unwrap()
        .created;
    let [AnyId::Text(note)] = created[..] else {
        panic!("{created:?}")
    };
    assert_eq!(
        doc.project().text_string(note).as_deref(),
        Some("Main supply")
    );
    assert_eq!(
        doc.project().text_position(note),
        Some(Point::new(20.0, 280.0))
    );
    // An unassigned symbol has no tag to show.
    let (empty, _) = doc
        .transact("x", |tx| {
            tx.add_text(
                page,
                TextContent::DeviceTag,
                placement(0.0, 0.0),
                Some(symbol.into()),
            )
        })
        .unwrap();
    assert_eq!(doc.project().text_string(empty).as_deref(), Some(""));

    let add = |doc: &mut Document, page, attached_to, edit: fn(&mut Text)| {
        doc.transact("x", |tx| {
            let id = tx.add_text(
                page,
                TextContent::DeviceTag,
                placement(0.0, 0.0),
                attached_to,
            )?;
            tx.update::<Text>(id, edit)
        })
        .unwrap_err()
    };
    assert!(matches!(
        add(&mut doc, page, Some(k1.into()), |_| {}),
        ModelError::InvalidAttachment { .. }
    ));
    assert!(matches!(
        add(&mut doc, other, Some(symbol.into()), |_| {}),
        ModelError::WrongPage { .. }
    ));
    assert!(matches!(
        add(&mut doc, page, None, |t| t.height_mm = 0.0),
        ModelError::InvalidValue {
            field: "height_mm",
            ..
        }
    ));
    assert!(matches!(
        add(&mut doc, page, None, |t| t.placement.offset.x = f64::NAN),
        ModelError::NonFinite {
            field: "placement",
            ..
        }
    ));
}

#[test]
fn set_text_changes_content_and_height_as_one_step() {
    let mut doc = Document::default();
    let ((page, note), _) = doc
        .transact("Setup", |tx| {
            let page = tx.add_page("1")?;
            let note = tx.add_text(
                page,
                TextContent::Literal {
                    text: "Main supply".into(),
                },
                placement(20.0, 280.0),
                None,
            )?;
            Ok((page, note))
        })
        .unwrap();
    let before = doc.project().to_json();
    let depth = doc.undo_depth();

    // Content and height in one command, one undo step.
    doc.execute(Command::SetText {
        text: note,
        content: Some(TextContent::Literal {
            text: "Control supply".into(),
        }),
        height_mm: Some(3.5),
    })
    .unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.undo_label(), Some("Edit text"));
    let text = doc.project().text(note).unwrap();
    assert_eq!(text.height_mm, 3.5);
    assert_eq!(
        doc.project().text_string(note).as_deref(),
        Some("Control supply")
    );
    // Placement and page are untouched.
    assert_eq!(text.placement, placement(20.0, 280.0));
    assert_eq!(text.page, page);
    doc.undo().unwrap();
    assert_eq!(doc.project().to_json(), before);
    doc.redo().unwrap();

    // `None` keeps the current value.
    doc.transact("Height only", |tx| tx.set_text(note, None, Some(5.0)))
        .unwrap();
    let text = doc.project().text(note).unwrap();
    assert_eq!(text.height_mm, 5.0);
    assert_eq!(
        doc.project().text_string(note).as_deref(),
        Some("Control supply")
    );

    // Refused edits leave the project unchanged and add no undo step.
    let before = doc.project().to_json();
    let depth = doc.undo_depth();
    for height in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let err = doc
            .execute(Command::SetText {
                text: note,
                content: Some(TextContent::WireNumber),
                height_mm: Some(height),
            })
            .unwrap_err();
        assert!(
            matches!(
                err,
                ModelError::InvalidValue {
                    field: "height_mm",
                    ..
                } | ModelError::NonFinite {
                    field: "height_mm",
                    ..
                }
            ),
            "{height}: {err:?}"
        );
    }
    let missing = TextId(9999);
    assert!(matches!(
        doc.execute(Command::SetText {
            text: missing,
            content: None,
            height_mm: Some(3.0),
        }),
        Err(ModelError::NotFound(_))
    ));
    assert_eq!(doc.project().to_json(), before);
    assert_eq!(doc.undo_depth(), depth);
}

#[test]
fn set_text_command_round_trips_through_json() {
    let command = Command::SetText {
        text: TextId(7),
        content: Some(TextContent::Property {
            key: "rating".into(),
        }),
        height_mm: None,
    };
    let json = serde_json::to_string(&command).unwrap();
    assert!(!json.contains("height_mm"), "{json}");
    assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
    let bare: Command = serde_json::from_str(r#"{"command":"set_text","text":7}"#).unwrap();
    assert_eq!(
        bare,
        Command::SetText {
            text: TextId(7),
            content: None,
            height_mm: None,
        }
    );
}
