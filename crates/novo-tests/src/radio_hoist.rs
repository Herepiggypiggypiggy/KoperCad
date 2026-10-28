//! A radio-controlled two-speed hoist, drawn over two pages: the fixture of the controller
//! lane C2 and the start of the crane fixture of C7.
//!
//! Page 1 "Radio control (48 V AC)": the receiver `-A1` (generic block, six output contacts on
//! one common) with its emergency-stop chain contact and its main contact in series ahead of
//! the common, so a pressed E-stop or a dropped main kills every output. Outputs 1 to 4 are
//! hoist up slow, up fast, down slow and down fast (step 2 closes both contacts of its
//! direction). Contactors: `-K1` up and `-K2` down (each interlocked by the other's break
//! contact), `-K3` slow winding and `-K4` fast winding (`-K3` follows a direction contactor and
//! the break contact of `-K4` drops it; `-K4` only pulls in behind a direction contactor). Lamp
//! `-H0` shows the main contact, `-H1` the brake feed (a direction contactor is in).
//!
//! Page 2 "Hoist motor": `-K1`/`-K2` reverse the phase sequence onto a bus, `-K3` feeds the
//! low-speed winding `1U 1V 1W` of the Dahlander motor `-M1`, `-K4` the high-speed winding
//! `2U 2V 2W`. A model of the wiring for the live view, not an engineered drive.
//!
//! The controller itself (instance `radio-1`, profile `generic-radio-3x2`) and its bindings
//! are stored in the project's `controllers.project` property by [`radio_hoist_with_controller`].

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, PageId, Placement, Point, PotentialId, PotentialKind,
    Result, SymbolId, SymbolRole, Tx, Value, WireId,
};
use novo_editor::catalog::{model_groups, model_pins};
use novo_symbols::Library;

/// Handles to the objects of the fixture.
#[derive(Debug, Clone)]
pub struct RadioHoist {
    pub control_page: PageId,
    pub power_page: PageId,
    pub receiver: SymbolId,
    pub a1: DeviceId,
    pub k1_coil: SymbolId,
    pub k2_coil: SymbolId,
    pub k3_coil: SymbolId,
    pub k4_coil: SymbolId,
    pub k1_main: SymbolId,
    pub k2_main: SymbolId,
    pub k3_main: SymbolId,
    pub k4_main: SymbolId,
    pub motor: SymbolId,
    pub main_lamp: SymbolId,
    pub brake_lamp: SymbolId,
    pub v48: PotentialId,
    pub zero_v: PotentialId,
}

/// The names of the receiver pins the fixture binds (`-A1:<name>`), in signal order.
pub const OUTPUT_PINS: [&str; 4] = ["1", "2", "3", "4"];
/// The chain contact and the main contact of the receiver.
pub const CHAIN_PIN: &str = "E1";
pub const MAIN_PIN: &str = "M1";

struct Placed {
    id: SymbolId,
    pins: Vec<String>,
}

impl Placed {
    fn pin(&self, name: &str) -> u16 {
        let index = self
            .pins
            .iter()
            .position(|p| p == name)
            .unwrap_or_else(|| panic!("no pin {name}"));
        u16::try_from(index).expect("few pins")
    }
    fn end(&self, name: &str) -> Endpoint {
        Endpoint::pin(self.id, self.pin(name))
    }
}

struct Builder<'a, 't> {
    tx: &'a mut Tx<'t>,
    library: Library,
    page: PageId,
}

impl Builder<'_, '_> {
    /// A library symbol placed with its own pins, bridges and pass-through groups.
    fn place(&mut self, key: &str, x: f64, y: f64, role: SymbolRole) -> Result<Placed> {
        let symbol = self.library.get(key).unwrap_or_else(|| panic!("{key}"));
        let pins = model_pins(symbol);
        let names = pins.iter().map(|p| p.name.clone()).collect();
        let (bridges, pass) = model_groups(symbol);
        let id =
            self.tx
                .place_symbol(self.page, key, Placement::at(Point::new(x, y)), pins, role)?;
        self.tx.update::<novo_core::SymbolInstance>(id, |s| {
            s.bridges = bridges;
            s.pass_through = pass;
        })?;
        Ok(Placed { id, pins: names })
    }

    /// Renames the pins of a placed symbol (an instance may renumber its pins).
    fn rename(&mut self, p: &mut Placed, names: &[&str]) -> Result<()> {
        let names: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
        let copy = names.clone();
        self.tx.update::<novo_core::SymbolInstance>(p.id, |s| {
            for (pin, name) in s.pins.iter_mut().zip(copy) {
                pin.name = name;
            }
        })?;
        p.pins = names;
        Ok(())
    }

    fn feed(&mut self, x: f64, y: f64, potential: PotentialId) -> Result<Placed> {
        self.place(
            "iec/connectivity/potential_feed",
            x,
            y,
            SymbolRole::PotentialFeed { potential },
        )
    }

    /// A wire with bend points (page mm), drawn orthogonally.
    fn wire(&mut self, from: Endpoint, to: Endpoint, route: &[(f64, f64)]) -> Result<WireId> {
        let wire = self.tx.connect(from, to)?;
        if !route.is_empty() {
            let points: Vec<Point> = route.iter().map(|&(x, y)| Point::new(x, y)).collect();
            self.tx
                .update::<novo_core::Wire>(wire, |w| w.route = points)?;
        }
        Ok(wire)
    }
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("fixture tags are valid")
}

fn device(tx: &mut Tx<'_>, text: &str, description: &str) -> Result<DeviceId> {
    tx.add_device(tag(text), description)
}

fn dev(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

/// Builds the project as one undo step. The controller is not stored; see
/// [`radio_hoist_with_controller`].
pub fn radio_hoist(doc: &mut Document) -> Result<RadioHoist> {
    Ok(doc.transact("Draw radio hoist", build)?.0)
}

fn build(tx: &mut Tx<'_>) -> Result<RadioHoist> {
    tx.set_info(|i| i.name = "Radio hoist (fixture)".into())?;
    let control_page = tx.add_page("1")?;
    let power_page = tx.add_page("2")?;
    tx.update::<novo_core::Page>(control_page, |p| p.title = "Radio control (48 V AC)".into())?;
    tx.update::<novo_core::Page>(power_page, |p| p.title = "Hoist motor".into())?;
    let v48 = tx.add_potential("48V", PotentialKind::Line)?;
    let zero_v = tx.add_potential("0V", PotentialKind::Neutral)?;
    let l1 = tx.add_potential("L1", PotentialKind::Line)?;
    let l2 = tx.add_potential("L2", PotentialKind::Line)?;
    let l3 = tx.add_potential("L3", PotentialKind::Line)?;

    let a1 = device(tx, "-A1", "Radio receiver")?;
    let k1 = device(tx, "-K1", "Hoist up")?;
    let k2 = device(tx, "-K2", "Hoist down")?;
    let k3 = device(tx, "-K3", "Slow winding")?;
    let k4 = device(tx, "-K4", "Fast winding")?;
    let h0 = device(tx, "-H0", "Main on")?;
    let h1 = device(tx, "-H1", "Brake released")?;
    let m1 = device(tx, "-M1", "Hoist motor, two speeds")?;
    for (d, color) in [(h0, "green"), (h1, "amber")] {
        tx.update::<novo_core::Device>(d, |dev| {
            dev.properties
                .insert("color".into(), Value::Text(color.into()));
        })?;
    }

    // ----- page 1: control ---------------------------------------------------------------
    let mut b = Builder {
        tx,
        library: Library::builtin().expect("the built-in library loads"),
        page: control_page,
    };
    let feed48 = b.feed(40.0, 20.0, v48)?;
    let mut chain = b.place("iec/controllers/output_dry", 40.0, 40.0, dev(a1, false))?;
    b.rename(&mut chain, &["E1", "E2"])?;
    let mut main = b.place("iec/controllers/output_dry", 40.0, 72.0, dev(a1, false))?;
    b.rename(&mut main, &["M1", "M2"])?;
    let rx = b.place("iec/controllers/outputs_6", 100.0, 104.0, dev(a1, true))?;
    // Main-on lamp.
    let lamp0 = b.place("iec/signalling/lamp", 60.0, 104.0, dev(h0, true))?;
    let n0 = b.feed(60.0, 120.0, zero_v)?;

    b.wire(feed48.end("1"), chain.end("E1"), &[])?;
    b.wire(chain.end("E2"), main.end("M1"), &[])?;
    b.wire(
        main.end("M2"),
        rx.end("COM"),
        &[(40.0, 96.0), (120.0, 96.0)],
    )?;
    b.wire(
        main.end("M2"),
        lamp0.end("X1"),
        &[(40.0, 96.0), (60.0, 96.0)],
    )?;
    b.wire(lamp0.end("X2"), n0.end("1"), &[])?;

    // Contacts under the outputs.
    let k2_nc = b.place("iec/contacts/nc", 100.0, 160.0, dev(k2, false))?;
    let k1_no = b.place("iec/contacts/no", 140.0, 160.0, dev(k1, false))?;
    let k1_nc = b.place("iec/contacts/nc", 180.0, 160.0, dev(k1, false))?;
    let k2_no = b.place("iec/contacts/no", 220.0, 160.0, dev(k2, false))?;
    b.wire(rx.end("1"), k2_nc.end("21"), &[])?;
    b.wire(
        rx.end("2"),
        k1_no.end("13"),
        &[(108.0, 148.0), (140.0, 148.0)],
    )?;
    b.wire(
        rx.end("3"),
        k1_nc.end("21"),
        &[(116.0, 144.0), (180.0, 144.0)],
    )?;
    b.wire(
        rx.end("4"),
        k2_no.end("13"),
        &[(124.0, 140.0), (220.0, 140.0)],
    )?;

    // Direction coils.
    let k1_coil = b.place("iec/coils/contactor", 100.0, 200.0, dev(k1, true))?;
    let k2_coil = b.place("iec/coils/contactor", 180.0, 200.0, dev(k2, true))?;
    let n1 = b.feed(100.0, 216.0, zero_v)?;
    let n2 = b.feed(180.0, 216.0, zero_v)?;
    b.wire(k2_nc.end("22"), k1_coil.end("A1"), &[])?;
    b.wire(k1_nc.end("22"), k2_coil.end("A1"), &[])?;
    b.wire(k1_coil.end("A2"), n1.end("1"), &[])?;
    b.wire(k2_coil.end("A2"), n2.end("1"), &[])?;

    // Slow winding: fed by either slow output, dropped by the fast contactor.
    let k4_nc = b.place("iec/contacts/nc", 160.0, 248.0, dev(k4, false))?;
    let k3_coil = b.place("iec/coils/contactor", 160.0, 280.0, dev(k3, true))?;
    let n3 = b.feed(160.0, 296.0, zero_v)?;
    b.wire(k4_nc.end("22"), k3_coil.end("A1"), &[])?;
    b.wire(k3_coil.end("A2"), n3.end("1"), &[])?;

    // Fast winding: either fast output behind its direction contactor.
    let k4_coil = b.place("iec/coils/contactor", 260.0, 200.0, dev(k4, true))?;
    let n4 = b.feed(260.0, 216.0, zero_v)?;
    b.wire(
        k1_no.end("14"),
        k4_coil.end("A1"),
        &[(140.0, 192.0), (260.0, 192.0)],
    )?;
    b.wire(
        k2_no.end("14"),
        k4_coil.end("A1"),
        &[(220.0, 192.0), (260.0, 192.0)],
    )?;
    b.wire(k4_coil.end("A2"), n4.end("1"), &[])?;

    // Brake lamp: a direction contactor is in.
    let k1_aux = b.place("iec/contacts/no", 300.0, 160.0, dev(k1, false))?;
    let k2_aux = b.place("iec/contacts/no", 340.0, 160.0, dev(k2, false))?;
    let lamp1 = b.place("iec/signalling/lamp", 320.0, 200.0, dev(h1, true))?;
    let n5 = b.feed(320.0, 216.0, zero_v)?;
    b.wire(
        main.end("M2"),
        k1_aux.end("13"),
        &[(40.0, 96.0), (300.0, 96.0)],
    )?;
    b.wire(
        main.end("M2"),
        k2_aux.end("13"),
        &[(40.0, 96.0), (340.0, 96.0)],
    )?;
    b.wire(
        k1_aux.end("14"),
        lamp1.end("X1"),
        &[(300.0, 188.0), (320.0, 188.0)],
    )?;
    b.wire(
        k2_aux.end("14"),
        lamp1.end("X1"),
        &[(340.0, 188.0), (320.0, 188.0)],
    )?;
    b.wire(lamp1.end("X2"), n5.end("1"), &[])?;
    // The slow winding follows "a direction contactor is in" (the same node as the brake lamp)
    // and is dropped by the fast contactor's break contact.
    b.wire(
        k1_aux.end("14"),
        k4_nc.end("21"),
        &[(300.0, 232.0), (160.0, 232.0)],
    )?;

    // ----- page 2: motor -----------------------------------------------------------------
    b.page = power_page;
    let phases = [l1, l2, l3];
    let mut feeds_k1 = Vec::new();
    let mut feeds_k2 = Vec::new();
    for (i, p) in phases.iter().enumerate() {
        feeds_k1.push(b.feed(40.0 + 8.0 * i as f64, 20.0, *p)?);
    }
    // K2 swaps L1 and L3.
    for (i, p) in [l3, l2, l1].iter().enumerate() {
        feeds_k2.push(b.feed(80.0 + 8.0 * i as f64, 20.0, *p)?);
    }
    let k1_main = b.place("iec/contactors/main_3p", 40.0, 48.0, dev(k1, false))?;
    let k2_main = b.place("iec/contactors/main_3p", 80.0, 48.0, dev(k2, false))?;
    let k3_main = b.place("iec/contactors/main_3p", 40.0, 100.0, dev(k3, false))?;
    let k4_main = b.place("iec/contactors/main_3p", 80.0, 100.0, dev(k4, false))?;
    let motor = b.place("iec/motors/motor_3ph_dahlander", 40.0, 150.0, dev(m1, true))?;
    for i in 0..3 {
        let (top, bottom) = (format!("{}", 2 * i + 1), format!("{}", 2 * i + 2));
        b.wire(feeds_k1[i].end("1"), k1_main.end(&top), &[])?;
        b.wire(feeds_k2[i].end("1"), k2_main.end(&top), &[])?;
        // The bus: both direction contactors feed the same three conductors.
        let y = 72.0 + 4.0 * i as f64;
        let (xa, xb) = (40.0 + 8.0 * i as f64, 80.0 + 8.0 * i as f64);
        b.wire(
            k2_main.end(&bottom),
            k1_main.end(&bottom),
            &[(xb, y), (xa, y)],
        )?;
        b.wire(k1_main.end(&bottom), k3_main.end(&top), &[])?;
        b.wire(k2_main.end(&bottom), k4_main.end(&top), &[])?;
        let low = ["1U", "1V", "1W"][i];
        let high = ["2U", "2V", "2W"][i];
        b.wire(k3_main.end(&bottom), motor.end(low), &[])?;
        let y = 190.0 + 4.0 * i as f64;
        b.wire(k4_main.end(&bottom), motor.end(high), &[(xb, y), (xa, y)])?;
    }

    Ok(RadioHoist {
        control_page,
        power_page,
        receiver: rx.id,
        a1,
        k1_coil: k1_coil.id,
        k2_coil: k2_coil.id,
        k3_coil: k3_coil.id,
        k4_coil: k4_coil.id,
        k1_main: k1_main.id,
        k2_main: k2_main.id,
        k3_main: k3_main.id,
        k4_main: k4_main.id,
        motor: motor.id,
        main_lamp: lamp0.id,
        brake_lamp: lamp1.id,
        v48,
        zero_v,
    })
}

/// The project with one generic radio bound to the receiver: the E-stop chain and the main
/// signal to the two dry contacts, hoist up/down slow/fast to outputs 1 to 4. Returns the
/// handles and the controller data as stored in the project.
pub fn radio_hoist_with_controller(doc: &mut Document) -> Result<(RadioHoist, String)> {
    use novo_controller::{
        BindingTable, ControllerSet, Profile, ProjectControllers, SignalKey, Target,
        import::library,
    };
    let handles = radio_hoist(doc)?;
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
    let key = |s: &str| SignalKey::new("radio-1", s);
    table.bind(key("ESTOP_OK"), Target::Pin(format!("-A1:{CHAIN_PIN}")));
    table.bind(key("MAIN"), Target::Pin(format!("-A1:{MAIN_PIN}")));
    for (signal, pin) in [
        "HOIST_UP_SLOW",
        "HOIST_UP_FAST",
        "HOIST_DN_SLOW",
        "HOIST_DN_FAST",
    ]
    .into_iter()
    .zip(OUTPUT_PINS)
    {
        table.bind(key(signal), Target::Pin(format!("-A1:{pin}")));
    }
    let text = ProjectControllers::capture(&set, &table)
        .and_then(|d| d.to_text())
        .expect("serialises");
    doc.transact("Store controller", |tx| {
        let text = text.clone();
        tx.set_info(|i| {
            i.properties
                .insert("controllers.project".into(), Value::Text(text));
        })
    })?;
    Ok((handles, text))
}
