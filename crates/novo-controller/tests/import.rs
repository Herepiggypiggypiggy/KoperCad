//! Import wizard tests. Tables are invented generic ones, written for these tests.

use novo_controller::import::{
    DevicePin, Options, Outcome, Severity, Table, WordList, library, propose, tokenize,
};

fn run(text: &str) -> novo_controller::import::Proposal {
    propose(
        &Table::from_text(text),
        &WordList::builtin(),
        &Options::default(),
    )
}

fn group_step(p: &novo_controller::import::Proposal, name: &str) -> (&'static str, u8) {
    let s = signal(p, name);
    let g = s.group.clone().expect("rocker signal has a group");
    let g: &'static str = Box::leak(g.into_boxed_str());
    (g, s.step.expect("rocker signal has a step"))
}

fn signal<'a>(
    p: &'a novo_controller::import::Proposal,
    name: &str,
) -> &'a novo_controller::import::Signal {
    p.profile
        .signals
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no signal {name}: {:?}", p.profile.signals))
}

#[test]
fn english_pasted_table_with_header() {
    let p = run("Terminal\tFunction\tStep\n\
                 COM\tCommon 48 VAC\t\n\
                 1\tHoist up\t1\n\
                 2\tHoist up\t2\n\
                 3\tHoist down\t1\n\
                 4\tHoist down\t2\n");
    assert_eq!(p.profile.common.as_deref(), Some("COM"));
    let s = p.profile.supply.expect("supply read from `48 VAC`");
    assert!(s.ac);
    assert_eq!(s.volts, 48.0);
    assert_eq!(p.profile.controls.len(), 1);
    let c = &p.profile.controls[0];
    assert_eq!(c.id, "hoist");
    assert_eq!(c.kind, "rocker2");
    assert_eq!(c.positions, ["off", "up1", "up2", "down1", "down2"]);
    // Step 2 includes step 1 (a real two-step switch) unless the profile says otherwise.
    assert!(p.profile.step2_includes_step1);
    assert_eq!(group_step(&p, "HOIST_UP_SLOW"), ("up", 1));
    assert_eq!(group_step(&p, "HOIST_UP_FAST"), ("up", 2));
    assert_eq!(group_step(&p, "HOIST_DN_SLOW"), ("down", 1));
    assert_eq!(group_step(&p, "HOIST_DN_FAST"), ("down", 2));
    assert_eq!(signal(&p, "HOIST_UP_FAST").pin, "2");
    let pins: Vec<(&str, &str)> = p
        .bindings
        .iter()
        .map(|b| (b.signal.as_str(), b.pin.as_str()))
        .collect();
    assert!(pins.contains(&("HOIST_UP_FAST", "2")));
    assert!(pins.contains(&("HOIST_DN_SLOW", "3")));
}

#[test]
fn step_two_can_drop_step_one() {
    let table = Table::from_text("1;Hoist up;1\n2;Hoist up;2\n3;Hoist down;1\n4;Hoist down;2");
    let options = Options {
        step2_includes_step1: false,
        ..Options::default()
    };
    let p = propose(&table, &WordList::builtin(), &options);
    assert!(!p.profile.step2_includes_step1);
    assert_eq!(group_step(&p, "HOIST_UP_FAST"), ("up", 2));
    assert!(p.profile.to_toml().contains("step2_includes_step1 = false"));
}

#[test]
fn dutch_words_without_header_and_step_words() {
    let p = run("11  Hijsen langzaam\n\
                 12  Hijsen snel\n\
                 13  Zakken langzaam\n\
                 14  Zakken snel\n\
                 15  Brug vooruit 1S\n\
                 16  Brug vooruit 2S\n\
                 17  Brug achteruit 1S\n\
                 18  Brug achteruit 2S\n\
                 19  Noodstop\n\
                 20  Claxon\n\
                 COM  Gemeenschappelijk\n");
    let ids: Vec<&str> = p.profile.controls.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["hoist", "bridge", "estop", "horn"]);
    assert_eq!(group_step(&p, "HOIST_UP_FAST"), ("up", 2));
    assert_eq!(group_step(&p, "BRIDGE_FWD_SLOW"), ("forward", 1));
    assert_eq!(group_step(&p, "BRIDGE_BACK_FAST"), ("back", 2));
    assert_eq!(signal(&p, "ESTOP_OK").when, ["released"]);
    assert!(signal(&p, "ESTOP_OK").ungated);
    assert_eq!(signal(&p, "HORN").when, ["on"]);
    assert_eq!(p.profile.common.as_deref(), Some("COM"));
}

#[test]
fn csv_with_semicolons_and_quotes() {
    let p = run("klem;omschrijving;stap\n\
                 1;\"Hijsen; hoofdhijs\";1\n\
                 2;\"Zakken; hoofdhijs\";1\n");
    // A quoted cell keeps its semicolon; the Dutch header words are understood.
    assert!(p.notes.iter().any(|n| n.contains("header")));
    assert_eq!(p.profile.controls[0].kind, "rocker2");
    assert_eq!(p.bindings.len(), 2);
    assert_eq!(p.bindings[0].pin, "1");
}

#[test]
fn three_steps_use_numbers_and_rocker3() {
    let p = run(
        "1;Trolley left;1\n2;Trolley left;2\n3;Trolley left;3\n4;Trolley right;1\n5;Trolley right;2\n6;Trolley right;3",
    );
    assert_eq!(p.profile.controls[0].kind, "rocker3");
    assert_eq!(group_step(&p, "TROLLEY_LEFT_1"), ("left", 1));
    assert_eq!(group_step(&p, "TROLLEY_RIGHT_3"), ("right", 3));
    assert_eq!(p.profile.controls[0].positions.len(), 7);
}

#[test]
fn combined_label_row_is_skipped_with_a_warning() {
    let p = run("1;Hoist up/down;1\n2;Hoist up;1\n3;Hoist down;1");
    assert!(matches!(&p.rows[0].outcome, Outcome::Skipped(r) if r.contains("both")));
    assert!(
        p.warnings
            .iter()
            .any(|w| w.line == 1 && w.severity == Severity::Warning)
    );
    assert_eq!(p.profile.signals.len(), 2);
}

#[test]
fn loose_number_is_not_taken_as_a_step() {
    let p = run("1;Hoist 2 up\n2;Hoist 2 down");
    assert!(p.warnings.iter().any(|w| w.message.contains("unit number")));
    // Both read as step 1 (default) with an info note.
    assert!(
        p.warnings
            .iter()
            .any(|w| w.severity == Severity::Info && w.message.contains("No step"))
    );
}

#[test]
fn unknown_rows_and_selectors_are_listed_not_guessed() {
    let p = run("1;Hoist up;1\n2;Hoist down;1\n3;Frobnicate\n4;Selector main or aux");
    assert!(matches!(&p.rows[2].outcome, Outcome::Skipped(r) if r == "not recognised"));
    assert!(matches!(&p.rows[3].outcome, Outcome::Skipped(r) if r == "selector"));
    assert!(p.warning_count() >= 2);
}

#[test]
fn missing_direction_step_and_common_are_reported() {
    let p = run("1;Hoist up;1\n2;Hoist up;2\n3;Bridge forward;1\n4;Bridge back;2");
    let msgs: Vec<&str> = p.warnings.iter().map(|w| w.message.as_str()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("one direction only")),
        "{msgs:?}"
    );
    assert!(msgs.iter().any(|m| m.contains("no step 1 pin")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("No common")), "{msgs:?}");
}

#[test]
fn two_rows_for_one_signal_and_one_terminal_are_kept_and_flagged() {
    let p = run("1;Hoist up;1\n1;Hoist up;1\n2;Hoist down;1");
    assert!(p.profile.signals.iter().any(|s| s.name == "HOIST_UP_1_2"));
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("Terminal 1 is used for"))
    );
}

#[test]
fn parallel_main_contacts_become_numbered_signals() {
    let p = run("1;Main 1;\n2;Main 2;\n3;Emergency stop");
    let names: Vec<&str> = p.profile.signals.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"MAIN") && names.contains(&"MAIN_2") && names.contains(&"ESTOP_OK"));
}

#[test]
fn main_hoist_is_a_hoist_not_the_main_contactor() {
    let p = run("1;Main hoist up\n2;Main hoist down");
    assert_eq!(p.profile.controls.len(), 1);
    assert_eq!(p.profile.controls[0].id, "hoist");
}

#[test]
fn rows_without_motion_borrow_the_only_named_motion() {
    let p = run("1;Hoist up\n2;Down");
    assert_eq!(p.profile.controls.len(), 1);
    // Several motions named: a bare direction goes by the crane convention.
    let q = run("1;Hoist up\n2;Bridge forward\n3;Down");
    assert!(matches!(&q.rows[2].outcome, Outcome::Signal(n) if n.starts_with("HOIST_DN")));
    // A bare direction the named motion cannot make (right) is not forced onto it.
    let r = run("1;Hoist up\n2;Hoist down\n3;Right");
    assert!(matches!(&r.rows[2].outcome, Outcome::Signal(n) if n.starts_with("TROLLEY")));
    // Slew has no convention: it borrows the only named motion.
    let s = run("1;Slew left\n2;Right");
    assert!(matches!(&s.rows[1].outcome, Outcome::Signal(n) if n.starts_with("SLEW")));
}

#[test]
fn bare_directions_follow_the_crane_convention() {
    let p = run("COM;Common\n1;Right\n2;Left\n3;Forward\n4;Back\n5;Up\n6;Down\n7;Fast");
    let names: Vec<&str> = p.profile.signals.iter().map(|s| s.name.as_str()).collect();
    for want in ["TROLLEY_RIGHT_1", "BRIDGE_FWD_1", "HOIST_UP_1"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("usual crane convention"))
    );
    // A speed alone is not a signal.
    assert!(matches!(&p.rows[7].outcome, Outcome::Skipped(r) if r == "step without motion"));
}

#[test]
fn step_column_beats_text_and_conflict_is_flagged() {
    let p = run("1;Hoist up 1S;2\n2;Hoist down 1S;1");
    // The column says step 2, so the pin is the fast one although the text says 1S.
    assert_eq!(group_step(&p, "HOIST_UP_FAST"), ("up", 2));
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("different steps"))
    );
}

#[test]
fn from_device_keeps_the_wire_targets() {
    let pins = vec![
        DevicePin {
            name: "COM".into(),
            text: "Common".into(),
            target: String::new(),
        },
        DevicePin {
            name: "13".into(),
            text: "Hoist up 1".into(),
            target: "-K61:A1".into(),
        },
        DevicePin {
            name: "14".into(),
            text: "Hoist down 1".into(),
            target: "-K62:A1".into(),
        },
        DevicePin {
            name: "15".into(),
            text: String::new(),
            target: "-Q hoist stop".into(),
        },
    ];
    let p = propose(
        &Table::from_pins(&pins),
        &WordList::builtin(),
        &Options::default(),
    );
    let b = p.bindings.iter().find(|b| b.pin == "13").unwrap();
    assert_eq!(b.target, "-K61:A1");
    assert_eq!(p.profile.common.as_deref(), Some("COM"));
}

#[test]
fn toml_has_profile_controls_and_signals() {
    let p = run("COM;Common;\n1;Hoist up;1\n2;Hoist down;1");
    let t = p.profile.to_toml();
    assert!(t.contains("[controller]") && t.contains("common = \"COM\""));
    assert!(t.contains("[[control]]") && t.contains("positions = [\"off\", \"up1\", \"down1\"]"));
    assert!(t.contains("[[signal]]") && t.contains("name = \"HOIST_UP_1\""));
    assert!(t.contains("group = \"up\"") && t.contains("step = 1") && t.contains("pin = \"1\""));
    assert!(p.bindings_csv().starts_with("signal;pin;target\n"));
}

#[test]
fn a_user_word_list_extends_the_built_in_one() {
    let mut words = WordList::builtin();
    let problems = words.extend("motion.hoist: winch\nnonsense line\nbogus.x: a");
    assert_eq!(problems.len(), 2);
    let table = Table::from_text("1;Winch up\n2;Winch down");
    let p = propose(&table, &words, &Options::default());
    assert_eq!(p.profile.controls[0].id, "hoist");
}

#[test]
fn tokenizer_and_phrases() {
    assert_eq!(
        tokenize("E-STOP / Nood-stop"),
        ["e", "stop", "nood", "stop"]
    );
    let words = WordList::builtin();
    let hits = words.scan(&tokenize("Emergency stop"));
    assert_eq!(hits.len(), 1);
}

#[test]
fn the_library_builds_clean_profiles() {
    for (id, _) in library::list() {
        let p = library::build(id).unwrap();
        assert_eq!(p.warning_count(), 0, "{id}: {:?}", p.warnings);
        assert!(p.profile.common.is_some());
        assert!(!p.profile.signals.is_empty());
    }
    let radio = library::build("generic-radio-5x2").unwrap();
    assert_eq!(
        radio
            .profile
            .controls
            .iter()
            .filter(|c| c.kind == "rocker2")
            .count(),
        5
    );
    let joy = library::build("generic-joystick-2axis").unwrap();
    assert_eq!(joy.profile.kind, "joystick");
}

#[test]
fn builtin_lists_have_no_problems_and_no_vendor_names() {
    let en = include_str!("../data/words-en.txt");
    let nl = include_str!("../data/words-nl.txt");
    let mut list = WordList::default();
    assert!(list.extend(en).is_empty() && list.extend(nl).is_empty());
    for vendor in ["demag", "telecrane", "hbc", "eplan"] {
        assert!(!en.to_lowercase().contains(vendor) && !nl.to_lowercase().contains(vendor));
    }
}

#[test]
fn target_column_and_text_round_trip() {
    use novo_controller::import::rows_to_text;
    let pins = vec![
        DevicePin {
            name: "13".into(),
            text: "Hoist up 1".into(),
            target: "-K61:A1".into(),
        },
        DevicePin {
            name: "14".into(),
            text: "Hoist down 1".into(),
            target: "-K62:A1".into(),
        },
    ];
    let text = rows_to_text(&Table::from_pins(&pins).rows);
    let table = Table::from_text(&text);
    assert!(table.had_header);
    assert_eq!(table.rows[0].target, "-K61:A1");
    let p = propose(&table, &WordList::builtin(), &Options::default());
    assert_eq!(p.bindings[1].target, "-K62:A1");
    assert!(library::table("generic-radio-3x2").is_some());
}

#[test]
fn main_start_stop_and_estop_make_a_gated_receiver() {
    let p = run("COM;Common\n1;Main start\n2;Stop\n3;Emergency stop\n4;Hoist up;1\n5;Hoist down;1");
    let start = p.profile.controls.iter().find(|c| c.id == "start").unwrap();
    assert_eq!(start.kind, "latch");
    assert_eq!(start.requires, ["estop=released"]);
    assert_eq!(start.drops_when, ["stop=on"]);
    assert_eq!(p.profile.gate, ["start=on"]);
    assert!(signal(&p, "MAIN").ungated);
    let t = p.profile.to_toml();
    assert!(t.contains("gate = [\"start=on\"]"));
    let off = propose(
        &Table::from_text("1;Main start\n4;Hoist up;1\n5;Hoist down;1"),
        &WordList::builtin(),
        &Options {
            gate_on_main: false,
            ..Options::default()
        },
    );
    assert!(off.profile.gate.is_empty());
}

#[test]
fn more_than_three_steps_are_left_out() {
    let p = run("1;Hoist up;1\n2;Hoist up;4\n3;Hoist down;1");
    assert!(matches!(&p.rows[1].outcome, Outcome::Skipped(r) if r.contains("3 steps")));
}

#[test]
fn a_single_direction_single_step_is_a_button() {
    let p = run("1;Bridge forward;1");
    assert_eq!(p.profile.controls[0].kind, "button");
    assert_eq!(signal(&p, "BRIDGE_FWD_1").when, ["on"]);
}

#[test]
fn proposals_parse_and_validate_with_the_engine() {
    use novo_controller::{Profile, Severity as Sev, validate};
    let mixed = run(
        "COM;Common 48 VAC\n1;Main start\n2;Stop\n3;Emergency stop\n4;Hijsen;1\n5;Hijsen;2\n6;Zakken;1\n7;Zakken;2\n8;Horn\n9;Bridge forward\n10;Bridge back",
    );
    let mut texts = vec![("mixed".to_owned(), mixed.profile.to_toml())];
    for (id, _) in library::list() {
        texts.push((id.to_owned(), library::build(id).unwrap().profile.to_toml()));
    }
    for (id, text) in texts {
        let profile = Profile::parse(&text).unwrap_or_else(|e| panic!("{id}: {e}\n{text}"));
        let issues = validate(&profile);
        assert!(issues.is_empty(), "{id}: {issues:?}\n{text}");
        assert!(issues.iter().all(|i| i.severity != Sev::Error));
    }
}

#[test]
fn the_profile_value_equals_the_profile_read_from_the_text() {
    use novo_controller::Profile;
    let mixed = run(
        "COM;Common 48 VAC\n1;Main start\n2;Stop\n3;Emergency stop\n4;Hijsen;1\n5;Hijsen;2\n6;Zakken;1\n7;Zakken;2\n8;Horn\n9;Bridge forward\n10;Bridge back",
    );
    let mut all = vec![("mixed".to_owned(), mixed.profile)];
    for (id, _) in library::list() {
        all.push((id.to_owned(), library::build(id).unwrap().profile));
    }
    for (id, p) in all {
        let direct = p.to_profile().unwrap_or_else(|e| panic!("{id}: {e}"));
        let text = Profile::parse(&p.to_toml()).unwrap();
        assert_eq!(direct, text, "{id}");
    }
}

#[test]
fn an_unknown_kind_is_refused_not_guessed() {
    let mut p = run("COM;Common\n1;Horn").profile;
    p.kind = "teapot".into();
    assert!(p.to_profile().is_err());
}

#[test]
fn a_huge_paste_is_cut_at_the_row_cap_and_stays_fast() {
    use novo_controller::import::MAX_ROWS;
    let mut text = String::from("Terminal;Function;Step\n");
    for i in 0..50_000 {
        text.push_str(&format!("{i};Hoist up;1\n"));
    }
    let started = std::time::Instant::now();
    let table = Table::from_text(&text);
    assert_eq!(table.rows.len(), MAX_ROWS);
    assert!(table.notes.iter().any(|n| n.contains("first 2000 rows")));
    let p = propose(&table, &WordList::builtin(), &Options::default());
    assert!(p.rows.len() <= MAX_ROWS);
    assert!(
        started.elapsed().as_secs() < 5,
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn slugs_are_never_empty_or_reserved_windows_names() {
    use novo_controller::import::slug;
    assert_eq!(slug("!!!"), "controller");
    assert_eq!(slug("é"), "controller");
    assert_eq!(slug(""), "controller");
    for name in [
        "con", "CON", "nul", "aux", "prn", "com1", "COM9", "lpt1", "lpt9",
    ] {
        assert_eq!(
            slug(name),
            format!("controller-{}", name.to_lowercase()),
            "{name}"
        );
    }
    assert_eq!(slug("com"), "com");
    assert_eq!(slug("com10"), "com10");
    assert_eq!(slug("con.txt"), "con-txt");
    assert_eq!(slug("Radio, 3 motions"), "radio-3-motions");
}

// Patterns seen on real receiver pages, reproduced with invented labels and terminals.

const SHARED_FAST_PAGE: &str = "1;NOODSTOP\n2;VOORUIT\n3;ACHTERUIT\n4;V/A SNEL\n5;HIJSEN\n6;ZAKKEN\n\
    7;H/Z SNEL\n8;RECHTS\n9;LINKS\n10;R/L SNEL\n11;STOP\n12;MAIN-1\n13;MAIN-2\n14;48Vac\n15;COM\n16;0Vac";

#[test]
fn one_fast_pin_for_both_directions_feeds_both_signals() {
    let p = run(SHARED_FAST_PAGE);
    assert_eq!(group_step(&p, "HOIST_UP_FAST"), ("up", 2));
    assert_eq!(group_step(&p, "HOIST_DN_FAST"), ("down", 2));
    assert_eq!(signal(&p, "HOIST_UP_FAST").pin, "7");
    assert_eq!(signal(&p, "HOIST_DN_FAST").pin, "7");
    assert_eq!(signal(&p, "TROLLEY_LEFT_FAST").pin, "10");
    assert_eq!(signal(&p, "TROLLEY_RIGHT_FAST").pin, "10");
    assert_eq!(signal(&p, "BRIDGE_FWD_FAST").pin, "4");
    let hoist = p.profile.controls.iter().find(|c| c.id == "hoist").unwrap();
    assert_eq!(hoist.kind, "rocker2");
    assert_eq!(hoist.positions, ["off", "up1", "up2", "down1", "down2"]);
    // The row shows both signals, and the shared pin is not reported as a clash.
    assert!(
        matches!(&p.rows[6].outcome, Outcome::Signal(n) if n == "HOIST_UP_FAST + HOIST_DN_FAST")
    );
    assert!(
        p.warnings
            .iter()
            .all(|w| !w.message.contains("is used for"))
    );
    assert!(p.warnings.iter().all(|w| w.severity != Severity::Warning));
}

#[test]
fn supply_rows_are_read_not_reported() {
    let p = run(SHARED_FAST_PAGE);
    // 48Vac is the supply, COM the common; 0Vac is the neutral of the supply, not a second common.
    let supply = p.profile.supply.expect("48Vac gives the supply");
    assert!(supply.ac);
    assert_eq!(supply.volts, 48.0);
    assert_eq!(p.profile.common.as_deref(), Some("15"));
    assert!(matches!(&p.rows[13].outcome, Outcome::Skipped(r) if r.contains("supply")));
    assert!(matches!(&p.rows[15].outcome, Outcome::Skipped(r) if r.contains("supply")));
    assert!(
        p.warnings
            .iter()
            .all(|w| !w.message.contains("not recognised"))
    );
    // Without COM the 0 V row is the common, as before.
    let q = run("1;Hoist up\n2;Hoist down\n3;0V");
    assert_eq!(q.profile.common.as_deref(), Some("3"));
}

#[test]
fn mains_contacts_are_main_contacts() {
    let p = run("1;Mains-1\n2;Mains-2\n3;Hijsen\n4;Zakken\n5;Stop");
    assert!(p.profile.controls.iter().any(|c| c.id == "start"));
    assert_eq!(signal(&p, "MAIN").pin, "1");
    assert_eq!(signal(&p, "MAIN_2").pin, "2");
}

#[test]
fn compass_words_and_bare_directions_in_a_ten_motion_page() {
    let p = run(
        "1;Up 1S\n2;Up 2S\n3;Down 1S\n4;Down 2S\n5;East 1S\n6;East 2S\n7;West 1S\n8;West 2S\n\
         9;North 1S\n10;North 2S\n11;South 1S\n12;South 2S\n13;STOP\n14;MAIN-1\n15;COM",
    );
    assert_eq!(group_step(&p, "HOIST_UP_FAST"), ("up", 2));
    assert_eq!(group_step(&p, "AUX_DN_SLOW"), ("down", 1));
    assert_eq!(group_step(&p, "SLEW_RIGHT_FAST"), ("right", 2));
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("Compass words"))
    );
    assert!(p.warnings.iter().all(|w| w.severity != Severity::Warning));
}

#[test]
fn a_bulleted_list_of_outputs_has_no_terminal_column() {
    let p = run(
        "- NOODSTOP (O-Q2 A1/A2)\n- VOORUIT (H-Q1 A1/A2)\n- ACHTERUIT (H-Q2 A1/A2)\n\
         - V/A SNEL (H-K41 A1/A2)\n- TOETER (-X1.1 3)",
    );
    assert_eq!(p.rows[0].row.terminal, "");
    assert!(matches!(&p.rows[0].outcome, Outcome::Signal(n) if n == "ESTOP_OK"));
    assert!(matches!(&p.rows[4].outcome, Outcome::Signal(n) if n == "HORN"));
    assert_eq!(group_step(&p, "BRIDGE_BACK_FAST"), ("back", 2));
    // A pin named like a bullet in a real column is kept.
    let q = run("-X1;Hoist up\n-X2;Hoist down");
    assert_eq!(q.rows[0].row.terminal, "-X1");
}

#[test]
fn the_shared_pin_proposal_passes_the_engine_check() {
    use novo_controller::{Severity as Sev, validate};
    let p = run(SHARED_FAST_PAGE);
    let profile = p.profile.to_profile().expect("builds");
    let issues = validate(&profile);
    assert!(
        issues.iter().all(|i| i.severity != Sev::Error),
        "{issues:?}"
    );
}
