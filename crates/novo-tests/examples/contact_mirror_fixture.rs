//! Builds the contact mirror QA fixture: coils whose tables hold every kind
//! of contact (a three-pole and a four-pole main contact, normally open,
//! normally closed, changeover) and spare contacts.
//!
//!   cargo run -p novo-tests --example contact_mirror_fixture -- qa/fixtures
//!
//! Writes `OUT_DIR/contact-mirror.novo`.

use novo_core::{DeviceTag, Document, Rotation};
use novo_editor::catalog::Catalog;
use novo_editor::edit::PlaceOptions;
use novo_storage::Session;
use novo_tests::text_stress::Builder;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let catalog = Catalog::builtin();
    let mut doc = Document::default();
    doc.transact("Contact mirror", |tx| {
        let mut b = Builder {
            tx,
            catalog: &catalog,
        };
        let r0 = (Rotation::R0, false);
        let opts = |tag: &str| PlaceOptions {
            parent: DeviceTag::parse(tag).ok(),
            ..PlaceOptions::new()
        };
        let power = b.page("1")?;
        let control = b.page("2")?;
        let aux = b.page("3")?;
        // -K1: a contactor with a three-pole main contact, a normally open,
        // a normally closed and a changeover contact, and two spare ones.
        b.put(
            control,
            "iec/coils/contactor",
            (80.0, 50.0),
            r0,
            &opts("-K1"),
        );
        b.put(
            power,
            "iec/contactors/main_3p",
            (60.0, 50.0),
            r0,
            &opts("-K1"),
        );
        b.put(aux, "iec/contacts/no", (40.0, 50.0), r0, &opts("-K1"));
        b.put(aux, "iec/contacts/nc", (80.0, 50.0), r0, &opts("-K1"));
        b.put(
            aux,
            "iec/contacts/changeover",
            (120.0, 50.0),
            r0,
            &opts("-K1"),
        );
        // -K2: a relay with a four-pole main contact and one normally open.
        b.put(control, "iec/coils/relay", (200.0, 50.0), r0, &opts("-K2"));
        b.put(
            power,
            "iec/contactors/main_4p",
            (140.0, 50.0),
            r0,
            &opts("-K2"),
        );
        b.put(aux, "iec/contacts/no", (160.0, 50.0), r0, &opts("-K2"));
        // -K3: a relay with a single contact, fully used.
        b.put(control, "iec/coils/relay", (320.0, 50.0), r0, &opts("-K3"));
        b.put(aux, "iec/contacts/no", (200.0, 50.0), r0, &opts("-K3"));
        Ok(())
    })
    .expect("fixture builds");
    let k1 = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string().ends_with("-K1"))
        .expect("-K1")
        .id;
    doc.transact("Spare contacts", |tx| {
        for (key, n) in [
            ("contacts.main", 1i64),
            ("contacts.no", 3),
            ("contacts.nc", 2),
            ("contacts.co", 1),
        ] {
            tx.set_property(k1, key, Some(n.into()))?;
        }
        Ok(())
    })
    .expect("limits");
    let path = out.join("contact-mirror.novo");
    let _ = std::fs::remove_file(&path);
    Session::create(&path, doc.project()).expect("project file");
    println!("wrote {}", path.display());
}
