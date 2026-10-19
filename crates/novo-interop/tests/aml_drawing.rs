//! AutomationML drawing profile (WS-24): pages, symbols and nets in a file
//! become drawn pages with wires that connect, with nothing rebuilt from
//! geometry. The file is written by hand; no vendor data.

use novo_core::{DeviceTag, Document, Endpoint, PinRef, Project, SymbolRole};
use novo_interop::aml::{plan_import, read_aml};

fn sym(
    id: &str,
    page: &str,
    kind: &str,
    x: f64,
    y: f64,
    extra: &str,
    pins: &[(&str, &str, f64, f64)],
) -> String {
    let mut s = format!(
        r#"<InternalElement ID="s-{id}" Name="{id}">
  <Attribute Name="Page" AttributeDataType="xs:string"><Value>{page}</Value></Attribute>
  <Attribute Name="Kind" AttributeDataType="xs:string"><Value>{kind}</Value></Attribute>
  {extra}
  <Attribute Name="Frame"><Attribute Name="x" AttributeDataType="xs:double"><Value>{x}</Value></Attribute><Attribute Name="y" AttributeDataType="xs:double"><Value>{y}</Value></Attribute></Attribute>
"#
    );
    for (pid, name, dx, dy) in pins {
        s += &format!(
            r#"  <ExternalInterface ID="{pid}" Name="{name}">
    <Attribute Name="X" AttributeDataType="xs:double"><Value>{dx}</Value></Attribute>
    <Attribute Name="Y" AttributeDataType="xs:double"><Value>{dy}</Value></Attribute>
    <Attribute Name="Faces" AttributeDataType="xs:string"><Value>down</Value></Attribute>
  </ExternalInterface>
"#
        );
    }
    s + "  <RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Symbol\"/>\n</InternalElement>\n"
}

fn net(id: &str, name: &str, points: &[&str]) -> String {
    let mut s = format!("<InternalElement ID=\"n-{id}\" Name=\"{name}\">\n");
    for p in points {
        s += &format!("<ExternalInterface ID=\"np-{id}-{p}\" Name=\"{p}\"/>\n");
    }
    s + "<RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Net\"/>\n</InternalElement>\n"
}

fn link(net: &str, pin: &str) -> String {
    format!(
        "<InternalLink Name=\"pin\" RefPartnerSideA=\"{pin}\" RefPartnerSideB=\"np-{net}-{pin}\"/>\n"
    )
}

fn file() -> String {
    let pages = r#"<InternalElement ID="pg" Name="Pages"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/PageGroup"/>
<InternalElement ID="pg1" Name="=A1+CAB1/01"><Attribute Name="Title" AttributeDataType="xs:string"><Value>Control</Value></Attribute><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Page"/></InternalElement>
<InternalElement ID="pg2" Name="=A1+CAB1/02"><Attribute Name="Title" AttributeDataType="xs:string"><Value>Outputs</Value></Attribute><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Page"/></InternalElement>
</InternalElement>"#;
    let k1 = format!(
        r#"<InternalElement ID="k1" Name="=A1+CAB1-K1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-K1</Value></Attribute>
{}{}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Device"/></InternalElement>"#,
        sym(
            "k1c",
            "=A1+CAB1/01",
            "coil",
            50.0,
            200.0,
            "",
            &[("k1a1", "A1", 0.0, 0.0), ("k1a2", "A2", 0.0, -10.0)]
        ),
        sym(
            "k1k",
            "=A1+CAB1/02",
            "contact",
            80.0,
            150.0,
            "",
            &[("k1c13", "13", 0.0, 0.0), ("k1c14", "14", 0.0, -10.0)]
        ),
    );
    let x1 = format!(
        r#"<InternalElement ID="x1" Name="=A1+CAB1-X1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-X1</Value></Attribute>
<ExternalInterface ID="t1" Name="1" RefBaseClassPath="KoperCADInterfaceClassLib/Terminal"/>
{}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/TerminalStrip"/></InternalElement>"#,
        sym(
            "x1s",
            "=A1+CAB1/01",
            "terminal",
            50.0,
            240.0,
            "<Attribute Name=\"Terminal\" AttributeDataType=\"xs:string\"><Value>1</Value></Attribute>",
            &[("x1a", "1", 0.0, 0.0), ("x1b", "2", 0.0, -6.0)],
        ),
    );
    let h1 = format!(
        r#"<InternalElement ID="h1" Name="=A1+CAB1-H1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-H1</Value></Attribute>
{}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Device"/></InternalElement>"#,
        sym(
            "h1l",
            "=A1+CAB1/02",
            "lamp",
            80.0,
            100.0,
            "",
            &[("h1x1", "X1", 0.0, 0.0), ("h1x2", "X2", 0.0, -10.0)]
        ),
    );
    let nets = format!(
        "<InternalElement ID=\"nets\" Name=\"Nets\"><RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/NetGroup\"/>{}{}{}</InternalElement>",
        net("1", "N1", &["x1b", "k1a1"]),
        net("2", "L1", &["k1a2", "k1c13"]),
        net("3", "N3", &["k1c14", "h1x1"]),
    );
    let links = [
        ("1", "x1b"),
        ("1", "k1a1"),
        ("2", "k1a2"),
        ("2", "k1c13"),
        ("3", "k1c14"),
        ("3", "h1x1"),
    ]
    .iter()
    .map(|(n, p)| link(n, p))
    .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<CAEXFile FileName="t.aml" SchemaVersion="2.15"><InstanceHierarchy Name="Test">
<InternalElement ID="proj" Name="Test"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Project"/>
{pages}{k1}{x1}{h1}{nets}{links}</InternalElement></InstanceHierarchy></CAEXFile>"#
    )
}

fn imported() -> Project {
    let plant = read_aml(&file()).unwrap();
    let mut doc = Document::new(Project::new("t"));
    let plan = plan_import(doc.project(), &plant);
    plan.apply(&mut doc).unwrap();
    doc.project().clone()
}

#[test]
fn reading_collects_pages_symbols_and_nets() {
    let plant = read_aml(&file()).unwrap();
    assert_eq!(plant.pages.len(), 2);
    assert_eq!(plant.symbols.len(), 4);
    assert_eq!(plant.nets.len(), 3);
    assert_eq!(plant.nets[1].name, "L1");
    assert!(
        plant
            .report
            .lossy
            .iter()
            .all(|l| l.what != "connection links"),
        "every link ended on a drawn pin"
    );
}

#[test]
fn import_draws_pages_with_connected_wires() {
    let p = imported();
    assert_eq!(p.pages().count(), 2);
    assert_eq!(
        p.symbols().count(),
        4 + 2,
        "four symbols and a pair of interruption points"
    );
    let tag = |s: &str| DeviceTag::parse(s).unwrap();
    let k1 = p.devices_by_tag(&tag("=A1+CAB1-K1"))[0];
    // The coil's A1 and the strip's terminal 1 are one net.
    let pin_of = |name: &str, role: &dyn Fn(&SymbolRole) -> bool| -> PinRef {
        let s = p
            .symbols()
            .find(|s| role(&s.role) && s.pins.iter().any(|x| x.name == name))
            .unwrap();
        PinRef::new(
            s.id,
            s.pins.iter().position(|x| x.name == name).unwrap() as u16,
        )
    };
    let coil = |r: &SymbolRole| matches!(r, SymbolRole::Device { device, .. } if *device == k1);
    let a1 = pin_of("A1", &coil);
    let strip = |r: &SymbolRole| matches!(r, SymbolRole::Terminal { .. });
    let t1 = pin_of("1", &strip);
    let nets = p.netlist();
    let net_of = |pin: PinRef| {
        nets.nets()
            .iter()
            .find(|n| n.endpoints.contains(&Endpoint::pin(pin.symbol, pin.pin)))
            .map(|n| n.id)
    };
    assert!(net_of(a1).is_some());
    assert_eq!(net_of(a1), net_of(t1), "coil A1 and terminal 1 are wired");
    // A2 on page 1 and 13 on page 2 meet through the interruption points named L1.
    let a2 = pin_of("A2", &coil);
    let c13 = pin_of("13", &coil);
    assert_eq!(net_of(a2), net_of(c13), "the net crosses pages by name");
    assert_ne!(net_of(a1), net_of(a2));
}

#[test]
fn a_second_import_adds_nothing() {
    let plant = read_aml(&file()).unwrap();
    let mut doc = Document::new(Project::new("t"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    let again = plan_import(doc.project(), &plant);
    assert!(again.is_empty());
}

/// Opt-in: a real crane export (`KOPERCAD_CRANE_AML` names an `.aml` the
/// configurator wrote). Every net of the file must end up as one net of the
/// project. The file is the user's own and never goes into the repository.
#[test]
fn a_real_crane_export_imports_with_every_net_connected() {
    let Some(path) = std::env::var_os("KOPERCAD_CRANE_AML") else {
        return;
    };
    let plant = read_aml(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut doc = Document::new(Project::new("real"));
    let plan = plan_import(doc.project(), &plant);
    eprintln!("{}", plan.summary());
    for l in &plan.report().lossy {
        eprintln!("lost {} {}: {}", l.count, l.what, l.why);
    }
    plan.apply(&mut doc).unwrap();
    let p = doc.project();
    let nets = p.netlist();
    eprintln!(
        "pages {} symbols {} wires {} project nets {} file nets {} texts {}",
        p.pages().count(),
        p.symbols().count(),
        p.wires().count(),
        nets.nets().len(),
        plant.nets.len(),
        p.texts().count()
    );
    // Nets that join at least two pins (the rest are single, unwired pins).
    let joined = nets
        .nets()
        .iter()
        .filter(|n| {
            n.endpoints
                .iter()
                .filter(|e| matches!(e, Endpoint::Pin { .. }))
                .count()
                >= 2
        })
        .count();
    eprintln!("nets with two or more pins: {joined}");
    assert!(
        joined <= plant.nets.len(),
        "no wire may join two nets of the file"
    );
    assert!(p.pages().count() >= plant.pages.len());
    assert!(p.wires().count() > 0);
}

fn net_of_pins(p: &Project, a: &str, b: &str) -> bool {
    let find = |name: &str| {
        p.symbols()
            .flat_map(|s| {
                s.pins
                    .iter()
                    .position(|x| x.name == name)
                    .map(|i| Endpoint::pin(s.id, i as u16))
            })
            .collect::<Vec<_>>()
    };
    let (ea, eb) = (find(a), find(b));
    p.netlist().nets().iter().any(|n| {
        ea.iter().any(|e| n.endpoints.contains(e)) && eb.iter().any(|e| n.endpoints.contains(e))
    })
}

#[test]
fn an_unnamed_net_across_pages_still_connects() {
    let text = file().replace("Name=\"L1\"", "Name=\"\"");
    let plant = read_aml(&text).unwrap();
    let mut doc = Document::new(Project::new("t"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    assert!(net_of_pins(doc.project(), "A2", "13"));
}

#[test]
fn a_repeated_page_entry_makes_one_page() {
    let whole = r#"<InternalElement ID="pg1" Name="=A1+CAB1/01"><Attribute Name="Title" AttributeDataType="xs:string"><Value>Control</Value></Attribute><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Page"/></InternalElement>"#;
    let text = file().replacen(whole, &format!("{whole}{whole}"), 1);
    let plant = read_aml(&text).unwrap();
    assert_eq!(plant.pages.len(), 3, "the file lists the page twice");
    let mut doc = Document::new(Project::new("t"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    assert_eq!(doc.project().pages().count(), 2);
}

#[test]
fn the_preview_counts_only_nets_it_would_wire() {
    let plant = read_aml(&file()).unwrap();
    let mut doc = Document::new(Project::new("t"));
    let plan = plan_import(doc.project(), &plant);
    assert!(plan.summary().contains("wire 3 nets"), "{}", plan.summary());
    plan.apply(&mut doc).unwrap();
    // Everything exists now: nothing to draw, nothing to promise.
    assert!(
        !plan_import(doc.project(), &plant)
            .summary()
            .contains("wire")
    );
}

#[test]
fn a_long_net_name_is_not_printed_beside_its_interruption_points() {
    let long = "=A1+CAB1/02 Outputs and a long page title.7";
    let plant = read_aml(&file().replace("\"L1\"", &format!("\"{long}\""))).unwrap();
    let mut doc = Document::new(Project::new("t"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    let names: Vec<String> = doc
        .project()
        .symbols()
        .filter_map(|s| match &s.role {
            SymbolRole::Interruption { name } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 2, "one point on each page");
    assert_eq!(names[0], names[1], "both ends pair by the same name");
    assert!(names[0].len() <= 8 && !names[0].contains(' '), "{names:?}");
}

#[test]
fn a_page_label_splits_into_structure_and_name_with_its_title_apart() {
    let p = imported();
    let page = p.pages().find(|p| p.name == "02").expect("page 02");
    assert_eq!(page.structure.to_string(), "=A1+CAB1");
    assert_eq!(page.title, "Outputs");
}
