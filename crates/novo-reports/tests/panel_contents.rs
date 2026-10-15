//! The contents list of one panel: only that panel's pages, kept current
//! with edits and renames.

use novo_core::*;
use novo_reports::{LiveReports, PanelContents, Report};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn add(doc: &mut Document, name: &str, title: &str, structure: &str, generator: Option<&str>) {
    doc.execute(Command::AddPage {
        name: name.into(),
        title: title.into(),
        kind: generator.map_or(PageKind::Schematic, |g| PageKind::Generated {
            generator: g.into(),
        }),
        format: Default::default(),
        structure: tag(structure),
    })
    .unwrap();
}

fn pages_of(reports: &mut LiveReports, doc: &Document, id: &str) -> Vec<String> {
    let out = reports.get(doc.project(), id).expect("report");
    out.sections[0]
        .text_rows()
        .into_iter()
        .map(|r| format!("{} {}", r[0], r[1]))
        .collect()
}

fn project() -> Document {
    let mut doc = Document::default();
    add(&mut doc, "1", "Supply", "=A+K1", None);
    add(&mut doc, "2", "Motor", "=A+K1", None);
    add(&mut doc, "1", "Other group", "=B+K1", None);
    add(&mut doc, "1", "Elsewhere", "=A+K2", None);
    add(&mut doc, "0", "", "", Some("toc"));
    add(&mut doc, "0", "", "+K1", Some("toc@+K1"));
    doc
}

#[test]
fn the_list_has_the_pages_of_its_panel_under_every_group() {
    let doc = project();
    let mut reports = LiveReports::standard();
    assert_eq!(
        pages_of(&mut reports, &doc, "toc@+K1"),
        ["0 ", "1 Supply", "2 Motor", "1 Other group"]
    );
    // The project list still has them all, and the panels table.
    let all = reports.get(doc.project(), "toc").unwrap();
    assert_eq!(all.sections[0].text_rows().len(), 6);
    assert_eq!(all.sections.len(), 2);
    // The panel list has the pages only.
    assert_eq!(
        reports
            .get(doc.project(), "toc@+K1")
            .unwrap()
            .sections
            .len(),
        1
    );
}

#[test]
fn it_follows_edits_and_renames_of_the_panel() {
    let mut doc = project();
    let mut reports = LiveReports::standard();
    assert_eq!(pages_of(&mut reports, &doc, "toc@+K1").len(), 4);

    let (_, changes) = doc
        .transact("Add page", |tx| {
            let page = tx.add_page("3")?;
            tx.update::<Page>(page, |p| p.structure = tag("=A+K1"))
        })
        .unwrap();
    reports.notify(doc.project(), &changes);
    assert_eq!(pages_of(&mut reports, &doc, "toc@+K1").len(), 5);

    let out = doc
        .execute(Command::RenameLevel {
            aspect: Aspect::Location,
            from: vec!["K1".into()],
            to: vec!["K7".into()],
        })
        .unwrap();
    reports.notify(doc.project(), &out.changes);
    assert_eq!(pages_of(&mut reports, &doc, "toc@+K7").len(), 5);
    // The old id lists nothing now, and the other panel is untouched.
    assert_eq!(pages_of(&mut reports, &doc, "toc@+K1").len(), 0);
    assert_eq!(pages_of(&mut reports, &doc, "toc@+K2"), ["1 Elsewhere"]);
}

#[test]
fn it_is_made_only_for_a_panel_id() {
    let doc = project();
    let mut reports = LiveReports::standard();
    for id in ["toc@", "toc@=A", "toc@+K1-X1", "nope"] {
        assert!(reports.get(doc.project(), id).is_none(), "{id}");
    }
    let list = PanelContents::of_generator("toc@+K1").unwrap();
    assert_eq!(list.id(), "toc@+K1");
    assert_eq!(list.panel(), &tag("+K1"));
    assert!(PanelContents::new(&tag("=A")).is_none());
    // On its own it gives what the live one gives.
    let alone = list.generate(doc.project());
    assert_eq!(
        alone.sections,
        reports.get(doc.project(), "toc@+K1").unwrap().sections
    );
}

#[test]
fn the_project_list_names_a_panel_list_as_one_page() {
    let doc = project();
    let mut reports = LiveReports::standard();
    let rows = reports.get(doc.project(), "toc").unwrap().sections[0].text_rows();
    assert!(
        rows.iter().any(|r| r[4] == "Generated: toc@+K1"),
        "{rows:?}"
    );
}
