//! Maker parts XML (Data Portal family) reader. Every file below is invented.

use novo_core::Value;
use novo_parts::{ConnectionKind, PortalError, PortalFile, looks_like_portal, read_portal};

fn file(name: &str, text: &str) -> PortalFile {
    PortalFile {
        name: name.into(),
        text: text.into(),
    }
}

const PART: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<partsmanagement length-unit="mm" weight-unit="kg">
  <part partnr="ACM.TB-2.5" ordernr="TB-2.5" manufacturer="ACM" typenr="TB 2.5"
        ean="4012345678901" width="5,2" height="48" depth="36.5" weight="0,012"
        descr1="en_US@Feed-through terminal 2.5 mm2;nl_NL@Doorgangsklem 2,5 mm2;"
        datasheeturl="https://example.invalid/tb25.pdf" eclassnumber="27-14-11-01"
        currentiec="24,0 A" voltageiec="800 V" powerdissipation="0,35 W"
        crosssectionfrom="0,2 mm2" crosssectionto="4,0 mm2"
        accessorylist="ACM.TB-ACC" colour="grey">
    <variant name="grey"/>
  </part>
</partsmanagement>"#;

const MAKER: &str = r#"<partsmanagement>
  <manufacturer shortname="ACM" longname="Acme Terminals GmbH" url="https://example.invalid"/>
</partsmanagement>"#;

const POINTS: &str = r#"<partsmanagement length-unit="mm">
  <connectionpoints partnr="ACM.TB-2.5">
    <connectionpoint name="1" x="0" y="-24" z="18,5" dirx="0" diry="-1" dirz="0"
        crosssectionmin="0,2 mm2" crosssectionmax="4 mm2" awgmin="24" awgmax="12"
        maxwirecount="2" strippinglength="10" torquemin="0,5 Nm" torquemax="60 Ncm"
        typeofterminal="Screw connection" screwdriver="PZ1" twinsleeve="no"/>
    <connectionpoint name="2" x="0" y="24" z="18,5" crosssectionmin="0,2 mm2" crosssectionmax="4 mm2"
        typeofterminal="Push-in"/>
  </connectionpoints>
</partsmanagement>"#;

const DRILL: &str = r#"<partsmanagement length-unit="mm">
  <construction partnr="ACM.TB-2.5">
    <drill x="0" y="0" diameter="4,2" type="drill" repeatcount="3" repeatx="35" repeaty="0"/>
    <drill x="10" y="5" diameter="3"/>
  </construction>
</partsmanagement>"#;

const ACC: &str = r#"<partsmanagement>
  <accessorylist>
    <accessory partnr="ACM.TB-JMP2" quantity="1"/>
    <accessory partnr="ACM.TB-END"/>
  </accessorylist>
</partsmanagement>"#;

fn all() -> Vec<PortalFile> {
    vec![
        file("ACM.TB-2.5.part.xml", PART),
        file("ACM.manufacturer.xml", MAKER),
        file("ACM.TB-2.5.connectionpoints.xml", POINTS),
        file("ACM.TB-2.5.construction.xml", DRILL),
        file("ACM.TB-ACC.accessorylist.xml", ACC),
    ]
}

fn text(v: Option<&Value>) -> &str {
    match v {
        Some(Value::Text(t)) => t,
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn a_part_with_its_companion_files_reads_as_one_part() {
    let r = read_portal(&all(), "", "2026-10-29").unwrap();
    assert_eq!(r.parts.len(), 1);
    let p = &r.parts[0];
    assert_eq!(
        p.manufacturer, "Acme Terminals GmbH",
        "from the manufacturer file"
    );
    assert_eq!(p.order_number, "TB-2.5");
    assert_eq!(p.type_number, "TB 2.5");
    assert_eq!(
        p.description, "Feed-through terminal 2.5 mm2",
        "English first"
    );
    assert_eq!(text(p.data.get("description_nl")), "Doorgangsklem 2,5 mm2");
    assert_eq!(text(p.data.get("gtin")), "4012345678901");
    assert_eq!(
        text(p.data.get("datasheet_url")),
        "https://example.invalid/tb25.pdf"
    );
    assert_eq!(p.class.eclass.as_deref(), Some("27-14-11-01"));
    let d = p.dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (5.2, 48.0, 36.5));
    assert_eq!(p.data.get("weight_kg"), Some(&Value::Float(0.012)));
    assert_eq!(p.data.get("rated_current_a"), Some(&Value::Float(24.0)));
    assert_eq!(p.data.get("rated_voltage_v"), Some(&Value::Float(800.0)));
    assert_eq!(p.data.get("power_loss_w"), Some(&Value::Float(0.35)));
    assert_eq!(p.data.get("section_min_mm2"), Some(&Value::Float(0.2)));
    assert_eq!(p.data.get("section_max_mm2"), Some(&Value::Float(4.0)));
    assert_eq!(text(p.data.get("eplan.colour")), "grey");
    let prov = p.provenance.as_ref().unwrap();
    assert_eq!(prov.source, "import:partsxml");
    assert!(!prov.redistributable());
    assert_eq!(prov.imported, "2026-10-29");
}

#[test]
fn connection_points_carry_wire_range_torque_and_strip_length() {
    let r = read_portal(&all(), "", "").unwrap();
    let t = &r.parts[0].terminals;
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].name, "1");
    assert_eq!(t[0].wire_mm2, Some((0.2, 4.0)));
    assert_eq!(t[0].awg, Some((24.0, 12.0)));
    assert_eq!(t[0].max_wires, Some(2));
    assert_eq!(t[0].strip_mm, Some(10.0));
    assert_eq!(t[0].torque_nm, Some((0.5, 0.6)), "50 Ncm is read as 0.5 Nm");
    assert_eq!(t[0].kind, ConnectionKind::Screw);
    assert_eq!(t[0].maker_position_mm, Some([0.0, -24.0, 18.5]));
    assert_eq!(t[0].direction, Some([0.0, -1.0, 0.0]));
    assert_eq!(
        t[0].position_mm, None,
        "the maker's frame is not trusted for routing"
    );
    assert_eq!(
        t[0].extra.get("screwdriver").map(String::as_str),
        Some("PZ1")
    );
    assert_eq!(t[1].kind, ConnectionKind::PushIn);
    assert_eq!(t[1].torque_nm, None);
    assert_eq!(
        r.parts[0].sources.get("connection_data"),
        Some(&novo_parts::DataSource::Estimated)
    );
    assert_eq!(r.counts.terminals, 2);
    assert_eq!(r.counts.with_wire_range, 2);
    assert_eq!(r.counts.with_torque, 1);
}

#[test]
fn drilling_repeats_expand_to_positions() {
    let r = read_portal(&all(), "", "").unwrap();
    let holes = &r.parts[0].drilling;
    assert_eq!(holes.len(), 2);
    assert_eq!(holes[0].diameter_mm, Some(4.2));
    assert_eq!(holes[0].kind, "drill");
    assert_eq!(
        holes[0].positions(),
        vec![[0.0, 0.0], [35.0, 0.0], [70.0, 0.0]]
    );
    assert_eq!(holes[1].positions(), vec![[10.0, 5.0]]);
    assert_eq!(text(r.parts[0].data.get("drill_basis")), "maker");
    assert_eq!(r.counts.holes, 4);
    assert_eq!(r.counts.parts_with_drilling, 1);
}

#[test]
fn an_accessory_list_named_by_the_part_is_attached() {
    let r = read_portal(&all(), "", "").unwrap();
    let acc = &r.parts[0].accessories;
    assert_eq!(acc.len(), 2);
    assert_eq!(acc[0].part.order_number, "TB-JMP2");
    assert_eq!(acc[0].quantity, Some(1));
    assert_eq!(acc[1].quantity, None, "no quantity stays unknown");
    assert_eq!(acc[1].part.manufacturer, "ACM");
}

#[test]
fn file_order_does_not_matter() {
    let mut files = all();
    files.reverse();
    let r = read_portal(&files, "", "").unwrap();
    assert_eq!(r.parts.len(), 1);
    assert_eq!(r.parts[0].terminals.len(), 2);
    assert_eq!(r.parts[0].drilling.len(), 2);
}

#[test]
fn what_it_did_not_understand_and_what_it_assumed_are_reported() {
    let r = read_portal(&all(), "", "").unwrap();
    assert!(
        r.unrecognised.contains_key("part/colour"),
        "{:?}",
        r.unrecognised
    );
    assert!(r.unrecognised.contains_key("part/variant"));
    assert!(r.unrecognised.contains_key("connection point/screwdriver"));
    assert!(
        r.assumed.iter().any(|a| a.contains("maker's own frame")),
        "{:?}",
        r.assumed
    );
    let report = r.format_report();
    assert!(report.contains("part/colour"));
    assert!(!report.contains("grey"), "values never leave in the report");
}

#[test]
fn companion_files_without_their_part_are_listed() {
    let files = vec![
        file("ACM.TB-2.5.part.xml", PART),
        file(
            "ACM.OTHER-1.connectionpoints.xml",
            POINTS.replace("ACM.TB-2.5", "ACM.OTHER-1").as_str(),
        ),
    ];
    let r = read_portal(&files, "", "").unwrap();
    assert!(r.parts[0].terminals.is_empty());
    assert_eq!(r.unattached.len(), 1);
    assert!(r.unattached[0].contains("OTHER-1"), "{:?}", r.unattached);
}

#[test]
fn companion_files_alone_say_what_is_missing() {
    let e = read_portal(&[file("a.construction.xml", DRILL)], "", "").unwrap_err();
    assert!(matches!(e, PortalError::NoParts));
    assert!(e.to_string().contains("part files"));
}

#[test]
fn other_files_are_skipped_not_fatal() {
    let bme = r#"<?xml version="1.0"?><BMECAT version="2005"></BMECAT>"#;
    let files = vec![
        file("cat.xml", bme),
        file("broken.xml", "<partsmanagement><part"),
        file("ACM.TB-2.5.part.xml", PART),
    ];
    let r = read_portal(&files, "", "").unwrap();
    assert_eq!(r.parts.len(), 1);
    assert_eq!(r.skipped.len(), 2, "{:?}", r.skipped);
    assert!(r.skipped[0].starts_with("cat.xml"));
    let none = read_portal(&[file("cat.xml", bme)], "", "").unwrap_err();
    assert!(matches!(none, PortalError::NoPortalFiles(1)));
}

#[test]
fn the_maker_comes_from_the_code_in_the_part_number_or_the_chosen_default() {
    let xml = |nr: &str| {
        format!(r#"<partsmanagement><part partnr="{nr}" descr1="x"/></partsmanagement>"#)
    };
    let r = read_portal(&[file("a.xml", &xml("PXC.3044076"))], "", "").unwrap();
    assert_eq!(r.parts[0].manufacturer, "Phoenix Contact");
    assert_eq!(r.parts[0].order_number, "3044076");
    let r = read_portal(&[file("a.xml", &xml("SK3305.500"))], "Rittal", "").unwrap();
    assert_eq!(r.parts[0].manufacturer, "Rittal");
    assert_eq!(
        r.parts[0].order_number, "SK3305.500",
        "a dot inside an article number stays"
    );
}

#[test]
fn a_part_with_no_maker_anywhere_is_left_out_with_the_reason() {
    let xml = r#"<partsmanagement><part partnr="SK3305.500"/></partsmanagement>"#;
    let r = read_portal(&[file("a.xml", xml)], "", "");
    // No part could be built: nothing to import.
    assert!(matches!(r, Err(PortalError::NoParts)));
}

#[test]
fn the_files_length_unit_is_applied() {
    let xml = r#"<partsmanagement length-unit="cm"><part partnr="ACM.X" manufacturer="ACM"
        width="2" height="4,5" depth="3"/></partsmanagement>"#;
    let r = read_portal(&[file("a.xml", xml)], "", "").unwrap();
    let d = r.parts[0].dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (20.0, 45.0, 30.0));
    assert!(r.assumed.iter().any(|a| a.contains("cm")));
}

#[test]
fn property_children_and_eplan_style_attribute_names_are_read() {
    let xml = r#"<partsmanagement length-unit="mm">
      <propertydefinition id="20999" name="Colour"/>
      <part P_ARTICLE_PARTNR="WAGO.2002-1201" P_ARTICLE_ORDERNR="2002-1201"
            P_ARTICLE_MANUFACTURER="WAGO" P_ARTICLE_WIDTH="5,2" P_ARTICLE_HEIGHT="85" P_ARTICLE_DEPTH="42">
        <property id="20999" value="grey"/>
        <property name="eCl@ss number" value="27141101"/>
      </part></partsmanagement>"#;
    let r = read_portal(&[file("a.xml", xml)], "", "").unwrap();
    let p = &r.parts[0];
    assert_eq!(p.manufacturer, "WAGO");
    assert_eq!(p.order_number, "2002-1201");
    assert_eq!(p.dimensions.unwrap().height_mm, 85.0);
    assert_eq!(text(p.data.get("eplan.colour")), "grey");
    assert_eq!(p.class.eclass.as_deref(), Some("27141101"));
}

#[test]
fn the_later_file_wins_for_the_same_part() {
    let a = PART.replace("TB 2.5", "FIRST");
    let b = PART.replace("TB 2.5", "SECOND");
    let r = read_portal(&[file("a.part.xml", &a), file("b.part.xml", &b)], "", "").unwrap();
    assert_eq!(r.parts.len(), 1);
    assert_eq!(r.parts[0].type_number, "SECOND");
    assert!(r.skipped.iter().any(|s| s.contains("the later file")));
}

#[test]
fn hostile_nesting_is_refused() {
    let deep = format!(
        "<partsmanagement>{}{}</partsmanagement>",
        "<a>".repeat(200),
        "</a>".repeat(200)
    );
    let r = read_portal(&[file("a.xml", &deep), file("ok.xml", PART)], "", "").unwrap();
    assert!(
        r.skipped.iter().any(|s| s.contains("nests deeper")),
        "{:?}",
        r.skipped
    );
}

#[test]
fn only_partsmanagement_files_are_recognised() {
    assert!(looks_like_portal(
        "\u{feff}<?xml version=\"1.0\"?>\r\n<partsmanagement length-unit=\"mm\">"
    ));
    assert!(!looks_like_portal("<BMECAT>"));
}

#[test]
fn the_part_survives_a_catalog_round_trip_with_the_new_fields() {
    let r = read_portal(&all(), "", "").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.json");
    let mut cat = novo_parts::Catalog::new();
    cat.upsert(r.parts[0].clone());
    cat.save(&path).unwrap();
    let back = novo_parts::Catalog::load(&path).unwrap();
    let p = back.get(&r.parts[0].key()).unwrap();
    assert_eq!(p.terminals, r.parts[0].terminals);
    assert_eq!(p.drilling, r.parts[0].drilling);
    assert_eq!(p.accessories, r.parts[0].accessories);
}

#[test]
fn reading_over_an_existing_part_keeps_its_price_and_files() {
    let r = read_portal(&all(), "", "").unwrap();
    let mut old = novo_parts::CatalogPart::new("Acme Terminals GmbH", "TB-2.5");
    old.price = Some(novo_parts::Price::new(0.31, "EUR", 1, "2026-10-01"));
    old.class.etim = Some("EC000897".into());
    let merged = old.with_maker_xml(&r.parts[0]);
    assert_eq!(merged.price.as_ref().unwrap().amount, 0.31);
    assert_eq!(merged.class.etim.as_deref(), Some("EC000897"));
    assert_eq!(merged.terminals.len(), 2);
    assert_eq!(merged.drilling.len(), 2);
}

#[test]
fn a_folder_of_maker_parts_xml_imports_through_the_inbox() {
    use novo_parts::{Catalog, CatalogPart, InboxOptions, Price, import_inbox};
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox");
    std::fs::create_dir_all(inbox.join("acme")).unwrap();
    for f in all() {
        std::fs::write(inbox.join("acme").join(&f.name), f.text).unwrap();
    }
    // A part from an earlier BMEcat import: its price must survive.
    let cat = dir.path().join("parts.json");
    let mut c = Catalog::new();
    let mut old = CatalogPart::new("Acme Terminals GmbH", "TB-2.5");
    old.price = Some(Price::new(0.31, "EUR", 1, "2026-10-01"));
    c.upsert(old);
    c.save(&cat).unwrap();

    let opts = InboxOptions {
        today: "2026-10-29".into(),
        dry_run: false,
    };
    let report = import_inbox(&inbox, Some(&cat), &opts).unwrap();
    assert_eq!(report.total_read(), 1, "{}", report.render());
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("2 connection points") && n.contains("4 holes")),
        "{:?}",
        report.notes
    );
    assert!(report.rejected.is_empty(), "{:?}", report.rejected);
    let c = Catalog::load(&cat).unwrap();
    let p = c
        .get(&novo_parts::PartKey::new("Acme Terminals GmbH", "TB-2.5"))
        .unwrap();
    assert_eq!(p.price.as_ref().unwrap().amount, 0.31, "the price stays");
    assert_eq!(p.terminals.len(), 2);
    assert_eq!(p.drilling.len(), 2);
}

#[test]
fn contact_counts_coil_data_and_poles_fill_the_description_keys() {
    let xml = r#"<partsmanagement><part partnr="ACM.K-1" manufacturer="Acme"
        poles="3" maincontactsno="3" auxcontactsno="1" auxcontactsnc="1"
        coilvoltage="24 V" coilfrequency="50/60" coiltype="AC/DC"
        ratedvoltage="690 V"/></partsmanagement>"#;
    let r = read_portal(&[file("a.xml", xml)], "", "").unwrap();
    let d = &r.parts[0].data;
    assert_eq!(d.get("poles"), Some(&Value::Int(3)));
    assert_eq!(d.get("main_contacts_no"), Some(&Value::Int(3)));
    assert_eq!(d.get("aux_contacts_no"), Some(&Value::Int(1)));
    assert_eq!(d.get("aux_contacts_nc"), Some(&Value::Int(1)));
    assert_eq!(d.get("coil_voltage_v"), Some(&Value::Float(24.0)));
    assert_eq!(text(d.get("coil_frequency_hz")), "50/60");
    assert_eq!(text(d.get("coil_current_type")), "AC/DC");
    assert_eq!(d.get("rated_voltage_v"), Some(&Value::Float(690.0)));
    assert!(r.unrecognised.is_empty(), "{:?}", r.unrecognised);
    let single = xml.replace("50/60", "50 Hz");
    let r = read_portal(&[file("a.xml", &single)], "", "").unwrap();
    assert_eq!(
        r.parts[0].data.get("coil_frequency_hz"),
        Some(&Value::Float(50.0))
    );
}

#[test]
fn ratings_and_currents_are_not_read_as_counts_or_coil_types() {
    let xml = r#"<partsmanagement><part partnr="ACM.K-2" manufacturer="Acme"
        auxcontactsbreakingcapacity="10 A" coilcurrent="0.02 A"
        coilratedcurrent="25 mA" coilfrequencyhz="NaN" poles="3+N"/></partsmanagement>"#;
    let r = read_portal(&[file("a.xml", xml)], "", "").unwrap();
    let d = &r.parts[0].data;
    assert!(d.get("aux_contacts_nc").is_none());
    assert!(d.get("aux_contacts_no").is_none());
    assert!(d.get("coil_current_type").is_none());
    assert!(d.get("coil_frequency_hz").is_none());
    assert_eq!(d.get("poles"), Some(&Value::Int(3)), "3+N reads as 3");
}
