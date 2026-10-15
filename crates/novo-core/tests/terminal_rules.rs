//! The explicit project rule for terminals that share a name.

use novo_core::*;

type Placed = (SymbolId, SymbolId, SymbolId, SymbolId);

/// Strip `-X2` with two `1` terminals on pages 1 and 2, a level-2 `1` and a `2`.
fn strip(doc: &mut Document) -> Placed {
    doc.transact("Strip -X2", |tx| {
        let p1 = tx.add_page("1")?;
        let p2 = tx.add_page("2")?;
        let x2 = tx.add_terminal_strip(DeviceTag::parse("-X2")?)?;
        let first = tx.add_terminal(x2, "1")?;
        let second = tx.add_terminal(x2, "1")?;
        let upper_level = tx.add_terminal(x2, "1")?;
        tx.update::<Terminal>(upper_level, |t| t.level = 2)?;
        let other = tx.add_terminal(x2, "2")?;
        let mut place = |page, terminal, x: f64| {
            tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(x, 0.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                SymbolRole::Terminal {
                    terminals: vec![terminal],
                },
            )
        };
        Ok((
            place(p1, first, 10.0)?,
            place(p2, second, 10.0)?,
            place(p2, upper_level, 30.0)?,
            place(p2, other, 50.0)?,
        ))
    })
    .unwrap()
    .0
}

fn joined(doc: &Document, x: SymbolId, y: SymbolId) -> bool {
    doc.project()
        .netlist()
        .connected(Endpoint::pin(x, 0), Endpoint::pin(y, 0))
}

#[test]
fn new_projects_join_and_files_without_the_rule_load_off() {
    assert_eq!(Project::new("A").info().rules, ProjectRules::recommended());
    assert_eq!(
        Project::new("A").info().rules.terminals,
        TerminalRules {
            same_name: SameName::Join,
            join_link: JoinLink::Connector,
        }
    );
    assert_eq!(
        Project::default().info().rules.terminals.same_name,
        SameName::Off
    );

    let json = Project::new("A").to_json();
    assert!(json.contains("\"same_name\": \"join\""));
    assert!(json.contains("\"join_link\": \"connector\""));
    let without_rules = json.replace(
        &json[json.find("\"rules\"").unwrap()..json.find("\"properties\"").unwrap()],
        "",
    );
    let loaded = Project::from_json(&without_rules).unwrap();
    assert_eq!(loaded.info().rules.terminals.same_name, SameName::Off);
}

#[test]
fn same_name_terminals_join_only_when_the_rule_says_so() {
    let mut doc = Document::new(Project::default());
    let (a, b, a_level2, two) = strip(&mut doc);
    let project = doc.project();
    // The rule's effect is inspectable before turning it on: one group.
    let groups = project.terminal_designation_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 2);
    assert!(!joined(&doc, a, b));

    // Turning the rule on joins exactly that group, as one undo step.
    doc.transact("Join same names", |tx| {
        tx.set_info(|info| info.rules.terminals.same_name = SameName::Join)
    })
    .unwrap();
    assert!(joined(&doc, a, b));
    assert!(!joined(&doc, a, a_level2));
    assert!(!joined(&doc, a, two));

    // The rule is saved, and only when set.
    let json = doc.project().to_json();
    assert!(json.contains("\"same_name\": \"join\""));
    let loaded = Project::from_json(&json).unwrap();
    assert!(
        loaded
            .netlist()
            .connected(Endpoint::pin(a, 0), Endpoint::pin(b, 0))
    );
    doc.undo().unwrap();
    assert!(!joined(&doc, a, b));
    assert!(!doc.project().to_json().contains("rules"));
}

#[test]
fn refuse_rejects_duplicate_names() {
    let mut doc = Document::new(Project::default());
    let _ = strip(&mut doc);
    // Existing duplicates keep the rule from being switched to refuse.
    let err = doc
        .transact("Refuse", |tx| {
            tx.set_info(|info| info.rules.terminals.same_name = SameName::Refuse)
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateTerminal(_)));

    let mut doc = Document::new(Project::default());
    doc.transact("Refuse", |tx| {
        tx.set_info(|info| info.rules.terminals.same_name = SameName::Refuse)
    })
    .unwrap();
    let ((x2, upper), _) = doc
        .transact("Strip", |tx| {
            let x2 = tx.add_terminal_strip(DeviceTag::parse("-X2")?)?;
            tx.add_terminal(x2, "1")?;
            let upper = tx.add_terminal(x2, "1.2")?;
            tx.update::<Terminal>(upper, |t| {
                t.name = "1".into();
                t.level = 2;
            })?;
            Ok((x2, upper))
        })
        .unwrap();
    // Another level may share the name; the same level may not, on insert or rename.
    let err = doc
        .transact("Duplicate", |tx| tx.add_terminal(x2, "1"))
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateTerminal(_)));
    let err = doc
        .transact("Move level", |tx| {
            tx.update::<Terminal>(upper, |t| t.level = 1)
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateTerminal(id) if id == upper));
    assert!(doc.project().terminal_designation_groups().is_empty());
}

#[test]
fn a_hand_toggle_of_the_pe_choice_is_stored_per_terminal_in_one_step() {
    let mut doc = Document::default();
    let ((a, b), _) = doc
        .transact("Strip", |tx| {
            let strip =
                tx.add_device_of_kind("-X1".parse().unwrap(), "", DeviceKind::TerminalStrip)?;
            Ok((tx.add_terminal(strip, "1")?, tx.add_terminal(strip, "2")?))
        })
        .unwrap();
    assert_eq!(
        doc.project().terminal(a).unwrap().pe_override,
        PeOverride::Auto
    );
    let depth = doc.undo_depth();

    doc.execute(Command::SetTerminalPe {
        terminals: vec![a, b],
        mode: PeOverride::ForcedPe,
    })
    .unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    for t in [a, b] {
        assert_eq!(
            doc.project().terminal(t).unwrap().pe_override,
            PeOverride::ForcedPe
        );
    }

    // The choice is a file field only while it is set.
    let json = serde_json::to_value(doc.project().terminal(a).unwrap()).unwrap();
    assert_eq!(json["pe_override"], "forced_pe");
    doc.execute(Command::SetTerminalPe {
        terminals: vec![a],
        mode: PeOverride::Auto,
    })
    .unwrap();
    let json = serde_json::to_value(doc.project().terminal(a).unwrap()).unwrap();
    assert!(json.get("pe_override").is_none());
    assert_eq!(
        doc.project().terminal(b).unwrap().pe_override,
        PeOverride::ForcedPe
    );

    doc.undo().unwrap();
    assert_eq!(
        doc.project().terminal(a).unwrap().pe_override,
        PeOverride::ForcedPe
    );

    let missing = doc.execute(Command::SetTerminalPe {
        terminals: vec![a, TerminalId(9999)],
        mode: PeOverride::ForcedNotPe,
    });
    assert!(missing.is_err());
    assert_eq!(
        doc.project().terminal(a).unwrap().pe_override,
        PeOverride::ForcedPe
    );
}
