//! Faults added from the drawing: stored in the study, run as scenarios.

use novo_sim::fault::FaultKind;
use novo_sim::study::{FaultWhat, Study};

#[test]
fn a_fault_on_a_known_device_runs_through_the_example_protection() {
    let mut study = Study::example();
    study.add_fault(
        "-F2",
        FaultWhat::Short {
            kind: FaultKind::PhaseEarth,
        },
        0.0,
        0.1,
    );
    let prepared = study.prepared();
    assert!(prepared.warnings.is_empty(), "{:?}", prepared.warnings);
    let index = prepared.study.scenarios.len() - 1;
    assert!(prepared.study.scenarios[index].name.starts_with("F1:"));
    let result = prepared.study.run(index).expect("it runs");
    assert_eq!(result.faults.len(), 1);
    assert_eq!(result.faults[0].node, "Pump");
    assert!(result.faults[0].ik > 0.0);
    assert!(
        result.faults[0].cleared_at.is_some(),
        "the breaker clears it"
    );
}

#[test]
fn an_unknown_target_hangs_on_the_busbar_and_says_so() {
    let mut study = Study::empty("Plant");
    study.add_fault(
        "L1",
        FaultWhat::Short {
            kind: FaultKind::ThreePhase,
        },
        0.0,
        0.1,
    );
    let prepared = study.prepared();
    assert_eq!(prepared.warnings.len(), 1);
    assert!(prepared.warnings[0].contains("not in the study"));
    let result = prepared.study.run(0).expect("it runs");
    assert!(result.faults[0].ik > 1000.0);
    assert!(result.faults[0].cleared_at.is_none(), "nothing protects it");
}

#[test]
fn a_device_fault_needs_a_device_of_the_study() {
    let mut study = Study::example();
    study.add_fault("-Q1", FaultWhat::DeviceOpens, 0.0, 1.0);
    study.add_fault("-Q99", FaultWhat::DeviceCloses, 0.0, 1.0);
    let prepared = study.prepared();
    assert_eq!(prepared.study.scenarios.len(), study.scenarios.len() + 1);
    assert_eq!(prepared.warnings.len(), 1);
    assert!(prepared.warnings[0].contains("-Q99"));
    assert!(prepared.study.run(study.scenarios.len()).is_ok());
}

#[test]
fn faults_survive_the_file_and_old_files_still_read() {
    let mut study = Study::example();
    let id = study.add_fault("-F2", FaultWhat::DeviceOpens, 0.0, 0.1);
    assert_eq!(id, "F1");
    assert_eq!(
        study.add_fault("x", FaultWhat::DeviceCloses, 0.0, 0.1),
        "F2"
    );
    let back = Study::from_json(&study.to_json()).expect("reads");
    assert_eq!(back, study);
    // A file written before faults existed has no such field.
    let plain = Study::example().to_json();
    assert!(!plain.contains("\"faults\""));
    assert!(Study::from_json(&plain).expect("reads").faults.is_empty());
    assert!(study.remove_fault("F1"));
    assert!(!study.remove_fault("F1"));
    assert_eq!(
        study.add_fault("y", FaultWhat::DeviceCloses, 0.0, 0.1),
        "F1"
    );
}
