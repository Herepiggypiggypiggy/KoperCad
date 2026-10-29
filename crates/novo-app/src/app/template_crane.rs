//! The crane project template (File > New from template > Overhead crane):
//! a starting point instead of a blank project. It carries a crane-shaped
//! structure tree (girder, hoist units, operator station, cabinets, limit
//! switches, collision protection, load cell, motors), a few empty drawing
//! pages filed under it and the cover pages as one generated "Front matter"
//! page that fills itself from the project. Generic overhead-crane
//! knowledge only; ratings are left for the project to enter.

use novo_core::{ChangeSet, DeviceTag, Document, Page, PageKind, Result as ModelResult, Tx, Value};
use novo_reports::{GeneratedPageSpec, add_generated_pages, front_keys};

/// What a project from this template is called until it is renamed.
pub(crate) const CRANE_TEMPLATE_NAME: &str = "New crane project";

const DESCRIPTION: &str =
    "Overhead crane: girder, hoist units, operator station and field devices.";

/// One node of the structure tree: tag, what it is, and whether it is a
/// cabinet or box (the internal side of a terminal strip faces it).
struct Node {
    tag: &'static str,
    description: &'static str,
    enclosure: bool,
}

const fn node(tag: &'static str, description: &'static str, enclosure: bool) -> Node {
    Node {
        tag,
        description,
        enclosure,
    }
}

/// The structure tree, in reading order. `+HU1` and `+HU2` are the same
/// shape: a crane has one or more hoist units, delete the one you do not
/// need.
const TREE: &[Node] = &[
    node("=CRN", "Overhead crane", false),
    node("=CRN+GIR", "Crane girder", false),
    node("=CRN+GIR+DST", "Supply distribution", true),
    node("=CRN+GIR+MCC", "Main control cabinet", true),
    node("=CRN+GIR+AUX", "Additional cabinet, girder", true),
    node("=CRN+HU1", "Hoist unit 1", false),
    node("=CRN+HU1+APP", "Apparatus set, hoist unit 1", true),
    node("=CRN+HU1+REL", "Relay cabinet, hoist unit 1", true),
    node("=CRN+HU1+CTL", "Control cabinet, hoist unit 1", true),
    node("=CRN+HU2", "Hoist unit 2", false),
    node("=CRN+HU2+APP", "Apparatus set, hoist unit 2", true),
    node("=CRN+HU2+REL", "Relay cabinet, hoist unit 2", true),
    node("=CRN+HU2+CTL", "Control cabinet, hoist unit 2", true),
    node("=CRN+OPS", "Operator station", false),
    node("=CRN+OPS+PLG", "Operator plug", true),
    node("=CRN+OPS+OP1", "Operator 1 (pendant or radio)", false),
    node("=CRN+OPS+OP2", "Operator 2 (pendant or radio)", false),
    node("=CRN+FLD", "Field devices", false),
    node("=CRN+FLD+LSB", "Limit switches, bridge travel", false),
    node("=CRN+FLD+CPB", "Collision protection, bridge", false),
    node("=CRN+FLD+LSC", "Limit switches, crab travel", false),
    node("=CRN+FLD+CPC", "Collision protection, crab", false),
    node("=CRN+FLD+LSH", "Limit switches, hoist", false),
    node(
        "=CRN+FLD+LCH",
        "Load cell, hoist (safe working load)",
        false,
    ),
    node("=CRN+FLD+SIG", "Signal column or beacon", false),
    node("=CRN+FLD+MB", "Bridge travel motors 1 to 4", false),
    node("=CRN+FLD+MC", "Crab travel motors 1 and 2", false),
    node("=CRN+FLD+MH", "Hoist motors (fine and coarse speed)", false),
    node(
        "=CRN+FLD+COM",
        "Communication module (radio or fieldbus)",
        false,
    ),
];

/// The drawing pages the project starts with: name, title and the cabinet
/// they are filed under. All empty; the structure and the numbering are the
/// template.
const PAGES: &[(&str, &str, &str)] = &[
    ("1", "Incoming supply and distribution", "=CRN+GIR+DST"),
    (
        "2",
        "Main control: protection and control supply",
        "=CRN+GIR+MCC",
    ),
    ("3", "Bridge travel", "=CRN+GIR+MCC"),
    ("4", "Crab travel", "=CRN+GIR+MCC"),
    ("5", "Hoist unit 1: power", "=CRN+HU1+CTL"),
    ("6", "Hoist unit 1: control", "=CRN+HU1+CTL"),
];

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("the template tags are valid")
}

/// Builds the template into a project that has only its empty first page.
/// `today` is the issue date the cover pages start with.
pub(crate) fn build_crane_template_on(doc: &mut Document, today: &str) -> ModelResult<ChangeSet> {
    let ((), changes) = doc.transact("New from template", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        for n in TREE {
            tx.add_structure(tag(n.tag), n.description, n.enclosure)?;
        }
        for (name, title, structure) in PAGES {
            let page = tx.add_page(*name)?;
            tx.update::<Page>(page, |p| {
                p.title = (*title).into();
                p.structure = tag(structure);
            })?;
        }
        // The cover pages open the set (page 00, ahead of the drawing).
        add_generated_pages(
            tx,
            &[GeneratedPageSpec {
                generator: novo_core::FRONT_GENERATOR.into(),
                name: "00".into(),
                title: "Front matter".into(),
            }],
        )?;
        let front = tx
            .project()
            .pages()
            .find(|p| matches!(&p.kind, PageKind::Generated { generator } if generator == novo_core::FRONT_GENERATOR))
            .map(|p| p.id);
        if let Some(front) = front {
            tx.update::<Page>(front, |p| {
                p.properties
                    .insert(novo_core::PAGE_ORDER.into(), Value::Int(0));
            })?;
        }
        tx.set_info(|info| {
            info.name = CRANE_TEMPLATE_NAME.into();
            info.description = DESCRIPTION.into();
            info.rules = novo_core::ProjectRules::recommended();
            let text = |v: &str| Value::Text(v.into());
            info.properties
                .insert(front_keys::ISSUE_DATE.into(), text(today));
            info.properties
                .insert(front_keys::CONTROL.into(), text("48 V AC"));
            info.properties.insert(
                novo_checks::calc_view::inputs::SUPPLY_VOLTAGE_V.into(),
                400.0.into(),
            );
        })
    })?;
    Ok(changes)
}

/// [`build_crane_template_on`] dated today.
pub(crate) fn build_crane_template(doc: &mut Document) -> ModelResult<ChangeSet> {
    build_crane_template_on(doc, &super::revisions::today())
}
