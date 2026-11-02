//! The crane simulation project: the overhead crane cabinet of `example_crane` with its control
//! drawn the way a radio receiver is wired, so the whole machine can be run from the circuit.
//!
//! * Three receiver blocks (`-A1` hoist, `-A2` bridge, `-A3` crab) close dry contacts onto the
//!   enabled 48 V rail. Their emergency-stop chain and main contacts sit in the line contactor
//!   `-K0` rung. The virtual radio of the project (Simulate > Controller panel) is bound to those
//!   pins and presses them, so every motion goes through the drawn relays and contactors.
//! * The hoist motor `-M11` carries a hook load and the device links that make it a hoist for
//!   Simulate > Hoist run; the bridge `-M21` and the crab `-M22` carry a moving mass and their
//!   drive inputs, for Simulate > Drive run.
//! * The picture in Simulate > Hoist in 3D is set to the same axes (`crane3d.*` properties).
//!
//! The numbers are a generic 1 t, 24 m runway crane: planning values for a model, not a
//! verified design and not data from any customer's project. The end limits stop the fast step
//! only; the slow step is still allowed past them, as on a real two-step crane.

use novo_controller::{
    BindingTable, ControllerSet, Profile, ProjectControllers, SignalKey, Target, import::library,
};
use novo_core::{
    AddonBlock, DeviceId, Document, Placement, Point, PotentialId, Result as ModelResult, SymbolId,
    SymbolInstance, SymbolRole, Tx, Value,
};
use novo_editor::catalog::{model_groups, model_pins};
use novo_symbols::Library;

use super::example_crane::{CAB, FLD};
use super::example_kit::{Col, End, Finish, Kit, enclosure, tag};

/// What the project is called.
pub(crate) const CRANE_SIM_EXAMPLE_NAME: &str = "Overhead crane simulation (example)";

const DESCRIPTION: &str = "Overhead crane you can run: a radio (Simulate > Controller panel) \
    closes the contacts of three receiver blocks, the drawn relays and contactors answer, the \
    two-speed hoist, the bridge and the crab drives move and Simulate > Hoist in 3D shows it. \
    Over six pages and a laid-out cabinet. 1 t hoist, 24 m runway, 400 V / 10 kA supply: planning \
    values to replace with your own. A model of the wiring and the mechanics, not a safety \
    function.";

/// The controller instance the project carries.
pub(crate) const RADIO: &str = "radio-1";

/// The rails of the control circuit the pages hang from.
pub(super) struct Rails {
    pub p48: PotentialId,
    pub p0: PotentialId,
}

/// The enabled 48 V rail behind `-K0`: an interruption point name, so it is a node that is
/// live only while the contactor is closed (a potential feed would be a source).
const RAIL: &str = "E48";

/// A rung that starts at the interruption point `name`.
fn rung_from<'a>(kit: &mut Kit<'_, '_>, name: &str, x: f64) -> Col<'a> {
    let ip = kit.ip(name, (x, 32.0), true);
    Col {
        x,
        y: 40.0,
        last: End::Pin((ip, "1")),
    }
}

const NO: &str = "iec/contacts/no";
const NC: &str = "iec/contacts/nc";
const COIL: &str = "iec/coils/contactor";
const RELAY_COIL: &str = "iec/coils/relay";
const LIMIT_NC: &str = "iec/sensors/proximity_nc";

fn k(n: &str) -> String {
    format!("{CAB}-K{n}")
}

/// Places a library symbol of a receiver (a device with its own tag) with the pins and groups of
/// the library, the pins renamed to `names` when given.
fn place_rx(
    kit: &mut Kit<'_, '_>,
    library: &Library,
    key: &str,
    at: (f64, f64),
    device: DeviceId,
    main: bool,
    names: &[&str],
) -> ModelResult<SymbolId> {
    let symbol = library.get(key).unwrap_or_else(|| panic!("{key}"));
    let pins = model_pins(symbol);
    let (bridges, pass) = model_groups(symbol);
    let id = kit.tx.place_symbol(
        kit.page,
        key,
        Placement::at(Point::new(at.0, at.1)),
        pins,
        SymbolRole::Device { device, main },
    )?;
    let names: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
    kit.tx.update::<SymbolInstance>(id, |s| {
        s.bridges = bridges;
        s.pass_through = pass;
        for (pin, name) in s.pins.iter_mut().zip(names) {
            pin.name = name;
        }
    })?;
    Ok(id)
}

/// A dry receiver contact hung in a rung, its pins named `names`.
fn series_rx(
    kit: &mut Kit<'_, '_>,
    library: &Library,
    col: &mut Col<'_>,
    device: DeviceId,
    names: &[&str],
) -> ModelResult<()> {
    let s = place_rx(
        kit,
        library,
        "iec/controllers/output_dry",
        (col.x, col.y),
        device,
        false,
        names,
    )?;
    kit.link(col.last, End::Idx(s, 0), &[]);
    col.last = End::Idx(s, 1);
    col.y += 24.0;
    Ok(())
}

/// One receiver block: `COM` on the enabled rail, the four outputs each leaving on an
/// interruption point of its own name.
fn receiver_block(
    kit: &mut Kit<'_, '_>,
    library: &Library,
    device: DeviceId,
    at: (f64, f64),
    names: [&str; 4],
) -> ModelResult<()> {
    let block = place_rx(
        kit,
        library,
        "iec/controllers/outputs_6",
        at,
        device,
        true,
        &[],
    )?;
    let rail = kit.ip(RAIL, (at.0 + 20.0, at.1 - 16.0), true);
    kit.link(End::Pin((rail, "1")), End::Pin((block, "COM")), &[]);
    for (i, name) in names.iter().enumerate() {
        let out = kit.ip(name, (at.0 + 8.0 * i as f64, at.1 + 36.0), false);
        kit.link(
            End::Pin((block, ["1", "2", "3", "4"][i])),
            End::Pin((out, "1")),
            &[],
        );
    }
    Ok(())
}

/// Two branches that feed one coil: `a` then `b` joined on a junction, then the coil of `coil`
/// below it. Each branch is `(output, limit, contact device)`: output, the end limit that drops
/// the fast step, the direction contactor that has to be in.
fn fast_coil(
    kit: &mut Kit<'_, '_>,
    x: f64,
    branches: [(&str, &str, &str); 2],
    coil_key: &str,
    coil: &str,
    zero: PotentialId,
) {
    let mut cols = Vec::new();
    for (i, (out, limit, direction)) in branches.into_iter().enumerate() {
        let mut c = rung_from(kit, out, x + 16.0 * i as f64);
        kit.series(&mut c, LIMIT_NC, limit);
        kit.series(&mut c, NO, direction);
        cols.push(c);
    }
    let y = cols[0].y;
    let j = kit.junction((x, y + 4.0));
    for c in &cols {
        kit.link(c.last, End::Junction(j), &[]);
    }
    let mut c = Col {
        x,
        y: y + 8.0,
        last: End::Junction(j),
    };
    kit.series(&mut c, coil_key, coil);
    kit.close(&c, zero);
}

/// Pages 4 (receiver and hoist control) and 5 (receivers and travel relays).
pub(super) fn control_pages(
    kit: &mut Kit<'_, '_>,
    p_hoist: novo_core::PageId,
    p_travel: novo_core::PageId,
    r: &Rails,
) -> ModelResult<()> {
    let library = Library::builtin().expect("the built-in library loads");
    let mk = |kit: &mut Kit<'_, '_>, n: &str, what: &str| -> ModelResult<DeviceId> {
        kit.tx.add_device(tag(&format!("{CAB}-{n}")), what)
    };
    let a1 = mk(
        kit,
        "A1",
        "Radio receiver: hoist outputs, emergency-stop chain and main",
    )?;
    let a2 = mk(kit, "A2", "Radio receiver output module: bridge travel")?;
    let a3 = mk(kit, "A3", "Radio receiver output module: crab travel")?;
    let s = |n: &str| format!("{CAB}-{n}");
    let q11 = s("Q11");

    // ----- page 4 -------------------------------------------------------------------------
    kit.on(p_hoist);
    // The line contactor: cabinet stop, the receiver's stop chain and its main contact.
    let mut c = kit.rung(r.p48, 30.0);
    kit.series(&mut c, "iec/operators/emergency_stop", &s("S0"));
    series_rx(kit, &library, &mut c, a1, &["E1", "E2"])?;
    series_rx(kit, &library, &mut c, a1, &["M1", "M2"])?;
    kit.series(&mut c, COIL, &k("0"));
    kit.close(&c, r.p0);
    kit.describe(&s("S0"), "Emergency stop (cabinet)");
    // The enabled 48 V rail: everything below runs only with K0 closed.
    let mut c = kit.rung(r.p48, 70.0);
    kit.series(&mut c, NO, &k("0"));
    let rail = kit.ip(RAIL, (c.x, c.y), false);
    kit.link(c.last, End::Pin((rail, "1")), &[]);
    // The hoist outputs: up step 1, up step 2, down step 1, down step 2.
    receiver_block(
        kit,
        &library,
        a1,
        (110.0, 56.0),
        ["H.UP1", "H.UP2", "H.DN1", "H.DN2"],
    )?;
    // K11 up, K12 down: each locked out by the other and by the motor protection.
    for (x, out, me, other, what) in [
        (170.0, "H.UP1", "11", "12", "Hoist up"),
        (210.0, "H.DN1", "12", "11", "Hoist down"),
    ] {
        let mut c = rung_from(kit, out, x);
        kit.series(&mut c, NC, &k(other));
        kit.series(&mut c, NC, &q11);
        kit.series(&mut c, COIL, &k(me));
        kit.close(&c, r.p0);
        kit.describe(&k(me), what);
    }
    // K13 slow winding: while moving and not fast.
    let mut c = rung_from(kit, RAIL, 250.0);
    kit.either(&mut c, NO, &k("11"), NO, &k("12"));
    kit.series(&mut c, NC, &k("14"));
    kit.series(&mut c, COIL, &k("13"));
    kit.close(&c, r.p0);
    kit.describe(&k("13"), "Hoist slow speed (many poles)");
    // K14 fast winding: step 2 behind its direction contactor, through the end limit of that
    // direction (the limit drops the fast step only).
    fast_coil(
        kit,
        290.0,
        [
            ("H.UP2", &s("S11"), &k("11")),
            ("H.DN2", &s("S12"), &k("12")),
        ],
        COIL,
        &k("14"),
        r.p0,
    );
    kit.describe(&k("14"), "Hoist fast speed (few poles)");
    kit.describe(&s("S11"), "Hoist upper limit (drops the fast step)");
    kit.describe(&s("S12"), "Hoist lower limit (drops the fast step)");
    // K15 star point with K14, K16 brake while either speed runs.
    let mut c = rung_from(kit, RAIL, 330.0);
    kit.series(&mut c, NO, &k("14"));
    kit.series(&mut c, COIL, &k("15"));
    kit.close(&c, r.p0);
    kit.describe(&k("15"), "Fast speed star point");
    let mut c = rung_from(kit, RAIL, 370.0);
    kit.either(&mut c, NO, &k("13"), NO, &k("14"));
    kit.series(&mut c, COIL, &k("16"));
    kit.close(&c, r.p0);
    kit.describe(&k("16"), "Brake contactor");

    // ----- page 5 -------------------------------------------------------------------------
    kit.on(p_travel);
    receiver_block(
        kit,
        &library,
        a2,
        (20.0, 56.0),
        ["B.FW1", "B.FW2", "B.BK1", "B.BK2"],
    )?;
    receiver_block(
        kit,
        &library,
        a3,
        (90.0, 56.0),
        ["C.FW1", "C.FW2", "C.BK1", "C.BK2"],
    )?;
    let fld = |n: &str| format!("{FLD}-{n}");
    // Direction relays, interlocked against each other, then the fast relay behind the end limits.
    for (x, out, me, other, what) in [
        (140.0, "B.FW1", "23", "24", "Bridge forward"),
        (168.0, "B.BK1", "24", "23", "Bridge back"),
        (250.0, "C.FW1", "26", "27", "Crab forward"),
        (278.0, "C.BK1", "27", "26", "Crab back"),
    ] {
        let mut c = rung_from(kit, out, x);
        kit.series(&mut c, NC, &k(other));
        kit.series(&mut c, RELAY_COIL, &k(me));
        kit.close(&c, r.p0);
        kit.describe(&k(me), what);
    }
    fast_coil(
        kit,
        196.0,
        [
            ("B.FW2", &fld("S21"), &k("23")),
            ("B.BK2", &fld("S22"), &k("24")),
        ],
        RELAY_COIL,
        &k("25"),
        r.p0,
    );
    fast_coil(
        kit,
        306.0,
        [
            ("C.FW2", &fld("S23"), &k("26")),
            ("C.BK2", &fld("S24"), &k("27")),
        ],
        RELAY_COIL,
        &k("28"),
        r.p0,
    );
    kit.describe(&k("25"), "Bridge fast");
    kit.describe(&k("28"), "Crab fast");
    kit.describe(
        &fld("S21"),
        "Bridge forward end limit (drops the fast step)",
    );
    kit.describe(&fld("S22"), "Bridge back end limit (drops the fast step)");
    kit.describe(&fld("S23"), "Crab forward end limit (drops the fast step)");
    kit.describe(&fld("S24"), "Crab back end limit (drops the fast step)");
    // The drives' line contactors close with the enabled rail.
    for (x, n, what) in [(352.0, "21", "Bridge"), (380.0, "22", "Crab")] {
        let mut c = rung_from(kit, RAIL, x);
        kit.series(&mut c, COIL, &k(n));
        kit.close(&c, r.p0);
        kit.describe(&k(n), &format!("{what} drive line contactor"));
    }
    Ok(())
}

fn device_of(tx: &Tx<'_>, text: &str) -> DeviceId {
    tx.project()
        .devices_by_tag(&tag(text))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("no device {text}"))
}

/// The motor properties, the controller and the 3D setup that make the drawing a machine.
fn machine(tx: &mut Tx<'_>) -> ModelResult<()> {
    let text = |s: &str| Value::Text(s.to_owned());
    let num = |v: f64| Value::Float(v);
    // The hoist: 1 t on two falls, a 5.5 / 1.4 kW pole-change motor.
    let m11 = device_of(tx, &format!("{FLD}-M11"));
    let hoist: Vec<(&str, Value)> = vec![
        ("mech_mass_kg", num(1000.0)),
        ("mech_falls", num(2.0)),
        ("mech_drum_radius_m", num(0.2)),
        ("mech_gear_ratio", num(120.0)),
        ("mech_coarse_kw", num(5.5)),
        ("mech_fine_kw", num(1.4)),
        ("mech_main_contactor", text(&k("0"))),
        ("mech_brake_feed", text(&k("16"))),
        ("mech_overload_amps", num(11.0)),
        ("mech_overload_contact", text(&format!("{CAB}-Q11"))),
        ("mech_upper_limit", text(&format!("{CAB}-S11"))),
        ("mech_upper_limit_m", num(4.5)),
        ("mech_lower_limit", text(&format!("{CAB}-S12"))),
        ("mech_lower_limit_m", num(-2.0)),
    ];
    for (key, value) in hoist {
        tx.set_property(m11, key, Some(value))?;
    }
    // The bridge and the crab: a 3 kW motor on a 3 kW drive each, 2-wire with two presets.
    for (n, mass, gear, line, lim_f, lim_r, dist_f, dist_r, pins) in [
        (
            "21",
            12_000.0,
            45.0,
            "21",
            "S21",
            "S22",
            15.0,
            -2.0,
            ("23", "24", "25"),
        ),
        (
            "22",
            3_000.0,
            60.0,
            "22",
            "S23",
            "S24",
            4.0,
            -4.0,
            ("26", "27", "28"),
        ),
    ] {
        let motor = device_of(tx, &format!("{FLD}-M{n}"));
        let props: Vec<(&str, Value)> = vec![
            ("trav_mass_kg", num(mass)),
            ("trav_wheel_radius_m", num(0.125)),
            ("trav_gear_ratio", num(gear)),
            ("trav_motor_kw", num(3.0)),
            ("trav_motor_a", num(6.5)),
            ("trav_motor_rpm", num(1440.0)),
            ("trav_step1_hz", num(10.0)),
            ("trav_step2_hz", num(50.0)),
            ("trav_in_fwd", text(&k(pins.0))),
            ("trav_in_rev", text(&k(pins.1))),
            ("trav_in_high", text(&k(pins.2))),
            ("trav_mains", text(&k(line))),
            ("trav_limit_fwd", text(&format!("{FLD}-{lim_f}"))),
            ("trav_limit_fwd_m", num(dist_f)),
            ("trav_limit_rev", text(&format!("{FLD}-{lim_r}"))),
            ("trav_limit_rev_m", num(dist_r)),
        ];
        for (key, value) in props {
            tx.set_property(motor, key, Some(value))?;
        }
    }
    // The picture: runway, span, start positions and which axis moves which part.
    let mut setup = super::crane_setup::CraneSetup::default();
    setup.layout.runway_m = 24.0;
    setup.layout.span_m = 12.0;
    setup.bridge_m = 3.0;
    setup.trolley_m = 6.0;
    setup.bridge_axis = Some(format!("{FLD}-M21"));
    setup.trolley_axis = Some(format!("{FLD}-M22"));
    tx.set_info(|i| setup.write_properties(&mut i.properties))?;
    store_controller(tx)
}

/// The radio of the project and its bindings to the receiver pins.
fn store_controller(tx: &mut Tx<'_>) -> ModelResult<()> {
    let profile = Profile::load(
        &library::build("generic-radio-3x2")
            .expect("shipped profile")
            .profile
            .to_toml(),
    )
    .expect("the shipped profile loads");
    let mut set = ControllerSet::new();
    set.add_instance(RADIO, "Radio, 3 motions, 2-step", &profile)
        .expect("a fresh set");
    let mut table = BindingTable::new();
    let key = |s: &str| SignalKey::new(RADIO, s);
    let pin = |device: &str, p: &str| Target::Pin(format!("{CAB}-{device}:{p}"));
    table.bind(key("ESTOP_OK"), pin("A1", "E1"));
    table.bind(key("MAIN"), pin("A1", "M1"));
    for (device, signals) in [
        (
            "A1",
            [
                "HOIST_UP_SLOW",
                "HOIST_UP_FAST",
                "HOIST_DN_SLOW",
                "HOIST_DN_FAST",
            ],
        ),
        (
            "A2",
            [
                "BRIDGE_FWD_SLOW",
                "BRIDGE_FWD_FAST",
                "BRIDGE_BACK_SLOW",
                "BRIDGE_BACK_FAST",
            ],
        ),
        (
            "A3",
            [
                "TROLLEY_RIGHT_SLOW",
                "TROLLEY_RIGHT_FAST",
                "TROLLEY_LEFT_SLOW",
                "TROLLEY_LEFT_FAST",
            ],
        ),
    ] {
        for (i, signal) in signals.into_iter().enumerate() {
            table.bind(key(signal), pin(device, ["1", "2", "3", "4"][i]));
        }
    }
    let text = ProjectControllers::capture(&set, &table)
        .and_then(|d| d.to_text())
        .expect("serialises");
    tx.set_addon_block("controllers", "project", AddonBlock { format: 1, text })
}

/// Draws the project into a project that has only its empty first page.
pub(crate) fn build_crane_sim_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        super::example_crane::draw(tx, true)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        machine(tx)?;
        let (plate, enclosure) = enclosure((960.0, 1360.0), (1000.0, 1400.0, 300.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: CRANE_SIM_EXAMPLE_NAME,
                description: DESCRIPTION,
                cabinet: "+CAB",
                plate,
                enclosure,
                supply_v: 400.0,
                short_circuit_ka: 10.0,
            },
        )
    })?;
    Ok(changes)
}
