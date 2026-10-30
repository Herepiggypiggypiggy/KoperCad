//! Instances, bindings, sequences, the validator and the schema.

use novo_controller::*;

const RADIO: &str = include_str!("data/generic-radio-3x2.controller.toml");

fn radio_profile() -> Profile {
    Profile::load(RADIO).unwrap()
}

fn two_radios() -> ControllerSet {
    let p = radio_profile();
    let mut set = ControllerSet::new();
    set.add_instance("radio-crane1", "Crane 1", &p).unwrap();
    set.add_instance("radio-crane2", "Crane 2", &p).unwrap();
    set
}

fn act(set: &mut ControllerSet, who: &str, a: Action) -> Applied {
    set.act(who, &a).unwrap()
}

fn press(c: &str) -> Action {
    Action::Press { control: c.into() }
}

fn pos(c: &str, p: &str) -> Action {
    Action::SetPosition {
        control: c.into(),
        position: p.into(),
    }
}

// ---- several instances ----

#[test]
fn same_profile_twice_gives_independent_state() {
    let mut set = two_radios();
    act(&mut set, "radio-crane1", press("start"));
    act(&mut set, "radio-crane1", pos("hoist", "up2"));
    let level = |who: &str, s: &str| set.level(&SignalKey::new(who, s));
    assert_eq!(level("radio-crane1", "H_UP_FAST"), Some(1.0));
    assert_eq!(level("radio-crane2", "H_UP_FAST"), Some(0.0));
    assert_eq!(level("radio-crane2", "MAIN"), Some(0.0));
    assert_eq!(level("radio-crane3", "MAIN"), None);

    // E-stop on one crane does not touch the other
    act(&mut set, "radio-crane2", press("start"));
    act(&mut set, "radio-crane1", press("estop"));
    assert_eq!(
        set.level(&SignalKey::new("radio-crane2", "MAIN")),
        Some(1.0)
    );
    assert_eq!(
        set.level(&SignalKey::new("radio-crane1", "MAIN")),
        Some(0.0)
    );
}

#[test]
fn instance_management() {
    let mut set = two_radios();
    assert_eq!(
        set.add_instance("radio-crane1", "again", &radio_profile()),
        Err(SetError::DuplicateId("radio-crane1".into()))
    );
    set.rename("radio-crane1", "North crane").unwrap();
    set.set_colour("radio-crane1", Some("#e5a50a")).unwrap();
    set.set_target_group("radio-crane1", Some("runway1/bridge1"))
        .unwrap();
    act(&mut set, "radio-crane1", press("start"));
    set.duplicate_instance("radio-crane1", "radio-crane3", "Crane 3")
        .unwrap();
    let c3 = set.instance("radio-crane3").unwrap();
    assert_eq!(c3.colour.as_deref(), Some("#e5a50a"));
    assert_eq!(c3.target_group.as_deref(), Some("runway1/bridge1"));
    assert_eq!(
        c3.controller.position("start"),
        Some("off"),
        "a duplicate starts fresh"
    );
    let order: Vec<&str> = set.instances().iter().map(|i| i.id.as_str()).collect();
    assert_eq!(order, ["radio-crane1", "radio-crane2", "radio-crane3"]);
    set.remove_instance("radio-crane2").unwrap();
    assert_eq!(
        set.remove_instance("radio-crane2"),
        Err(SetError::UnknownInstance("radio-crane2".into()))
    );
    assert_eq!(
        set.rename("zzz", "x"),
        Err(SetError::UnknownInstance("zzz".into()))
    );
}

#[test]
fn project_data_round_trips_through_toml_and_json() {
    let mut set = two_radios();
    act(&mut set, "radio-crane1", press("start"));
    act(&mut set, "radio-crane1", pos("bridge", "rev2"));
    set.set_colour("radio-crane2", Some("blue")).unwrap();
    set.advance(250);
    let mut table = BindingTable::new();
    table.bind(
        SignalKey::new("radio-crane1", "B_REV_FAST"),
        Target::Pin("-A1:11".into()),
    );
    table.bind(
        SignalKey::new("radio-crane2", "MAIN"),
        Target::ModulePath("runway1/bridge2".into()),
    );
    let data = ProjectControllers {
        instances: set.to_records(),
        bindings: table.bindings.clone(),
        ..ProjectControllers::default()
    };

    let mut lib = ProfileLibrary::new();
    lib.insert(radio_profile());

    for back in [
        toml::from_str::<ProjectControllers>(&toml::to_string_pretty(&data).unwrap()).unwrap(),
        serde_json::from_str::<ProjectControllers>(&serde_json::to_string(&data).unwrap()).unwrap(),
    ] {
        assert_eq!(back, data);
        let restored = ControllerSet::from_records(&back.instances, &lib).unwrap();
        assert_eq!(restored.signals(), set.signals());
        assert_eq!(restored.time_ms(), 250);
        assert_eq!(
            restored.instance("radio-crane2").unwrap().colour.as_deref(),
            Some("blue")
        );
    }

    assert_eq!(
        ControllerSet::from_records(&data.instances, &ProfileLibrary::new()).err(),
        Some(SetError::UnknownProfile("generic-radio-3x2".into()))
    );
}

// ---- bindings ----

#[test]
fn binding_checks_fire_on_each_defect() {
    let set = two_radios();
    let mut t = BindingTable::new();
    let k = |c: &str, s: &str| SignalKey::new(c, s);
    t.bind(k("radio-crane1", "B_FWD_SLOW"), Target::Pin("-A1:1".into()));
    assert!(
        !t.bind(k("radio-crane1", "B_FWD_SLOW"), Target::Pin("-A1:1".into())),
        "exact duplicate refused by bind"
    );
    t.bindings.push(Binding {
        key: k("radio-crane1", "B_FWD_SLOW"),
        target: Target::Pin("-A1:1".into()),
        invert: false,
    });
    t.bind(k("radio-crane1", "NOPE"), Target::Pin("-A1:2".into()));
    t.bind(k("radio-crane1", "MAIN"), Target::Pin("-A1:9".into()));
    t.bind(k("radio-crane2", "MAIN"), Target::Pin("-A1:9".into()));

    let findings = t.check(&set);
    let count = |p: BindingProblem| findings.iter().filter(|f| f.problem == p).count();
    assert_eq!(count(BindingProblem::Duplicate), 1);
    assert_eq!(count(BindingProblem::UnknownSignal), 1);
    assert_eq!(
        count(BindingProblem::TargetConflict),
        2,
        "one finding per signal on the contested target"
    );
    // 13 signals per radio in the fixture, two radios, 3 bound
    let total = set.signals().len();
    assert_eq!(total, 26);
    let bound = ["B_FWD_SLOW"].len() + 2; // B_FWD_SLOW of crane1, MAIN of both
    assert_eq!(count(BindingProblem::Unbound), total - bound);
    assert!(
        findings
            .iter()
            .filter(|f| f.problem == BindingProblem::TargetConflict)
            .all(|f| f.severity == Severity::Error)
    );
}

#[test]
fn shared_controllers_may_drive_one_target() {
    let radio = radio_profile();
    let pendant = Profile::load(
        &RADIO
            .replace("id = \"generic-radio-3x2\"", "id = \"service\"")
            .replace("generic = true", "generic = true\nshared = true"),
    )
    .unwrap();
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &radio).unwrap();
    set.add_instance("service", "Service pendant", &pendant)
        .unwrap();
    let mut t = BindingTable::new();
    let target = Target::ModulePath("runway1/bridge1".into());
    t.bind(SignalKey::new("radio", "B_FWD_FAST"), target.clone());
    t.bind(SignalKey::new("service", "B_FWD_FAST"), target.clone());
    // the radio is not marked shared, so this is still a conflict
    assert!(
        t.check(&set)
            .iter()
            .any(|f| f.problem == BindingProblem::TargetConflict)
    );

    let shared_radio =
        Profile::load(&RADIO.replace("generic = true", "generic = true\nshared = true")).unwrap();
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &shared_radio).unwrap();
    set.add_instance("service", "Service pendant", &pendant)
        .unwrap();
    assert!(
        !t.check(&set)
            .iter()
            .any(|f| f.problem == BindingProblem::TargetConflict)
    );

    // who owns the target now
    assert!(t.owners(&target, &set).is_empty());
    act(&mut set, "service", press("start"));
    act(&mut set, "service", pos("bridge", "fwd2"));
    assert_eq!(
        t.owners(&target, &set),
        [SignalKey::new("service", "B_FWD_FAST")]
    );
    assert_eq!(t.resolve(&set)[&target], 1.0);
}

#[test]
fn resolve_gives_each_target_its_level_and_analog_wins_by_magnitude() {
    let text = r#"
[controller]
id = "stick"
name = "Stick"
[[control]]
id = "x"
kind = "axis"
positions = ["center", "left", "right"]
[[signal]]
name = "REF"
kind = "analog"
control = "x"
levels = { left = -6.0, right = 4.0 }
"#;
    let p = Profile::load(text).unwrap();
    let mut set = ControllerSet::new();
    set.add_instance("a", "A", &p).unwrap();
    set.add_instance("b", "B", &p).unwrap();
    let mut t = BindingTable::new();
    let drive = Target::PlcAddress("%IW4".into());
    t.bind(SignalKey::new("a", "REF"), drive.clone());
    t.bind(SignalKey::new("b", "REF"), drive.clone());
    assert_eq!(t.resolve(&set)[&drive], 0.0);
    act(&mut set, "a", pos("x", "right"));
    act(&mut set, "b", pos("x", "left"));
    assert_eq!(t.resolve(&set)[&drive], -6.0);
}

// ---- sequences ----

fn script() -> Sequence {
    Sequence::parse(
        r#"
name = "hoist then e-stop"

[[event]]
t_ms = 0
instance = "radio-crane1"
action = "press"
control = "start"

[[event]]
t_ms = 200
instance = "radio-crane1"
action = "set_position"
control = "hoist"
position = "up1"

[[event]]
t_ms = 700
instance = "radio-crane1"
action = "set_position"
control = "hoist"
position = "up2"

[[event]]
t_ms = 700
instance = "radio-crane2"
action = "press"
control = "start"

[[event]]
t_ms = 1500
instance = "radio-crane1"
action = "release"
control = "hoist"

[[event]]
t_ms = 1600
instance = "radio-crane1"
action = "link"
ok = false

[[event]]
t_ms = 2000
instance = "radio-crane1"
action = "press"
control = "estop"
"#,
    )
    .unwrap()
}

#[test]
fn replay_gives_identical_traces_twice_and_the_expected_steps() {
    let seq = script();
    assert!(seq.check(&two_radios()).is_empty());
    let run = || {
        let mut set = two_radios();
        seq.replay(&mut set, 100, 500).unwrap()
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b, "same sequence, same trace");

    let at = |t: u64, who: &str, sig: &str| {
        let s = a
            .samples
            .iter()
            .find(|s| s.t_ms == t)
            .unwrap_or_else(|| panic!("no sample at {t}"));
        s.signals
            .iter()
            .find(|(k, _)| *k == SignalKey::new(who, sig))
            .unwrap()
            .1
    };
    assert_eq!(at(100, "radio-crane1", "MAIN"), 1.0);
    assert_eq!(at(100, "radio-crane1", "H_UP_SLOW"), 0.0);
    assert_eq!(at(300, "radio-crane1", "H_UP_SLOW"), 1.0);
    assert_eq!(at(300, "radio-crane1", "H_UP_FAST"), 0.0);
    assert_eq!(at(700, "radio-crane1", "H_UP_FAST"), 1.0);
    assert_eq!(at(700, "radio-crane2", "MAIN"), 1.0);
    assert_eq!(at(1500, "radio-crane1", "H_UP_SLOW"), 0.0);
    assert_eq!(at(1600, "radio-crane1", "MAIN"), 0.0, "link loss");
    assert_eq!(at(1600, "radio-crane1", "ESTOP_OK"), 0.0);
    assert_eq!(
        at(1600, "radio-crane2", "MAIN"),
        1.0,
        "other crane unaffected"
    );
    assert_eq!(a.samples.last().unwrap().t_ms, 2500);
}

#[test]
fn sampling_step_does_not_change_what_happens() {
    let seq = script();
    let mut fine = two_radios();
    let mut coarse = two_radios();
    seq.replay(&mut fine, 10, 0).unwrap();
    seq.replay(&mut coarse, 0, 0).unwrap();
    assert_eq!(fine.signals(), coarse.signals());
    assert_eq!(fine.time_ms(), coarse.time_ms());
}

#[test]
fn recording_then_replaying_reproduces_the_signals() {
    let mut live = two_radios();
    live.advance(5000); // some time before recording starts
    live.start_recording();
    act(&mut live, "radio-crane1", press("start"));
    live.advance(300);
    act(&mut live, "radio-crane1", pos("bridge", "fwd1"));
    live.advance(400);
    act(&mut live, "radio-crane1", pos("bridge", "fwd2"));
    act(&mut live, "radio-crane1", pos("hoist", "up1")); // refused? no, allowed: main is on
    live.advance(100);
    act(&mut live, "radio-crane1", press("estop"));
    live.advance(50);
    let seq = live.stop_recording("my test").unwrap();
    assert_eq!(seq.events[0].t_ms, 0, "first event shifted to 0");
    assert_eq!(seq.events.len(), 5);
    assert!(!live.is_recording());

    // text round trip, then replay on a fresh set
    let seq = Sequence::parse(&seq.to_toml().unwrap()).unwrap();
    let mut fresh = two_radios();
    seq.replay(&mut fresh, 0, 50).unwrap();
    assert_eq!(fresh.signals(), live.signals());
    assert_eq!(
        fresh
            .instance("radio-crane1")
            .unwrap()
            .controller
            .position("estop"),
        Some("pressed")
    );
}

#[test]
fn replay_starts_at_the_sets_current_time() {
    let seq = script();
    let mut a = two_radios();
    a.advance(10_000);
    let mut b = two_radios();
    let ta = seq.replay(&mut a, 0, 0).unwrap();
    let tb = seq.replay(&mut b, 0, 0).unwrap();
    let signals = |t: &Trace| {
        t.samples
            .iter()
            .map(|s| s.signals.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(signals(&ta), signals(&tb));
    assert_eq!(a.time_ms(), 10_000 + seq.duration_ms());
}

#[test]
fn player_steps_with_a_fixed_loop() {
    let seq = script();
    let mut set = two_radios();
    let mut player = Player::new(&seq, &set);
    let mut highs = Vec::new();
    for _ in 0..30 {
        let next = player.elapsed_ms(&set) + 100;
        player.advance_to(&mut set, next).unwrap();
        highs.push(
            set.level(&SignalKey::new("radio-crane1", "H_UP_FAST"))
                .unwrap(),
        );
    }
    assert!(player.finished());
    // fast contact is high from 700 ms to 1500 ms: loop ticks end at 700..=1400
    let want: Vec<f64> = (1..=30u64)
        .map(|i| {
            if (700..1500).contains(&(i * 100)) {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    assert_eq!(highs, want);
}

#[test]
fn same_time_events_keep_file_order() {
    let seq = Sequence::parse(
        r#"
[[event]]
t_ms = 0
instance = "radio-crane1"
action = "press"
control = "estop"
[[event]]
t_ms = 0
instance = "radio-crane1"
action = "press"
control = "start"
"#,
    )
    .unwrap();
    let mut set = two_radios();
    seq.replay(&mut set, 0, 0).unwrap();
    // estop first, so start is refused; reversed order would leave main off too, so check state
    let c = &set.instance("radio-crane1").unwrap().controller;
    assert_eq!(c.position("estop"), Some("pressed"));
    assert_eq!(c.position("start"), Some("off"));
}

#[test]
fn sequence_check_names_every_problem() {
    let seq = Sequence::parse(
        r#"
[[event]]
t_ms = 0
instance = "nobody"
action = "press"
control = "start"
[[event]]
t_ms = 1
instance = "radio-crane1"
action = "press"
control = "ghost"
[[event]]
t_ms = 2
instance = "radio-crane1"
action = "set_position"
control = "hoist"
position = "sideways"
"#,
    )
    .unwrap();
    let problems = seq.check(&two_radios());
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(seq.replay(&mut two_radios(), 0, 0).is_err());
    assert!(Sequence::parse("[[event]]\nt_ms = 0\n").is_err());
}

// ---- validator ----

fn errors(text: &str) -> Vec<String> {
    let profile = Profile::parse(text).expect("parses");
    validate(&profile)
        .into_iter()
        .filter(|i| i.severity == Severity::Error)
        .map(|i| i.to_string())
        .collect()
}

const HEAD: &str = "[controller]\nid = \"t\"\nname = \"T\"\ncommon = \"C\"\n";

#[test]
fn the_fixture_is_clean() {
    let issues = validate(&radio_profile());
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn validator_reports_each_kind_of_defect() {
    let cases: &[(&str, &str)] = &[
        (
            "[[control]]\nid = \"a\"\nkind = \"rocker2\"\n",
            "needs explicit `positions`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[control]]\nid = \"a\"\nkind = \"button\"\n",
            "duplicate control id",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"selector\"\npositions = [\"x\", \"x\"]\n",
            "duplicate position",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"selector\"\npositions = [\"x\", \"y\"]\nrest = \"z\"\n",
            "rest `z`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"rocker2\"\npositions = [\"off\", \"up\"]\n",
            "group name and a step",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"rocker2\"\npositions = [\"off\", \"up3\"]\n",
            "step 1..=2",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"estop\"\nspring = true\n",
            "E-stop latches",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"latch\"\nrequires = [\"b=on\"]\n",
            "control `b` does not exist",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"latch\"\nrequires = [\"a=on\"]\n",
            "refers to the control itself",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"latch\"\nrequires = [\"nonsense\"]\n",
            "not control=position",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"zz\"\nwhen = [\"on\"]\n",
            "control `zz` does not exist",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\nwhen = [\"half\"]\n",
            "unknown position `half`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\n",
            "either `when` or `group`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\nwhen = [\"on\"]\ngroup = \"up\"\nstep = 1\n",
            "either `when` or `group`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\ngroup = \"up\"\nstep = 1\n",
            "only works on rocker",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"rocker2\"\npositions = [\"off\", \"up1\", \"up2\"]\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\ngroup = \"up\"\nstep = 3\n",
            "outside 1..=2",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"rocker2\"\npositions = [\"off\", \"up1\", \"up2\"]\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\ngroup = \"down\"\nstep = 1\n",
            "no group `down`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\nkind = \"analog\"\ncontrol = \"a\"\n",
            "needs `levels`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\nkind = \"analog\"\ncontrol = \"a\"\nlevels = { half = 1.0 }\n",
            "unknown position `half`",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\nwhen = [\"on\"]\nlevels = { on = 1.0 }\n",
            "only for analog",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\nwhen = [\"on\"]\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\nwhen = [\"on\"]\n",
            "duplicate signal name",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[interlock]]\nbetween = [\"a\"]\n",
            "at least two",
        ),
        (
            "[[control]]\nid = \"a\"\nkind = \"button\"\n[[interlock]]\nbetween = [\"a\", \"b\"]\n",
            "control `b` does not exist",
        ),
        (
            "[[control]]\nid = \"a b\"\nkind = \"button\"\n",
            "id must be",
        ),
    ];
    for (body, want) in cases {
        let errs = errors(&format!("{HEAD}{body}"));
        assert!(
            errs.iter().any(|e| e.contains(want)),
            "wanted `{want}` for\n{body}\ngot {errs:?}"
        );
    }
    assert!(errors(HEAD).iter().any(|e| e.contains("no controls")));
    assert!(errors("[controller]\nid = \"\"\nname = \"\"\n").len() >= 2);
    assert!(errors("[controller]\nid = \"x\"\nname = \"x\"\nsupply = { kind = \"ac\", volts = 0 }\n[[control]]\nid=\"a\"\nkind=\"button\"\n")
        .iter()
        .any(|e| e.contains("supply volts")));
    assert!(
        errors(&format!(
            "{HEAD}gate = [\"q=on\"]\n[[control]]\nid = \"a\"\nkind = \"button\"\n"
        ))
        .iter()
        .any(|e| e.contains("control `q` does not exist"))
    );
}

#[test]
fn validator_warnings() {
    let warns = |text: &str| -> Vec<String> {
        validate(&Profile::parse(text).unwrap())
            .into_iter()
            .filter(|i| i.severity == Severity::Warning)
            .map(|i| i.message)
            .collect()
    };
    let no_common = "[controller]\nid = \"t\"\nname = \"T\"\nkind = \"radio\"\n[[control]]\nid = \"a\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"a\"\nwhen = [\"on\"]\n";
    let w = warns(no_common);
    assert!(w.iter().any(|m| m.contains("no `common` pin")), "{w:?}");
    assert!(w.iter().any(|m| m.contains("no emergency stop")), "{w:?}");
    assert!(
        warns(
            "[controller]\nid = \"t\"\nname = \"T\"\n[[control]]\nid = \"a\"\nkind = \"button\"\n"
        )
        .iter()
        .any(|m| m.contains("no signals"))
    );
    let spring_requires = format!(
        "{HEAD}[[control]]\nid = \"a\"\nkind = \"button\"\nrequires = [\"b=on\"]\n[[control]]\nid = \"b\"\nkind = \"key\"\n"
    );
    assert!(
        warns(&spring_requires)
            .iter()
            .any(|m| m.contains("meant for latches"))
    );
}

#[test]
fn load_rejects_unknown_fields_and_bad_values() {
    for bad in [
        "[controller]\nid = \"t\"\nname = \"T\"\ncolour = \"red\"\n",
        "[controller]\nid = \"t\"\nname = \"T\"\nkind = \"spaceship\"\n",
        "[controller]\nid = \"t\"\nname = \"T\"\n[[control]]\nid = \"a\"\nkind = \"slider\"\n",
        "this is not toml",
    ] {
        assert!(
            matches!(Profile::load(bad), Err(ProfileError::Syntax(_))),
            "{bad}"
        );
    }
    let err = Profile::load(&format!(
        "{HEAD}[[control]]\nid = \"a\"\nkind = \"rocker2\"\n"
    ))
    .unwrap_err();
    assert!(matches!(err, ProfileError::Invalid(ref v) if !v.is_empty()));
    assert!(err.to_string().contains("positions"));
    assert!(
        Controller::new(
            Profile::parse(&format!(
                "{HEAD}[[control]]\nid = \"a\"\nkind = \"rocker2\"\n"
            ))
            .unwrap()
        )
        .is_err()
    );
}

#[test]
fn profile_round_trips_through_toml() {
    let p = radio_profile();
    let again = Profile::load(&p.to_toml().unwrap()).unwrap();
    assert_eq!(p, again);
}

// ---- schema ----

fn schema() -> serde_json::Value {
    serde_json::from_str(PROFILE_SCHEMA).expect("schema is JSON")
}

#[test]
fn schema_lists_exactly_the_kinds_the_engine_knows() {
    let s = schema();
    let enum_of = |v: &serde_json::Value| -> Vec<String> {
        v["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    };
    let mut kinds = enum_of(&s["properties"]["control"]["items"]["properties"]["kind"]);
    kinds.sort();
    let mut want: Vec<String> = ControlKind::ALL
        .iter()
        .map(|k| k.as_str().to_string())
        .collect();
    want.sort();
    assert_eq!(kinds, want);

    let mut ck = enum_of(&s["properties"]["controller"]["properties"]["kind"]);
    ck.sort();
    let mut want: Vec<String> = ControllerKind::ALL
        .iter()
        .map(|k| {
            serde_json::to_value(k)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    want.sort();
    assert_eq!(ck, want);

    let mut sk = enum_of(&s["properties"]["signal"]["items"]["properties"]["kind"]);
    sk.sort();
    let mut want: Vec<String> = SignalKind::ALL
        .iter()
        .map(|k| {
            serde_json::to_value(k)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    want.sort();
    assert_eq!(sk, want);
}

#[test]
fn schema_properties_match_the_serialized_field_names() {
    // Serialize a profile that sets every optional field and check the schema knows each key.
    let text = r#"
[controller]
id = "x"
name = "X"
kind = "radio"
supply = { kind = "ac", volts = 48 }
common = "COM"
step2_includes_step1 = true
shared = true
generic = true
gate = ["a=on"]
[effects]
self_test_ms = 5
[[control]]
id = "a"
label = "A"
kind = "latch"
positions = ["off", "on"]
rest = "off"
spring = false
needs_link = true
requires = ["b=on"]
drops_when = ["b=on"]
[[control]]
id = "b"
kind = "key"
[[signal]]
name = "S"
label = "S"
kind = "analog"
control = "a"
levels = { on = 1.0 }
unit = "V"
requires = ["b=on"]
ungated = true
needs_link = false
pin = "1"
[[signal]]
name = "G"
control = "a"
when = ["on"]
group = "up"
step = 1
[[interlock]]
between = ["a", "b"]
mode = "both_off"
"#;
    let profile = Profile::parse(text).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&profile).unwrap()).unwrap();
    let s = schema();
    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    let section = |name: &str, schema_obj: &serde_json::Value| {
        let node = &value[name];
        let item = if node.is_array() { &node[0] } else { node };
        let have: Vec<String> = keys(item)
            .into_iter()
            .filter(|k| !item[k.as_str()].is_null())
            .collect();
        let known = keys(schema_obj);
        for k in &have {
            assert!(known.contains(k), "schema section `{name}` lacks `{k}`");
        }
        // `when`, `group` and `step` are alternatives, so the fixture sets them on separate signals
        for k in &known {
            assert!(
                have.contains(k) || matches!(k.as_str(), "group" | "step" | "when"),
                "schema `{name}` has stale `{k}`"
            );
        }
    };
    section("controller", &s["properties"]["controller"]["properties"]);
    section("effects", &s["properties"]["effects"]["properties"]);
    section(
        "control",
        &s["properties"]["control"]["items"]["properties"],
    );
    section("signal", &s["properties"]["signal"]["items"]["properties"]);
    section(
        "interlock",
        &s["properties"]["interlock"]["items"]["properties"],
    );
}

// ---- review follow-ups ----

fn radio_pendant_set(radio_shared: bool) -> ControllerSet {
    let mut radio_text = RADIO.to_string();
    if radio_shared {
        radio_text = radio_text.replace("generic = true", "generic = true\nshared = true");
    }
    let radio = Profile::load(&radio_text).unwrap();
    let pendant = Profile::load(
        &radio_text
            .replace("id = \"generic-radio-3x2\"", "id = \"pendant\"")
            .replace("kind = \"radio\"", "kind = \"pendant\""),
    )
    .unwrap();
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &radio).unwrap();
    set.add_instance("pendant", "Pendant", &pendant).unwrap();
    set
}

#[test]
fn shared_estop_chain_is_an_and_so_a_pressed_pendant_estop_opens_it() {
    let set_up = || {
        let mut set = radio_pendant_set(true);
        for who in ["radio", "pendant"] {
            act(&mut set, who, press("start"));
        }
        set
    };
    let chain = Target::Pin("-K1:chain".into());
    let mut t = BindingTable::new();
    t.bind(SignalKey::new("radio", "ESTOP_OK"), chain.clone());
    t.bind(SignalKey::new("pendant", "ESTOP_OK"), chain.clone());

    let mut set = set_up();
    assert_eq!(t.resolve(&set)[&chain], 1.0);
    act(&mut set, "pendant", press("estop"));
    assert_eq!(set.level(&SignalKey::new("radio", "ESTOP_OK")), Some(1.0));
    assert_eq!(
        t.resolve(&set)[&chain],
        0.0,
        "radio E-stop released must not hide the pendant E-stop"
    );
    assert!(
        t.check(&set)
            .iter()
            .all(|f| f.problem != BindingProblem::SharedChainNotAnd)
    );

    // a profile that forgot `combine = "and"` on its chain signal gets a warning
    let text = RADIO
        .replace("combine = \"and\"", "")
        .replace("generic = true", "generic = true\nshared = true");
    let lax = Profile::load(&text).unwrap();
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &lax).unwrap();
    set.add_instance("other", "Other", &lax).unwrap();
    let mut t = BindingTable::new();
    t.bind(SignalKey::new("radio", "ESTOP_OK"), chain.clone());
    t.bind(SignalKey::new("other", "ESTOP_OK"), chain);
    let findings = t.check(&set);
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.problem == BindingProblem::SharedChainNotAnd)
            .count(),
        1
    );
    assert!(
        findings
            .iter()
            .all(|f| f.problem != BindingProblem::TargetConflict)
    );
}

#[test]
fn a_target_that_mixes_an_and_chain_with_an_ordinary_signal_is_reported() {
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &radio_profile())
        .unwrap();
    let chain = Target::Pin("-A1:E1".into());
    fn mixed(t: &BindingTable, set: &ControllerSet) -> Vec<BindingFinding> {
        t.check(set)
            .into_iter()
            .filter(|f| f.problem == BindingProblem::MixedOrAnd)
            .collect()
    }

    let mut t = BindingTable::new();
    t.bind(SignalKey::new("radio", "ESTOP_OK"), chain.clone());
    assert!(mixed(&t, &set).is_empty(), "a chain alone is fine");

    // an ordinary signal on the same target hides an open chain: the check says so
    t.bind(SignalKey::new("radio", "MAIN"), chain.clone());
    let found = mixed(&t, &set);
    assert_eq!(found.len(), 1, "one line per target");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(found[0].message.contains("radio/ESTOP_OK"));
    assert!(found[0].message.contains("radio/MAIN"));

    // the behaviour the warning is about: the ordinary signal (here MAIN, inverted so it is high
    // while the main switch is off, as an E-stop pressed drops it) overrides the open chain
    let mut t = BindingTable::new();
    t.bind(SignalKey::new("radio", "ESTOP_OK"), chain.clone());
    t.bind_with(SignalKey::new("radio", "MAIN"), chain.clone(), true);
    assert_eq!(mixed(&t, &set).len(), 1);
    assert_eq!(t.resolve(&set)[&chain], 1.0);
    act(&mut set, "radio", press("estop"));
    assert_eq!(set.level(&SignalKey::new("radio", "ESTOP_OK")), Some(0.0));
    assert_eq!(
        t.resolve(&set)[&chain],
        1.0,
        "the pressed E-stop is hidden: this is what the check warns about"
    );

    // two ordinary signals alone are not this problem
    let mut t = BindingTable::new();
    t.bind(SignalKey::new("radio", "MAIN"), chain.clone());
    t.bind(SignalKey::new("radio", "H_UP_SLOW"), chain);
    assert!(mixed(&t, &set).is_empty());
}

#[test]
fn two_signals_of_one_controller_on_one_target_are_reported() {
    let set = two_radios();
    let mut t = BindingTable::new();
    let coil = Target::Pin("-K5:A1".into());
    t.bind(SignalKey::new("radio-crane1", "B_FWD_SLOW"), coil.clone());
    t.bind(SignalKey::new("radio-crane1", "B_FWD_FAST"), coil);
    let n = t
        .check(&set)
        .iter()
        .filter(|f| f.problem == BindingProblem::SameControllerTwice)
        .count();
    assert_eq!(n, 1);
}

#[test]
fn absurd_replays_are_refused_instead_of_allocating_billions_of_samples() {
    let typo = Sequence::parse(
        "[[event]]\nt_ms = 3600000000\ninstance = \"radio-crane1\"\naction = \"press\"\ncontrol = \"start\"\n",
    )
    .unwrap();
    assert!(
        typo.check(&two_radios())
            .iter()
            .any(|p| p.contains("24 hours"))
    );
    assert_eq!(
        typo.replay(&mut two_radios(), 1, 0).err(),
        Some(SetError::TooManySamples)
    );
    // without periodic samples it is only one event and cheap
    assert!(typo.replay(&mut two_radios(), 0, 0).is_ok());

    let huge = Sequence {
        name: String::new(),
        events: vec![TimedAction {
            t_ms: u64::MAX,
            instance: "radio-crane1".into(),
            action: press("start"),
        }],
    };
    // adding the settle time must not overflow
    assert_eq!(
        huge.replay(&mut two_radios(), 1, 10).err(),
        Some(SetError::TooManySamples)
    );
    assert!(huge.replay(&mut two_radios(), 0, 10).is_ok());
}

#[test]
fn sampling_period_does_not_change_the_signals_at_common_times() {
    let seq = script();
    let run = |step| seq.replay(&mut two_radios(), step, 500).unwrap();
    let (fine, coarse) = (run(10), run(100));
    for c in &coarse.samples {
        let f = fine
            .samples
            .iter()
            .find(|s| s.t_ms == c.t_ms)
            .expect("fine run has every coarse time");
        assert_eq!(f.signals, c.signals, "at {} ms", c.t_ms);
    }
}

#[test]
fn instance_ids_must_be_usable_in_keys() {
    let p = radio_profile();
    let mut set = ControllerSet::new();
    for bad in ["", "a/b", "has space"] {
        assert_eq!(
            set.add_instance(bad, "x", &p),
            Err(SetError::InvalidId(bad.into()))
        );
    }
    assert!(set.instances().is_empty());
}

#[test]
fn from_records_brings_lagging_instances_to_the_set_clock() {
    let mut set = two_radios();
    set.advance(1000);
    set.add_instance("late", "Late", &radio_profile()).unwrap();
    let mut records = set.to_records();
    // a record saved while its instance was behind the others
    records[0].state.time_ms = 0;
    let mut lib = ProfileLibrary::new();
    lib.insert(radio_profile());
    let back = ControllerSet::from_records(&records, &lib).unwrap();
    assert_eq!(back.time_ms(), 1000);
    assert!(
        back.instances()
            .iter()
            .all(|i| i.controller.time_ms() == 1000)
    );
}

#[test]
fn requires_cycle_is_a_warning() {
    let text = format!(
        "{HEAD}[[control]]\nid = \"a\"\nkind = \"latch\"\nrequires = [\"b=on\"]\n[[control]]\nid = \"b\"\nkind = \"latch\"\nrequires = [\"a=on\"]\n"
    );
    let issues = validate(&Profile::parse(&text).unwrap());
    assert!(
        issues
            .iter()
            .any(|i| i.severity == Severity::Warning && i.message.contains("lead back to itself")),
        "{issues:?}"
    );
    assert!(validate(&radio_profile()).is_empty());
}

// ---- review items of the C1 follow-up (folded into C2) ----

#[test]
fn restoring_records_applies_the_instance_id_rules() {
    let mut lib = ProfileLibrary::new();
    lib.insert(radio_profile());
    let record = |id: &str| InstanceRecord {
        id: id.into(),
        label: "x".into(),
        colour: None,
        target_group: None,
        profile: "generic-radio-3x2".into(),
        state: Controller::new(radio_profile()).unwrap().state().clone(),
    };
    for bad in ["", "a/b", "has space", "tab\there"] {
        assert_eq!(
            ControllerSet::from_records(&[record(bad)], &lib).err(),
            Some(SetError::InvalidId(bad.into())),
            "{bad:?}"
        );
    }
    assert!(ControllerSet::from_records(&[record("radio-1")], &lib).is_ok());
}

#[test]
fn a_binding_to_an_unknown_instance_adds_nothing_to_a_target() {
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &radio_profile())
        .unwrap();
    act(&mut set, "radio", press("start"));
    let chain = Target::Pin("-K1:chain".into());
    let mut t = BindingTable::new();
    t.bind(SignalKey::new("radio", "ESTOP_OK"), chain.clone());
    t.bind(SignalKey::new("gone", "ESTOP_OK"), chain.clone());
    assert_eq!(
        t.resolve(&set)[&chain],
        1.0,
        "the missing controller must not read as an open chain"
    );
    let lonely = Target::Pin("-K9:1".into());
    t.bind(SignalKey::new("gone", "MAIN"), lonely.clone());
    assert!(!t.resolve(&set).contains_key(&lonely));
    assert!(
        t.check(&set)
            .iter()
            .any(|f| f.problem == BindingProblem::UnknownSignal)
    );
}

#[test]
fn one_and_signal_does_not_turn_the_whole_target_into_an_and() {
    // MAIN (ordinary) and ESTOP_OK (and) of two radios on one pin: the E-stop chain of the
    // radios combine as AND among themselves, the MAIN signals as OR, and the target takes the
    // larger of the two results.
    let shared = RADIO.replace("generic = true", "generic = true\nshared = true");
    let p = Profile::load(&shared).unwrap();
    let mut set = ControllerSet::new();
    set.add_instance("a", "A", &p).unwrap();
    set.add_instance("b", "B", &p).unwrap();
    let pin = Target::Pin("-K1:1".into());
    let mut t = BindingTable::new();
    t.bind(SignalKey::new("a", "ESTOP_OK"), pin.clone());
    t.bind(SignalKey::new("b", "ESTOP_OK"), pin.clone());
    t.bind(SignalKey::new("a", "MAIN"), pin.clone());
    t.bind(SignalKey::new("b", "MAIN"), pin.clone());
    act(&mut set, "a", press("start"));
    // a: chain 1, main 1; b: chain 1, main 0
    assert_eq!(t.resolve(&set)[&pin], 1.0);
    act(&mut set, "a", press("estop"));
    // a: chain 0, main 0; b: chain 1, main 0 -> AND group 0, OR group 0
    assert_eq!(t.resolve(&set)[&pin], 0.0);
    act(&mut set, "b", press("start"));
    // b main 1 keeps the OR group high although a's chain is open: the target is not an AND
    assert_eq!(t.resolve(&set)[&pin], 1.0);
}

#[test]
fn an_inverted_binding_drives_the_opposite_level() {
    let mut set = ControllerSet::new();
    set.add_instance("radio", "Radio", &radio_profile())
        .unwrap();
    let estop_symbol = Target::Symbol("-S0".into());
    let mut t = BindingTable::new();
    assert!(t.bind_with(
        SignalKey::new("radio", "ESTOP_OK"),
        estop_symbol.clone(),
        true
    ));
    assert!(
        !t.bind(SignalKey::new("radio", "ESTOP_OK"), estop_symbol.clone()),
        "same pair, whatever the flag"
    );
    assert_eq!(
        t.resolve(&set)[&estop_symbol],
        0.0,
        "released: not operated"
    );
    act(&mut set, "radio", press("estop"));
    assert_eq!(t.resolve(&set)[&estop_symbol], 1.0, "pressed: operated");
}

#[test]
fn project_text_carries_the_profiles_and_the_project_copy_wins() {
    let mut set = two_radios();
    act(&mut set, "radio-crane1", press("start"));
    let mut table = BindingTable::new();
    table.bind(
        SignalKey::new("radio-crane1", "MAIN"),
        Target::Pin("-A1:1".into()),
    );
    table.bind_with(
        SignalKey::new("radio-crane1", "ESTOP_OK"),
        Target::Symbol("-S0".into()),
        true,
    );
    let data = ProjectControllers::capture(&set, &table).unwrap();
    assert_eq!(data.profiles.len(), 1, "one profile for two instances");
    let text = data.to_text().unwrap();
    let back = ProjectControllers::from_text(&text).unwrap();
    assert_eq!(back, data);

    // an empty library: the project still opens, with the profile it was saved with
    let (restored, restored_table, notes) = back.restore(&ProfileLibrary::new()).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored.signals(), set.signals());
    assert_eq!(restored_table, table);

    // the library holds a different copy under the same id: the project's copy is used and a note says so
    let mut other = radio_profile();
    other.controller.name = "Edited since".into();
    let mut lib = ProfileLibrary::new();
    lib.insert(other);
    let (again, _, notes) = back.restore(&lib).unwrap();
    assert_eq!(again.signals(), set.signals());
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("differs"), "{notes:?}");
    assert_eq!(
        again
            .instance("radio-crane1")
            .unwrap()
            .controller
            .profile()
            .controller
            .name,
        radio_profile().controller.name
    );

    // old project data without the profile block still loads
    let old = "[[instance]]\nid = \"a\"\nlabel = \"A\"\nprofile = \"generic-radio-3x2\"\n[instance.state]\npositions = {}\nlink_ok = true\nbattery_low = false\npowered = true\ntime_ms = 0\npowered_at_ms = 0\n";
    let parsed = ProjectControllers::from_text(old).unwrap();
    assert!(parsed.profiles.is_empty());
    assert!(!parsed.is_empty());
}

// ---- ownership of targets ----

#[test]
fn ownership_names_owners_conflicts_and_contested_moments() {
    let mut set = two_radios();
    let mut t = BindingTable::new();
    let shared = Target::Pin("-A1:9".into());
    let own = Target::Pin("-A1:1".into());
    t.bind(SignalKey::new("radio-crane1", "MAIN"), shared.clone());
    t.bind(SignalKey::new("radio-crane2", "MAIN"), shared.clone());
    t.bind(SignalKey::new("radio-crane1", "B_FWD_SLOW"), own.clone());

    assert!(!set.is_active("radio-crane1"));
    assert!(!set.is_active("nope"));
    let rows = t.ownership(&set);
    assert_eq!(rows.len(), 2);
    let row = |target: &Target| rows.iter().find(|r| &r.target == target).unwrap();
    assert_eq!(row(&shared).controllers, ["radio-crane1", "radio-crane2"]);
    assert!(row(&shared).conflict, "neither radio is marked shared");
    assert!(!row(&shared).contested, "nobody presses yet");
    assert!(!row(&own).conflict);
    assert_eq!(row(&own).controllers, ["radio-crane1"]);

    act(&mut set, "radio-crane1", press("start"));
    assert!(set.is_active("radio-crane1"));
    assert!(!set.is_active("radio-crane2"));
    let rows = t.ownership(&set);
    let r = rows.iter().find(|r| r.target == shared).unwrap();
    assert_eq!(r.driving, [SignalKey::new("radio-crane1", "MAIN")]);
    assert!(!r.contested, "one controller drives it");

    act(&mut set, "radio-crane2", press("start"));
    let rows = t.ownership(&set);
    let r = rows.iter().find(|r| r.target == shared).unwrap();
    assert!(r.contested, "both radios are high on the same pin");
    assert_eq!(r.driving.len(), 2);
}
