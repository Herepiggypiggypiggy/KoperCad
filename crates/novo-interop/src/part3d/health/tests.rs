use super::*;
use crate::part3d::build::test_support::block_step;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/step-pack");

/// Adds entities before the end of the DATA section.
fn with_extra(base: &str, extra: &str) -> String {
    base.replace(
        "ENDSEC;\nEND-ISO-10303-21;",
        &format!("{extra}ENDSEC;\nEND-ISO-10303-21;"),
    )
}

fn solid_id(text: &str) -> u32 {
    StepFile::parse(text).unwrap().ids_of("MANIFOLD_SOLID_BREP")[0]
}

fn coloured(name: &str, rgb: (f64, f64, f64)) -> String {
    let base = block_step();
    let s = solid_id(&base);
    with_extra(
        &base,
        &format!(
            "#9000=PRODUCT('P','{name}','',(#9001));\n#9002=PRODUCT_DEFINITION_FORMATION('','',#9000);\n\
             #9003=PRODUCT_DEFINITION('design','',#9002,#9004);\n#9005=PRODUCT_DEFINITION_SHAPE('','',#9003);\n\
             #9006=SHAPE_DEFINITION_REPRESENTATION(#9005,#{});\n\
             #9010=STYLED_ITEM('color',(#9011),#{s});\n#9011=PRESENTATION_STYLE_ASSIGNMENT((#9012));\n\
             #9012=SURFACE_STYLE_USAGE(.BOTH.,#9013);\n#9013=SURFACE_SIDE_STYLE('',(#9014));\n\
             #9014=SURFACE_STYLE_FILL_AREA(#9015);\n#9015=FILL_AREA_STYLE('',(#9016));\n\
             #9016=FILL_AREA_STYLE_COLOUR('',#9017);\n#9017=COLOUR_RGB('',{},{},{});\n",
            StepFile::parse(&base)
                .unwrap()
                .ids_of("ADVANCED_BREP_SHAPE_REPRESENTATION")[0],
            rgb.0,
            rgb.1,
            rgb.2
        ),
    )
}

/// Writes the files of the committed pack when asked
/// (`KOPERCAD_WRITE_STEP_PACK=<dir>`); otherwise does nothing.
#[test]
fn write_pack_when_asked() {
    let Some(dir) = std::env::var_os("KOPERCAD_WRITE_STEP_PACK") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let w = |n: &str, t: &str| std::fs::write(dir.join(n), t).unwrap();
    w("block-plain.stp", &block_step());
    w(
        "block-orange-plate.stp",
        &coloured("Mounting plate", (1.0, 0.5, 0.0)),
    );
    w(
        "block-revolved.stp",
        &with_extra(
            &block_step(),
            "#9100=SURFACE_OF_REVOLUTION('',#9101,#9102);\n#9103=OFFSET_SURFACE('',#9101,1.,.F.);\n",
        ),
    );
    w("not-a-step-file.stp", "this is a text file, not a model\n");
    w(
        "no-solids.stp",
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\nENDSEC;\nEND-ISO-10303-21;\n",
    );
}

#[test]
fn a_plain_block_reads_cleanly() {
    let h = step_health(&block_step());
    assert_eq!(h.verdict, Verdict::Clean, "{:?}", h.notes);
    assert_eq!((h.solids, h.faces, h.approximated_faces), (1, 6, 0));
    let s = h.size_mm().unwrap();
    for (got, want) in s.iter().zip([10.0, 20.0, 30.0]) {
        assert!((got - want).abs() < 1e-6, "{s:?}");
    }
    assert!(h.entities > 50 && h.kinds > 5);
    assert!(h.unsupported.is_empty());
    // No colours: said, but not a gap.
    assert!(h.notes.iter().any(|n| n.contains("no colours")));
}

#[test]
fn colour_and_name_are_reported() {
    let h = step_health(&coloured("Mounting plate", (1.0, 0.5, 0.0)));
    assert_eq!(h.verdict, Verdict::Clean);
    assert_eq!(h.style.dominant, Some([255, 128, 0]));
    assert_eq!(h.style.part_name(), Some("Mounting plate"));
    assert_eq!(h.style.solids[0].name, "Mounting plate");
    assert!(!h.notes.iter().any(|n| n.contains("no colours")));
}

#[test]
fn unknown_geometry_kinds_are_listed_and_make_the_result_partial() {
    let t = with_extra(
        &block_step(),
        "#9100=SURFACE_OF_REVOLUTION('',#9101,#9102);\n#9103=OFFSET_SURFACE('',#9101,1.,.F.);\n#9104=SURFACE_STYLE_USAGE(.BOTH.,#9101);\n",
    );
    let h = step_health(&t);
    assert_eq!(h.verdict, Verdict::Partial);
    let names: Vec<&str> = h.unsupported.iter().map(|u| u.entity.as_str()).collect();
    assert_eq!(names, ["OFFSET_SURFACE", "SURFACE_OF_REVOLUTION"]);
    assert!(h.unsupported[0].effect.contains("flat"));
    // Style entities are not geometry.
    assert!(!names.iter().any(|n| n.contains("STYLE")));
}

#[test]
fn garbage_and_empty_files_say_why() {
    let h = step_health("hello");
    assert_eq!(h.verdict, Verdict::Unreadable);
    assert!(h.error.unwrap().contains("not a STEP file"));
    let h = step_health(
        "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\nENDSEC;\nEND-ISO-10303-21;\n",
    );
    assert_eq!(h.verdict, Verdict::Unreadable);
    assert!(h.error.unwrap().contains("none of them a solid"));
}

#[test]
fn header_schema_and_originator() {
    let t = block_step().replace(
        "HEADER;\nENDSEC;",
        "HEADER;\nFILE_SCHEMA(('AP242_MANAGED_MODEL_GEOMETRY'));\nFILE_NAME('x.stp','2020',('a'),('b'),'pre','CREO PARAMETRIC','');\nENDSEC;",
    );
    let h = step_health(&t);
    assert_eq!(h.schema.as_deref(), Some("AP242_MANAGED_MODEL_GEOMETRY"));
    assert_eq!(h.originator.as_deref(), Some("CREO PARAMETRIC"));
}

#[test]
fn a_name_with_brackets_and_quotes_does_not_cut_the_header() {
    let t = block_step().replace(
        "HEADER;\nENDSEC;",
        "HEADER;\nFILE_NAME('x.stp','2020',('a'),('b'),'pre','Maker (v1); it''s odd','');\nENDSEC;",
    );
    assert_eq!(
        step_health(&t).originator.as_deref(),
        Some("Maker (v1); it's odd")
    );
}

#[test]
fn a_header_with_comments_and_spaces_reads() {
    let t = block_step().replace(
        "HEADER;\nENDSEC;",
        "HEADER;\n/* (a 'comment') */\nFILE_NAME(\n/* name */ 'x', 'y', ('a'), ('b'), 'p', 'PARTsolutions', ' ');\nFILE_SCHEMA (('AUTOMOTIVE_DESIGN {1 0}'));\nENDSEC;",
    );
    let h = step_health(&t);
    assert_eq!(h.originator.as_deref(), Some("PARTsolutions"));
    assert_eq!(h.schema.as_deref(), Some("AUTOMOTIVE_DESIGN {1 0}"));
}

#[test]
fn the_committed_pack_gives_the_expected_verdicts() {
    let results = health_of_folder(Path::new(PACK));
    let by = |n: &str| {
        &results
            .iter()
            .find(|(p, _)| p.file_name().unwrap().to_string_lossy() == n)
            .unwrap_or_else(|| panic!("{n} missing from the pack"))
            .1
    };
    assert_eq!(results.len(), 5);
    assert_eq!(by("block-plain.stp").verdict, Verdict::Clean);
    assert_eq!(
        by("block-orange-plate.stp").style.dominant,
        Some([255, 128, 0])
    );
    assert_eq!(by("block-revolved.stp").verdict, Verdict::Partial);
    assert_eq!(by("not-a-step-file.stp").verdict, Verdict::Unreadable);
    assert_eq!(by("no-solids.stp").verdict, Verdict::Unreadable);
}

#[test]
fn the_report_lists_every_file() {
    let results = health_of_folder(Path::new(PACK));
    let md = markdown_report(&results, Path::new(PACK));
    assert!(
        md.contains("5 files: 2 read cleanly, 1 with gaps, 2 cannot be read."),
        "{md}"
    );
    assert!(md.contains("#ff8000"));
    assert!(md.contains("Mounting plate"));
    assert!(markdown_report(&[], Path::new("x")).contains("No STEP files"));
}

#[test]
fn folder_scan_finds_step_files_in_subfolders_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/A.STP"), block_step()).unwrap();
    std::fs::write(dir.path().join("b.step"), block_step()).unwrap();
    std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
    let found = step_files_in(dir.path());
    assert_eq!(found.len(), 2);
    assert!(health_of_folder(&dir.path().join("missing")).is_empty());
}

#[test]
fn step_files_inside_a_zip_are_read_in_place() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let mut z =
        zip::ZipWriter::new(std::fs::File::create(dir.path().join("RITTAL_x.zip")).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    z.start_file("part.stp", opts).unwrap();
    z.write_all(block_step().as_bytes()).unwrap();
    z.start_file("readme.txt", opts).unwrap();
    z.write_all(b"terms").unwrap();
    z.finish().unwrap();
    let r = health_of_folder(dir.path());
    assert_eq!(r.len(), 1);
    assert!(
        r[0].0.ends_with("RITTAL_x.zip/part.stp") || r[0].0.ends_with("RITTAL_x.zip\\part.stp")
    );
    assert_eq!(r[0].1.verdict, Verdict::Clean);
}

#[test]
fn default_folder_is_under_documents() {
    // Reads the default without touching the environment (tests run in threads).
    if std::env::var_os(SAMPLES_ENV).is_none()
        && let Some(p) = default_samples_folder()
    {
        assert!(p.ends_with(Path::new("KoperCAD-tests").join("step-samples")));
    }
}
