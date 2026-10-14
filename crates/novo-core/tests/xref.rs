//! Cross-reference queries.

use novo_core::*;

fn symbol(tx: &mut Tx<'_>, page: PageId, x: f64, role: SymbolRole) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/contacts/no",
        Placement::at(Point::new(x, 50.0)),
        vec![Pin::new("1", 0.0, 0.0)],
        role,
    )
}

#[test]
fn device_parts_in_reading_order() {
    let mut doc = Document::default();
    let ((coil, contacts), _) = doc
        .transact("K1 over three pages", |tx| {
            let [p1, p2, p10] = ["1", "2", "10"].map(|name| tx.add_page(name));
            let (p1, p2, p10) = (p1?, p2?, p10?);
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "Contactor")?;
            let aux = SymbolRole::Device {
                device: k1,
                main: false,
            };
            // A3 landscape is 420 mm wide: 42 mm per column.
            let late = symbol(tx, p10, 10.0, aux.clone())?;
            let right = symbol(tx, p2, 200.0, aux.clone())?;
            let first = symbol(tx, p1, 300.0, aux)?;
            let coil = symbol(
                tx,
                p2,
                50.0,
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            Ok((coil, [first, right, late]))
        })
        .unwrap();
    let project = doc.project();
    let k1 = project.devices().next().unwrap().id;
    let refs = project.cross_references(k1);
    let labels: Vec<String> = refs.iter().map(|r| r.location.to_string()).collect();
    assert_eq!(labels, ["1.8", "2.2", "2.5", "10.1"]);
    let order: Vec<SymbolId> = refs.iter().map(|r| r.symbol).collect();
    assert_eq!(order, [contacts[0], coil, contacts[1], contacts[2]]);
    assert_eq!(
        refs.iter()
            .filter(|r| r.main)
            .map(|r| r.symbol)
            .collect::<Vec<_>>(),
        [coil]
    );
}

#[test]
fn interruption_points_find_each_other() {
    let mut doc = Document::default();
    let ((a1, a3, a3b, b), _) = doc
        .transact("Interruptions", |tx| {
            let p1 = tx.add_page("1")?;
            let p3 = tx.add_page("3")?;
            let named = |name: &str| SymbolRole::Interruption { name: name.into() };
            let a1 = symbol(tx, p1, 400.0, named("A"))?;
            let a3b = symbol(tx, p3, 100.0, named("A"))?;
            let a3 = symbol(tx, p3, 10.0, named("A"))?;
            let b = symbol(tx, p3, 50.0, named("B"))?;
            Ok((a1, a3, a3b, b))
        })
        .unwrap();
    let project = doc.project();
    let partners = |s| -> Vec<SymbolId> {
        project
            .interruption_partners(s)
            .iter()
            .map(|r| r.symbol)
            .collect()
    };
    assert_eq!(partners(a1), [a3, a3b]);
    assert_eq!(partners(a3), [a1, a3b]);
    assert!(partners(b).is_empty());
    let first = &project.interruption_partners(a3)[0];
    assert_eq!(first.location.to_string(), "1.10");
}

#[test]
fn grid_columns() {
    let a3 = PageFormat::default();
    assert_eq!(a3.columns, 10);
    assert_eq!(a3.column_at(0.0), 1);
    assert_eq!(a3.column_at(41.9), 1);
    assert_eq!(a3.column_at(42.0), 2);
    assert_eq!(a3.column_at(420.0), 10);
    assert_eq!(a3.column_at(-5.0), 1);
    assert_eq!(a3.column_at(1000.0), 10);
    let a4 = PageFormat {
        size: PaperSize::A4,
        landscape: false,
        columns: 4,
    };
    assert_eq!(a4.column_at(105.0), 3);
    let none = PageFormat { columns: 0, ..a3 };
    assert_eq!(none.column_at(300.0), 1);
    // Pages saved before columns existed get the default.
    let old: PageFormat = serde_json::from_str(r#"{"size": "A3", "landscape": true}"#).unwrap();
    assert_eq!(old.columns, 10);
}
