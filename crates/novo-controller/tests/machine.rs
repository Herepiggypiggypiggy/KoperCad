//! Machine-module bindings: target syntax, the catalog checks, levels and the project round trip.

use novo_controller::*;

const RADIO: &str = include_str!("data/generic-radio-3x2.controller.toml");

fn catalog() -> MachineCatalog {
    let port = |name: &str, kind| PortEntry {
        name: name.into(),
        label: name.into(),
        kind,
    };
    MachineCatalog {
        modules: vec![
            ModuleEntry {
                path: "hoist/=A-M1".into(),
                label: "Hoist =A-M1".into(),
                ports: vec![
                    port("up", PortKind::Command),
                    port("down", PortKind::Command),
                    port("fast", PortKind::Command),
                    port("permit", PortKind::Chain),
                    port("deadman", PortKind::Enable),
                ],
            },
            ModuleEntry {
                path: "bridge/=A-M2".into(),
                label: "Bridge =A-M2".into(),
                ports: vec![
                    port("forward", PortKind::Command),
                    port("reverse", PortKind::Command),
                    port("permit", PortKind::Chain),
                ],
            },
        ],
    }
}

fn set_of(shared: &[&str]) -> ControllerSet {
    let text = RADIO.replace("generic = true", "generic = true\nshared = true");
    let shared_profile = Profile::load(&text).unwrap();
    let plain = Profile::load(RADIO).unwrap();
    let mut set = ControllerSet::new();
    for id in ["radio-1", "radio-2"] {
        let p = if shared.contains(&id) {
            &shared_profile
        } else {
            &plain
        };
        set.add_instance(id, id, p).unwrap();
    }
    set
}

fn target(text: &str) -> Target {
    Target::machine(&MachinePath::parse(text).unwrap())
}

fn problems(table: &BindingTable, set: &ControllerSet) -> Vec<BindingProblem> {
    table
        .check_with(set, Some(&catalog()))
        .into_iter()
        .map(|f| f.problem)
        .collect()
}

#[test]
fn a_valid_binding_to_a_port_has_no_machine_finding() {
    let set = set_of(&[]);
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    t.bind(
        SignalKey::new("radio-1", "ESTOP_OK"),
        target("hoist/=A-M1@permit"),
    );
    let p = problems(&t, &set);
    assert!(
        p.iter().all(|p| matches!(p, BindingProblem::Unbound)),
        "{p:?}"
    );
}

#[test]
fn unknown_module_unknown_port_missing_port_and_bad_text_are_each_named() {
    let set = set_of(&[]);
    let cases = [
        ("hoist/=A-M9@up", BindingProblem::MachineUnknownModule),
        ("hoist/=A-M1@sideways", BindingProblem::MachineUnknownPort),
        ("hoist/=A-M1", BindingProblem::MachineNoPort),
        ("bridge/=A-M2@up", BindingProblem::MachineUnknownPort),
    ];
    for (text, expected) in cases {
        let mut t = BindingTable::new();
        t.bind(SignalKey::new("radio-1", "H_UP_FAST"), target(text));
        assert!(problems(&t, &set).contains(&expected), "{text}");
    }
    // text that does not parse is kept as written and reported
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        Target::ModulePath("a b@up".into()),
    );
    assert!(problems(&t, &set).contains(&BindingProblem::MachineSyntax));
    // without a catalog the machine part is not run
    assert!(
        !t.check(&set)
            .iter()
            .any(|f| f.problem == BindingProblem::MachineSyntax)
    );
}

#[test]
fn a_signal_of_the_wrong_kind_for_the_port_is_reported() {
    let set = set_of(&[]);
    // chain signal on a motion port: an error
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "ESTOP_OK"),
        target("hoist/=A-M1@up"),
    );
    let f = t.check_with(&set, Some(&catalog()));
    let wrong = f
        .iter()
        .find(|f| f.problem == BindingProblem::MachineWrongKind)
        .expect("wrong kind");
    assert_eq!(wrong.severity, Severity::Error);
    // motion signal on a chain port: a warning
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@permit"),
    );
    let f = t.check_with(&set, Some(&catalog()));
    let wrong = f
        .iter()
        .find(|f| f.problem == BindingProblem::MachineWrongKind)
        .expect("wrong kind");
    assert_eq!(wrong.severity, Severity::Warning);
}

#[test]
fn two_controllers_on_one_module_need_shared_and_two_on_one_port_conflict() {
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    t.bind(
        SignalKey::new("radio-2", "H_DOWN_FAST"),
        target("hoist/=A-M1@down"),
    );
    let p = problems(&t, &set_of(&[]));
    assert!(p.contains(&BindingProblem::MachineTwoOwners), "{p:?}");
    // both marked shared: allowed
    let p = problems(&t, &set_of(&["radio-1", "radio-2"]));
    assert!(!p.contains(&BindingProblem::MachineTwoOwners), "{p:?}");

    // one port reached by two controllers is the ordinary target conflict
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    t.bind(
        SignalKey::new("radio-2", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    assert!(problems(&t, &set_of(&[])).contains(&BindingProblem::TargetConflict));
}

#[test]
fn two_spellings_of_one_port_are_noticed() {
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    t.bind(SignalKey::new("radio-1", "H_UP_SLOW"), target("=A-M1@up"));
    let f = t.check_with(&set_of(&[]), Some(&catalog()));
    assert!(
        f.iter()
            .any(|f| f.problem == BindingProblem::TargetConflict
                && f.message.contains("one spelling"))
    );
}

#[test]
fn a_chain_and_a_motion_signal_on_one_port_is_the_existing_mixed_warning() {
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "ESTOP_OK"),
        target("hoist/=A-M1@permit"),
    );
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@permit"),
    );
    assert!(problems(&t, &set_of(&[])).contains(&BindingProblem::MixedOrAnd));
}

#[test]
fn levels_of_the_ports_follow_the_pressed_controls() {
    let mut set = set_of(&[]);
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    t.bind(
        SignalKey::new("radio-1", "ESTOP_OK"),
        target("hoist/=A-M1@permit"),
    );
    let up = MachinePath::parse("hoist/=A-M1@up").unwrap();
    let permit = MachinePath::parse("hoist/=A-M1@permit").unwrap();
    let levels = t.resolve_machine(&set);
    assert_eq!(levels[&up], 0.0);
    set.act(
        "radio-1",
        &Action::Press {
            control: "start".into(),
        },
    )
    .unwrap();
    set.act(
        "radio-1",
        &Action::SetPosition {
            control: "hoist".into(),
            position: "up2".into(),
        },
    )
    .unwrap();
    let levels = t.resolve_machine(&set);
    assert_eq!(levels[&up], 1.0);
    assert_eq!(levels[&permit], 1.0);
    set.act(
        "radio-1",
        &Action::Press {
            control: "estop".into(),
        },
    )
    .unwrap();
    let levels = t.resolve_machine(&set);
    assert_eq!(levels[&permit], 0.0, "a pressed E-stop opens the chain");
}

#[test]
fn machine_bindings_survive_the_project_text() {
    let set = set_of(&[]);
    let mut t = BindingTable::new();
    t.bind(
        SignalKey::new("radio-1", "H_UP_FAST"),
        target("hoist/=A-M1@up"),
    );
    t.bind_with(
        SignalKey::new("radio-1", "ESTOP_OK"),
        target("hoist/=A-M1@permit"),
        true,
    );
    let data = ProjectControllers::capture(&set, &t).unwrap();
    let text = data.to_text().unwrap();
    let back = ProjectControllers::from_text(&text).unwrap();
    let mut lib = ProfileLibrary::new();
    lib.insert(Profile::load(RADIO).unwrap());
    let (_, table, _) = back.restore(&lib).unwrap();
    assert_eq!(table, t);
    assert_eq!(
        table.bindings[0].target.machine_path().unwrap().to_string(),
        "hoist/=A-M1@up"
    );
}
