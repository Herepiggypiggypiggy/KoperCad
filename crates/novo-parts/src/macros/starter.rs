//! The starter macros that ship with the program: common circuits drawn
//! with the IEC pack and captured through `Tx::define_block`, the same path
//! a user's own macro takes. They come out fully wired inside: protective
//! earth runs through a PE terminal to every motor, coil returns share one
//! rail, and the only open points left are the ones the user connects to the
//! supply (the "Three-phase supply" macro has matching outputs).

use novo_core::{
    AnyId, BlockParameter, Device, DeviceId, DeviceTag, Direction, Document, Endpoint, JunctionId,
    PageId, Pin, Placement, Point, Project, Result, SymbolId, SymbolInstance, SymbolRole, Tx,
    Value, Wire, WireId,
};
use novo_symbols::Library;

use super::{Imported, MacroError, MacroFile, MacroLibrary};

mod more;
mod safety;

/// Folder the starter macros are kept in.
pub const STARTER_FOLDER: &str = "Starter";

/// Every starter macro with the folder it belongs in.
pub fn starter_macros() -> Vec<(String, MacroFile)> {
    let mut build: Vec<fn() -> Result<MacroFile>> =
        vec![motor_starter, star_delta, estop_chain, power_supply];
    build.extend(more::all());
    build.extend(safety::all());
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

fn direction(d: novo_symbols::Direction) -> Direction {
    match d {
        novo_symbols::Direction::Up => Direction::Up,
        novo_symbols::Direction::Down => Direction::Down,
        novo_symbols::Direction::Left => Direction::Left,
        novo_symbols::Direction::Right => Direction::Right,
    }
}

/// Drawing helpers on one scratch page.
struct Sheet<'a, 'b> {
    tx: &'a mut Tx<'b>,
    page: PageId,
    items: Vec<AnyId>,
    lib: Library,
}

impl Sheet<'_, '_> {
    fn device(&mut self, tag: &str, description: &str) -> Result<DeviceId> {
        self.tx.add_device(
            DeviceTag::parse(tag).expect("starter tags are valid"),
            description,
        )
    }

    /// Places a symbol of the library for a device. Pins (with their exit
    /// directions and spare points), bridges and pass-through paths come
    /// from the symbol library, so a macro cannot drift from its symbols.
    fn put(&mut self, key: &str, x: f64, y: f64, device: DeviceId, main: bool) -> Result<SymbolId> {
        self.place(
            key,
            Placement::at(Point::new(x, y)),
            SymbolRole::Device { device, main },
        )
    }

    /// Places a symbol that belongs to no device, such as a terminal.
    fn put_plain(&mut self, key: &str, x: f64, y: f64) -> Result<SymbolId> {
        self.place(key, Placement::at(Point::new(x, y)), SymbolRole::Unassigned)
    }

    fn place(&mut self, key: &str, placement: Placement, role: SymbolRole) -> Result<SymbolId> {
        let sym = self.lib.get(key).expect("starter symbols exist");
        let pins: Vec<Pin> = sym
            .connections
            .iter()
            .map(|c| {
                let pin =
                    Pin::new(c.id.as_str(), c.at[0], c.at[1]).with_direction(direction(c.dir));
                if c.optional { pin.optional() } else { pin }
            })
            .collect();
        let to_u16 = |groups: &[Vec<usize>]| -> Vec<Vec<u16>> {
            groups
                .iter()
                .map(|g| g.iter().map(|&i| i as u16).collect())
                .collect()
        };
        let bridges = to_u16(&sym.bridges());
        let passes: Vec<Vec<u16>> = to_u16(sym.pass_through().paths())
            .into_iter()
            .filter(|g| !bridges.contains(g))
            .collect();
        let id = self
            .tx
            .place_symbol(self.page, key, placement, pins, role)?;
        self.items.push(id.into());
        if !bridges.is_empty() || !passes.is_empty() {
            self.tx.update::<SymbolInstance>(id, |s| {
                s.bridges = bridges;
                s.pass_through = passes;
            })?;
        }
        Ok(id)
    }

    /// Names the pins of a symbol, in connection order: the second contact
    /// of a device is 23-24 or 33-34, not 13-14 again.
    fn number(&mut self, symbol: SymbolId, names: &[&str]) -> Result<()> {
        self.tx.update::<SymbolInstance>(symbol, |s| {
            for (pin, name) in s.pins.iter_mut().zip(names) {
                pin.name = (*name).into();
            }
        })
    }

    fn describe(&mut self, device: DeviceId, key: &str, text: &str) -> Result<()> {
        self.tx.update::<Device>(device, |d| {
            d.properties.insert(key.into(), Value::Text(text.into()));
        })
    }

    fn at(&self, symbol: SymbolId, pin: u16) -> Point {
        self.tx
            .project()
            .endpoint_position(Endpoint::pin(symbol, pin))
            .expect("the symbol was just placed")
    }

    fn wire(&mut self, a: (SymbolId, u16), b: (SymbolId, u16)) -> Result<WireId> {
        self.tx
            .connect(Endpoint::pin(a.0, a.1), Endpoint::pin(b.0, b.1))
    }

    /// A wire that follows the given corner points between its ends.
    fn wire_via(
        &mut self,
        from: impl Into<Endpoint>,
        to: impl Into<Endpoint>,
        corners: &[(f64, f64)],
    ) -> Result<()> {
        let id = self.tx.connect(from, to)?;
        let route: Vec<Point> = corners.iter().map(|&(x, y)| Point::new(x, y)).collect();
        self.tx.update::<Wire>(id, |w| w.route = route)
    }

    fn junction(&mut self, x: f64, y: f64) -> Result<JunctionId> {
        let j = self.tx.add_junction(self.page, Point::new(x, y))?;
        self.items.push(j.into());
        Ok(j)
    }

    /// A horizontal bus on row `y`: one junction at the x of every tap, in a
    /// chain from left to right, each joined to its tap pins (above or
    /// below the row) by a straight wire. Taps at the same x share a
    /// junction. Returns the junctions from left to right.
    fn bus(&mut self, y: f64, taps: &[(SymbolId, u16)]) -> Result<Vec<JunctionId>> {
        let mut at: Vec<(f64, (SymbolId, u16))> =
            taps.iter().map(|&t| (self.at(t.0, t.1).x, t)).collect();
        at.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut junctions: Vec<(f64, JunctionId)> = Vec::new();
        for (x, (symbol, pin)) in at {
            let j = match junctions.last() {
                Some(&(last, j)) if (last - x).abs() < 1e-6 => j,
                _ => {
                    let j = self.junction(x, y)?;
                    if let Some(&(_, prev)) = junctions.last() {
                        self.tx
                            .connect(Endpoint::junction(prev), Endpoint::junction(j))?;
                    }
                    junctions.push((x, j));
                    j
                }
            };
            self.tx
                .connect(Endpoint::junction(j), Endpoint::pin(symbol, pin))?;
        }
        Ok(junctions.into_iter().map(|(_, j)| j).collect())
    }

    /// Protective earth enters through a PE terminal on the top row, next
    /// to the three line phases, and runs on to `to` (a motor or supply PE
    /// point).
    fn pe_entry(&mut self, x: f64, y: f64, to: (SymbolId, u16)) -> Result<SymbolId> {
        let terminal = self.put_plain("iec/terminals/terminal_pe", x, y)?;
        self.wire((terminal, 1), to)?;
        Ok(terminal)
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
            lib: Library::builtin().expect("the built-in symbols load"),
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

/// Gives every wire whose ends are not in line a right-angle route, so a
/// placed macro has no diagonal wires. A wire between a pin and a junction
/// leaves the junction sideways and enters the pin from above or below (so
/// wires of one junction never share a stretch); a wire between two pins
/// runs down, across, down, parallel runs at slightly different heights.
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
        let corner = match (w.from, w.to) {
            (Endpoint::Pin { .. }, Endpoint::Junction { .. }) => Some(Point::new(a.x, b.y)),
            (Endpoint::Junction { .. }, Endpoint::Pin { .. }) => Some(Point::new(b.x, a.y)),
            _ => None,
        };
        bends.push((w.id, a, b, corner));
    }
    // Crossing heights already taken, with the x range of the run.
    let mut taken: Vec<(f64, f64, f64)> = Vec::new();
    for (id, a, b, corner) in bends {
        if let Some(corner) = corner {
            tx.update::<Wire>(id, |w| w.route = vec![corner])?;
            continue;
        }
        let span = b.y - a.y;
        let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
        let room = (span.abs() / 2.0 - 3.0).max(0.0);
        let mut mid = a.y + span / 2.0;
        for step in 0..8 {
            let nudge = ((step + 1) / 2) as f64 * 2.0 * if step % 2 == 0 { 1.0 } else { -1.0 };
            if nudge.abs() > room {
                break;
            }
            let candidate = a.y + span / 2.0 + nudge;
            let clash = taken
                .iter()
                .any(|&(l, h, m)| l <= hi && lo <= h && (m - candidate).abs() < 1.0);
            if !clash {
                mid = candidate;
                break;
            }
        }
        taken.push((lo, hi, mid));
        tx.update::<Wire>(id, |w| {
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

/// Draws the control column shared by the DOL and star-delta starters at
/// `x`: control breaker, stop, start in parallel with the self-hold contact
/// of `k1`, and the overload contact. Returns the overload contact; its
/// pin 1 feeds the coils.
fn start_stop(s: &mut Sheet<'_, '_>, x: f64, devices: [DeviceId; 5]) -> Result<SymbolId> {
    let [f3, s0, s1, k1, f2] = devices;
    let f3s = s.put("iec/protection/mcb_1p", x, 40.0, f3, true)?;
    let s0s = s.put("iec/operators/pushbutton_nc", x, 72.0, s0, true)?;
    let s1s = s.put("iec/operators/pushbutton_no", x, 104.0, s1, true)?;
    let k1a = s.put("iec/contacts/no", x + 16.0, 104.0, k1, false)?;
    let thermal = s.put("iec/contacts/thermal_nc", x, 136.0, f2, false)?;
    s.wire((f3s, 1), (s0s, 0))?;
    s.bus(96.0, &[(s0s, 1), (s1s, 0), (k1a, 0)])?;
    s.bus(128.0, &[(s1s, 1), (k1a, 1), (thermal, 0)])?;
    Ok(thermal)
}

/// Direct-on-line starter: fuses, contactor, overload relay and motor, with
/// the start/stop control beside it. Open points: L1 L2 L3 and PE above the
/// fuses, the control supply above the breaker, N below the coil.
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

            let f1s = s.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
            let k1m = s.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let f2s = s.put("iec/protection/overload_relay", 40.0, 104.0, f2, true)?;
            let m1s = s.put("iec/motors/motor_3ph", 40.0, 150.0, m1, true)?;
            for p in 0..3u16 {
                s.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
                s.wire((k1m, 2 * p + 1), (f2s, 2 * p))?;
                s.wire((f2s, 2 * p + 1), (m1s, p))?;
            }
            s.pe_entry(64.0, 40.0, (m1s, 3))?;

            let thermal = start_stop(s, 100.0, [f3, s0, s1, k1, f2])?;
            let k1c = s.put("iec/coils/contactor", 100.0, 168.0, k1, true)?;
            s.wire((thermal, 1), (k1c, 0))?;
            Ok(())
        },
    )
}

/// Star-delta starter: line, delta and star contactors, overload relay, a
/// six-terminal motor and a timer relay. Open points as for the DOL starter.
/// The delta and star wires run beside and below the motor in separate
/// channels, so no two lie on each other.
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

            let f1s = s.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
            let k1m = s.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let f2s = s.put("iec/protection/overload_relay", 40.0, 104.0, f2, true)?;
            let m1s = s.put("iec/motors/motor_3ph_6t", 40.0, 200.0, m1, true)?;
            let k2m = s.put("iec/contactors/main_3p", 88.0, 200.0, k2, false)?;
            let k3m = s.put("iec/contactors/main_3p", 40.0, 256.0, k3, false)?;
            s.pe_entry(64.0, 40.0, (m1s, 6))?;

            // Line side: fuses, line contactor, overload relay, then a tap
            // per phase for the delta contactor, down to U1 V1 W1.
            for p in 0..3u16 {
                s.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
                s.wire((k1m, 2 * p + 1), (f2s, 2 * p))?;
            }
            let x = |p: u16| 40.0 + 8.0 * f64::from(p);
            let tap_y = [180.0, 172.0, 164.0];
            for p in 0..3u16 {
                let tap = s.junction(x(p), tap_y[usize::from(p)])?;
                s.tx.connect(Endpoint::pin(f2s, 2 * p + 1), Endpoint::junction(tap))?;
                s.tx.connect(Endpoint::junction(tap), Endpoint::pin(m1s, p))?;
                s.wire_via(
                    Endpoint::junction(tap),
                    Endpoint::pin(k2m, 2 * p),
                    &[(88.0 + 8.0 * f64::from(p), tap_y[usize::from(p)])],
                )?;
            }
            // Winding ends U2 V2 W2: a junction under each, the star
            // contactor straight below, the delta wire from the right at
            // its own height.
            let join_y = [250.0, 244.0, 238.0];
            for b in 0..3u16 {
                let j = s.junction(x(b), join_y[usize::from(b)])?;
                s.tx.connect(Endpoint::pin(m1s, 3 + b), Endpoint::junction(j))?;
                s.tx.connect(Endpoint::junction(j), Endpoint::pin(k3m, 2 * b))?;
                // Delta: U1-W2, V1-U2, W1-V2; pole q of the delta
                // contactor ends on winding (q + 2) % 3.
                let q = (b + 1) % 3;
                s.wire_via(
                    Endpoint::pin(k2m, 2 * q + 1),
                    Endpoint::junction(j),
                    &[(88.0 + 8.0 * f64::from(q), join_y[usize::from(b)])],
                )?;
            }
            // Star point: the star contactor's lower ends joined.
            s.wire((k3m, 1), (k3m, 3))?;
            s.wire((k3m, 3), (k3m, 5))?;

            // Control: stop, start with self-hold on the line contactor; the
            // timer starts with it, star runs first, delta follows.
            let thermal = start_stop(s, 140.0, [f3, s0, s1, k1, f2])?;
            let k1c = s.put("iec/coils/contactor", 140.0, 168.0, k1, true)?;
            let k4c = s.put("iec/coils/timer_on_delay", 164.0, 168.0, k4, true)?;
            let k4n = s.put("iec/contacts/nc_delay_on", 188.0, 168.0, k4, false)?;
            let k4o = s.put("iec/contacts/no_delay_on", 212.0, 168.0, k4, false)?;
            let k2i = s.put("iec/contacts/nc", 188.0, 192.0, k2, false)?;
            let k3i = s.put("iec/contacts/nc", 212.0, 192.0, k3, false)?;
            let k3c = s.put("iec/coils/contactor", 188.0, 216.0, k3, true)?;
            let k2c = s.put("iec/coils/contactor", 212.0, 216.0, k2, true)?;
            s.bus(
                160.0,
                &[(thermal, 1), (k1c, 0), (k4c, 0), (k4n, 0), (k4o, 0)],
            )?;
            s.wire((k4n, 1), (k2i, 0))?;
            s.wire((k2i, 1), (k3c, 0))?;
            s.wire((k4o, 1), (k3i, 0))?;
            s.wire((k3i, 1), (k2c, 0))?;
            s.bus(248.0, &[(k1c, 1), (k4c, 1), (k3c, 1), (k2c, 1)])?;
            Ok(())
        },
    )
}

/// Two emergency stops in series feeding a safety relay, with the relay's
/// feedback contact. Open points: supply above the first button, N below
/// the coil, the feedback contact's two ends.
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
            let s1s = s.put("iec/operators/emergency_stop", 40.0, 40.0, s1, true)?;
            let s2s = s.put("iec/operators/emergency_stop", 40.0, 72.0, s2, true)?;
            let k1c = s.put("iec/coils/relay", 40.0, 104.0, k1, true)?;
            s.put("iec/contacts/no", 72.0, 104.0, k1, false)?;
            s.wire((s1s, 1), (s2s, 0))?;
            s.wire((s2s, 1), (k1c, 0))?;
            Ok(())
        },
    )
}

/// Mains breaker, 24 V DC supply and output breaker. Open points: L and N
/// above the breaker, PE above the supply, the 0 V and 24 V outputs below.
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
            let f1s = s.put("iec/protection/mcb_2p", 40.0, 40.0, f1, true)?;
            let g1s = s.put("iec/supplies/power_supply_dc", 40.0, 72.0, g1, true)?;
            let f2s = s.put("iec/protection/mcb_1p", 40.0, 112.0, f2, true)?;
            s.wire((f1s, 1), (g1s, 0))?;
            s.wire((f1s, 3), (g1s, 1))?;
            s.wire((g1s, 3), (f2s, 0))?;
            s.pe_entry(64.0, 40.0, (g1s, 2))?;
            Ok(())
        },
    )
}
