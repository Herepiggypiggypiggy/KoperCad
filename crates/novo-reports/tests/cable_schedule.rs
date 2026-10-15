//! Cable schedule.

use novo_core::Document;
use novo_reports::{CsvOptions, LiveReports, to_csv};
use novo_tests::motor_starter;

#[test]
fn cable_schedule_has_one_row_per_core() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let mut reports = LiveReports::standard();
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
