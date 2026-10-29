//! The crane project template (File > New from template > Overhead crane):
//! a starting point instead of a blank project. It carries a crane-shaped
//! structure tree (girder, hoist units, operator station, cabinets, limit
//! switches, collision protection, load cell, motors), a few empty drawing
//! pages filed under it and the cover pages as one generated "Front matter"
//! page that fills itself from the project. The pages carry circuits from the
//! standard circuit library (docs/CIRCUIT-LIBRARY.md), chosen by the options
//! of the New crane project window: the number of hoist units and the drive
//! of the hoists and of the travel motions. Generic overhead-crane knowledge
//! only; ratings are left for the project to enter.

use novo_core::{
    ChangeSet, DeviceTag, Document, ModelError, Page, PageKind, Point, Result as ModelResult, Tx,
    Value,
};
use novo_parts::circuit_library;
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

/// How a motion is driven; picks the circuit its page carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Drive {
    /// Pole-change motor on contactors, with a brake.
    #[default]
    TwoSpeed,
    /// Frequency drive, with a brake.
    Frequency,
}

/// What the New crane project window asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CraneOptions {
    /// Hoist units, 1 to [`MAX_HOIST_UNITS`].
    pub hoist_units: u8,
    pub hoist_drive: Drive,
    pub travel_drive: Drive,
}

pub(crate) const MAX_HOIST_UNITS: u8 = 4;

impl Default for CraneOptions {
    fn default() -> Self {
        CraneOptions {
            hoist_units: 1,
            hoist_drive: Drive::TwoSpeed,
            travel_drive: Drive::Frequency,
        }
    }
}

impl CraneOptions {
    fn hoist_circuit(self) -> &'static str {
        match self.hoist_drive {
            Drive::TwoSpeed => "Hoist two-speed with rectifier brake",
            Drive::Frequency => "Hoist frequency controlled with brake and braking resistor",
        }
    }

    fn travel_circuit(self) -> &'static str {
        match self.travel_drive {
            Drive::TwoSpeed => "Travel two-speed with brake motor",
            Drive::Frequency => "Travel drive frequency controlled with brake",
        }
    }
}

const SUPPLY_CIRCUIT: &str = "Control transformer 400 V to 48 V";
const ESTOP_CIRCUIT: &str = "Main contactor with emergency stop chain";

/// The structure nodes of hoist unit `n`.
fn hoist_nodes(n: u8) -> Vec<(String, String, bool)> {
    let t = |s: &str| format!("=CRN+HU{n}{s}");
    vec![
        (t(""), format!("Hoist unit {n}"), false),
        (t("+APP"), format!("Apparatus set, hoist unit {n}"), true),
        (t("+REL"), format!("Relay cabinet, hoist unit {n}"), true),
        (t("+CTL"), format!("Control cabinet, hoist unit {n}"), true),
    ]
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("the template tags are valid")
}

/// Builds the template into a project that has only its empty first page.
/// `today` is the issue date the cover pages start with.
pub(crate) fn build_crane_template_on(
    doc: &mut Document,
    today: &str,
    options: CraneOptions,
) -> ModelResult<ChangeSet> {
    let units = options.hoist_units.clamp(1, MAX_HOIST_UNITS);
    let ((), changes) = doc.transact("New from template", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        for n in TREE {
            tx.add_structure(tag(n.tag), n.description, n.enclosure)?;
        }
        for unit in 1..=units {
            for (t, description, enclosure) in hoist_nodes(unit) {
                tx.add_structure(tag(&t), description, enclosure)?;
            }
        }
        let mut circuits = Circuits::default();
        let mut add = |tx: &mut Tx<'_>, name: &str, title: &str, structure: &str, circuit: &str| {
            let page = tx.add_page(name)?;
            tx.update::<Page>(page, |p| {
                p.title = title.into();
                p.structure = tag(structure);
            })?;
            circuits.place(tx, page, circuit)
        };
        add(
            tx,
            "1",
            "Incoming supply and control transformer",
            "=CRN+GIR+DST",
            SUPPLY_CIRCUIT,
        )?;
        add(
            tx,
            "2",
            "Main contactor and emergency stop",
            "=CRN+GIR+MCC",
            ESTOP_CIRCUIT,
        )?;
        add(
            tx,
            "3",
            "Bridge travel",
            "=CRN+GIR+MCC",
            options.travel_circuit(),
        )?;
        add(
            tx,
            "4",
            "Crab travel",
            "=CRN+GIR+MCC",
            options.travel_circuit(),
        )?;
        for unit in 1..=units {
            add(
                tx,
                &(4 + u32::from(unit)).to_string(),
                &format!("Hoist unit {unit}"),
                &format!("=CRN+HU{unit}+CTL"),
                options.hoist_circuit(),
            )?;
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
                .insert(front_keys::KIND.into(), text("crane"));
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

/// The circuits of the standard library by name, each adopted into the
/// project once and then placed on as many pages as needed.
#[derive(Default)]
pub(super) struct Circuits {
    adopted: Vec<(String, novo_core::BlockDefinitionId)>,
    /// The library, built on the first circuit asked for.
    library: Option<Vec<(String, novo_parts::MacroFile)>>,
}

impl Circuits {
    /// Places circuit `name` at the top left of `page`.
    pub(super) fn place(
        &mut self,
        tx: &mut Tx<'_>,
        page: novo_core::PageId,
        name: &str,
    ) -> ModelResult<()> {
        self.place_at(tx, page, name, Point::new(20.0, 20.0))
    }

    /// Places circuit `name` with its origin at `at` on `page`.
    pub(super) fn place_at(
        &mut self,
        tx: &mut Tx<'_>,
        page: novo_core::PageId,
        name: &str,
        at: Point,
    ) -> ModelResult<()> {
        let id = match self.adopted.iter().find(|(n, _)| n == name) {
            Some((_, id)) => *id,
            None => {
                if self.library.is_none() {
                    self.library = Some(circuit_library().map_err(|e| ModelError::Refused {
                        what: "Circuit library".into(),
                        reason: e.to_string(),
                    })?);
                }
                let file = self
                    .library
                    .iter()
                    .flatten()
                    .find(|(_, f)| f.name == name)
                    .map(|(_, f)| f.clone())
                    .ok_or_else(|| ModelError::Refused {
                        what: "Circuit library".into(),
                        reason: format!("no circuit named \"{name}\""),
                    })?;
                let id = tx.adopt_block_definition(file.definition)?;
                self.adopted.push((name.to_owned(), id));
                id
            }
        };
        tx.place_block(id, page, at, Vec::new())?;
        Ok(())
    }
}
