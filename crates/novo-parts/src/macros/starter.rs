//! The starter macros that ship with the program: common circuits drawn
//! with the IEC pack and captured through `Tx::define_block`, the same path
//! a user's own macro takes. They are starting points: the supply and the
//! protective earth are left open for the user to connect.

use novo_core::{
    AnyId, BlockParameter, DeviceId, DeviceTag, Document, Endpoint, PageId, Pin, Placement, Point,
    Project, Result, SymbolId, SymbolInstance, SymbolRole, Tx, Value,
};

use super::{Imported, MacroError, MacroFile, MacroLibrary};

mod more;

/// Folder the starter macros are kept in.
pub const STARTER_FOLDER: &str = "Starter";

/// Every starter macro with the folder it belongs in.
pub fn starter_macros() -> Vec<(String, MacroFile)> {
    let mut build: Vec<fn() -> Result<MacroFile>> =
        vec![motor_starter, star_delta, estop_chain, power_supply];
    build.extend(more::all());
    build
        .into_iter()
        .map(|f| {
            (
                STARTER_FOLDER.to_string(),
                f().expect("starter macros are built from valid symbols"),
            )
        })
        .collect()
}

impl MacroLibrary {
    /// Adds the starter macros that are not in the library yet. A starter
    /// the user renamed, moved or deleted is not put back over their work
    /// except by name: one that already exists is left alone.
    pub fn install_starters(&self) -> std::result::Result<Vec<Imported>, MacroError> {
        let mut done = Vec::new();
        for (folder, file) in starter_macros() {
            match self.save(&folder, &file) {
                Ok(path) => done.push(Imported::Added(path)),
                Err(MacroError::Exists(name)) => done.push(Imported::Skipped(name)),
                Err(e) => return Err(e),
            }
        }
        Ok(done)
    }
}

fn pins(list: &[(&str, f64, f64)]) -> Vec<Pin> {
    list.iter().map(|&(n, x, y)| Pin::new(n, x, y)).collect()
}

fn three_pole() -> Vec<Pin> {
    pins(&[
        ("1", 0.0, 0.0),
        ("2", 0.0, 16.0),
        ("3", 8.0, 0.0),
        ("4", 8.0, 16.0),
        ("5", 16.0, 0.0),
        ("6", 16.0, 16.0),
    ])
}

fn two_pin(a: &str, b: &str) -> Vec<Pin> {
    pins(&[(a, 0.0, 0.0), (b, 0.0, 16.0)])
}

/// Drawing helpers on one scratch page.
struct Sheet<'a, 'b> {
    tx: &'a mut Tx<'b>,
    page: PageId,
    items: Vec<AnyId>,
}

impl Sheet<'_, '_> {
    fn device(&mut self, tag: &str, description: &str) -> Result<DeviceId> {
        self.tx.add_device(
            DeviceTag::parse(tag).expect("starter tags are valid"),
            description,
        )
    }

    fn symbol(
        &mut self,
        key: &str,
        x: f64,
        y: f64,
        pins: Vec<Pin>,
        device: DeviceId,
        main: bool,
    ) -> Result<SymbolId> {
        let id = self.tx.place_symbol(
            self.page,
            key,
            Placement::at(Point::new(x, y)),
            pins,
            SymbolRole::Device { device, main },
        )?;
        self.items.push(id.into());
        Ok(id)
    }

    /// Pass-through groups of switched poles, as the symbol library says.
    fn passes(&mut self, symbol: SymbolId, groups: Vec<Vec<u16>>) -> Result<()> {
        self.tx
            .update::<SymbolInstance>(symbol, |s| s.pass_through = groups)
    }

    fn wire(&mut self, a: (SymbolId, u16), b: (SymbolId, u16)) -> Result<()> {
        self.tx
            .connect(Endpoint::pin(a.0, a.1), Endpoint::pin(b.0, b.1))?;
        Ok(())
    }

    fn junction(&mut self, x: f64, y: f64) -> Result<novo_core::JunctionId> {
        let j = self.tx.add_junction(self.page, Point::new(x, y))?;
        self.items.push(j.into());
        Ok(j)
    }

    fn describe(&mut self, device: DeviceId, key: &str, text: &str) -> Result<()> {
        self.tx.update::<novo_core::Device>(device, |d| {
            d.properties.insert(key.into(), Value::Text(text.into()));
        })
    }
}

/// Draws a macro in a scratch project and captures it.
fn capture(
    name: &str,
    description: &str,
    parameters: Vec<BlockParameter>,
    tags: &[&str],
    draw: impl FnOnce(&mut Sheet<'_, '_>) -> Result<()>,
) -> Result<MacroFile> {
    let mut doc = Document::new(Project::new(""));
    let (id, _) = doc.transact("Draw starter macro", |tx| {
        let page = tx.add_page("1")?;
        let mut sheet = Sheet {
            tx,
            page,
            items: Vec::new(),
        };
        draw(&mut sheet)?;
        orthogonal_routes(sheet.tx)?;
        let items = std::mem::take(&mut sheet.items);
        sheet
            .tx
            .define_block(name, &items, Point::new(0.0, 0.0), parameters)
    })?;
    let mut definition = doc
        .project()
        .block_definition(id)
        .expect("just defined")
        .clone();
    definition.description = description.into();
    Ok(MacroFile::new(definition)
        .with_author("KoperCAD")
        .with_tags(tags))
}

/// Gives every wire whose ends are not in line a right-angle route (down,
/// across, down), so a placed macro has no diagonal wires. Parallel runs get
/// slightly different crossing heights so they do not lie on top of each
/// other.
fn orthogonal_routes(tx: &mut Tx<'_>) -> Result<()> {
    const EPS: f64 = 1e-6;
    let mut bends = Vec::new();
    for w in tx.project().wires() {
        let (Some(a), Some(b)) = (
            tx.project().endpoint_position(w.from),
            tx.project().endpoint_position(w.to),
        ) else {
            continue;
        };
        if (a.x - b.x).abs() < EPS || (a.y - b.y).abs() < EPS || !w.route.is_empty() {
            continue;
        }
        bends.push((w.id, a, b));
    }
    for (n, (id, a, b)) in bends.into_iter().enumerate() {
        let span = b.y - a.y;
        let nudge = if span.abs() >= 12.0 {
            ((n % 4) as f64 - 1.5) * 2.0
        } else {
            0.0
        };
        let mid = a.y + span / 2.0 + nudge;
        tx.update::<novo_core::Wire>(id, |w| {
            w.route = vec![Point::new(a.x, mid), Point::new(b.x, mid)];
        })?;
    }
    Ok(())
}

fn parameter(name: &str, default: Value, description: &str) -> BlockParameter {
    BlockParameter {
        name: name.into(),
        default,
        description: description.into(),
    }
}

/// Direct-on-line starter: fuses, contactor, overload relay and motor, with
/// the start/stop control beside it. Open ends: L1 L2 L3 above the fuses,
/// the control supply above the breaker, N below the coil, motor PE.
fn motor_starter() -> Result<MacroFile> {
    capture(
        "Motor starter DOL",
        "Direct-on-line motor starter with start and stop buttons and self-hold",
        vec![parameter("power", Value::Float(2.2), "Motor power in kW")],
        &["motor", "starter", "dol", "contactor"],
        |s| {
            let f1 = s.device("-F1", "Main circuit fuses")?;
            let k1 = s.device("-K1", "Motor contactor")?;
            let f2 = s.device("-F2", "Overload relay")?;
            let m1 = s.device("-M1", "Motor {power} kW")?;
            let f3 = s.device("-F3", "Control circuit breaker")?;
            let s0 = s.device("-S0", "Stop")?;
            let s1 = s.device("-S1", "Start")?;
            s.describe(m1, "power_kw", "{power}")?;

            let f1s = s.symbol("iec/protection/fuse_3p", 40.0, 40.0, three_pole(), f1, true)?;
            let k1m = s.symbol(
                "iec/contactors/main_3p",
                40.0,
                72.0,
                three_pole(),
                k1,
                false,
            )?;
            let f2s = s.symbol(
                "iec/protection/overload_relay",
                40.0,
                104.0,
                three_pole(),
                f2,
                true,
            )?;
            s.tx.update::<SymbolInstance>(f2s, |x| {
                x.bridges = vec![vec![0, 1], vec![2, 3], vec![4, 5]]
            })?;
            let m1s = s.symbol(
                "iec/motors/motor_3ph",
                40.0,
                150.0,
                pins(&[
                    ("U1", 0.0, 0.0),
                    ("V1", 8.0, 0.0),
                    ("W1", 16.0, 0.0),
                    ("PE", 24.0, 0.0),
                ]),
                m1,
                true,
            )?;
            let three = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
            s.passes(f1s, three.clone())?;
            s.passes(k1m, three)?;
            for p in 0..3u16 {
                s.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
                s.wire((k1m, 2 * p + 1), (f2s, 2 * p))?;
                s.wire((f2s, 2 * p + 1), (m1s, p))?;
            }

            let f3s = s.symbol(
                "iec/protection/mcb_1p",
                100.0,
                40.0,
                two_pin("1", "2"),
                f3,
                true,
            )?;
            let s0s = s.symbol(
                "iec/operators/pushbutton_nc",
                100.0,
                72.0,
                two_pin("11", "12"),
                s0,
                true,
            )?;
            let s1s = s.symbol(
                "iec/operators/pushbutton_no",
                100.0,
                104.0,
                two_pin("13", "14"),
                s1,
                true,
            )?;
            let k1a = s.symbol(
                "iec/contacts/no",
                116.0,
                104.0,
                two_pin("13", "14"),
                k1,
                false,
            )?;
            let f2a = s.symbol(
                "iec/contacts/thermal_nc",
                100.0,
                136.0,
                two_pin("95", "96"),
                f2,
                false,
            )?;
            let k1c = s.symbol(
                "iec/coils/contactor",
                100.0,
                168.0,
                two_pin("A1", "A2"),
                k1,
                true,
            )?;
            for sym in [f3s, s0s, s1s, k1a, f2a] {
                s.passes(sym, vec![vec![0, 1]])?;
            }
            s.wire((f3s, 1), (s0s, 0))?;
            let split = s.junction(100.0, 96.0)?;
            s.tx.connect(Endpoint::pin(s0s, 1), Endpoint::junction(split))?;
            s.tx.connect(Endpoint::junction(split), Endpoint::pin(s1s, 0))?;
            s.tx.connect(Endpoint::junction(split), Endpoint::pin(k1a, 0))?;
            let join = s.junction(100.0, 128.0)?;
            s.tx.connect(Endpoint::pin(s1s, 1), Endpoint::junction(join))?;
            s.tx.connect(Endpoint::pin(k1a, 1), Endpoint::junction(join))?;
            s.tx.connect(Endpoint::junction(join), Endpoint::pin(f2a, 0))?;
            s.wire((f2a, 1), (k1c, 0))?;
            Ok(())
        },
    )
}

/// Star-delta starter: line, delta and star contactors, overload relay, a
/// six-terminal motor and a timer relay. Open ends as for the DOL starter.
fn star_delta() -> Result<MacroFile> {
    capture(
        "Star-delta starter",
        "Star-delta motor starter with line, delta and star contactors and an on-delay timer",
        vec![
            parameter("power", Value::Float(11.0), "Motor power in kW"),
            parameter("time", Value::Float(8.0), "Star time in seconds"),
        ],
        &["motor", "starter", "star-delta", "contactor", "timer"],
        |s| {
            let f1 = s.device("-F1", "Main circuit fuses")?;
            let k1 = s.device("-K1", "Line contactor")?;
            let f2 = s.device("-F2", "Overload relay")?;
            let k2 = s.device("-K2", "Delta contactor")?;
            let k3 = s.device("-K3", "Star contactor")?;
            let k4 = s.device("-K4", "Star time relay {time} s")?;
            let m1 = s.device("-M1", "Motor {power} kW")?;
            let f3 = s.device("-F3", "Control circuit breaker")?;
            let s0 = s.device("-S0", "Stop")?;
            let s1 = s.device("-S1", "Start")?;
            s.describe(m1, "power_kw", "{power}")?;
            s.describe(k4, "delay_s", "{time}")?;

            let three = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
            let f1s = s.symbol("iec/protection/fuse_3p", 40.0, 40.0, three_pole(), f1, true)?;
            let k1m = s.symbol(
                "iec/contactors/main_3p",
                40.0,
                72.0,
                three_pole(),
                k1,
                false,
            )?;
            let f2s = s.symbol(
                "iec/protection/overload_relay",
                40.0,
                104.0,
                three_pole(),
                f2,
                true,
            )?;
            s.tx.update::<SymbolInstance>(f2s, |x| {
                x.bridges = vec![vec![0, 1], vec![2, 3], vec![4, 5]]
            })?;
            let k2m = s.symbol(
                "iec/contactors/main_3p",
                80.0,
                150.0,
                three_pole(),
                k2,
                false,
            )?;
            let k3m = s.symbol(
                "iec/contactors/main_3p",
                8.0,
                150.0,
                three_pole(),
                k3,
                false,
            )?;
            let m1s = s.symbol(
                "iec/motors/motor_3ph_6t",
                40.0,
                200.0,
                pins(&[
                    ("U1", 0.0, 0.0),
                    ("V1", 8.0, 0.0),
                    ("W1", 16.0, 0.0),
                    ("U2", 0.0, 32.0),
                    ("V2", 8.0, 32.0),
                    ("W2", 16.0, 32.0),
                    ("PE", 24.0, 0.0),
                ]),
                m1,
                true,
            )?;
            for sym in [f1s, k1m, k2m, k3m] {
                s.passes(sym, three.clone())?;
            }
            for p in 0..3u16 {
                s.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
                s.wire((k1m, 2 * p + 1), (f2s, 2 * p))?;
                // Line side of the motor: U1, V1, W1.
                s.wire((f2s, 2 * p + 1), (m1s, p))?;
                // Delta: U1-W2, V1-U2, W1-V2.
                s.wire((k2m, 2 * p), (m1s, p))?;
                s.wire((k2m, 2 * p + 1), (m1s, 3 + (p + 2) % 3))?;
                // Star: the far winding ends U2, V2, W2 meet at the star point.
                s.wire((k3m, 2 * p), (m1s, 3 + p))?;
            }
            s.wire((k3m, 1), (k3m, 3))?;
            s.wire((k3m, 3), (k3m, 5))?;

            // Control: stop, start with self-hold on the line contactor; the
            // timer starts with it, star runs first, delta follows.
            let f3s = s.symbol(
                "iec/protection/mcb_1p",
                140.0,
                40.0,
                two_pin("1", "2"),
                f3,
                true,
            )?;
            let s0s = s.symbol(
                "iec/operators/pushbutton_nc",
                140.0,
                72.0,
                two_pin("11", "12"),
                s0,
                true,
            )?;
            let s1s = s.symbol(
                "iec/operators/pushbutton_no",
                140.0,
                104.0,
                two_pin("13", "14"),
                s1,
                true,
            )?;
            let k1a = s.symbol(
                "iec/contacts/no",
                156.0,
                104.0,
                two_pin("13", "14"),
                k1,
                false,
            )?;
            let f2a = s.symbol(
                "iec/contacts/thermal_nc",
                140.0,
                136.0,
                two_pin("95", "96"),
                f2,
                false,
            )?;
            for sym in [f3s, s0s, s1s, k1a, f2a] {
                s.passes(sym, vec![vec![0, 1]])?;
            }
            s.wire((f3s, 1), (s0s, 0))?;
            let split = s.junction(140.0, 96.0)?;
            s.tx.connect(Endpoint::pin(s0s, 1), Endpoint::junction(split))?;
            s.tx.connect(Endpoint::junction(split), Endpoint::pin(s1s, 0))?;
            s.tx.connect(Endpoint::junction(split), Endpoint::pin(k1a, 0))?;
            let join = s.junction(140.0, 128.0)?;
            s.tx.connect(Endpoint::pin(s1s, 1), Endpoint::junction(join))?;
            s.tx.connect(Endpoint::pin(k1a, 1), Endpoint::junction(join))?;
            s.tx.connect(Endpoint::junction(join), Endpoint::pin(f2a, 0))?;

            // Three branches from the overload contact: line coil, timer
            // coil, star and delta coils behind the timer contacts.
            let bus = s.junction(140.0, 152.0)?;
            s.tx.connect(Endpoint::pin(f2a, 1), Endpoint::junction(bus))?;
            let k1c = s.symbol(
                "iec/coils/contactor",
                140.0,
                168.0,
                two_pin("A1", "A2"),
                k1,
                true,
            )?;
            let k4c = s.symbol(
                "iec/coils/timer_on_delay",
                164.0,
                168.0,
                two_pin("A1", "A2"),
                k4,
                true,
            )?;
            let k4n = s.symbol(
                "iec/contacts/nc_delay_on",
                188.0,
                160.0,
                two_pin("15", "16"),
                k4,
                false,
            )?;
            let k2i = s.symbol(
                "iec/contacts/nc",
                188.0,
                184.0,
                two_pin("21", "22"),
                k2,
                false,
            )?;
            let k3c = s.symbol(
                "iec/coils/contactor",
                188.0,
                208.0,
                two_pin("A1", "A2"),
                k3,
                true,
            )?;
            let k4o = s.symbol(
                "iec/contacts/no_delay_on",
                212.0,
                160.0,
                two_pin("17", "18"),
                k4,
                false,
            )?;
            let k3i = s.symbol(
                "iec/contacts/nc",
                212.0,
                184.0,
                two_pin("21", "22"),
                k3,
                false,
            )?;
            let k2c = s.symbol(
                "iec/coils/contactor",
                212.0,
                208.0,
                two_pin("A1", "A2"),
                k2,
                true,
            )?;
            for sym in [k4n, k2i, k4o, k3i] {
                s.passes(sym, vec![vec![0, 1]])?;
            }
            for target in [(k1c, 0), (k4c, 0), (k4n, 0), (k4o, 0)] {
                s.tx.connect(Endpoint::junction(bus), Endpoint::pin(target.0, target.1))?;
            }
            s.wire((k4n, 1), (k2i, 0))?;
            s.wire((k2i, 1), (k3c, 0))?;
            s.wire((k4o, 1), (k3i, 0))?;
            s.wire((k3i, 1), (k2c, 0))?;
            Ok(())
        },
    )
}

/// Two emergency stops in series feeding a safety relay, with the relay's
/// feedback contact. Open ends: supply above the first button, N below the
/// coil.
fn estop_chain() -> Result<MacroFile> {
    capture(
        "E-stop chain",
        "Two emergency stop buttons in series that drop a safety relay",
        Vec::new(),
        &["safety", "emergency stop", "e-stop", "relay"],
        |s| {
            let s1 = s.device("-S1", "Emergency stop 1")?;
            let s2 = s.device("-S2", "Emergency stop 2")?;
            let k1 = s.device("-K1", "Safety relay")?;
            let s1s = s.symbol(
                "iec/operators/emergency_stop",
                40.0,
                40.0,
                two_pin("11", "12"),
                s1,
                true,
            )?;
            let s2s = s.symbol(
                "iec/operators/emergency_stop",
                40.0,
                72.0,
                two_pin("11", "12"),
                s2,
                true,
            )?;
            let k1c = s.symbol(
                "iec/coils/relay",
                40.0,
                104.0,
                two_pin("A1", "A2"),
                k1,
                true,
            )?;
            let k1a = s.symbol(
                "iec/contacts/no",
                72.0,
                104.0,
                two_pin("13", "14"),
                k1,
                false,
            )?;
            for sym in [s1s, s2s, k1a] {
                s.passes(sym, vec![vec![0, 1]])?;
            }
            s.wire((s1s, 1), (s2s, 0))?;
            s.wire((s2s, 1), (k1c, 0))?;
            Ok(())
        },
    )
}

/// Mains breaker, 24 V DC supply and output breaker. Open ends: L, N and PE
/// above the supply, the 24 V and 0 V rails below the output breaker.
fn power_supply() -> Result<MacroFile> {
    capture(
        "Power supply 24 V",
        "Switched-mode power supply behind a two-pole breaker, with an output breaker",
        vec![parameter(
            "voltage",
            Value::Int(24),
            "Output voltage in V DC",
        )],
        &["power supply", "24 V", "dc", "supply"],
        |s| {
            let f1 = s.device("-F1", "Supply input breaker")?;
            let g1 = s.device("-G1", "Power supply {voltage} V DC")?;
            let f2 = s.device("-F2", "Output breaker")?;
            s.describe(g1, "output_voltage_v", "{voltage}")?;
            let f1s = s.symbol(
                "iec/protection/mcb_2p",
                40.0,
                40.0,
                pins(&[
                    ("1", 0.0, 0.0),
                    ("2", 0.0, 16.0),
                    ("3", 8.0, 0.0),
                    ("4", 8.0, 16.0),
                ]),
                f1,
                true,
            )?;
            let g1s = s.symbol(
                "iec/supplies/power_supply_dc",
                40.0,
                72.0,
                pins(&[
                    ("L", 0.0, 0.0),
                    ("N", 8.0, 0.0),
                    ("PE", 16.0, 0.0),
                    ("L+", 0.0, 24.0),
                    ("M", 8.0, 24.0),
                ]),
                g1,
                true,
            )?;
            let f2s = s.symbol(
                "iec/protection/mcb_1p",
                40.0,
                112.0,
                two_pin("1", "2"),
                f2,
                true,
            )?;
            s.passes(f1s, vec![vec![0, 1], vec![2, 3]])?;
            s.passes(f2s, vec![vec![0, 1]])?;
            s.wire((f1s, 1), (g1s, 0))?;
            s.wire((f1s, 3), (g1s, 1))?;
            s.wire((g1s, 3), (f2s, 0))?;
            Ok(())
        },
    )
}
