//! Closed-loop winches in walk-throughs (lane L2c): the winch and pair loops run on the healthy
//! and the faulted plant, so a circuit defect in the script (an open coil on an STO channel, a
//! lost encoder signal device) reaches the drive and the script asks what the winch did.

use novo_core::{Document, Project, Value};
use novo_machine::cl_pair_loop::ClPairLoop;
use novo_machine::cl_winch_loop::ClWinchLoop;
use novo_machine::cl_winch_spec;
use novo_machine::cosim::StepConfig;
use novo_sim::inject::Defect;
use novo_sim::machine::Machinery;
use novo_sim::walkthrough::{Action, Check, Verdict, Walkthrough, run_with_machinery};
use novo_sim::winch_check::{FindingName, WinchIs};
use novo_tests::closed_winch::{closed_winch, closed_winch_pair};

fn winches(project: &Project) -> ClWinchLoop {
    let mut cw = ClWinchLoop::new(StepConfig::default()).unwrap();
    let members: Vec<String> = project
        .devices()
        .filter_map(|d| {
            let get = |k: &str| d.properties.get(k).map(|v: &Value| v.to_string());
            cl_winch_spec::pair_partner(&get).map(|p| (d.tag.to_string(), p))
        })
        .flat_map(|(a, b)| [a, b])
        .collect();
    for d in project.devices() {
        let tag = d.tag.to_string();
        if members.contains(&tag) {
            continue;
        }
        let get = |k: &str| d.properties.get(k).map(|v: &Value| v.to_string());
        if let Some((m, _)) = cl_winch_spec::closed_winch_from_properties(&tag, &get).unwrap() {
            cw.modules.push(m);
        }
    }
    cw
}

fn pairs(project: &Project) -> ClPairLoop {
    let mut cp = ClPairLoop::new(StepConfig::default()).unwrap();
    for a in project.devices() {
        let get_a = |k: &str| a.properties.get(k).map(|v: &Value| v.to_string());
        let Some(partner) = cl_winch_spec::pair_partner(&get_a) else {
            continue;
        };
        let b = project
            .devices()
            .find(|d| d.tag.to_string() == partner)
            .expect("the partner is a device");
        let get_b = |k: &str| b.properties.get(k).map(|v: &Value| v.to_string());
        let (m, _) = cl_winch_spec::closed_pair_from_properties(
            &a.tag.to_string(),
            &get_a,
            &partner,
            &get_b,
        )
        .unwrap()
        .unwrap();
        cp.modules.push(m);
    }
    cp
}

fn press(device: &str, operated: bool) -> Action {
    Action::Operate {
        device: device.into(),
        pin: None,
        operated,
    }
}

fn winch(id: &str, is: WinchIs) -> Action {
    Action::Expect {
        check: Check::Winch {
            winch: id.into(),
            is,
        },
    }
}

fn open_coil(tag: &str) -> Action {
    Action::Inject {
        with: Defect::CoilOpen { device: tag.into() },
    }
}

const BUTTON: &str = "=A1+RAD-S1";
const M1: &str = "=A1+FLD-M1";
const M2: &str = "=A1+FLD-M2";

fn single(w: &Walkthrough, doc: &Document) -> novo_sim::walkthrough::Report {
    let p = doc.project();
    let make = || winches(p);
    run_with_machinery(
        p,
        w,
        &Machinery {
            winches: Some(&make),
            ..Machinery::default()
        },
    )
}

fn pair(w: &Walkthrough, doc: &Document) -> novo_sim::walkthrough::Report {
    let p = doc.project();
    let make = || pairs(p);
    run_with_machinery(
        p,
        w,
        &Machinery {
            pairs: Some(&make),
            ..Machinery::default()
        },
    )
}

fn no_failures(report: &novo_sim::walkthrough::Report) {
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
}

#[test]
fn a_healthy_winch_lifts_and_the_two_plants_agree() {
    let mut doc = Document::default();
    closed_winch(&mut doc).unwrap();
    let w = Walkthrough::new("healthy winch")
        .at(1.0, press(BUTTON, true))
        .at(8.0, winch(M1, WinchIs::Moving))
        .at(8.0, winch(M1, WinchIs::Above { m: 0.3 }))
        .at(8.0, winch(M1, WinchIs::Clear))
        .at(9.0, press(BUTTON, false))
        .at(14.0, winch(M1, WinchIs::Standing));
    let report = single(&w, &doc);
    no_failures(&report);
    assert!(
        report.end_differences.is_empty(),
        "{:?}",
        report.end_differences
    );
}

#[test]
fn an_open_sto_coil_trips_the_drive_and_the_report_says_where_the_hook_is_not() {
    let mut doc = Document::default();
    closed_winch(&mut doc).unwrap();
    let w = Walkthrough::new("open STO coil")
        .at(1.0, press(BUTTON, true))
        .at(4.0, open_coil("=A1+CAB-STOA"))
        .at(9.0, winch(M1, WinchIs::Tripped))
        .at(9.0, winch(M1, WinchIs::Fault { code: "std".into() }))
        .at(
            9.0,
            winch(
                M1,
                WinchIs::Found {
                    finding: FindingName::Trip,
                },
            ),
        )
        .at(9.0, winch(M1, WinchIs::Standing));
    let report = single(&w, &doc);
    no_failures(&report);
    assert!(
        report.end_differences.iter().any(|d| d.contains("trips")),
        "{:?}",
        report.end_differences
    );
}

#[test]
fn a_lost_encoder_signal_is_found_and_a_wrong_statement_fails() {
    let mut doc = Document::default();
    closed_winch(&mut doc).unwrap();
    let w = Walkthrough::new("lost encoder")
        .at(1.0, press(BUTTON, true))
        .at(4.0, open_coil("=A1+CAB-ENC"))
        .at(
            8.0,
            winch(
                M1,
                WinchIs::Found {
                    finding: FindingName::EncoderSignalLost,
                },
            ),
        )
        .at(8.0, winch(M1, WinchIs::Clear));
    let report = single(&w, &doc);
    let verdicts: Vec<Verdict> = report
        .steps
        .iter()
        .filter(|s| s.label.starts_with("Expect"))
        .map(|s| s.verdict)
        .collect();
    assert_eq!(verdicts, vec![Verdict::Pass, Verdict::Fail]);
}

#[test]
fn a_winch_that_is_not_in_the_project_is_named() {
    let mut doc = Document::default();
    closed_winch(&mut doc).unwrap();
    let w = Walkthrough::new("no such winch").at(1.0, winch("=A1+FLD-M9", WinchIs::Clear));
    let report = single(&w, &doc);
    let step = report.steps.last().unwrap();
    assert_eq!(step.verdict, Verdict::Fail);
    assert!(
        step.detail.contains("no closed-loop winch"),
        "{}",
        step.detail
    );
}

#[test]
fn an_sto_coil_of_winch_b_trips_only_b_and_the_pair_stops() {
    let mut doc = Document::default();
    closed_winch_pair(&mut doc).unwrap();
    let w = Walkthrough::new("pair STO")
        .at(1.0, press(BUTTON, true))
        .at(4.0, open_coil("=A1+CAB-STOC"))
        .at(9.0, winch(M2, WinchIs::Tripped))
        .at(9.0, winch(M1, WinchIs::Clear))
        .at(9.0, winch(M1, WinchIs::Standing))
        .at(
            9.0,
            winch(
                M1,
                WinchIs::Found {
                    finding: FindingName::PairStopped,
                },
            ),
        );
    let report = pair(&w, &doc);
    no_failures(&report);
}

#[test]
fn a_healthy_pair_lifts_the_beam_and_both_plants_agree() {
    let mut doc = Document::default();
    closed_winch_pair(&mut doc).unwrap();
    let w = Walkthrough::new("healthy pair")
        .at(1.0, press(BUTTON, true))
        .at(8.0, winch(M1, WinchIs::Above { m: 0.3 }))
        .at(8.0, winch(M2, WinchIs::Above { m: 0.3 }))
        .at(8.0, winch(M2, WinchIs::Clear));
    let report = pair(&w, &doc);
    no_failures(&report);
    assert!(
        report.end_differences.is_empty(),
        "{:?}",
        report.end_differences
    );
}
