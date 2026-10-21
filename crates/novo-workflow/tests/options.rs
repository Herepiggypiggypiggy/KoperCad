//! Project options and variants: conditions, configuration, diff and the
//! variant project.

use std::collections::BTreeMap;

use novo_core::*;
use novo_workflow::options::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

/// A machine: page 1 "Main" (always), page 2 "Conveyor" (only with the
/// conveyor), and a heater contactor -K9 on page 1 that exists only for the
/// 480 V supply. Parts: every device has one; two devices share contactor
/// part C1.
fn machine() -> Document {
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        let main = tx.add_page("1")?;
        let conv = tx.add_page("2")?;
        tx.update::<Page>(main, |p| {
            p.title = "Main".into();
            p.structure = tag("=A1");
        })?;
        tx.update::<Page>(conv, |p| {
            p.title = "Conveyor".into();
            p.structure = tag("=A1");
        })?;
        let c1 = tx.add_part("ABB", "C1")?;
        let m1p = tx.add_part("SEW", "M1")?;
        let place = |tx: &mut Tx<'_>, page: PageId, t: &str, x: f64, part: PartId| {
            let d = tx.add_device(tag(t), "")?;
            tx.assign_part(d, part)?;
            let s = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(x, 0.0)),
                pins(),
                SymbolRole::Device {
                    device: d,
                    main: true,
                },
            )?;
            Ok::<_, ModelError>((d, s))
        };
        let (_, qs) = place(tx, main, "=A1-K1", 10.0, c1)?;
        let (k9, _) = place(tx, main, "=A1-K9", 30.0, c1)?;
        let (_, ms) = place(tx, conv, "=A1-M1", 10.0, m1p)?;
        let (_, ks) = place(tx, conv, "=A1-K2", 30.0, c1)?;
        tx.connect(Endpoint::pin(ms, 1), Endpoint::pin(ks, 0))?;
        let _ = qs;
        define_option(tx, OptionDef::yes_no("conveyor", "Conveyor", false))?;
        define_option(
            tx,
            OptionDef::choice("supply", "Supply", &["400V", "480V"], "400V"),
        )?;
        set_condition(tx, conv, Some("opt.conveyor"))?;
        set_condition(tx, k9, Some("opt.supply == '480V'"))?;
        Ok(())
    })
    .unwrap();
    doc
}

fn assign(doc: &Document, pairs: &[(&str, Value)]) -> Assignment {
    let given: BTreeMap<String, Value> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    complete(doc.project(), &given).unwrap()
}

#[test]
fn defaults_and_current_values() {
    let mut doc = machine();
    let cur = current(doc.project());
    assert_eq!(cur["conveyor"], Value::Bool(false));
    assert_eq!(cur["supply"], Value::Text("400V".into()));
    doc.transact("Set", |tx| set_option(tx, "conveyor", Value::Bool(true)))
        .unwrap();
    assert_eq!(current(doc.project())["conveyor"], Value::Bool(true));
    doc.undo().unwrap();
    assert_eq!(current(doc.project())["conveyor"], Value::Bool(false));
}

#[test]
fn values_are_checked() {
    let mut doc = machine();
    assert!(
        doc.transact("Set", |tx| set_option(
            tx,
            "supply",
            Value::Text("230V".into())
        ))
        .is_err()
    );
    assert!(
        doc.transact("Set", |tx| set_option(tx, "speed", Value::Int(1)))
            .is_err()
    );
    // Case does not matter for a choice, and the stored value is the listed one.
    doc.transact("Set", |tx| {
        set_option(tx, "supply", Value::Text("480v".into()))
    })
    .unwrap();
    assert_eq!(current(doc.project())["supply"], Value::Text("480V".into()));
}

#[test]
fn conveyor_option_adds_a_page_devices_and_bom() {
    let doc = machine();
    let off = assign(&doc, &[]);
    let on = assign(&doc, &[("conveyor", Value::Bool(true))]);
    let cfg = configuration(doc.project(), &off);
    assert_eq!(cfg.pages.iter().filter(|p| p.active).count(), 1);
    assert_eq!(cfg.devices, vec!["=A1-K1".to_string()]);
    assert_eq!(cfg.devices_off.len(), 3, "{:?}", cfg.devices_off);

    let d = diff(doc.project(), &off, &on);
    assert_eq!(d.pages_added, vec!["2 Conveyor".to_string()]);
    assert!(d.pages_removed.is_empty());
    assert_eq!(
        d.devices_added,
        vec!["=A1-K2".to_string(), "=A1-M1".to_string()]
    );
    // The conveyor adds one motor and one contactor: BOM lines, not guesses.
    let changes: Vec<(String, u32, u32)> = d
        .bom_changes
        .iter()
        .map(|((_, o), a, b)| (o.clone(), *a, *b))
        .collect();
    assert_eq!(changes, vec![("C1".into(), 1, 2), ("M1".into(), 0, 1)]);
    assert_eq!(d.options_changed.len(), 1);
}

#[test]
fn a_choice_option_switches_a_single_device() {
    let doc = machine();
    let us = assign(&doc, &[("supply", Value::Text("480V".into()))]);
    let cfg = configuration(doc.project(), &us);
    assert!(cfg.devices.contains(&"=A1-K9".to_string()));
    let d = diff(doc.project(), &assign(&doc, &[]), &us);
    assert_eq!(d.devices_added, vec!["=A1-K9".to_string()]);
    assert!(d.pages_added.is_empty());
}

#[test]
fn variant_project_drops_what_is_off_and_leaves_the_original() {
    let doc = machine();
    let (pages, devices) = (
        doc.project().pages().count(),
        doc.project().devices().count(),
    );
    let v = variant_project(doc.project(), &assign(&doc, &[])).unwrap();
    assert_eq!(v.pages().count(), 1);
    assert_eq!(v.devices().count(), 1);
    assert_eq!(
        v.wires().count(),
        0,
        "the conveyor's wire went with its page"
    );
    assert_eq!(doc.project().pages().count(), pages);
    assert_eq!(doc.project().devices().count(), devices);
    let full = variant_project(
        doc.project(),
        &assign(
            &doc,
            &[
                ("conveyor", Value::Bool(true)),
                ("supply", Value::Text("480V".into())),
            ],
        ),
    )
    .unwrap();
    assert_eq!(full.pages().count(), 2);
    assert_eq!(full.devices().count(), 4);
}

#[test]
fn named_variants_save_and_apply() {
    let mut doc = machine();
    doc.transact("Variants", |tx| {
        set_option(tx, "conveyor", Value::Bool(true))?;
        save_variant(tx, "With conveyor", "Standard plus belt")?;
        set_option(tx, "conveyor", Value::Bool(false))?;
        save_variant(tx, "Bare", "")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(variants(doc.project()).len(), 2);
    doc.transact("Apply", |tx| apply_variant(tx, "with CONVEYOR"))
        .unwrap();
    assert_eq!(current(doc.project())["conveyor"], Value::Bool(true));
    assert!(
        doc.transact("Apply", |tx| apply_variant(tx, "nope"))
            .is_err()
    );
    doc.transact("Delete", |tx| delete_variant(tx, "Bare"))
        .unwrap();
    assert_eq!(variants(doc.project()).len(), 1);
}

#[test]
fn removing_an_option_cleans_variants() {
    let mut doc = machine();
    doc.transact("v", |tx| save_variant(tx, "A", "")).unwrap();
    doc.transact("rm", |tx| remove_option(tx, "supply"))
        .unwrap();
    assert!(!variants(doc.project())[0].values.contains_key("supply"));
    assert!(!current(doc.project()).contains_key("supply"));
}

#[test]
fn lint_finds_typos_and_unused_options() {
    let mut doc = machine();
    assert!(lint(doc.project()).is_empty());
    doc.transact("Add", |tx| {
        define_option(tx, OptionDef::yes_no("spare", "Spare", false))?;
        let k = tx.project().devices().next().unwrap().id;
        // Parses, but names an option that does not exist.
        tx.set_property(k, CONDITION_KEY, Some(Value::Text("opt.convayor".into())))
    })
    .unwrap();
    let f = lint(doc.project());
    assert!(
        f.iter()
            .any(|x| matches!(x, Finding::UnusedOption(o) if o == "spare"))
    );
    assert!(f.iter().any(|x| matches!(x, Finding::BadCondition { .. })));
}

#[test]
fn bad_condition_syntax_is_refused_when_set() {
    let mut doc = machine();
    let page = doc.project().pages().next().unwrap().id;
    assert!(
        doc.transact("Cond", |tx| set_condition(tx, page, Some("opt.a &&")))
            .is_err()
    );
}

#[test]
fn undecidable_condition_keeps_the_object_and_says_so() {
    let mut doc = machine();
    let page = doc.project().pages().next().unwrap().id;
    doc.transact("Cond", |tx| {
        tx.set_property(page, CONDITION_KEY, Some(Value::Text("opt.ghost".into())))
    })
    .unwrap();
    let e = evaluate(doc.project(), &assign(&doc, &[]));
    assert!(e.is_active(page));
    assert_eq!(e.errors.len(), 1);
}

#[test]
fn combinations_cover_bool_and_choice_options() {
    let doc = machine();
    let all = combinations(doc.project(), 64);
    assert_eq!(all.len(), 4); // 2 x 2
    let none = combinations(doc.project(), 3);
    assert_eq!(none.len(), 3);
}

#[test]
fn tying_a_wire_and_a_device_to_an_option_follows_the_switch() {
    let mut doc = machine();
    let wire = doc.project().wires().next().unwrap().id;
    let k1 = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string() == "=A1-K1")
        .unwrap()
        .id;
    let defs = options(doc.project());
    let conveyor = defs.iter().find(|d| d.id == "conveyor").unwrap().clone();
    let supply = defs.iter().find(|d| d.id == "supply").unwrap().clone();
    doc.transact("Tie", |tx| {
        tie_to(tx, &[wire.into()], &conveyor, &Value::Bool(true))?;
        tie_to(tx, &[k1.into()], &supply, &Value::Text("480V".into()))
    })
    .unwrap();
    assert_eq!(
        describe_condition(doc.project(), wire).as_deref(),
        Some("Conveyor")
    );
    assert_eq!(
        describe_condition(doc.project(), k1).as_deref(),
        Some("Supply 480V")
    );
    // Off: both go. The conveyor page is off by its own condition, so the
    // wire check uses the full conveyor.
    let mut on = assign(&doc, &[("conveyor", Value::Bool(true))]);
    let v = variant_project(doc.project(), &on).unwrap();
    assert_eq!(v.wires().count(), 1);
    assert!(v.devices().all(|d| d.tag.to_string() != "=A1-K1"));
    on.insert("supply".into(), Value::Text("480V".into()));
    let v = variant_project(doc.project(), &on).unwrap();
    assert!(v.devices().any(|d| d.tag.to_string() == "=A1-K1"));
}

#[test]
fn a_wire_alone_can_be_tied() {
    let mut doc = machine();
    let main = doc.project().pages().next().unwrap().id;
    let (a, b) = {
        let mut s = doc.project().symbols().filter(|s| s.page == main);
        (s.next().unwrap().id, s.next().unwrap().id)
    };
    let wire = doc
        .transact("Wire", |tx| {
            tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 0))
        })
        .unwrap()
        .0;
    let fan = OptionDef::yes_no("fan", "Fan", false);
    doc.transact("Define", |tx| define_option(tx, fan.clone()))
        .unwrap();
    doc.transact("Tie", |tx| {
        tie_to(tx, &[wire.into()], &fan, &Value::Bool(true))
    })
    .unwrap();
    let before = doc.project().wires().count();
    let off = variant_project(doc.project(), &assign(&doc, &[])).unwrap();
    assert_eq!(before, 2, "the conveyor's wire and this one");
    assert_eq!(off.wires().count(), 0);
    let with_fan = variant_project(
        doc.project(),
        &assign(
            &doc,
            &[
                ("fan", Value::Bool(true)),
                ("supply", Value::Text("480V".into())),
            ],
        ),
    )
    .unwrap();
    assert_eq!(
        with_fan.wires().count(),
        1,
        "the conveyor page is still off"
    );
    assert_eq!(
        off.devices().count(),
        1,
        "the conveyor page is off, the wire alone is tied"
    );
}

#[test]
fn the_summary_and_the_machine_name_say_the_choice() {
    let doc = machine();
    let a = assign(&doc, &[("conveyor", Value::Bool(true))]);
    assert_eq!(summary(doc.project(), &a), "Conveyor, Supply 400V");
    let m = machine_project(doc.project(), &a).unwrap();
    assert_eq!(
        m.info().name,
        format!("{} [Conveyor, Supply 400V]", doc.project().info().name)
    );
    assert_eq!(m.pages().count(), 2);
}

#[test]
fn a_condition_that_is_not_a_tie_is_shown_as_typed() {
    let mut doc = machine();
    let page = doc.project().pages().next().unwrap().id;
    doc.transact("Cond", |tx| {
        set_condition(tx, page, Some("opt.conveyor && opt.supply == '480V'"))
    })
    .unwrap();
    assert_eq!(
        describe_condition(doc.project(), page).as_deref(),
        Some("opt.conveyor && opt.supply == '480V'")
    );
    assert_eq!(
        describe_condition(doc.project(), doc.project().devices().next().unwrap().id),
        None
    );
}
