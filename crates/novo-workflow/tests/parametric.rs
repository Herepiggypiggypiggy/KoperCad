//! Parametric macros: typed and calculated parameters, optional parts and
//! repeats, placed through the same path as any macro.

use novo_core::*;
use novo_workflow::expr::{Val, Vars};
use novo_workflow::parametric::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

/// A starter drawn as breaker -Q1, overload -F1 and motor -M1 in a row,
/// wired Q1-F1-M1, captured as the block `starter`, with parameters.
fn starter() -> BlockDefinition {
    let mut doc = Document::default();
    let (def, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.update::<Page>(page, |p| p.structure = tag("=A1"))?;
            let q = tx.add_device(tag("=A1-Q1"), "Breaker {rating} A")?;
            let f = tx.add_device(tag("=A1-F1"), "Overload")?;
            let m = tx.add_device(tag("=A1-M1"), "Motor {power} kW")?;
            let mut sym = |x: f64, d: DeviceId| {
                tx.place_symbol(
                    page,
                    "x",
                    Placement::at(Point::new(x, 0.0)),
                    pins(),
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )
            };
            let (q1, f1, m1) = (sym(10.0, q)?, sym(30.0, f)?, sym(50.0, m)?);
            tx.connect(Endpoint::pin(q1, 1), Endpoint::pin(f1, 0))?;
            tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(m1, 0))?;
            let id = tx.define_block(
                "starter",
                &[q1.into(), f1.into(), m1.into()],
                Point::new(10.0, 0.0),
                vec![
                    BlockParameter {
                        name: "power".into(),
                        default: Value::Float(4.0),
                        description: "Motor power".into(),
                    },
                    BlockParameter {
                        name: "rating".into(),
                        default: Value::Int(0),
                        description: String::new(),
                    },
                ],
            )?;
            Ok(tx.project().block_definition(id).unwrap().clone())
        })
        .unwrap();
    def
}

fn parametric_starter() -> BlockDefinition {
    let mut def = starter();
    set_param(
        &mut def,
        "power",
        Value::Float(4.0),
        ParamSpec {
            kind: ParamKind::Number,
            min: Some(0.09),
            max: Some(110.0),
            unit: "kW".into(),
            ..ParamSpec::default()
        },
    );
    set_param(
        &mut def,
        "rating",
        Value::Int(0),
        ParamSpec {
            derive: Some(
                "next_size(power * 1000 / (sqrt(3) * 400 * 0.85) * 1.25, 6, 10, 16, 20, 25, 32, 40, 50, 63)"
                    .into(),
            ),
            ..ParamSpec::default()
        },
    );
    // The overload relay is only wanted when the motor is not tiny.
    let overload = def
        .members
        .iter()
        .find_map(|m| match m {
            Record::Device(d) if d.tag.to_string().contains("F1") => Some(d.id),
            _ => None,
        })
        .unwrap();
    set_when(&mut def, overload.into(), Some("power > 0.75"));
    def
}

fn target() -> (Document, PageId) {
    let mut doc = Document::default();
    let (page, _) = doc
        .transact("Page", |tx| {
            let page = tx.add_page("1")?;
            tx.update::<Page>(page, |p| p.structure = tag("=A1"))?;
            Ok(page)
        })
        .unwrap();
    (doc, page)
}

fn given(pairs: &[(&str, Value)]) -> Vec<(String, Value)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[test]
fn calculated_rating_follows_power() {
    let def = parametric_starter();
    let none = Vars::new();
    let v = resolve(&def, &given(&[("power", Value::Float(7.5))]), &none).unwrap();
    assert!(v.contains(&("rating".to_string(), Value::Int(16))));
    let v = resolve(
        &def,
        &given(&[("power", Value::Text("0,55".into()))]),
        &none,
    )
    .unwrap();
    assert!(v.contains(&("rating".to_string(), Value::Int(6))));
    assert!(v.contains(&("power".to_string(), Value::Float(0.55))));
}

#[test]
fn bad_input_is_named() {
    let def = parametric_starter();
    let none = Vars::new();
    let errs = resolve(&def, &given(&[("power", Value::Float(500.0))]), &none).unwrap_err();
    assert!(matches!(&errs[0], Problem::AboveMax { name, .. } if name == "power"));
    let errs = resolve(
        &def,
        &given(&[("power", Value::Text("lots".into()))]),
        &none,
    )
    .unwrap_err();
    assert!(matches!(&errs[0], Problem::NotANumber { .. }));
    let errs = resolve(&def, &given(&[("rating", Value::Int(10))]), &none).unwrap_err();
    assert_eq!(errs, vec![Problem::Derived("rating".into())]);
    let errs = resolve(&def, &given(&[("speed", Value::Int(1))]), &none).unwrap_err();
    assert_eq!(errs, vec![Problem::UnknownParameter("speed".into())]);
    // Too big for the table: the calculation says so instead of guessing.
    let errs = resolve(&def, &given(&[("power", Value::Float(90.0))]), &none).unwrap_err();
    assert!(matches!(&errs[0], Problem::Expression { name, .. } if name == "rating"));
}

#[test]
fn optional_part_and_what_dangles_with_it() {
    let def = parametric_starter();
    let (mut doc, page) = target();
    let (placed, _) = doc
        .transact("Place", |tx| {
            place_parametric(
                tx,
                &def,
                page,
                Point::new(100.0, 0.0),
                &ParametricOptions {
                    values: given(&[("power", Value::Float(0.37))]),
                    ..Default::default()
                },
            )
        })
        .unwrap();
    let placed = placed.unwrap();
    assert_eq!(placed.len(), 1);
    // The overload is left out; the two wires through it are gone, so only
    // the breaker and the motor are on the page and no wire dangles.
    let tags: Vec<String> = placed[0]
        .report
        .tags
        .iter()
        .map(|t| t.to_string())
        .collect();
    assert_eq!(tags.len(), 2, "{tags:?}");
    assert!(tags.iter().all(|t| !t.contains('F')));
    assert_eq!(doc.project().wires().count(), 0);
    // The filtered definition is named after its shape and remembers the macro.
    let stored = doc
        .project()
        .block_definitions()
        .find(|d| d.name.starts_with("starter~"))
        .expect("filtered definition");
    assert_eq!(base_name(stored), "starter");
}

#[test]
fn full_starter_keeps_the_original_definition() {
    let def = parametric_starter();
    let (mut doc, page) = target();
    let (placed, _) = doc
        .transact("Place", |tx| {
            place_parametric(
                tx,
                &def,
                page,
                Point::new(100.0, 0.0),
                &ParametricOptions {
                    values: given(&[("power", Value::Float(7.5))]),
                    ..Default::default()
                },
            )
        })
        .unwrap();
    let placed = placed.unwrap();
    assert_eq!(placed[0].report.tags.len(), 3);
    assert_eq!(doc.project().wires().count(), 2);
    assert!(doc.project().block_definition_by_name("starter").is_some());
    // The rating was filled into the breaker's description.
    let q = doc
        .project()
        .devices()
        .find(|d| d.description.starts_with("Breaker"))
        .unwrap();
    assert_eq!(q.description, "Breaker 16 A");
}

#[test]
fn repeat_places_copies_with_their_own_tags_in_one_undo_step() {
    let mut def = parametric_starter();
    set_param(
        &mut def,
        "motors",
        Value::Int(1),
        ParamSpec {
            kind: ParamKind::Number,
            min: Some(1.0),
            max: Some(12.0),
            ..ParamSpec::default()
        },
    );
    set_repeat(
        &mut def,
        Some(&RepeatSpec {
            count: "motors".into(),
            pitch: [0.0, 40.0],
        }),
    );
    let (mut doc, page) = target();
    let (placed, _) = doc
        .transact("Place", |tx| {
            place_parametric(
                tx,
                &def,
                page,
                Point::new(100.0, 0.0),
                &ParametricOptions {
                    values: given(&[("motors", Value::Int(3)), ("power", Value::Float(7.5))]),
                    ..Default::default()
                },
            )
        })
        .unwrap();
    assert_eq!(placed.unwrap().len(), 3);
    let mut tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
    tags.sort();
    assert_eq!(tags.len(), 9);
    tags.dedup();
    assert_eq!(tags.len(), 9, "every copy takes fresh tags");
    // Pitch moves each copy down the page.
    let ys: std::collections::BTreeSet<i64> = doc
        .project()
        .symbols()
        .map(|s| s.placement.origin.y as i64)
        .collect();
    assert_eq!(ys.len(), 3);
    doc.undo().unwrap();
    assert_eq!(doc.project().devices().count(), 0, "one undo step");
}

#[test]
fn repeat_count_is_bounded() {
    let mut def = parametric_starter();
    set_repeat(
        &mut def,
        Some(&RepeatSpec {
            count: "1000".into(),
            pitch: [0.0, 1.0],
        }),
    );
    let problems = instantiate(&def, &[], &Vars::new()).unwrap_err();
    assert!(matches!(&problems[0], Problem::Repeat(_)));
}

#[test]
fn conditions_can_read_project_options() {
    let mut def = parametric_starter();
    let overload = def
        .members
        .iter()
        .find_map(|m| match m {
            Record::Device(d) if d.tag.to_string().contains("F1") => Some(d.id),
            _ => None,
        })
        .unwrap();
    set_when(&mut def, overload.into(), Some("opt.thermal_protection"));
    let mut options = Vars::new();
    options.insert("opt.thermal_protection".into(), Val::Bool(false));
    let off = instantiate(&def, &[], &options).unwrap();
    assert!(off.left_out >= 2);
    options.insert("opt.thermal_protection".into(), Val::Bool(true));
    let on = instantiate(&def, &[], &options).unwrap();
    assert_eq!(on.left_out, 0);
    assert_eq!(on.definition.name, "starter");
    // A condition that reads an option the project lacks is an error, not a guess.
    assert!(instantiate(&def, &[], &Vars::new()).is_err());
}

#[test]
fn specs_survive_a_json_round_trip() {
    let def = parametric_starter();
    let json = serde_json::to_string(&def).unwrap();
    let back: BlockDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(params(&back), params(&def));
    assert!(params(&back)[1].is_derived());
}
