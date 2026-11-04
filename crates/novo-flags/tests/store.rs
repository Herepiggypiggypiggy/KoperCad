use novo_flags::*;

fn draft(rule: &str, sev: Severity, anchors: Vec<Anchor>) -> FlagDraft {
    FlagDraft {
        severity: sev,
        unchecked: false,
        source: Source::Issues("check.x".into()),
        rule: rule.into(),
        anchors,
        title: format!("Title {rule}"),
        detail: Detail::new([
            "a".to_string(),
            "".into(),
            "b".into(),
            "c".into(),
            "d".into(),
        ]),
        actions: vec![Action::Locate],
        waiver_key: Some(format!("key.{rule}")),
        life: Life::Live,
    }
}

fn dev(i: &str) -> Anchor {
    Anchor::Device(i.into())
}

#[test]
fn ids_are_stable_and_order_independent() {
    let a = Flag::from_draft(draft("r", Severity::Red, vec![dev("1"), dev("2")]));
    let b = Flag::from_draft(draft("r", Severity::Yellow, vec![dev("2"), dev("1")]));
    assert_eq!(a.id, b.id);
    let c = Flag::from_draft(draft("r2", Severity::Red, vec![dev("1"), dev("2")]));
    assert_ne!(a.id, c.id);
    assert_ne!(FlagId::of(["ab", "c"]), FlagId::of(["a", "bc"]));
}

#[test]
fn detail_keeps_three_nonempty_lines_and_title_is_short() {
    let f = Flag::from_draft(draft("r", Severity::Red, vec![Anchor::Project]));
    assert_eq!(f.detail.0, vec!["a", "b", "c"]);
    let mut d = draft("r", Severity::Red, vec![Anchor::Project]);
    d.title = "x".repeat(60);
    assert_eq!(Flag::from_draft(d).title.chars().count(), 28);
}

#[test]
fn replace_keeps_waivers_and_reports_fresh() {
    let mut s = FlagStore::new();
    let src = Source::Issues("any".into());
    s.replace_source(&src, vec![draft("a", Severity::Red, vec![dev("1")])]);
    assert_eq!(s.take_fresh().len(), 1);
    let id = s.list(false)[0].id;
    s.waive(id);
    assert_eq!(s.counts().total(), 0);
    s.replace_source(
        &src,
        vec![
            draft("a", Severity::Red, vec![dev("1")]),
            draft("b", Severity::Yellow, vec![dev("2")]),
        ],
    );
    assert_eq!(s.take_fresh().len(), 1, "only b is new");
    assert_eq!(s.counts(), RollUp { red: 0, yellow: 1 });
    assert_eq!(s.list(true).len(), 2);
    s.unwaive(id);
    assert_eq!(s.counts(), RollUp { red: 1, yellow: 1 });
}

#[test]
fn waiver_key_keeps_working_across_ids() {
    let mut s = FlagStore::new();
    let src = Source::Issues("k".into());
    s.waive_key("key.a");
    s.replace_source(&src, vec![draft("a", Severity::Red, vec![dev("1")])]);
    assert_eq!(s.counts().total(), 0);
}

#[test]
fn replace_only_touches_its_engine() {
    let mut s = FlagStore::new();
    s.replace_source(
        &Source::Issues("k".into()),
        vec![draft("a", Severity::Red, vec![dev("1")])],
    );
    let mut heat = draft("h", Severity::Yellow, vec![dev("1")]);
    heat.source = Source::Heat;
    s.replace_source(&Source::Heat, vec![heat]);
    s.replace_source(&Source::Issues("other".into()), vec![]);
    assert_eq!(s.counts(), RollUp { red: 0, yellow: 1 });
    // A draft of another engine in the batch is ignored.
    s.replace_source(
        &Source::Heat,
        vec![draft("z", Severity::Red, vec![dev("9")])],
    );
    assert_eq!(s.counts().total(), 0);
}

#[test]
fn roll_up_counts_a_multi_anchor_flag_once() {
    let mut s = FlagStore::new();
    let src = Source::Issues("k".into());
    s.replace_source(
        &src,
        vec![
            draft("a", Severity::Yellow, vec![dev("1"), dev("2")]),
            draft("b", Severity::Red, vec![Anchor::Wire("w".into())]),
        ],
    );
    let r = s.roll_up(&[dev("1"), dev("2")]);
    assert_eq!((r.red, r.yellow, r.worst()), (0, 1, Some(Severity::Yellow)));
    let r = s.roll_up_where(|a| matches!(a, Anchor::Device(_) | Anchor::Wire(_)));
    assert_eq!(r.worst(), Some(Severity::Red));
    assert_eq!(s.at(&dev("2"), false).len(), 1);
    assert_eq!(s.roll_up(&[Anchor::Project]).worst(), None);
}

#[test]
fn f8_order_wraps_red_first() {
    let mut s = FlagStore::new();
    let src = Source::Issues("k".into());
    assert_eq!(s.next(None, true), None);
    s.replace_source(
        &src,
        vec![
            draft("y", Severity::Yellow, vec![dev("1")]),
            draft("r1", Severity::Red, vec![Anchor::Wire("w".into())]),
            draft("r2", Severity::Red, vec![dev("3")]),
        ],
    );
    let order: Vec<Severity> = s.list(false).iter().map(|f| f.severity).collect();
    assert_eq!(order, vec![Severity::Red, Severity::Red, Severity::Yellow]);
    // Schematic area (wire) before cabinet (device) among reds.
    assert!(matches!(s.list(false)[0].anchors[0], Anchor::Wire(_)));
    let first = s.next(None, true).unwrap();
    let second = s.next(Some(first), true).unwrap();
    let third = s.next(Some(second), true).unwrap();
    assert_eq!(s.next(Some(third), true), Some(first), "wraps");
    assert_eq!(
        s.next(Some(first), false),
        Some(third),
        "Shift+F8 wraps back"
    );
    assert_eq!(s.next(None, false), Some(third));
    assert_eq!(s.next(Some(FlagId(1)), true), Some(first), "unknown start");
}

#[test]
fn by_area_groups_by_first_anchor() {
    let mut s = FlagStore::new();
    s.replace_source(
        &Source::Issues("k".into()),
        vec![
            draft("a", Severity::Red, vec![Anchor::Symbol("s".into())]),
            draft("b", Severity::Red, vec![Anchor::Duct("d".into())]),
            draft("c", Severity::Red, vec![Anchor::Project]),
        ],
    );
    let g = s.by_area();
    assert_eq!(g.len(), 3);
    assert_eq!(g[&Area::Schematic].len(), 1);
    assert_eq!(g[&Area::Cabinet].len(), 1);
    assert_eq!(g[&Area::Project].len(), 1);
}

#[test]
fn set_and_clear_job_flag() {
    let mut s = FlagStore::new();
    let mut d = draft("export", Severity::Red, vec![Anchor::Job("pdf".into())]);
    d.source = Source::Job;
    d.life = Life::Until;
    let id = s.set(d);
    assert_eq!(s.counts().red, 1);
    assert!(s.clear(id));
    assert!(!s.clear(id));
    assert_eq!(s.counts().total(), 0);
}

#[test]
fn severity_words_of_all_vocabularies() {
    for w in ["Error", "Problem", "Blocking", "Red", "Over"] {
        assert_eq!(map_word(w), Mapping::Flag(Severity::Red), "{w}");
    }
    for w in ["Warning", "Major", "Busy", "yellow"] {
        assert_eq!(map_word(w), Mapping::Flag(Severity::Yellow), "{w}");
    }
    for w in ["Info", "Note", "Minor", "Idle", "Ok", "S1", "S2"] {
        assert_eq!(map_word(w), Mapping::NoFlag, "{w}");
    }
    assert_eq!(map_word("Catastrophic"), Mapping::Unknown);
}

#[test]
fn accessible_names_separate_the_three_looks() {
    let red = Flag::from_draft(draft("r", Severity::Red, vec![Anchor::Project]));
    let mut y = draft("y", Severity::Yellow, vec![Anchor::Project]);
    let yellow = Flag::from_draft(y.clone());
    y.unchecked = true;
    let dashed = Flag::from_draft(y);
    assert!(red.accessible_name().starts_with("Error: "));
    assert!(yellow.accessible_name().starts_with("Warning: "));
    assert!(dashed.accessible_name().starts_with("Not checked: "));
}

#[test]
fn flags_round_trip_through_json() {
    let f = Flag::from_draft(draft(
        "r",
        Severity::Red,
        vec![dev("1"), Anchor::World("e".into(), [1, 2, 3])],
    ));
    let text = serde_json::to_string(&f).unwrap();
    let back: Flag = serde_json::from_str(&text).unwrap();
    assert_eq!(f, back);
}
