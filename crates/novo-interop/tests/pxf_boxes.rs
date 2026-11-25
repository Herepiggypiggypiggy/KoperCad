//! Boxes with a drawn rectangle of their own (busbar frames, black boxes)
//! on a synthetic export: the size is read, kept on the symbol, and the
//! points of one potential inside the frame are joined into a rail.

use novo_core::{Document, Value};
use novo_interop::eplan::pxf::{PxfOptions, plan_pxf, read_pxf_str};

/// A busbar frame 356 x 16 mm with three connection points of the potential
/// `L1` and one of `L2` inside it. Every name and number is made up.
const BUSBAR: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- Synthetic test fixture written for KoperCAD. Not exported from EPLAN. -->
<EplanPxfRoot Type="Project" Version="2024.0" PxfVersion="1.23" SchemaVersion="1.0" NumMainObjects="1">
  <ESymLib Id="1" Name="DEMO_SYMBOLS"/>
  <O60 A1="60/1" A1381="1303" A1382="1" A1383="1" A1041="en_US@Black box;" A1044="U" A1045="\n"/>
  <O60 A1="60/2" A1381="1304" A1382="1" A1383="1" A1041="en_US@Busbar connection point;" A1044="U" A1045=""/>
  <O20 A1="20/1" A242="0" A247="1" A1381="1303" A1382="1" A1383="1" A241="BB">
    <P11 P16011="en_US@Black box;"/>
    <O45 A1="45/1" A822="0">
        <S48x823 A861="0" A862="0/0" A863="1"/>
        <S48x823 A861="1" A862="2/0" A863="1"/>
    </O45>
  </O20>
  <O20 A1="20/2" A242="30" A247="1" A1381="1304" A1382="1" A1383="1" A241="BCP">
    <P11 P16011="en_US@Busbar connection point;"/>
    <O45 A1="45/2" A822="0">
        <S48x823 A861="0" A862="0/3" A863="1"/>
    </O45>
  </O20>
  <O4 A1="4/1" A1101="1">
    <P49 P1100="A1" P1200="DK" P11012="1" P11013=""/>
    <P11 P11011="en_US@Supply;"/>
    <O17 A1="17/10" A1261="0" A1262="0" A1263="0" A1381="1303" A1382="1" A1383="1">
      <P150 P1100="A1" P1200="DK" P20012="-" P20013="U" P20014="3/0" P20015=""/>
      <S40x1201 A762="40/266"/>
      <S89x5 A411="305" A1651="0/-16" A1652="356/0"/>
    </O17>
    <O17 A1="17/11" A1261="0" A1262="30" A1263="0" A1381="1304" A1382="1" A1383="1" A185="L1.1">
      <P150 P1100="A1" P1200="DK" P20012="-" P20013="U" P20014="3/0" P20015=""/>
      <S40x1201 A762="52/260"/>
    </O17>
    <O17 A1="17/12" A1261="0" A1262="30" A1263="0" A1381="1304" A1382="1" A1383="1" A185="L1.4">
      <P150 P1100="A1" P1200="DK" P20012="-" P20013="U" P20014="3/0" P20015=""/>
      <S40x1201 A762="80/260"/>
    </O17>
    <O17 A1="17/13" A1261="0" A1262="30" A1263="0" A1381="1304" A1382="1" A1383="1" A185="L1.5">
      <P150 P1100="A1" P1200="DK" P20012="-" P20013="U" P20014="3/0" P20015=""/>
      <S40x1201 A762="132/260"/>
    </O17>
    <O17 A1="17/14" A1261="0" A1262="30" A1263="0" A1381="1304" A1382="1" A1383="1" A185="L2.4">
      <P150 P1100="A1" P1200="DK" P20012="-" P20013="U" P20014="3/0" P20015=""/>
      <S40x1201 A762="88/256"/>
    </O17>
  </O4>
</EplanPxfRoot>
"#;

#[test]
fn the_frame_a_box_carries_is_read() {
    let f = read_pxf_str(BUSBAR).unwrap();
    let frame = f.devices[0].frame.expect("the black box has a rectangle");
    assert_eq!((frame.a, frame.b), ((0.0, -16.0), (356.0, 0.0)));
    assert_eq!(frame.layer, 305);
    assert!(f.devices[1].frame.is_none(), "a point has no rectangle");
}

#[test]
fn the_busbar_is_drawn_at_its_size_with_a_rail_per_potential() {
    let file = read_pxf_str(BUSBAR).unwrap();
    let mut doc = Document::default();
    let plan = plan_pxf(doc.project(), &file, &PxfOptions::default());
    assert!(
        plan.report().text().contains("2 busbar rail pieces"),
        "{}",
        plan.report().text()
    );
    plan.apply(&mut doc).unwrap();
    let p = doc.project();
    let framed: Vec<_> = p
        .symbols()
        .filter(|s| s.properties.contains_key("box.x1"))
        .collect();
    assert_eq!(framed.len(), 1);
    let s = framed[0];
    let get = |k: &str| match s.properties.get(k) {
        Some(Value::Float(v)) => *v,
        other => panic!("{k}: {other:?}"),
    };
    assert_eq!((get("box.x0"), get("box.x1")), (0.0, 356.0));
    // y is down on the page: the box hangs 16 mm below its insertion point
    assert_eq!((get("box.y0"), get("box.y1")), (0.0, 16.0));
    assert_eq!(s.properties.get("box.dashed"), Some(&Value::Bool(true)));
    // L1.1 - L1.4 - L1.5 are two rail pieces; L2.4 stands alone
    assert_eq!(p.wires().count(), 2);
}
