//! Builds the pin names QA fixture: a motor whose pins are `U V W PE` and a
//! relay coil whose pin is `a1`, so Check > Issues lists two Pin names rows
//! with a clear rename.
//!
//!   cargo run -p novo-tests --example pin_names_fixture -- qa/fixtures
//!
//! Writes `OUT_DIR/pin-names.kopercad` and `OUT_DIR/pin-offer.kopercad` (one relay
//! coil -K1 with the usual pins A1 A2 and no part, for the part terminal offer).

use novo_core::{DeviceTag, Document, Rotation, SymbolInstance};
use novo_editor::catalog::Catalog;
use novo_editor::edit::PlaceOptions;
use novo_storage::Session;
use novo_tests::text_stress::Builder;

fn rename(tx: &mut novo_core::Tx<'_>, id: novo_core::SymbolId, from: &[&str], to: &[&str]) {
    tx.update::<SymbolInstance>(id, |s| {
        for pin in &mut s.pins {
            if let Some(at) = from.iter().position(|f| *f == pin.name) {
                pin.name = to[at].to_owned();
            }
        }
    })
    .expect("rename");
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let catalog = Catalog::builtin();
    let mut doc = Document::default();
    doc.transact("Pin names", |tx| {
        let mut b = Builder {
            tx,
            catalog: &catalog,
        };
        let r0 = (Rotation::R0, false);
        let opts = |tag: &str| PlaceOptions {
            parent: DeviceTag::parse(tag).ok(),
            ..PlaceOptions::new()
        };
        let page = b.page("1")?;
        let motor = b.put(page, "iec/motors/motor_3ph", (80.0, 60.0), r0, &opts("-M1"));
        let coil = b.put(page, "iec/coils/relay", (160.0, 60.0), r0, &opts("-K1"));
        rename(
            b.tx,
            motor.expect("motor"),
            &["U1", "V1", "W1"],
            &["U", "V", "W"],
        );
        rename(b.tx, coil.expect("coil"), &["A1"], &["a1"]);
        Ok(())
    })
    .expect("fixture builds");
    let path = out.join("pin-names.kopercad");
    let _ = std::fs::remove_file(&path);
    Session::create(&path, doc.project()).expect("project file");
    println!("wrote {}", path.display());

    let mut doc = Document::default();
    doc.transact("Pin offer", |tx| {
        let mut b = Builder {
            tx,
            catalog: &catalog,
        };
        let page = b.page("1")?;
        b.put(
            page,
            "iec/coils/relay",
            (120.0, 60.0),
            (Rotation::R0, false),
            &PlaceOptions {
                parent: DeviceTag::parse("-K1").ok(),
                ..PlaceOptions::new()
            },
        );
        Ok(())
    })
    .expect("fixture builds");
    let path = out.join("pin-offer.kopercad");
    let _ = std::fs::remove_file(&path);
    Session::create(&path, doc.project()).expect("project file");
    println!("wrote {}", path.display());
}
