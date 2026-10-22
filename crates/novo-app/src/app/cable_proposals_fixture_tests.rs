//! The committed projects behind the Cables drawer scroll scenarios: wires from
//! cabinet devices to field devices and no cable yet, so the drawer lists proposals.

use novo_core::{DeviceTag, Endpoint, Pin, Placement, Point, SymbolRole, Tx};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures")
        .join(name)
}

/// `pairs` cabinet devices (`=A1+CAB-K<n>`), each wired to its own field device
/// (`=A1+FIELD-M<n>`): one proposed cable each.
fn build(doc: &mut novo_core::Document, pairs: u32) {
    doc.transact("Proposals fixture", |tx| {
        let page = tx.add_page("1")?;
        tx.update::<novo_core::Page>(page, |p| p.structure = tag("=A1+CAB"))?;
        let pins = || vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)];
        let place = |tx: &mut Tx<'_>, x: f64, y: f64, t: &str| {
            let device = tx.add_device(tag(t), "Device")?;
            tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(x, y)),
                pins(),
                SymbolRole::Device { device, main: true },
            )
        };
        for n in 0..pairs {
            let y = 20.0 + f64::from(n) * 16.0;
            let k = place(tx, 40.0, y, &format!("=A1+CAB-K{}", n + 1))?;
            let m = place(tx, 120.0, y, &format!("=A1+FIELD-M{}", n + 1))?;
            tx.connect(Endpoint::pin(k, 1), Endpoint::pin(m, 0))?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_fixtures_propose_the_expected_number_of_cables() {
    for (pairs, name) in [(14, "many"), (2, "few"), (0, "none")] {
        let mut doc = novo_core::Document::default();
        build(&mut doc, pairs);
        let got = novo_cables::propose_cables(doc.project()).len();
        assert_eq!(got, pairs as usize, "{name}");
    }
}

/// Rewrites the fixtures: `cargo test -p novo-app write_cable_proposal_fixtures -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/cable-proposals-*.novo"]
fn write_cable_proposal_fixtures() {
    use crate::backend::ProjectBackend;
    for (pairs, name) in [
        (14, "cable-proposals-many.novo"),
        (2, "cable-proposals-few.novo"),
        (0, "cable-proposals-none.novo"),
    ] {
        let mut backend = crate::backend::StorageBackend::default();
        backend.new_project("Cable proposals").unwrap();
        build(backend.document_mut().unwrap(), pairs);
        let path = fixture_path(name);
        let _ = std::fs::remove_file(&path);
        backend.save(&path).unwrap();
    }
}
