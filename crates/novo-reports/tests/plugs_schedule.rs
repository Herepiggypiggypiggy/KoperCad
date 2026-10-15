//! Plug connection lists and the cable schedule.

use novo_core::{
    DeviceKind, DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole, Wire,
};
use novo_reports::{CsvOptions, LiveReports, to_csv};
use novo_tests::motor_starter;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("tag")
}

fn pins(names: &[&str]) -> Vec<Pin> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
        .collect()
}

#[test]
fn plug_list_follows_wires_through_junctions_and_lists_free_pins() {
    let mut doc = Document::default();
    doc.transact("plug", |tx| {
        let page = tx.add_page("1")?;
        let plug = tx.add_device_of_kind(tag("=A1+CAB-X5"), "Motor plug", DeviceKind::Connector)?;
        let valve = tx.add_device(tag("=A1+FLD-Y1"), "Valve")?;
        let sensor = tx.add_device(tag("=A1+FLD-B1"), "Sensor")?;
        let role = |device| SymbolRole::Device { device, main: true };
        let p = tx.place_symbol(
            page,
            "iec/connectivity/plug",
            Placement::at(Point::new(10.0, 10.0)),
            pins(&["10", "2", "3"]),
            role(plug),
        )?;
        let y = tx.place_symbol(
            page,
            "iec/actuators/valve",
            Placement::at(Point::new(60.0, 10.0)),
            pins(&["A1", "A2"]),
            role(valve),
        )?;
        let b = tx.place_symbol(
            page,
            "iec/sensors/sensor",
            Placement::at(Point::new(60.0, 40.0)),
            pins(&["1"]),
            role(sensor),
        )?;
        let wire = tx.connect(Endpoint::pin(p, 0), Endpoint::pin(y, 0))?;
        tx.update::<Wire>(wire, |w| w.number = Some("21".into()))?;
        // Pin 2 branches through a junction to two destinations.
        let j = tx.add_junction(page, Point::new(40.0, 20.0))?;
        tx.connect(Endpoint::pin(p, 1), Endpoint::junction(j))?;
        tx.connect(Endpoint::junction(j), Endpoint::pin(y, 1))?;
        tx.connect(Endpoint::junction(j), Endpoint::pin(b, 0))?;
        Ok(())
    })
    .expect("build");
    let plugs = LiveReports::standard()
        .get(doc.project(), "plugs")
        .expect("built in")
        .clone();
    assert_eq!(plugs.sections.len(), 1);
    assert_eq!(plugs.sections[0].title, "=A1+CAB-X5");
    let rows = plugs.sections[0].text_rows();
    // Natural pin order: 2, 3, 10.
    let pin_col: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(pin_col, ["2", "2", "3", "10"]);
    assert_eq!(rows[0][4], "=A1+FLD-B1:1");
    assert_eq!(rows[1][4], "=A1+FLD-Y1:A2");
    assert_eq!(rows[2][6], "free");
    assert_eq!(rows[3][1], "21");
    assert_eq!(rows[3][4], "=A1+FLD-Y1:A1");
}

#[test]
fn cable_schedule_has_one_row_per_core() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let reports = LiveReports::standard();
    let mut reports = reports;
    let schedule = reports
        .get(doc.project(), "cableschedule")
        .expect("built in")
        .clone();
    let cables = reports
        .get(doc.project(), "cables")
        .expect("built in")
        .clone();
    let cores: usize = cables
        .sections
        .iter()
        .filter(|s| s.key != "overview")
        .map(|s| s.rows.len())
        .sum();
    assert!(cores > 0);
    assert_eq!(schedule.sections[0].rows.len(), cores);
    let csv = to_csv(&schedule, &CsvOptions::default());
    assert!(csv.trim_start_matches('\u{feff}').starts_with("Cable"));
}
