//! The explicit project rule for terminals that share a designation.

use novo_core::*;

#[test]
fn same_designation_terminals_join_only_when_the_rule_says_so() {
    let mut doc = Document::default();
    let ((a, b, a_level2, two), _) = doc
        .transact("Strip -X2", |tx| {
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
        .unwrap();
    let project = doc.project();
    // The rule's effect is inspectable before turning it on: one group.
    let groups = project.terminal_designation_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 2);
    assert_eq!(
        project.info().rules.same_designation_terminals,
        SameDesignation::Separate
    );
    let joined = |doc: &Document, x, y| {
        doc.project()
            .netlist()
            .connected(Endpoint::pin(x, 0), Endpoint::pin(y, 0))
    };
    assert!(!joined(&doc, a, b));

    // Turning the rule on joins exactly that group, as one undo step.
    doc.transact("Join same designations", |tx| {
        tx.set_info(|info| info.rules.same_designation_terminals = SameDesignation::Join)
    })
    .unwrap();
    assert!(joined(&doc, a, b));
    assert!(!joined(&doc, a, a_level2));
    assert!(!joined(&doc, a, two));

    // The rule is saved, and only when set.
    let json = doc.project().to_json();
    assert!(json.contains("\"same_designation_terminals\": \"join\""));
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
