use novo_core::*;
use novo_revisions::{ChangeKind, change_list};

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

#[test]
fn the_change_list_names_each_change_in_page_order() {
    let mut doc = Document::default();
    let ((p1, p2, k1, wire), _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1").unwrap(), "Relay")?;
            let role = SymbolRole::Device {
                device: k1,
                main: true,
            };
            let a = tx.place_symbol(p1, "coil", Placement::default(), pins(), role.clone())?;
            let b = tx.place_symbol(p2, "lamp", Placement::default(), pins(), role)?;
            let _ = a;
            let c = tx.place_symbol(
                p2,
                "lamp",
                Placement::at(Point::new(20.0, 0.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let wire = tx.connect(Endpoint::pin(b, 1), Endpoint::pin(c, 0))?;
            Ok((p1, p2, k1, wire))
        })
        .unwrap();
    let issued = doc.project().clone();
    assert!(change_list(&issued, doc.project()).is_empty());

    doc.transact("Edit", |tx| {
        tx.update::<Device>(k1, |d| d.description = "Pump relay".into())?;
        tx.remove(wire)?;
        tx.add_part("Siemens", "3RT2015")?;
        Ok(())
    })
    .unwrap();
    let rows = change_list(&issued, doc.project());
    let summary: Vec<_> = rows
        .iter()
        .map(|r| (r.page, r.label.as_str(), r.kind.clone()))
        .collect();
    let device = ChangeKind::Changed {
        fields: vec!["description".into()],
    };
    assert_eq!(
        summary,
        [
            (Some(p1), "Device -K1", device.clone()),
            (Some(p2), "Device -K1", device),
            (Some(p2), "Wire -K1:2 to lamp:1", ChangeKind::Removed),
            (None, "Part Siemens 3RT2015", ChangeKind::Added),
        ]
    );
}
