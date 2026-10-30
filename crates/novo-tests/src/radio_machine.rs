//! A crane described to the machine model only, driven by a generic radio through machine ports
//! (the fixture of lane L5b, controller machine-module bindings).
//!
//! One page "Machine" with two motor symbols and no circuit: `-M1` is a hoist (hook load set under
//! "Mechanical load") and `-M2` a travel axis (moving mass set under "Travel drive"). The radio
//! `radio-1` (profile `generic-radio-3x2`) is bound to ports of those two modules, for example
//! hoist up to `hoist/-M1@up` and the E-stop chain to `hoist/-M1@permit`. Nothing is drawn between
//! the receiver and the motors: the bound ports replace the drawn inputs, which is the point of
//! this kind of binding (see `docs/design/controller-live.md`, "Machine ports").

use novo_controller::{
    BindingTable, ControllerSet, MachinePath, Profile, ProjectControllers, SignalKey, Target,
    import::library,
};
use novo_core::{
    DeviceId, DeviceTag, Document, Placement, Point, Result, SymbolId, SymbolRole, Tx,
};
use novo_editor::catalog::{model_groups, model_pins};
use novo_symbols::Library;

/// Handles to the objects of the fixture.
#[derive(Debug, Clone)]
pub struct RadioMachine {
    pub hoist_motor: SymbolId,
    pub travel_motor: SymbolId,
    pub m1: DeviceId,
    pub m2: DeviceId,
}

/// Signals bound to the hoist `-M1`: `(signal, port)`.
pub const HOIST_BINDINGS: [(&str, &str); 7] = [
    ("HOIST_UP_SLOW", "up"),
    ("HOIST_UP_FAST", "up"),
    ("HOIST_DN_SLOW", "down"),
    ("HOIST_DN_FAST", "down"),
    ("HOIST_UP_FAST", "fast"),
    ("HOIST_DN_FAST", "fast"),
    ("ESTOP_OK", "permit"),
];

/// Signals bound to the travel axis `-M2`.
pub const TRAVEL_BINDINGS: [(&str, &str); 7] = [
    ("BRIDGE_FWD_SLOW", "forward"),
    ("BRIDGE_FWD_FAST", "forward"),
    ("BRIDGE_BACK_SLOW", "reverse"),
    ("BRIDGE_BACK_FAST", "reverse"),
    ("BRIDGE_FWD_FAST", "fast"),
    ("BRIDGE_BACK_FAST", "fast"),
    ("ESTOP_OK", "permit"),
];

fn put(tx: &mut Tx<'_>, page: novo_core::PageId, x: f64, device: DeviceId) -> Result<SymbolId> {
    let library = Library::builtin().expect("the built-in library loads");
    let key = "iec/motors/motor_3ph";
    let symbol = library.get(key).expect("motor symbol");
    let pins = model_pins(symbol);
    let (bridges, pass) = model_groups(symbol);
    let id = tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(x, 60.0)),
        pins,
        SymbolRole::Device { device, main: true },
    )?;
    tx.update::<novo_core::SymbolInstance>(id, |s| {
        s.bridges = bridges;
        s.pass_through = pass;
    })?;
    Ok(id)
}

/// Builds the project as one undo step. The controller is not stored; see
/// [`radio_machine_with_controller`].
pub fn radio_machine(doc: &mut Document) -> Result<RadioMachine> {
    Ok(doc
        .transact("Draw radio machine", |tx| {
            tx.set_info(|i| i.name = "Radio machine ports (fixture)".into())?;
            let page = tx.add_page("1")?;
            tx.update::<novo_core::Page>(page, |p| p.title = "Machine".into())?;
            let m1 = tx.add_device(DeviceTag::parse("-M1").expect("tag"), "Hoist motor")?;
            let m2 = tx.add_device(DeviceTag::parse("-M2").expect("tag"), "Bridge travel motor")?;
            // A hook load makes -M1 a hoist, a moving mass makes -M2 a travel axis; the other
            // numbers default (a generic 12/2 pole hoist motor and a generic 1.5 kW travel drive).
            tx.set_property(m1, "mech_mass_kg", Some(100.0.into()))?;
            tx.set_property(m2, "trav_mass_kg", Some(2000.0.into()))?;
            let hoist_motor = put(tx, page, 40.0, m1)?;
            let travel_motor = put(tx, page, 100.0, m2)?;
            Ok(RadioMachine {
                hoist_motor,
                travel_motor,
                m1,
                m2,
            })
        })?
        .0)
}

/// The project with one generic radio bound to the ports of `-M1` and `-M2`. Returns the handles
/// and the controller data as stored in the project.
pub fn radio_machine_with_controller(doc: &mut Document) -> Result<(RadioMachine, String)> {
    let handles = radio_machine(doc)?;
    let profile = Profile::load(
        &library::build("generic-radio-3x2")
            .expect("shipped profile")
            .profile
            .to_toml(),
    )
    .expect("the shipped profile loads");
    let mut set = ControllerSet::new();
    set.add_instance("radio-1", "Radio, 3 motions, 2-step", &profile)
        .expect("a fresh set");
    let mut table = BindingTable::new();
    for (module, list) in [
        ("hoist/-M1", HOIST_BINDINGS.as_slice()),
        ("travel/-M2", TRAVEL_BINDINGS.as_slice()),
    ] {
        for (signal, port) in list {
            let path = MachinePath::new(module, port).expect("valid path");
            table.bind(SignalKey::new("radio-1", signal), Target::machine(&path));
        }
    }
    let text = ProjectControllers::capture(&set, &table)
        .and_then(|d| d.to_text())
        .expect("serialises");
    doc.transact("Store controller", |tx| {
        tx.set_addon_block(
            "controllers",
            "project",
            novo_core::AddonBlock {
                format: 1,
                text: text.clone(),
            },
        )
    })?;
    Ok((handles, text))
}
