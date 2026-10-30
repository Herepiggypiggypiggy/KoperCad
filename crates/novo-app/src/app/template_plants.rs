//! The pump station and motor control centre project templates
//! (File > Templates and examples > Pump station / Motor control centre). Like
//! the crane template they start a project with a plant-shaped structure
//! tree, drawing pages filed under it with a circuit from the standard
//! circuit library (docs/CIRCUIT-LIBRARY.md) on each, and the cover pages as
//! one generated "Front matter" page. The New project window picks the size:
//! pumps or feeders, and whether each pump gets a thermistor relay or each
//! feeder reverses. Generic knowledge only; ratings are left for the project.

use novo_core::{
    ChangeSet, DeviceTag, Document, Page, PageKind, Point, Result as ModelResult, Tx, Value,
};
use novo_reports::{GeneratedPageSpec, add_generated_pages, front_keys};

use super::template_crane::Circuits;

/// Which plant the template builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Plant {
    PumpStation,
    ControlCentre,
}

impl Plant {
    /// What a project from this template is called until it is renamed.
    pub(crate) fn project_name(self) -> &'static str {
        match self {
            Plant::PumpStation => "New pump station project",
            Plant::ControlCentre => "New motor control centre project",
        }
    }

    /// The window title and the heading of the count choice.
    pub(crate) fn window_title(self) -> &'static str {
        self.project_name()
    }

    pub(crate) fn count_label(self) -> &'static str {
        match self {
            Plant::PumpStation => "Pumps",
            Plant::ControlCentre => "Motor feeders",
        }
    }

    pub(crate) fn max_count(self) -> u8 {
        match self {
            Plant::PumpStation => 4,
            Plant::ControlCentre => 8,
        }
    }

    pub(crate) fn extra_label(self) -> &'static str {
        match self {
            Plant::PumpStation => "Thermistor relay on each pump motor",
            Plant::ControlCentre => "Reversing feeders (contactor pair)",
        }
    }

    /// The `project.kind` value the cover pages read.
    fn kind(self) -> &'static str {
        match self {
            Plant::PumpStation => "pump",
            Plant::ControlCentre => "mcc",
        }
    }

    fn root(self) -> &'static str {
        match self {
            Plant::PumpStation => "=PMP",
            Plant::ControlCentre => "=MCC",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Plant::PumpStation => "Pump station: control cabinet, pump motors and field devices.",
            Plant::ControlCentre => {
                "Motor control centre: incoming section, control section and motor feeder sections."
            }
        }
    }
}

/// What the New project window asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlantOptions {
    pub plant: Plant,
    /// Pumps or feeders, 1 to [`Plant::max_count`].
    pub count: u8,
    /// Pump station: thermistor relay per pump. Control centre: reversing
    /// feeders.
    pub extra: bool,
    /// Pump station: each pump on a frequency drive instead of direct on
    /// line. Not used by the control centre.
    pub frequency: bool,
}

impl PlantOptions {
    pub(crate) fn new(plant: Plant) -> Self {
        PlantOptions {
            plant,
            count: match plant {
                Plant::PumpStation => 2,
                Plant::ControlCentre => 4,
            },
            extra: false,
            frequency: false,
        }
    }
}

const CONTROL_SUPPLY: &str = "Control supply 48 V AC and 24 V DC";
const PUMP_FEEDER_FREQUENCY: &str = "Pump feeder frequency controlled";
const PUMP_LEVEL_CONTROL: &str = "Pump level control with dry-run protection";
const FEEDER_DOL: &str = "Motor feeder DOL";
const FEEDER_REVERSING: &str = "Motor feeder reversing";
const THERMISTOR: &str = "Motor thermistor relay";

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("the template tags are valid")
}

/// The structure nodes: (tag, description, enclosure).
fn nodes(options: PlantOptions) -> Vec<(String, String, bool)> {
    let n = |t: &str, d: &str, e: bool| (t.to_owned(), d.to_owned(), e);
    let mut out = Vec::new();
    match options.plant {
        Plant::PumpStation => {
            out.push(n("=PMP", "Pump station", false));
            out.push(n("=PMP+CAB", "Pump control cabinet", true));
            for i in 1..=options.count {
                out.push((format!("=PMP+PM{i}"), format!("Pump {i}"), false));
            }
            out.push(n("=PMP+FLD", "Field devices", false));
            out.push(n("=PMP+FLD+LVL", "Level and float switches", false));
            out.push(n("=PMP+FLD+PRS", "Pressure transmitter", false));
            out.push(n("=PMP+FLD+FLW", "Flow meter", false));
        }
        Plant::ControlCentre => {
            out.push(n("=MCC", "Motor control centre", false));
            out.push(n("=MCC+INC", "Incoming section", true));
            out.push(n("=MCC+CTL", "Control section", true));
            for i in 1..=options.count {
                out.push((format!("=MCC+F{i}"), format!("Feeder section {i}"), true));
            }
            out.push(n("=MCC+FLD", "Motors and field devices", false));
            for i in 1..=options.count {
                out.push((format!("=MCC+FLD+M{i}"), format!("Motor {i}"), false));
            }
        }
    }
    out
}

/// Builds the template into a project that has only its empty first page.
/// `today` is the issue date the cover pages start with.
pub(crate) fn build_plant_template_on(
    doc: &mut Document,
    today: &str,
    options: PlantOptions,
) -> ModelResult<ChangeSet> {
    let options = PlantOptions {
        count: options.count.clamp(1, options.plant.max_count()),
        ..options
    };
    let root = options.plant.root();
    let ((), changes) = doc.transact("New from template", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        for (t, description, enclosure) in nodes(options) {
            tx.add_structure(tag(&t), description, enclosure)?;
        }
        let mut circuits = Circuits::default();
        let page_of = |tx: &mut Tx<'_>, name: u32, title: &str, structure: &str| {
            let page = tx.add_page(name.to_string())?;
            tx.update::<Page>(page, |p| {
                p.title = title.into();
                p.structure = tag(structure);
            })?;
            Ok::<_, novo_core::ModelError>(page)
        };
        let control = match options.plant {
            Plant::PumpStation => format!("{root}+CAB"),
            Plant::ControlCentre => format!("{root}+CTL"),
        };
        let page = page_of(tx, 1, "Control supply", &control)?;
        circuits.place(tx, page, CONTROL_SUPPLY)?;
        // The pumps' run requests come from the level control: one page for
        // the float switches and the duty and standby relays.
        let mut next = 2;
        if options.plant == Plant::PumpStation {
            let page = page_of(tx, next, "Level control", &control)?;
            circuits.place(tx, page, PUMP_LEVEL_CONTROL)?;
            next += 1;
        }
        for i in 1..=options.count {
            let name = next + u32::from(i) - 1;
            match options.plant {
                Plant::PumpStation => {
                    let page = page_of(
                        tx,
                        name,
                        &format!("Pump {i} feeder and motor protection"),
                        &format!("{root}+CAB"),
                    )?;
                    let feeder = if options.frequency {
                        PUMP_FEEDER_FREQUENCY
                    } else {
                        FEEDER_DOL
                    };
                    circuits.place(tx, page, feeder)?;
                    if options.extra {
                        circuits.place_at(tx, page, THERMISTOR, Point::new(200.0, 20.0))?;
                    }
                }
                Plant::ControlCentre => {
                    let page = page_of(
                        tx,
                        name,
                        &format!("Feeder {i}"),
                        &format!("{root}+F{i}"),
                    )?;
                    let circuit = if options.extra {
                        FEEDER_REVERSING
                    } else {
                        FEEDER_DOL
                    };
                    circuits.place(tx, page, circuit)?;
                }
            }
        }
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
            info.name = options.plant.project_name().into();
            info.description = options.plant.description().into();
            info.rules = novo_core::ProjectRules::recommended();
            let text = |v: &str| Value::Text(v.into());
            info.properties
                .insert(front_keys::ISSUE_DATE.into(), text(today));
            info.properties
                .insert(front_keys::KIND.into(), text(options.plant.kind()));
            info.properties
                .insert(front_keys::CONTROL.into(), text("48 V AC / 24 V DC"));
            info.properties.insert(
                novo_checks::calc_view::inputs::SUPPLY_VOLTAGE_V.into(),
                400.0.into(),
            );
        })
    })?;
    Ok(changes)
}
