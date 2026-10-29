//! The example project (docs/ONBOARDING-DESIGN.md §1): "Open example" on
//! the start panel and in the command field. It is the Gate 1 motor
//! starter, built by the same fixture the tests use, so it cannot drift
//! from what is tested.

use novo_core::{
    Device, DeviceTag, Document, MountingPlate, Part, Result as ModelResult, Structure,
    StructureId, Tx, Value,
};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::{apply_plan, load, set_enclosure, set_plate};
use novo_layout::rules::RuleSet;
use novo_parts::{Catalog, DataSource, PartKey};

use super::example_kit;
use super::{Level, NovoApp};

/// What the example project is called.
pub(crate) const EXAMPLE_NAME: &str = "Motor starter (example)";

const EXAMPLE_DESCRIPTION: &str = "Direct-on-line starter for a pump motor: main and control \
    circuit. The parts are illustrative catalogue entries, not a verified parts list, and the \
    400 V / 6 kA supply is an illustrative value to replace with your own.";

/// Draws the example into a project that has only its empty first page, as
/// one undo step.
pub(crate) fn build_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    build_example_with(doc, |_| Ok(()))
}

/// [`build_example`] with `extra` run after the starter is drawn and before
/// the cabinet is planned, so devices it adds are placed on the plate too
/// (the QA fixture with machine options).
pub(crate) fn build_example_with(
    doc: &mut Document,
    extra: impl FnOnce(&mut Tx<'_>) -> ModelResult<()>,
) -> ModelResult<novo_core::ChangeSet> {
    build_example_named(doc, EXAMPLE_NAME, extra)
}

/// [`build_example_with`] under another project name (the EMC example).
pub(crate) fn build_example_named(
    doc: &mut Document,
    name: &str,
    extra: impl FnOnce(&mut Tx<'_>) -> ModelResult<()>,
) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        novo_tests::motor_starter_in(tx)?;
        // The pages are filed by their devices' tags (=A1, +CAB), so the
        // outline shows the example as a tree.
        super::outline_tree::auto_file(tx)?;
        // Numbered with the project's scheme, so the wire list, the labels and
        // the commissioning list name wires by number and not by id.
        novo_numbering::number_new_wires(tx)?;
        extra(tx)?;
        give_parts_their_data(tx)?;
        add_cabinet(tx)?;
        add_safety_function(tx)?;
        tx.set_info(|info| {
            info.name = name.into();
            info.description = EXAMPLE_DESCRIPTION.into();
            // A plausible small-machine supply (400 V, 6 kA, within what the example's
            // control-circuit protection breaks), stated, so the calculations and the single-line
            // diagram start from numbers and not from question marks. Illustrative:
            // the user replaces them with the installation's own.
            info.properties.insert(
                novo_checks::calc_view::inputs::SUPPLY_VOLTAGE_V.into(),
                400.0.into(),
            );
            info.properties.insert(
                novo_checks::standards::keys::SUPPLY_SHORT_CIRCUIT_KA.into(),
                6.0.into(),
            );
        })
    })?;
    Ok(changes)
}

/// Mounting plate of the example cabinet, mm.
const PLATE: MountingPlate = MountingPlate {
    width: 450.0,
    height: 550.0,
    margin: 10.0,
};

/// The fixture's parts are bare (maker and order number). Gives each one
/// that the starter catalog knows its sizes and technical data, so the
/// cabinet, 3D and heat have real numbers to work with. Catalog data is
/// copied, never invented here.
pub(super) fn give_parts_their_data(tx: &mut Tx<'_>) -> ModelResult<()> {
    let catalog = Catalog::starter();
    let parts: Vec<_> = tx.project().all::<Part>().map(|p| p.id).collect();
    for id in parts {
        let Some(part) = tx.project().get::<Part>(id) else {
            continue;
        };
        let Some(known) = catalog.get(&PartKey::new(&part.manufacturer, &part.order_number)) else {
            continue;
        };
        let estimated = known.source_of(novo_parts::keys::POWER_LOSS_W) == DataSource::Estimated;
        let known = known.to_model(id);
        tx.update::<Part>(id, |p| {
            p.type_number = known.type_number;
            p.description = known.description;
            p.properties = known.properties;
            if estimated {
                p.properties.insert(
                    novo_thermal::bridge::LOSS_SOURCE_PROPERTY.into(),
                    Value::Text("estimated".into()),
                );
            }
        })?;
    }
    Ok(())
}

/// The cabinet `+CAB` of the example: enclosure interior with a plate
/// setback, a right hinged door, and the rails, ducts and device places the
/// rules planner proposes for the devices of the cabinet (what Place all
/// would do), so Cabinet, 3D and Simulate work on the first click.
fn add_cabinet(tx: &mut Tx<'_>) -> ModelResult<StructureId> {
    let (_, mut enclosure) =
        example_kit::enclosure((PLATE.width, PLATE.height), (500.0, 600.0, 250.0));
    enclosure.front_clearance = Some(400.0);
    let cab = prepare_cabinet(tx, "+CAB", PLATE, &enclosure)?;
    // The terminal strip -X1 is four blocks side by side, so it has no
    // single part to read its footprint from: its own size, the 4 blocks of
    // 5.2 mm and two end covers, and the 0.3 W the starter catalog gives a
    // terminal block at rated current.
    let strip = tx
        .project()
        .devices_by_tag(&DeviceTag::parse("=A1+CAB-X1")?)
        .first()
        .copied();
    if let Some(strip) = strip {
        tx.update::<Device>(strip, |d| {
            for (key, value) in [
                ("width_mm", 26.0),
                ("height_mm", 47.5),
                ("depth_mm", 42.5),
                ("power_loss_w", 1.2),
            ] {
                d.properties.insert(key.into(), Value::Float(value));
            }
            // 4 x 0.3 W, and the catalog marks that figure as an estimate.
            d.properties.insert(
                novo_thermal::bridge::LOSS_SOURCE_PROPERTY.into(),
                Value::Text("estimated".into()),
            );
        })?;
    }
    // The stop and start buttons stay on the plate here: a button on the door
    // needs a wiring opening before its wires can be routed, and the example
    // routes every wire. In a real cabinet Place all puts them on the door.
    for tag in ["=A1+CAB-S0", "=A1+CAB-S1"] {
        let button = tx
            .project()
            .devices_by_tag(&DeviceTag::parse(tag)?)
            .first()
            .copied();
        if let Some(button) = button {
            tx.set_property(
                button,
                novo_layout::site::SITE_KEY,
                Some(Value::Text("plate".into())),
            )?;
        }
    }
    plan_cabinet(tx, cab)?;
    Ok(cab)
}

/// The cabinet structure at `location` (`+CAB`), made an enclosure with its
/// mounting plate and enclosure interior.
pub(super) fn prepare_cabinet(
    tx: &mut Tx<'_>,
    location: &str,
    plate: MountingPlate,
    enclosure: &Enclosure,
) -> ModelResult<StructureId> {
    // The outline filing has already made the structure of the location.
    let tag = DeviceTag::parse(location)?;
    let existing = tx
        .project()
        .structures()
        .find(|s| s.tag == tag)
        .map(|s| s.id);
    let cab = match existing {
        Some(id) => id,
        None => tx.add_structure(tag, "Control cabinet", true)?,
    };
    tx.update::<Structure>(cab, |s| {
        s.enclosure = true;
        s.description = "Control cabinet".into();
    })?;
    set_plate(tx, cab, plate)?;
    set_enclosure(tx, cab, enclosure)?;
    Ok(cab)
}

/// Lets the rules planner propose rails, ducts and device places for the
/// devices of the cabinet (what Place all does).
pub(super) fn plan_cabinet(tx: &mut Tx<'_>, cab: StructureId) -> ModelResult<()> {
    // Pilot devices (buttons, lamps) would go to the door, which needs a
    // wiring opening before their wires can be routed; the examples draw
    // every wire, so they stay on the plate.
    let location = tx
        .project()
        .structures()
        .find(|s| s.id == cab)
        .map(|s| s.tag.location.clone());
    if let Some(location) = location {
        let pilots: Vec<_> = tx
            .project()
            .devices()
            .filter(|d| {
                d.tag.location == location
                    && matches!(
                        d.tag.letter_code().to_ascii_uppercase().as_str(),
                        "S" | "H" | "P"
                    )
                    && novo_layout::site::stated_site(tx.project(), d).is_none()
            })
            .map(|d| d.id)
            .collect();
        for pilot in pilots {
            tx.set_property(
                pilot,
                novo_layout::site::SITE_KEY,
                Some(Value::Text("plate".into())),
            )?;
        }
    }
    let layout = load(tx.project(), cab).expect("the plate was just set");
    let (plan, _) = super::cabinet_rules::plan(tx.project(), cab, &layout, &RuleSet::standard());
    apply_plan(tx, cab, &plan)
}

impl NovoApp {
    /// Replaces the open project with a fresh copy of the example. It is an
    /// untitled project like any other, marked saved so that closing it
    /// untouched leaves nothing under Unsaved; the first edit makes it
    /// dirty and the usual autosave and Unsaved flow take over.
    pub(super) fn open_example(&mut self) {
        self.open_built_example(EXAMPLE_NAME, build_example);
    }

    /// The EMC example: the starter plus a drive, a motor cable and signals.
    pub(super) fn open_emc_example(&mut self) {
        self.open_built_example(
            super::emc_demo::EMC_EXAMPLE_NAME,
            super::emc_demo::build_emc_example,
        );
    }

    /// The motor control centre example: four feeders and a full cabinet.
    pub(super) fn open_mcc_example(&mut self) {
        self.open_built_example(
            super::example_mcc::MCC_EXAMPLE_NAME,
            super::example_mcc::build_mcc_example,
        );
    }

    /// The overhead crane example: two-speed hoist, drives and radio control.
    pub(super) fn open_crane_example(&mut self) {
        self.open_built_example(
            super::example_crane::CRANE_EXAMPLE_NAME,
            super::example_crane::build_crane_example,
        );
    }

    /// The PLC cabinet example: CPU, I/O cards and a full cabinet.
    pub(super) fn open_plc_example(&mut self) {
        self.open_built_example(
            super::example_plc::PLC_EXAMPLE_NAME,
            super::example_plc::build_plc_example,
        );
    }

    /// The pump station example: two frequency drives, pressure control and a full cabinet.
    pub(super) fn open_pump_example(&mut self) {
        self.open_built_example(
            super::example_pump::PUMP_EXAMPLE_NAME,
            super::example_pump::build_pump_example,
        );
    }

    /// File > New from template > Overhead crane: a structure tree, a few
    /// empty pages and the cover pages, in a new unsaved project.
    pub(super) fn new_crane_project(&mut self) {
        self.open_built(
            super::template_crane::CRANE_TEMPLATE_NAME,
            super::template_crane::build_crane_template,
            "New crane project. Fill in Project > Project settings > Project information; the cover pages follow.",
            false,
        );
    }

    fn open_built_example(
        &mut self,
        name: &str,
        build: fn(&mut Document) -> ModelResult<novo_core::ChangeSet>,
    ) {
        self.open_built(
            name,
            build,
            "This is an example. Edit freely; Ctrl+S keeps your own copy.",
            true,
        );
    }

    /// Opens a project drawn by `build`. An example starts as saved (nothing
    /// to ask about on close); a template is a new project the person owns.
    fn open_built(
        &mut self,
        name: &str,
        build: fn(&mut Document) -> ModelResult<novo_core::ChangeSet>,
        welcome: &str,
        saved: bool,
    ) {
        if let Err(e) = self.backend.new_project(name) {
            self.message(Level::Error, format!("Could not open the project: {e}"));
            return;
        }
        self.project_path = None;
        self.current_page = None;
        let built = self.backend.document_mut().map(build);
        match built {
            Some(Ok(changes)) => self.backend.record(&changes),
            Some(Err(e)) => {
                self.message(Level::Error, format!("Could not draw the example: {e}"));
            }
            None => {}
        }
        // Resets the views and adds the generated pages (contents, terminal
        // and cable diagrams), which belong to the example too.
        self.document_replaced();
        self.fix_current_page();
        if let Some(doc) = self.backend.document_mut() {
            if saved {
                doc.mark_saved();
            }
            // Opening starts with an empty history, like a fresh project:
            // Ctrl+Z must not silently undo the example's own drawing.
            doc.clear_history();
        }
        self.message(Level::Info, welcome);
    }
}

/// One illustrative safety function so Check > Safety functions and the
/// sheets export have something to show: the stop button and the motor
/// contactor of the starter. The reliability values are typed in as
/// illustrative numbers, not taken from a datasheet, and the function says so.
fn add_safety_function(tx: &mut Tx<'_>) -> ModelResult<()> {
    use novo_checks::safety::{Basis, Block, PartRef, Role, SafetyFunction, encode};
    let part = |name: &str, device: &str, b10d: f64| PartRef {
        name: name.into(),
        device: Some(device.into()),
        basis: Basis::B10d,
        value: Some(b10d),
    };
    let mut f = SafetyFunction::new("Motor stop, ordinary stop not e-stop (illustrative values)");
    f.blocks = vec![
        Block {
            category: "1".into(),
            dc_percent: 0.0,
            ccf: 0,
            channel1: vec![part("Stop button", "=A1+CAB-S0", 150_000.0)],
            ..Block::new(Role::Sensor, "Stop")
        },
        Block {
            category: "1".into(),
            dc_percent: 0.0,
            ccf: 0,
            channel1: vec![part("Motor contactor", "=A1+CAB-K1", 1_300_000.0)],
            ..Block::new(Role::Actuator, "Motor contactor")
        },
    ];
    novo_checks::standards::set_info_property(tx, novo_checks::safety::FUNCTIONS_KEY, encode(&[f]))
}
