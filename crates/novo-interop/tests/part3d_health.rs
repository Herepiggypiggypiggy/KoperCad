//! Model health report (lane STEP-HEALTH) on synthetic files.

use std::sync::atomic::AtomicBool;

use novo_interop::part3d::health::{Severity, Support, check_step, decode_text};

const MM: &str = "#9=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#8))REPRESENTATION_CONTEXT('',''));\n#8=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n";

/// A cylinder of radius `r` and height `h` along z, written the way CAD
/// programs do: circles, one seam line, planar caps and a cylindrical wall.
fn cylinder_step(r: f64, h: f64, extra: &str) -> String {
    let mut s = String::from(
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n",
    );
    s.push_str(MM);
    s.push_str(&format!(
        "#100=CARTESIAN_POINT('',(0.,0.,0.));\n#101=CARTESIAN_POINT('',(0.,0.,{h}));\n\
#102=DIRECTION('',(0.,0.,1.));\n#103=DIRECTION('',(0.,0.,-1.));\n#104=DIRECTION('',(1.,0.,0.));\n\
#105=AXIS2_PLACEMENT_3D('',#100,#102,#104);\n#106=AXIS2_PLACEMENT_3D('',#101,#102,#104);\n\
#107=AXIS2_PLACEMENT_3D('',#100,#103,#104);\n\
#110=CARTESIAN_POINT('',({r},0.,0.));\n#111=CARTESIAN_POINT('',({r},0.,{h}));\n\
#112=VERTEX_POINT('',#110);\n#113=VERTEX_POINT('',#111);\n\
#120=CIRCLE('',#105,{r});\n#121=CIRCLE('',#106,{r});\n\
#122=DIRECTION('',(0.,0.,1.));\n#123=VECTOR('',#122,1.);\n#124=LINE('',#110,#123);\n\
#130=EDGE_CURVE('',#112,#112,#120,.T.);\n#131=EDGE_CURVE('',#113,#113,#121,.T.);\n\
#132=EDGE_CURVE('',#112,#113,#124,.T.);\n\
#140=ORIENTED_EDGE('',*,*,#130,.T.);\n#141=ORIENTED_EDGE('',*,*,#131,.T.);\n\
#142=ORIENTED_EDGE('',*,*,#132,.T.);\n#143=ORIENTED_EDGE('',*,*,#132,.F.);\n\
#144=ORIENTED_EDGE('',*,*,#131,.F.);\n#145=ORIENTED_EDGE('',*,*,#130,.F.);\n\
#150=EDGE_LOOP('',(#145));\n#151=EDGE_LOOP('',(#141));\n\
#152=EDGE_LOOP('',(#140,#142,#144,#143));\n\
#160=FACE_OUTER_BOUND('',#150,.T.);\n#161=FACE_OUTER_BOUND('',#151,.T.);\n#162=FACE_OUTER_BOUND('',#152,.T.);\n\
#170=PLANE('',#107);\n#171=PLANE('',#106);\n#172=CYLINDRICAL_SURFACE('',#105,{r});\n\
#180=ADVANCED_FACE('',(#160),#170,.T.);\n#181=ADVANCED_FACE('',(#161),#171,.T.);\n\
#182=ADVANCED_FACE('',(#162),#172,.T.);\n\
#190=CLOSED_SHELL('',(#180,#181,#182));\n#191=MANIFOLD_SOLID_BREP('Mounting plate',#190);\n\
#192=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#191),#9);\nENDSEC;\nEND-ISO-10303-21;\n"
    ));
    s = s.replace("ENDSEC;\nEND-ISO", &format!("{extra}ENDSEC;\nEND-ISO"));
    s
}

const STYLE: &str = "#300=COLOUR_RGB('',0.5,0.25,1.);\n\
#301=FILL_AREA_STYLE_COLOUR('',#300);\n#302=FILL_AREA_STYLE('',(#301));\n\
#303=SURFACE_STYLE_FILL_AREA(#302);\n#304=SURFACE_SIDE_STYLE('',(#303));\n\
#305=SURFACE_STYLE_USAGE(.BOTH.,#304);\n#306=PRESENTATION_STYLE_ASSIGNMENT((#305));\n\
#307=STYLED_ITEM('',(#306),#191);\n\
#310=PRODUCT('P1','T\\X2\\00FC\\X0\\r plate','',(#9));\n";

#[test]
fn reads_a_clean_file_and_its_names_and_colours() {
    let h = check_step(
        cylinder_step(5.0, 10.0, STYLE).as_bytes(),
        &AtomicBool::new(false),
    );
    assert_eq!(h.worst(), Severity::Info, "{}", h.to_text());
    assert_eq!(h.schema.as_deref(), Some("AUTOMOTIVE_DESIGN"));
    assert_eq!(h.unit_mm, Some(1.0));
    let s = h.stats.unwrap();
    assert_eq!((s.solids, s.faces, s.skipped_faces), (1, 3, 0));
    let sz = h.size_mm.unwrap();
    assert!(
        (sz[0] - 10.0).abs() < 0.1 && (sz[2] - 10.0).abs() < 0.1,
        "{sz:?}"
    );
    assert!(h.surfaces.iter().all(|s| s.support == Support::Exact));
    assert_eq!(h.appearance.products, vec!["T\u{fc}r plate".to_owned()]);
    assert_eq!(h.appearance.solids[0].name, "Mounting plate");
    assert_eq!(h.appearance.solids[0].rgb, Some([128, 64, 255]));
    assert_eq!(h.appearance.colours[0].hex(), "#8040ff");
    assert_eq!(h.appearance.colours[0].solids, 1);
}

#[test]
fn face_colours_give_the_solid_its_dominant_colour() {
    let style = STYLE.replace(
        "#307=STYLED_ITEM('',(#306),#191);",
        "#307=STYLED_ITEM('',(#306),#180);\n#308=STYLED_ITEM('',(#306),#181);",
    );
    let h = check_step(
        cylinder_step(5.0, 10.0, &style).as_bytes(),
        &AtomicBool::new(false),
    );
    assert_eq!(h.appearance.solids[0].rgb, Some([128, 64, 255]));
    assert_eq!(h.appearance.colours[0].faces, 2);
    assert_eq!(h.appearance.colours[0].solids, 0);
}

#[test]
fn a_plain_file_notes_missing_colours() {
    let h = check_step(
        cylinder_step(5.0, 10.0, "").as_bytes(),
        &AtomicBool::new(false),
    );
    assert!(h.appearance.colours.is_empty());
    assert!(h.findings.iter().any(|f| f.text.contains("no colours")));
}

#[test]
fn text_that_is_not_step_is_a_problem_not_a_panic() {
    let h = check_step(b"hello", &AtomicBool::new(false));
    assert_eq!(h.worst(), Severity::Problem);
    assert!(h.build_error.is_some());
    let h = check_step(
        b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n",
        &AtomicBool::new(false),
    );
    assert_eq!(h.worst(), Severity::Problem);
}

#[test]
fn unknown_surface_types_are_listed_as_approximated() {
    let extra = "#400=DEGENERATE_TOROIDAL_SURFACE('',#105,10.,2.,.T.);\n#401=ADVANCED_FACE('',(#160),#400,.T.);\n";
    let h = check_step(
        cylinder_step(5.0, 10.0, extra).as_bytes(),
        &AtomicBool::new(false),
    );
    let s = h
        .surfaces
        .iter()
        .find(|s| s.entity == "DEGENERATE_TOROIDAL_SURFACE")
        .unwrap();
    assert_eq!((s.faces, s.support), (1, Support::Approximate));
    assert!(
        h.findings
            .iter()
            .any(|f| f.text.contains("DEGENERATE_TOROIDAL_SURFACE"))
    );
}

#[test]
fn step_text_escapes_decode() {
    assert_eq!(decode_text("a\\X2\\00E9\\X0\\b"), "a\u{e9}b");
    assert_eq!(decode_text("\\X\\E9"), "\u{e9}");
    assert_eq!(decode_text("\\S\\i"), "\u{e9}");
    assert_eq!(decode_text("plain"), "plain");
    assert_eq!(decode_text("\\X2\\00"), "");
}
