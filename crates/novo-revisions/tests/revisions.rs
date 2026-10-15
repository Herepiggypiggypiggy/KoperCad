use novo_core::*;
use novo_revisions::{ChangeKind, IndexRule, diff};

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

#[test]
fn the_next_index() {
    let rule = IndexRule::Letters;
    assert_eq!(rule.first(), "A");
    assert_eq!(rule.after("A").as_deref(), Some("B"));
    assert_eq!(rule.after("H").as_deref(), Some("J"), "I is skipped");
    assert_eq!(rule.after("N").as_deref(), Some("P"), "O is skipped");
    assert_eq!(rule.after("Z").as_deref(), Some("AA"));
    assert_eq!(rule.after("AZ").as_deref(), Some("BA"));
    assert_eq!(rule.after("ZZ").as_deref(), Some("AAA"));
    assert_eq!(rule.after("b"), None);
    assert_eq!(rule.after(""), None);
    assert_eq!(IndexRule::Numbers.after("9").as_deref(), Some("10"));
    assert_eq!(IndexRule::Padded.first(), "01");
    assert_eq!(IndexRule::Padded.after("09").as_deref(), Some("10"));
    assert_eq!(IndexRule::Padded.after("099").as_deref(), Some("100"));
    assert_eq!(IndexRule::Padded.after("A"), None);

    let mut doc = Document::default();
    assert_eq!(rule.next(doc.project()).as_deref(), Some("A"));
    doc.execute(Command::IssueRevision {
        index: "A".into(),
        date: "2026-10-15".into(),
        author: "TV".into(),
        description: "First issue".into(),
    })
    .unwrap();
    assert_eq!(rule.next(doc.project()).as_deref(), Some("B"));
}

#[test]
fn changes_land_on_the_pages_that_show_them() {
    let mut doc = Document::default();
    let ((p1, p2, k1, coil, contact, wire), _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1").unwrap(), "Relay")?;
            let role = |main| SymbolRole::Device { device: k1, main };
            let coil = tx.place_symbol(p1, "coil", Placement::default(), pins(), role(true))?;
            let contact = tx.place_symbol(p2, "no", Placement::default(), pins(), role(false))?;
            let lamp = tx.place_symbol(
                p2,
                "lamp",
                Placement::at(Point::new(20.0, 0.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let wire = tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(lamp, 0))?;
            Ok((p1, p2, k1, coil, contact, wire))
        })
        .unwrap();
    let issued = doc.project().clone();
    assert!(diff(&issued, doc.project()).is_empty());

    doc.transact("Edit", |tx| {
        // A device change shows on every page that draws it.
        tx.update::<Device>(k1, |d| d.description = "Pump relay".into())?;
        // A moved symbol shows on its page.
        tx.update::<SymbolInstance>(coil, |s| s.placement = Placement::at(Point::new(40.0, 0.0)))?;
        // A removed wire shows on its page; a new part has no page.
        tx.remove(wire)?;
        tx.add_part("Siemens", "3RT2015")?;
        Ok(())
    })
    .unwrap();
    let changes = diff(&issued, doc.project());
    assert_eq!(changes.changed_pages().collect::<Vec<_>>(), [p1, p2]);

    let on = |page| {
        changes
            .on_page(page)
            .iter()
            .map(|c| (c.id, c.kind.clone()))
            .collect::<Vec<_>>()
    };
    let device = (
        AnyId::from(k1),
        ChangeKind::Changed {
            fields: vec!["description".into()],
        },
    );
    assert_eq!(
        on(p1),
        [
            device.clone(),
            (
                coil.into(),
                ChangeKind::Changed {
                    fields: vec!["placement".into()]
                }
            ),
        ]
    );
    assert_eq!(on(p2), [device, (wire.into(), ChangeKind::Removed)]);
    assert!(on(p2).iter().all(|(id, _)| *id != AnyId::from(contact)));
    assert_eq!(changes.project.len(), 1);
    assert!(matches!(changes.project[0].id, AnyId::Part(_)));
    assert_eq!(changes.project[0].kind, ChangeKind::Added);
}
