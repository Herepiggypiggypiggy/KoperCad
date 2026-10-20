//! AutomationML import: the symbol kinds a crane drawing is full of (power
//! poles, motors, tap terminals, overload contacts, plugs) are drawn with the
//! library symbol of their own kind, pin for pin, not as labelled boxes. The
//! file is written by hand with invented tags.

use std::fmt::Write as _;

use novo_core::{Document, Project};
use novo_interop::aml::{plan_import, read_aml};

const PAGE: &str = "=A1+CAB1/01";

/// One device with one symbol; `pins` are `(name, dx, dy, faces)`.
fn device(n: usize, kind: &str, x: f64, pins: &[(&str, f64, f64, &str)]) -> String {
    let mut s = format!(
        r#"<InternalElement ID="d{n}" Name="=A1+CAB1-D{n}"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-D{n}</Value></Attribute>
<InternalElement ID="s{n}" Name="S{n}">
<Attribute Name="Page" AttributeDataType="xs:string"><Value>{PAGE}</Value></Attribute>
<Attribute Name="Kind" AttributeDataType="xs:string"><Value>{kind}</Value></Attribute>
<Attribute Name="Frame"><Attribute Name="x" AttributeDataType="xs:double"><Value>{x}</Value></Attribute><Attribute Name="y" AttributeDataType="xs:double"><Value>200</Value></Attribute></Attribute>
"#
    );
    for (j, (name, dx, dy, faces)) in pins.iter().enumerate() {
        let _ = write!(
            s,
            r#"<ExternalInterface ID="p{n}-{j}" Name="{name}">
<Attribute Name="X" AttributeDataType="xs:double"><Value>{dx}</Value></Attribute>
<Attribute Name="Y" AttributeDataType="xs:double"><Value>{dy}</Value></Attribute>
<Attribute Name="Faces" AttributeDataType="xs:string"><Value>{faces}</Value></Attribute>
</ExternalInterface>
"#
        );
    }
    s + "<RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Symbol\"/></InternalElement><RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Device\"/></InternalElement>\n"
}

/// Pins in a row along the top at 4 mm (half of the library's 8 mm).
fn row(names: &[&'static str]) -> Vec<(&'static str, f64, f64, &'static str)> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, 4.0 * i as f64, 0.0, "up"))
        .collect()
}

/// Pole pairs: the top pins of a row, the bottom pins 8 mm below.
fn poles(names: &[&'static str]) -> Vec<(&'static str, f64, f64, &'static str)> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (col, low) = (i / 2, i % 2 == 1);
            (
                *n,
                4.0 * col as f64,
                if low { -8.0 } else { 0.0 },
                if low { "down" } else { "up" },
            )
        })
        .collect()
}

fn import(devices: &[String]) -> Project {
    let text = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<CAEXFile FileName="t.aml" SchemaVersion="2.15"><InstanceHierarchy Name="Test">
<InternalElement ID="proj" Name="Test"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Project"/>
<InternalElement ID="pg" Name="Pages"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/PageGroup"/>
<InternalElement ID="pg1" Name="{PAGE}"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Page"/></InternalElement>
</InternalElement>
{}</InternalElement></InstanceHierarchy></CAEXFile>"#,
        devices.concat()
    );
    let plant = read_aml(&text).unwrap();
    let mut doc = Document::new(Project::new("t"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    doc.project().clone()
}

fn keys(p: &Project) -> Vec<String> {
    let mut v: Vec<String> = p.symbols().map(|s| s.symbol.0.clone()).collect();
    v.sort();
    v
}

#[test]
fn power_poles_motors_tap_terminals_and_plugs_get_their_own_symbols() {
    let p = import(&[
        device(1, "power_poles", 20.0, &poles(&["1", "2", "3", "4", "5", "6"])),
        device(2, "motor", 60.0, &row(&["U1", "V1", "W1", "PE"])),
        device(
            3,
            "terminal",
            100.0,
            &[
                ("1", 0.0, 0.0, "up"),
                ("2", 0.0, -4.0, "down"),
                ("3", 2.0, -4.0, "down"),
            ],
        ),
        device(
            4,
            "thermal_switch",
            120.0,
            &[("95", 0.0, 0.0, "up"), ("96", 0.0, -8.0, "down")],
        ),
        device(5, "plug", 140.0, &row(&["L1", "L2", "L3", "PE"])),
    ]);
    assert_eq!(
        keys(&p),
        [
            "iec/contactors/main_3p",
            "iec/contacts/thermal_nc",
            "iec/motors/motor_3ph",
            "iec/terminals/plug_3p_pe",
            "iec/terminals/terminal_tap",
        ]
    );
}

#[test]
fn a_symbol_of_a_kind_without_one_stays_a_box() {
    let p = import(&[device(1, "motor", 20.0, &row(&["A", "B", "C"]))]);
    assert_eq!(keys(&p), ["import/box"]);
}
