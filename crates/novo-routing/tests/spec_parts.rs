//! Factory ducts and the cable entry from the enclosure's spec change the
//! routes; a stored entry point of the designer wins over the spec's.

use novo_core::{
    DeviceTag, MountingPlate, PinRef, Placement, Point as CorePoint, Project, StructureId,
    SymbolRole, Value,
};
use novo_layout::spec::{CableEntry, EnclosureSpec, FactoryPart, PartKind, Source};
use novo_routing::model::{Connection, SPEC_DUCT_BASE, route_connections};
use novo_routing::{Kind, Rules};

fn pin(tx: &mut novo_core::Tx<'_>, page: novo_core::PageId, device: novo_core::DeviceId) -> PinRef {
    let symbol = tx
        .place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(CorePoint::new(0.0, 0.0)),
            vec![novo_core::Pin::new("1", 0.0, 0.0)],
            SymbolRole::Device { device, main: true },
        )
        .unwrap();
    PinRef::new(symbol, 0)
}

/// K1 at the top left and K2 at the bottom right of a 400 mm plate, M1 outside.
fn project() -> (Project, StructureId, Vec<PinRef>) {
    let mut p = Project::new("t");
    let (out, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, x, y, placed) in [
                ("=A1+CAB1-K1", 50.0, 50.0, true),
                ("=A1+CAB1-K2", 300.0, 250.0, true),
                ("=A1+FLD-M1", 0.0, 0.0, false),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                if placed {
                    tx.place_device(d, cab, CorePoint::new(x, y), false, None)?;
                }
                pins.push(pin(tx, page, d));
            }
            Ok((cab, pins))
        })
        .unwrap();
    (p, out.0, out.1)
}

fn conn(id: u64, a: PinRef, b: PinRef) -> Connection {
    Connection {
        id,
        from: a,
        to: b,
        diameter_mm: Some(3.0),
    }
}

fn set_spec(p: &mut Project, cab: StructureId, spec: &EnclosureSpec) {
    p.transact("spec", |tx| {
        let old: Vec<String> = tx
            .project()
            .get::<novo_core::Structure>(cab)
            .unwrap()
            .properties
            .keys()
            .filter(|k| k.starts_with("encl."))
            .cloned()
            .collect();
        for k in old {
            tx.set_property(cab, &k, None)?;
        }
        let mut props = novo_core::Properties::default();
        spec.write(&mut props);
        for (k, v) in props {
            tx.set_property(cab, &k, Some(v))?;
        }
        Ok(())
    })
    .unwrap();
}

fn duct(x: f64, source: Source) -> FactoryPart {
    FactoryPart {
        kind: PartKind::Duct,
        x,
        y: 0.0,
        length: 400.0,
        vertical: true,
        width: 40.0,
        height: 60.0,
        source,
    }
}

fn entry(x: f64, y: f64, source: Source) -> CableEntry {
    CableEntry {
        name: "gland plate".into(),
        wall: "bottom".into(),
        x,
        y,
        diameter: 32.0,
        source,
    }
}

#[test]
fn a_factory_duct_from_the_spec_carries_the_wire() {
    let (mut p, cab, pins) = project();
    let wire = |p: &Project| route_connections(p, &[conn(1, pins[0], pins[1])], &Rules::default());
    let none = wire(&p);
    assert_eq!(none.routes[&1].kind, Kind::FreeAir, "no duct, no spec");
    assert!(none.spec_estimate.is_empty());

    let spec = |x: f64, source: Source| EnclosureSpec {
        article: "T1".into(),
        parts: vec![duct(x, source)],
        ..EnclosureSpec::default()
    };
    set_spec(&mut p, cab, &spec(150.0, Source::Catalogue));
    let a = wire(&p);
    let ra = &a.routes[&1];
    assert_eq!(ra.kind, Kind::Ducted);
    assert!(
        ra.ducts.iter().all(|d| *d >= SPEC_DUCT_BASE),
        "{:?}",
        ra.ducts
    );
    // A catalogue position is an estimate.
    assert!(a.spec_estimate.contains(&1));

    // Another duct position: another wire length.
    set_spec(&mut p, cab, &spec(380.0, Source::Catalogue));
    let b = wire(&p);
    assert_ne!(ra.length_mm, b.routes[&1].length_mm);

    // Read from the STEP: used the same, but no estimate.
    set_spec(&mut p, cab, &spec(150.0, Source::Step));
    let c = wire(&p);
    assert_eq!(c.routes[&1].length_mm, ra.length_mm);
    assert!(c.spec_estimate.is_empty());
}

#[test]
fn the_spec_entry_is_the_cabinet_entry_until_one_is_stored() {
    use novo_routing::entry::{entry_from_spec, entry_point, set_entry_point};
    let (mut p, cab, pins) = project();
    assert_eq!(entry_point(&p, cab), None);
    assert!(!entry_from_spec(&p, cab));
    let spec = |x: f64, source: Source| EnclosureSpec {
        article: "T1".into(),
        entries: vec![entry(x, 400.0, source)],
        ..EnclosureSpec::default()
    };
    set_spec(&mut p, cab, &spec(100.0, Source::Catalogue));
    assert_eq!(
        entry_point(&p, cab).map(|e| (e.x, e.y)),
        Some((100.0, 400.0))
    );
    assert!(entry_from_spec(&p, cab));
    let wire = |p: &Project| route_connections(p, &[conn(2, pins[0], pins[2])], &Rules::default());
    let a = wire(&p);
    assert_eq!(a.entry_legs.get(&2), Some(&cab));
    assert!(a.spec_estimate.contains(&2));
    // A different entry position: a different length.
    set_spec(&mut p, cab, &spec(350.0, Source::Catalogue));
    let b = wire(&p);
    assert_ne!(a.routes[&2].length_mm, b.routes[&2].length_mm);
    // The designer's own entry point wins, and is not an estimate of the spec.
    p.transact("entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(200.0, 400.0)))
    })
    .unwrap();
    assert!(!entry_from_spec(&p, cab));
    let c = wire(&p);
    assert!(!c.spec_estimate.contains(&2));
}
