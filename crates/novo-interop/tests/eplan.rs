//! EPLAN parts import (WS-24).

use novo_core::Value;
use novo_interop::eplan::{EplanError, import_auto, import_delimited, import_xml};

const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- exported -->
<EplanPxfRoot xmlns:x="urn:x">
  <Parts>
    <Part P_ARTICLE_PARTNR="SIE.3RT2016-1BB41" P_ARTICLE_TYPENR="3RT2016-1BB41"
          P_ARTICLE_MANUFACTURER="Siemens" P_ARTICLE_ORDERNR="3RT2016-1BB41">
      <P_ARTICLE_DESCR1><x:string lang="en_US">Contactor 24 V DC &amp; aux</x:string></P_ARTICLE_DESCR1>
      <P_ARTICLE_DESCR2>3-pole, S00</P_ARTICLE_DESCR2>
      <P_ARTICLE_WIDTH>45</P_ARTICLE_WIDTH>
      <P_ARTICLE_HEIGHT>81,5</P_ARTICLE_HEIGHT>
      <P_ARTICLE_DEPTH>57</P_ARTICLE_DEPTH>
      <P_ARTICLE_PRODUCTTOPGROUP>7</P_ARTICLE_PRODUCTTOPGROUP>
    </Part>
    <Part>
      <Property id="P_ARTICLE_PARTNR" value="PHO.2866763"/>
      <Property id="P_ARTICLE_MANUFACTURER">Phoenix Contact</Property>
      <Property id="P_ARTICLE_DESCR1"><![CDATA[Power supply <24 V>]]></Property>
      <Property id="P_ARTICLE_WIDTH" value="90"/>
    </Part>
    <Part><P_ARTICLE_DESCR1>No number</P_ARTICLE_DESCR1><P_ARTICLE_ORDERNR/></Part>
    <Part P_ARTICLE_PARTNR="X.1"/>
  </Parts>
</EplanPxfRoot>"#;

fn text(v: Option<&Value>) -> &str {
    match v {
        Some(Value::Text(t)) => t,
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn xml_parts_map_to_catalog_fields() {
    let r = import_xml(XML, "").unwrap();
    let p = &r.parts[0];
    assert_eq!(p.manufacturer, "Siemens");
    assert_eq!(p.order_number, "3RT2016-1BB41");
    assert_eq!(p.type_number, "3RT2016-1BB41");
    assert_eq!(p.description, "Contactor 24 V DC & aux");
    let d = p.dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (45.0, 81.5, 57.0));
    assert_eq!(text(p.data.get("eplan.descr2")), "3-pole, S00");
    assert_eq!(text(p.data.get("eplan.partnr")), "SIE.3RT2016-1BB41");
    assert_eq!(text(p.data.get("eplan.producttopgroup")), "7");
}

#[test]
fn property_elements_and_part_number_fallback() {
    let r = import_xml(XML, "").unwrap();
    let p = &r.parts[1];
    assert_eq!(p.manufacturer, "Phoenix Contact");
    assert_eq!(p.order_number, "2866763", "the maker id prefix is dropped");
    assert_eq!(p.description, "Power supply <24 V>");
    assert!(p.dimensions.is_none());
    assert_eq!(p.data.get("width_mm"), Some(&Value::Float(90.0)));
}

#[test]
fn unusable_parts_are_reported_with_their_line() {
    let r = import_xml(XML, "").unwrap();
    assert_eq!(r.parts.len(), 2);
    // `X.1` has no manufacturer and no default.
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(r.skipped[0].1, "no manufacturer");
    assert_eq!(r.skipped[0].0, 21);
}

#[test]
fn default_manufacturer_fills_gaps() {
    let r = import_xml(XML, "Acme").unwrap();
    assert_eq!(r.parts.len(), 3);
    assert_eq!(r.parts[2].manufacturer, "Acme");
}

#[test]
fn xml_without_parts_or_with_broken_markup() {
    assert_eq!(
        import_xml("<a><b>1</b></a>", "").unwrap_err(),
        EplanError::NoParts
    );
    assert!(matches!(
        import_xml("<a><b></a>", "").unwrap_err(),
        EplanError::Xml(_)
    ));
    assert!(matches!(
        import_xml("<a><b>", "").unwrap_err(),
        EplanError::Xml(_)
    ));
    assert!(matches!(
        import_xml("", "").unwrap_err(),
        EplanError::Xml(_)
    ));
}

#[test]
fn doctype_entities_are_never_expanded() {
    let x = r#"<!DOCTYPE p [<!ENTITY e SYSTEM "file:///etc/passwd">]>
<p><Part P_ARTICLE_ORDERNR="A1" P_ARTICLE_MANUFACTURER="M" P_ARTICLE_DESCR1="&e; &#65;&#x42;"/></p>"#;
    let r = import_xml(x, "").unwrap();
    assert_eq!(r.parts[0].description, "&e; AB");
}

#[test]
fn very_deep_xml_is_refused_not_overflowed() {
    let deep = "<a>".repeat(500);
    assert!(matches!(
        import_xml(&deep, "").unwrap_err(),
        EplanError::Xml(_)
    ));
}

const TSV: &str = "Part number\tType number\tManufacturer\tDescription 1\tWidth\tHeight\tDepth\n\
SIE.3RT2016\t3RT2016\tSiemens\tContactor\t45\t81,5\t57\n\
\t\tSiemens\tno number\t\t\t\n";

#[test]
fn delimited_export_with_eplan_headers() {
    let r = import_delimited(TSV, "").unwrap();
    assert_eq!(r.parts.len(), 1);
    let p = &r.parts[0];
    assert_eq!(
        p.order_number, "SIE.3RT2016",
        "no order column: the part number is used"
    );
    assert_eq!(p.description, "Contactor");
    assert_eq!(p.dimensions.unwrap().height_mm, 81.5);
    assert_eq!(r.skipped.len(), 1);
}

#[test]
fn delimited_prefers_the_order_number_column() {
    let t = "P_ARTICLE_PARTNR;P_ARTICLE_ORDERNR;P_ARTICLE_MANUFACTURER;P_ARTICLE_DESCR1\nSIE.1;REAL-1;Siemens;Relay\n";
    let r = import_delimited(t, "").unwrap();
    assert_eq!(r.parts[0].order_number, "REAL-1");
    assert_eq!(r.parts[0].description, "Relay");
}

#[test]
fn auto_picks_by_content() {
    assert_eq!(import_auto(XML, "").unwrap().parts.len(), 2);
    assert_eq!(import_auto(TSV, "").unwrap().parts.len(), 1);
    assert!(import_auto("nothing,useful\n1,2\n", "").is_err());
}
