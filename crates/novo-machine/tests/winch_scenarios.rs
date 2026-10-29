//! The scripted winch scenarios run end to end and show what they promise.

use novo_drive::closed_loop::{PreTorque, Severity};
use novo_machine::winch_scenarios::{Params, ScenarioId, SyncChoice, Trace, run};

fn text(t: &Trace) -> String {
    t.events
        .iter()
        .map(|e| format!("{:.2} {}", e.t_s, e.text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn has(t: &Trace, needle: &str) -> bool {
    t.events.iter().any(|e| e.text.contains(needle))
}

fn sag_mm(t: &Trace) -> f64 {
    let summary = t
        .summary
        .iter()
        .find(|(k, _)| k.starts_with("Sag"))
        .unwrap();
    summary.1.split(' ').next().unwrap().parse().unwrap()
}

#[test]
fn every_scenario_runs_with_the_defaults() {
    for id in ScenarioId::ALL {
        let t = run(id, &Params::default()).unwrap();
        assert!(!t.channels.is_empty() && t.duration_s() > 8.0, "{id:?}");
        assert!(
            t.channels
                .iter()
                .all(|c| c.values.iter().all(|v| v.is_finite())),
            "{id:?}"
        );
        assert!(!t.charts.is_empty() && !t.summary.is_empty());
        assert!(!id.title().is_empty() && !id.summary().is_empty());
    }
}

#[test]
fn the_cycle_proves_torque_then_opens_the_brake_and_ends_held() {
    let t = run(ScenarioId::Cycle, &Params::default()).unwrap();
    let ev = text(&t);
    let relay = t
        .events
        .iter()
        .position(|e| e.text.starts_with("Brake relay on"))
        .unwrap();
    let opens = t
        .events
        .iter()
        .position(|e| e.text == "Brake is open")
        .unwrap();
    assert!(relay < opens, "{ev}");
    assert!(t.events.iter().all(|e| !e.text.contains("Fault")), "{ev}");
    assert!(sag_mm(&t) < 1.0, "sag {}", sag_mm(&t));
    assert!(
        t.findings.iter().all(|f| f.severity != Severity::Problem),
        "{:?}",
        t.findings
    );
}

#[test]
fn without_a_pre_torque_the_load_sags_and_the_check_says_so() {
    let p = Params {
        pre_torque: PreTorque::None,
        ..Params::default()
    };
    let t = run(ScenarioId::Cycle, &p).unwrap();
    assert!(sag_mm(&t) > 3.0, "{}", sag_mm(&t));
    assert!(t.findings.iter().any(|f| f.text.contains("No pre-torque")));
}

#[test]
fn a_short_brake_hold_drops_the_load_and_a_missing_resistor_trips_the_bus() {
    let p = Params {
        brake_hold_s: 0.1,
        ..Params::default()
    };
    let t = run(ScenarioId::Cycle, &p).unwrap();
    assert!(
        t.findings
            .iter()
            .any(|f| f.severity == Severity::Problem && f.text.contains("falls about"))
    );
    let p = Params {
        braking_resistor: false,
        ..Params::default()
    };
    let t = run(ScenarioId::Cycle, &p).unwrap();
    assert!(has(&t, "OBV"), "{}", text(&t));
    assert!(t.findings.iter().any(|f| f.text.contains("overvoltage")));
}

#[test]
fn encoder_loss_is_caught_only_while_monitored() {
    let t = run(ScenarioId::EncoderLoss, &Params::default()).unwrap();
    assert!(has(&t, "ENC"), "{}", text(&t));
    let p = Params {
        encoder_monitor: false,
        ..Params::default()
    };
    let t = run(ScenarioId::EncoderLoss, &p).unwrap();
    assert!(!has(&t, "ENC"));
    assert!(
        t.findings
            .iter()
            .any(|f| f.text.contains("monitoring is off"))
    );
}

#[test]
fn a_lost_direction_channel_goes_unnoticed_going_up_and_shows_when_the_shaft_reverses() {
    let p = Params {
        direction_channel_only: true,
        ..Params::default()
    };
    let t = run(ScenarioId::EncoderLoss, &p).unwrap();
    let trip = t
        .events
        .iter()
        .find(|e| e.text.contains("ENC"))
        .expect("trips");
    // The encoder failed at 3 s, the lift runs to 5 s: no trip while the shaft only goes up.
    assert!(trip.t_s > 5.0, "{}", text(&t));
}

#[test]
fn safe_torque_off_removes_torque_and_the_brake_stops_the_load() {
    let t = run(ScenarioId::SafeTorqueOff, &Params::default()).unwrap();
    assert!(has(&t, "safe torque off active"), "{}", text(&t));
    assert!(has(&t, "Brake holds"));
}

#[test]
fn matched_winches_stay_level_and_within_the_rope_rating() {
    let t = run(ScenarioId::PairMatched, &Params::default()).unwrap();
    let tilt = t.channel("Tilt").unwrap();
    assert!(tilt.values.iter().all(|v| v.abs() < 0.1));
    assert!(!has(&t, "above its rating"), "{}", text(&t));
}

#[test]
fn a_drum_error_overloads_a_rope_unless_the_load_is_shared_by_torque() {
    let mut p = Params {
        drum_error_pct: 2.0,
        ..Params::default()
    };
    let t = run(ScenarioId::PairDrumError, &p).unwrap();
    assert!(has(&t, "above its rating"), "{}", text(&t));
    p.sync = SyncChoice::PositionLock;
    let t = run(ScenarioId::PairDrumError, &p).unwrap();
    assert!(t.findings.iter().any(|f| f.text.contains("cannot see")));
    p.sync = SyncChoice::TorqueFollower;
    let t = run(ScenarioId::PairDrumError, &p).unwrap();
    assert!(!has(&t, "above its rating"), "{}", text(&t));
}

#[test]
fn a_tripped_drive_stops_its_partner_only_with_a_link() {
    let t = run(ScenarioId::PairDriveTrip, &Params::default()).unwrap();
    assert!(has(&t, "PartnerFault"), "{}", text(&t));
    let p = Params {
        partner_link: false,
        ..Params::default()
    };
    let t = run(ScenarioId::PairDriveTrip, &p).unwrap();
    assert!(
        !has(&t, "PartnerFault") && has(&t, "goes slack"),
        "{}",
        text(&t)
    );
    assert!(t.findings.iter().any(|f| f.text.contains("No link")));
}
