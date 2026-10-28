//! The matcher against real maker CAD files, which stay out of the
//! repository (the makers' terms limit copying). Point `KOPERCAD_REAL_CAD_DIR`
//! at a folder holding them and run
//! `cargo test -p novo-parts --test cadmatch_real -- --nocapture`;
//! without the variable the tests skip.
//!
//! Verified so far (2026-10-28, files downloaded by hand from se.com):
//! - Schneider LC1D09 `MCADPP0003371_3D-simplified.stp` and
//!   `MCADPP0003371_2D-front.dxf`.

use std::path::PathBuf;
use std::time::Instant;

use novo_parts::*;

fn dir() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var_os("KOPERCAD_REAL_CAD_DIR")?);
    d.is_dir().then_some(d)
}

fn contactor(order: &str) -> CatalogPart {
    CatalogPart {
        manufacturer: "Schneider Electric".into(),
        order_number: order.into(),
        description: "TeSys D contactor 9 A, coil 24 V DC".into(),
        dimensions: Some(Dimensions {
            width_mm: 45.0,
            height_mm: 77.0,
            depth_mm: 86.0,
        }),
        ..Default::default()
    }
}

#[test]
fn the_real_schneider_step_belongs_to_the_lc1d09_to_lc1d18_family_and_to_its_drawing() {
    let Some(dir) = dir() else {
        eprintln!("KOPERCAD_REAL_CAD_DIR not set: skipped");
        return;
    };
    let step_path = dir.join("MCADPP0003371_3D-simplified.stp");
    let dxf_path = dir.join("MCADPP0003371_2D-front.dxf");
    if !step_path.exists() || !dxf_path.exists() {
        eprintln!("the Schneider LC1D09 files are not in the folder: skipped");
        return;
    }
    let step_bytes = std::fs::read(&step_path).unwrap();
    let dxf_bytes = std::fs::read(&dxf_path).unwrap();

    let t = Instant::now();
    let step_id = CadIdentity::read("MCADPP0003371_3D-simplified.stp", &step_bytes);
    eprintln!("identity read in {:?}: {:#?}", t.elapsed(), step_id.names);
    assert!(
        t.elapsed().as_secs() < 5,
        "reading the identity must be quick"
    );
    assert!(
        step_id
            .names
            .iter()
            .any(|n| n.text == "LC1D09-18-DC-3D_ASM"),
        "{:?}",
        step_id.names
    );
    assert_eq!(step_id.maker.as_deref(), Some("Schneider-Electric"));
    assert_eq!(step_id.doc_id.as_deref(), Some("MCADPP0003371"));

    let dxf_id = CadIdentity::read("MCADPP0003371_2D-front.dxf", &dxf_bytes);
    assert_eq!(dxf_id.doc_id.as_deref(), Some("MCADPP0003371"));

    let mut catalog = Catalog::new();
    for o in ["LC1D09BD", "LC1D12BD", "LC1D18BD", "LC1D25BD", "LC1D32BD"] {
        catalog.upsert(contactor(o));
    }
    let out = match_batch(vec![step_id, dxf_id], &catalog, &CadLinks::default(), 10);
    for m in &out {
        let labels: Vec<_> = m.candidates.iter().map(|c| &c.label).collect();
        eprintln!("{} -> {labels:?}", m.identity.file_name);
    }
    let step_labels: Vec<&str> = out[0].candidates.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        step_labels,
        [
            "Schneider Electric LC1D09BD",
            "Schneider Electric LC1D12BD",
            "Schneider Electric LC1D18BD"
        ]
    );
    // The drawing names nothing; it is placed by the set it arrived with.
    assert_eq!(out[1].candidates.len(), 3);

    // The size from all points of this file is 6 to 12 times too large: it
    // must neither confirm nor reject the part.
    let text = String::from_utf8_lossy(&step_bytes);
    let rough = step_box(&text).unwrap();
    eprintln!("all-points box: {:?}", rough.size_mm);
    let with_size =
        CadIdentity::read("MCADPP0003371_3D-simplified.stp", &step_bytes).with_size(SizeMeasure {
            size_mm: rough.size_mm,
            exact: false,
        });
    let c = assess_part(
        &with_size,
        catalog.find("Schneider Electric", "LC1D09BD").unwrap(),
        Some(&catalog),
        &CadLinks::default(),
    );
    assert_eq!(c.verdict, Verdict::Likely, "{c:#?}");
}
