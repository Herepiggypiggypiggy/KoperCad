use novo_core::Value;
use novo_parts::{
    MacroLibrary, OnConflict, ParamDraft, ParamKind, param_drafts, starter_macros,
    validate_parameters,
};

fn draft(name: &str, kind: ParamKind, default: &str) -> ParamDraft {
    ParamDraft {
        name: name.into(),
        kind,
        default: default.into(),
        description: String::new(),
    }
}

#[test]
fn each_type_parses_its_default() {
    let p = |k, d: &str| draft("x", k, d).to_parameter().map(|p| p.default);
    assert_eq!(p(ParamKind::Number, "4,5"), Ok(Value::Float(4.5)));
    assert_eq!(p(ParamKind::Integer, " 3 "), Ok(Value::Int(3)));
    assert_eq!(p(ParamKind::Switch, "Yes"), Ok(Value::Bool(true)));
    assert_eq!(p(ParamKind::Switch, ""), Ok(Value::Bool(false)));
    assert_eq!(
        p(ParamKind::Text, " 24 V "),
        Ok(Value::Text(" 24 V ".into()))
    );
    assert!(p(ParamKind::Number, "abc").is_err());
    assert!(p(ParamKind::Integer, "1.5").is_err());
    assert!(p(ParamKind::Switch, "maybe").is_err());
}

#[test]
fn names_must_be_usable_in_braces_and_unique() {
    assert!(draft("", ParamKind::Text, "").to_parameter().is_err());
    assert!(draft("a b", ParamKind::Text, "").to_parameter().is_err());
    assert!(draft("{a}", ParamKind::Text, "").to_parameter().is_err());
    let twice = [
        draft("p", ParamKind::Text, ""),
        draft("p", ParamKind::Number, "1"),
    ];
    assert!(validate_parameters(&twice).unwrap_err().contains("twice"));
    let ok = validate_parameters(&[draft("power", ParamKind::Number, "4")]).unwrap();
    assert_eq!(ok[0].name, "power");
}

#[test]
fn changing_the_type_converts_or_resets_the_default() {
    let mut d = draft("x", ParamKind::Text, "7");
    d.set_kind(ParamKind::Integer);
    assert_eq!(d.default, "7");
    d.set_kind(ParamKind::Number);
    d.set_kind(ParamKind::Switch);
    assert_eq!(d.default, "no");
    let mut t = draft("x", ParamKind::Text, "abc");
    t.set_kind(ParamKind::Number);
    assert_eq!(t.default, "0");
}

#[test]
fn a_library_macro_gets_its_parameters_edited_and_the_revision_grows() {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    let (_, file) = starter_macros().into_iter().next().unwrap();
    let name = file.name.clone();
    let path = lib.save("", &file).unwrap();
    let before = lib.load(&path).unwrap();

    let mut drafts = param_drafts(&before.definition.parameters);
    drafts.push(draft("voltage", ParamKind::Integer, "24"));
    lib.set_parameters(&path, validate_parameters(&drafts).unwrap())
        .unwrap();

    let after = lib.load(&path).unwrap();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.name, name);
    assert_eq!(
        after.definition.parameters.last().unwrap().default,
        Value::Int(24)
    );
    assert_eq!(after.definition.members, before.definition.members);

    // The edited macro travels in a pack with its parameters.
    let pack = dir.path().join("p.kcmacropack");
    lib.export_pack(&[path.as_str()], &pack).unwrap();
    let other = MacroLibrary::new(dir.path().join("Other"), "user");
    other.import(&pack, "", OnConflict::Skip).unwrap();
    let got = other.search("");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].parameters, after.definition.parameters.len());
}

#[test]
fn references_without_a_parameter_are_reported() {
    let (_, file) = starter_macros()
        .into_iter()
        .find(|(_, f)| !f.definition.parameters.is_empty())
        .expect("a starter with parameters");
    let name = file.definition.parameters[0].name.clone();
    assert!(
        file.unknown_references(&file.definition.parameters)
            .is_empty()
    );
    // Removing the parameter leaves any {name} in the members dangling.
    let rest: Vec<_> = file.definition.parameters[1..].to_vec();
    let json = serde_json::to_string(&file.definition.members).unwrap();
    assert_eq!(
        file.unknown_references(&rest).contains(&name),
        json.contains(&format!("{{{name}}}"))
    );
}
