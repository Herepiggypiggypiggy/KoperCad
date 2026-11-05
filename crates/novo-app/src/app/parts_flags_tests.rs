//! Parts job flags: a failed action keeps its triangle until the same action works.
//! `parts_job_failed` / `parts_job_ok` deliberately parallel `job_fail` / `job_ok`.

use crate::app::gate1_tests::Drive;
use novo_flags::Severity;

#[test]
fn a_failed_parts_action_keeps_its_triangle_until_the_same_action_works() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    for job in ["catalog", "inbox", "eplan", "assign", "remove", "set"] {
        d.app
            .parts_job_failed(job, Severity::Red, "Failed", "disk full");
        assert_eq!(d.app.parts_job_flags().len(), 1, "{job} raises one flag");
        // Another job succeeding leaves it alone.
        d.app.parts_job_ok("table");
        assert_eq!(d.app.parts_job_flags().len(), 1, "{job} stays");
        d.app.parts_job_ok(job);
        assert!(
            d.app.parts_job_flags().is_empty(),
            "{job} clears on success"
        );
    }
}

#[test]
fn a_yellow_flag_is_cleared_by_the_same_job_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.parts_job_failed(
        "eplan.makers",
        Severity::Yellow,
        "Maker names not saved",
        "x",
    );
    assert_eq!(d.app.parts_job_flags().len(), 1);
    d.app.parts_job_ok("eplan.makers");
    assert!(d.app.parts_job_flags().is_empty());
}
