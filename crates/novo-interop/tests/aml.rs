//! AutomationML round trip (WS-24): a project out, a plant back in.

use novo_core::{
    Cable, CableCore, DeviceKind, DeviceTag, Document, Endpoint, MountingPlate, Pin, PinRef,
    Placement, Point, Project, SymbolRole, Terminal, Value,
};
use novo_interop::aml::{AmlError, AmlOptions, export_project, plan_import, read_aml};

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

fn project() -> Project {
    let mut p = Project::new("Pump station");
    p.transact("setup", |tx| {
        let cab = tx.add_structure(tag("+CAB1"), "Main cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 600.0,
                height: 800.0,
                margin: 10.0,
            }),
        )?;
        tx.add_structure(tag("=A1"), "Pump section", false)?;
        let part = tx.add_part("ACME", "C-100")?;
        let k1 = tx.add_device(tag("=A1+CAB1-K1"), "Main contactor")?;
        tx.assign_part(k1, part)?;
        tx.set_property(k1, "rated_current_a", Some(Value::Float(25.0)))?;
        let x1 = tx.add_terminal_strip(tag("=A1+CAB1-X1"))?;
        for n in ["1", "2", "PE"] {
            let t = tx.add_terminal(x1, n)?;
            if n == "2" {
                tx.update::<Terminal>(t, |t| {
                    t.jumper = Some(1);
                    t.level = 2;
                })?;
            }
        }
        let cable = tx.add_cable(
            tag("=A1-W1"),
            vec![
                CableCore {
                    cross_section_mm2: Some(1.5),
                    ..CableCore::new("BN")
                },
                CableCore {
                    shield: true,
                    ..CableCore::new("SH")
                },
            ],
        )?;
        tx.update::<Cable>(cable, |c| c.length_m = Some(12.5))?;

        // A wire from -K1:A1 to the strip's terminal 1.
        let page = tx.add_page("1")?;
        let ks = tx.place_symbol(
            page,
            "iec/coil",
            Placement::at(Point::new(10.0, 10.0)),
            vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)],
            SymbolRole::Device {
                device: k1,
                main: true,
            },
        )?;
        let t1 = tx
            .project()
            .terminals_of_strip(x1)
            .into_iter()
            .next()
            .unwrap()
            .id;
        let xs = tx.place_symbol(
            page,
            "iec/terminal",
            Placement::at(Point::new(50.0, 10.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
            SymbolRole::Terminal {
                terminals: vec![t1],
            },
        )?;
        tx.connect(Endpoint::pin(ks, 0), Endpoint::pin(xs, 0))?;
        let _ = PinRef::new(ks, 0);
        Ok(())
    })
    .unwrap();
    p
}

#[test]
fn the_export_is_caex_with_the_plant_in_it() {
    let out = export_project(&project(), &AmlOptions::default());
    let t = &out.text;
    assert!(t.starts_with("<?xml"));
    assert!(t.contains("SchemaVersion=\"2.15\""));
    assert!(t.contains("<InstanceHierarchy Name=\"Pump station\">"));
    assert!(t.contains("Name=\"=A1+CAB1-K1\""));
    assert!(t.contains("RefBaseRoleClassPath=\"KoperCADRoleClassLib/TerminalStrip\""));
    assert!(t.contains("<InternalLink"));
    assert_eq!(out.report.count_of("devices"), 2);
    assert_eq!(out.report.count_of("terminals"), 3);
    assert_eq!(out.report.count_of("nets"), 1);
    assert!(!out.report.is_lossless());
    // Deterministic.
    assert_eq!(t, &export_project(&project(), &AmlOptions::default()).text);
}

#[test]
fn a_plant_survives_the_round_trip() {
    let out = export_project(&project(), &AmlOptions::default());
    let plant = read_aml(&out.text).unwrap();
    assert_eq!(plant.name, "Pump station");
    assert_eq!(plant.structures.len(), 2);
    let cab = plant
        .structures
        .iter()
        .find(|s| s.tag.to_string() == "+CAB1")
        .unwrap();
    assert!(cab.enclosure);
    assert_eq!(cab.plate, Some((600.0, 800.0)));
    assert_eq!(plant.devices.len(), 2);
    let k1 = plant
        .devices
        .iter()
        .find(|d| d.tag.to_string() == "=A1+CAB1-K1")
        .unwrap();
    assert_eq!(k1.description, "Main contactor");
    assert_eq!(
        (k1.manufacturer.as_str(), k1.order_number.as_str()),
        ("ACME", "C-100")
    );
    assert_eq!(
        k1.properties.get("rated_current_a"),
        Some(&Value::Float(25.0))
    );
    let x1 = plant
        .devices
        .iter()
        .find(|d| d.kind == DeviceKind::TerminalStrip)
        .unwrap();
    assert_eq!(x1.terminals.len(), 3);
    assert_eq!(x1.terminals[1].jumper, Some(1));
    assert_eq!(x1.terminals[1].level, 2);
    assert_eq!(plant.cables.len(), 1);
    assert_eq!(plant.cables[0].length_m, Some(12.5));
    assert_eq!(plant.cables[0].cores.len(), 2);
    assert!(plant.cables[0].cores[1].shield);
    // The one connection is reported, not silently dropped.
    assert!(
        plant
            .report
            .lossy
            .iter()
            .any(|l| l.what == "connection links" && l.count == 2)
    );
}

#[test]
fn importing_into_an_empty_project_recreates_it_in_one_undo_step() {
    let out = export_project(&project(), &AmlOptions::default());
    let plant = read_aml(&out.text).unwrap();
    let mut doc = Document::new(Project::new("New"));
    let plan = plan_import(doc.project(), &plant);
    assert!(
        plan.summary()
            .starts_with("Would add 2 devices (3 terminals), 1 cables and 2 sections")
    );
    let n = plan.apply(&mut doc).unwrap();
    assert_eq!(n, 2 + 3 + 1 + 2);
    let p = doc.project();
    assert_eq!(p.devices().count(), 2);
    assert_eq!(p.terminals().count(), 3);
    assert_eq!(p.cables().count(), 1);
    assert_eq!(p.structures().count(), 2);
    let k1 = p.devices_by_tag(&tag("=A1+CAB1-K1"));
    assert_eq!(k1.len(), 1);
    assert_eq!(
        p.part(p.device(k1[0]).unwrap().parts[0])
            .unwrap()
            .order_number,
        "C-100"
    );
    assert_eq!(
        p.structure_by_tag(&tag("+CAB1"))
            .unwrap()
            .plate
            .map(|pl| pl.width),
        Some(600.0)
    );

    // A second import changes nothing: everything exists.
    let again = plan_import(doc.project(), &plant);
    assert!(again.is_empty());
    assert_eq!(again.existing.len(), 3);

    // One undo removes the whole import.
    doc.undo().unwrap();
    assert_eq!(doc.project().devices().count(), 0);
}

#[test]
fn a_stale_plan_is_refused() {
    let out = export_project(&project(), &AmlOptions::default());
    let plant = read_aml(&out.text).unwrap();
    let mut doc = Document::new(Project::new("New"));
    let plan = plan_import(doc.project(), &plant);
    doc.transact("edit", |tx| tx.add_device(tag("-Q9"), ""))
        .unwrap();
    assert!(plan.apply(&mut doc).is_err());
}

#[test]
fn a_foreign_file_with_its_own_vocabulary_is_read_and_the_rest_is_counted() {
    let aml = r#"<?xml version="1.0" encoding="utf-8"?>
<CAEXFile SchemaVersion="2.15" FileName="x.aml">
 <InstanceHierarchy Name="Plant">
  <InternalElement ID="a" Name="Cabinet 1">
   <Attribute Name="Designation"><Value>+CAB7</Value></Attribute>
   <RoleRequirements RefBaseRoleClassPath="SomeLib/Location"/>
   <InternalElement ID="b" Name="Motor starter">
     <Attribute Name="TagName"><Value>-Q1</Value></Attribute>
     <Attribute Name="Hersteller"><Value>Muster</Value></Attribute>
     <Attribute Name="Bestellnummer"><Value>MS-1</Value></Attribute>
     <Attribute Name="Voltage" AttributeDataType="xs:double"><Value>400</Value></Attribute>
   </InternalElement>
   <InternalElement ID="c" Name="Some note"/>
   <InternalElement ID="d" Name="-Q1"/>
   <InternalLink Name="l" RefPartnerSideA="x" RefPartnerSideB="y"/>
  </InternalElement>
 </InstanceHierarchy>
</CAEXFile>"#;
    let plant = read_aml(aml).unwrap();
    assert_eq!(plant.structures.len(), 1);
    assert_eq!(plant.structures[0].tag.to_string(), "+CAB7");
    assert_eq!(plant.devices.len(), 1);
    let q1 = &plant.devices[0];
    // The location above completes the tag.
    assert_eq!(q1.tag.to_string(), "+CAB7-Q1");
    assert_eq!(q1.manufacturer, "Muster");
    assert_eq!(q1.order_number, "MS-1");
    assert_eq!(q1.properties.get("aml.Voltage"), Some(&Value::Float(400.0)));
    let lost: Vec<_> = plant.report.lossy.iter().map(|l| l.what.as_str()).collect();
    assert!(lost.contains(&"elements without a usable tag"), "{lost:?}");
    assert!(lost.contains(&"repeated tags"), "{lost:?}");
    assert!(lost.contains(&"connection links"), "{lost:?}");
}

#[test]
fn other_files_are_refused_with_a_clear_message() {
    assert_eq!(read_aml("<a/>"), Err(AmlError::NotCaex("a".into())));
    assert!(matches!(
        read_aml("<CAEXFile/>"),
        Err(AmlError::NoHierarchy)
    ));
    assert!(matches!(read_aml("<CAEXFile>"), Err(AmlError::Xml(_))));
}

#[test]
fn hostile_sizes_and_repeated_structures_are_dropped_and_counted() {
    let element = |id: &str, tag: &str, w: &str, h: &str| {
        format!(
            r#"<InternalElement ID="{id}" Name="{tag}">
   <Attribute Name="Designation"><Value>{tag}</Value></Attribute>
   <Attribute Name="PlateWidth"><Value>{w}</Value></Attribute>
   <Attribute Name="PlateHeight"><Value>{h}</Value></Attribute>
   <RoleRequirements RefBaseRoleClassPath="SomeLib/Location"/>
  </InternalElement>"#
        )
    };
    let aml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<CAEXFile SchemaVersion="2.15" FileName="x.aml">
 <InstanceHierarchy Name="Plant">
  {}
  {}
  {}
  {}
  {}
 </InstanceHierarchy>
</CAEXFile>"#,
        element("a", "+CAB1", "NaN", "800"),
        element("b", "+CAB2", "600", "-5"),
        element("c", "+CAB3", "inf", "800"),
        element("d", "+CAB4", "600", "800"),
        element("e", "+CAB4", "600", "800"),
    );
    let plant = read_aml(&aml).unwrap();
    let plate = |t: &str| {
        plant
            .structures
            .iter()
            .find(|s| s.tag.to_string() == t)
            .map(|s| s.plate)
    };
    assert_eq!(plate("+CAB1"), Some(None));
    assert_eq!(plate("+CAB2"), Some(None));
    assert_eq!(plate("+CAB3"), Some(None));
    assert_eq!(plate("+CAB4"), Some(Some((600.0, 800.0))));
    assert_eq!(plant.structures.len(), 4, "the repeated tag is read once");
}
