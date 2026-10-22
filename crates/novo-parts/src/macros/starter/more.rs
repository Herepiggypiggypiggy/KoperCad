//! The second batch of starter macros: reversing, drives, brakes, crane
//! hoist, pump and heater circuits. Pins, bridges and pass-through paths come
//! from the symbol library, so a macro cannot drift from its symbols.

use novo_core::{DeviceId, Endpoint, JunctionId, Pin, Result, SymbolId, SymbolInstance, Value};
use novo_symbols::Library;

use super::{MacroFile, Sheet, capture, parameter};

/// Every macro of this batch, in the order they are listed.
pub(super) fn all() -> Vec<fn() -> Result<MacroFile>> {
    vec![
        reversing_starter,
        start_stop_latch,
        plc_input_card,
        vfd_drive,
        soft_starter,
        supply_230_24,
        motor_with_brake,
        contactor_indication,
        crane_hoist,
        pump_float,
        heater_thermostat,
    ]
}

/// A sheet plus the symbol library.
pub(super) struct Kit<'a, 'b, 'c> {
    pub(super) s: &'a mut Sheet<'b, 'c>,
    lib: Library,
}

impl<'a, 'b, 'c> Kit<'a, 'b, 'c> {
    pub(super) fn new(s: &'a mut Sheet<'b, 'c>) -> Self {
        Kit {
            s,
            lib: Library::builtin().expect("the built-in symbols load"),
        }
    }

    pub(super) fn dev(&mut self, tag: &str, description: &str) -> Result<DeviceId> {
        self.s.device(tag, description)
    }

    /// Places a symbol with the pins, bridges and pass-through paths of its
    /// library entry.
    pub(super) fn put(
        &mut self,
        key: &str,
        x: f64,
        y: f64,
        device: DeviceId,
        main: bool,
    ) -> Result<SymbolId> {
        let sym = self.lib.get(key).expect("starter symbols exist");
        let pins: Vec<Pin> = sym
            .connections
            .iter()
            .map(|c| Pin::new(c.id.as_str(), c.at[0], c.at[1]))
            .collect();
        let to_u16 = |groups: Vec<Vec<usize>>| -> Vec<Vec<u16>> {
            groups
                .into_iter()
                .map(|g| g.into_iter().map(|i| i as u16).collect())
                .collect()
        };
        let bridges = to_u16(sym.bridges());
        let passes: Vec<Vec<u16>> = to_u16(sym.pass_through().paths().to_vec())
            .into_iter()
            .filter(|g| !bridges.contains(g))
            .collect();
        let id = self.s.symbol(key, x, y, pins, device, main)?;
        if !bridges.is_empty() {
            self.s
                .tx
                .update::<SymbolInstance>(id, |i| i.bridges = bridges)?;
        }
        if !passes.is_empty() {
            self.s.passes(id, passes)?;
        }
        Ok(id)
    }

    pub(super) fn wire(&mut self, a: (SymbolId, u16), b: (SymbolId, u16)) -> Result<()> {
        self.s.wire(a, b)
    }

    pub(super) fn junction(&mut self, x: f64, y: f64) -> Result<JunctionId> {
        self.s.junction(x, y)
    }

    pub(super) fn pin_to_junction(&mut self, sym: SymbolId, pin: u16, j: JunctionId) -> Result<()> {
        self.s
            .tx
            .connect(Endpoint::pin(sym, pin), Endpoint::junction(j))?;
        Ok(())
    }

    pub(super) fn junction_to_pin(&mut self, j: JunctionId, sym: SymbolId, pin: u16) -> Result<()> {
        self.s
            .tx
            .connect(Endpoint::junction(j), Endpoint::pin(sym, pin))?;
        Ok(())
    }

    /// Wires three phases between two three-pole symbols: `from` pins
    /// 1, 3, 5 of the lower side to `to` pins 0, 2, 4 of the upper side.
    fn three_phase(&mut self, from: SymbolId, to: SymbolId) -> Result<()> {
        for p in 0..3u16 {
            self.wire((from, 2 * p + 1), (to, 2 * p))?;
        }
        Ok(())
    }

    /// Start button with a self-holding contact, optionally an interlock
    /// contact of the opposite direction, then the coil. `bus` feeds both the
    /// button and the holding contact.
    #[allow(clippy::too_many_arguments)]
    fn hold_branch(
        &mut self,
        x: f64,
        y: f64,
        bus: JunctionId,
        start: DeviceId,
        hold: DeviceId,
        interlock: Option<DeviceId>,
        coil: DeviceId,
        coil_key: &str,
    ) -> Result<SymbolId> {
        let s1 = self.put("iec/operators/pushbutton_no", x, y, start, true)?;
        let k_hold = self.put("iec/contacts/no", x + 16.0, y, hold, false)?;
        self.junction_to_pin(bus, s1, 0)?;
        self.junction_to_pin(bus, k_hold, 0)?;
        let join = self.junction(x, y + 24.0)?;
        self.pin_to_junction(s1, 1, join)?;
        self.pin_to_junction(k_hold, 1, join)?;
        let mut tail_y = y + 32.0;
        let mut upper: Option<(SymbolId, u16)> = None;
        if let Some(dev) = interlock {
            let il = self.put("iec/contacts/nc", x, tail_y, dev, false)?;
            self.junction_to_pin(join, il, 0)?;
            upper = Some((il, 1));
            tail_y += 24.0;
        }
        let c = self.put(coil_key, x, tail_y, coil, true)?;
        match upper {
            Some(from) => self.wire(from, (c, 0))?,
            None => self.junction_to_pin(join, c, 0)?,
        }
        Ok(c)
    }

    /// Fuses, two contactors and an overload relay feeding a motor, the
    /// second contactor swapping L1 and L3 to reverse.
    fn reversing_power(&mut self, devices: [DeviceId; 5], motor_key: &str) -> Result<()> {
        let [f1, k1, k2, f2, m1] = devices;
        let f1s = self.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
        let k1m = self.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
        let k2m = self.put("iec/contactors/main_3p", 80.0, 72.0, k2, false)?;
        let f2s = self.put("iec/protection/overload_relay", 40.0, 104.0, f2, true)?;
        let m1s = self.put(motor_key, 40.0, 150.0, m1, true)?;
        for p in 0..3u16 {
            self.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
            self.wire((f1s, 2 * p + 1), (k2m, 2 * (2 - p)))?;
            self.wire((k1m, 2 * p + 1), (f2s, 2 * p))?;
            self.wire((k2m, 2 * p + 1), (f2s, 2 * p))?;
            self.wire((f2s, 2 * p + 1), (m1s, p))?;
        }
        Ok(())
    }
}

/// Reversing starter: forward and reverse contactors, electrical interlock,
/// separate start buttons. Open ends: L1 L2 L3 above the fuses, the control
/// supply above the breaker, N below both coils, motor PE.
fn reversing_starter() -> Result<MacroFile> {
    capture(
        "Motor starter reversing",
        "Reversing motor starter with forward and reverse buttons, interlock and self-hold",
        vec![parameter("power", Value::Float(2.2), "Motor power in kW")],
        &[
            "motor",
            "starter",
            "reversing",
            "forward",
            "reverse",
            "contactor",
        ],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Main circuit fuses")?;
            let k1 = k.dev("-K1", "Forward contactor")?;
            let k2 = k.dev("-K2", "Reverse contactor")?;
            let f2 = k.dev("-F2", "Overload relay")?;
            let m1 = k.dev("-M1", "Motor {power} kW")?;
            let f3 = k.dev("-F3", "Control circuit breaker")?;
            let s0 = k.dev("-S0", "Stop")?;
            let s1 = k.dev("-S1", "Forward")?;
            let s2 = k.dev("-S2", "Reverse")?;
            k.s.describe(m1, "power_kw", "{power}")?;
            k.reversing_power([f1, k1, k2, f2, m1], "iec/motors/motor_3ph")?;

            let f3s = k.put("iec/protection/mcb_1p", 140.0, 40.0, f3, true)?;
            let s0s = k.put("iec/operators/pushbutton_nc", 140.0, 64.0, s0, true)?;
            let f2a = k.put("iec/contacts/thermal_nc", 140.0, 88.0, f2, false)?;
            k.wire((f3s, 1), (s0s, 0))?;
            k.wire((s0s, 1), (f2a, 0))?;
            let bus = k.junction(140.0, 112.0)?;
            k.pin_to_junction(f2a, 1, bus)?;
            k.hold_branch(
                160.0,
                128.0,
                bus,
                s1,
                k1,
                Some(k2),
                k1,
                "iec/coils/contactor",
            )?;
            k.hold_branch(
                208.0,
                128.0,
                bus,
                s2,
                k2,
                Some(k1),
                k2,
                "iec/coils/contactor",
            )?;
            Ok(())
        },
    )
}

/// Start/stop latch on a relay with a run lamp. Open ends: supply above the
/// breaker, N below the coil and the lamp.
fn start_stop_latch() -> Result<MacroFile> {
    capture(
        "Start/stop latch",
        "Relay latched by start and stop buttons with a run lamp",
        Vec::new(),
        &[
            "latch",
            "start",
            "stop",
            "self-hold",
            "relay",
            "lamp",
            "control",
        ],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Control circuit breaker")?;
            let s0 = k.dev("-S0", "Stop")?;
            let s1 = k.dev("-S1", "Start")?;
            let k1 = k.dev("-K1", "Latching relay")?;
            let h1 = k.dev("-H1", "Run lamp")?;
            let f1s = k.put("iec/protection/mcb_1p", 40.0, 40.0, f1, true)?;
            let sup = k.junction(40.0, 64.0)?;
            k.pin_to_junction(f1s, 1, sup)?;
            let s0s = k.put("iec/operators/pushbutton_nc", 40.0, 72.0, s0, true)?;
            k.junction_to_pin(sup, s0s, 0)?;
            let bus = k.junction(40.0, 96.0)?;
            k.pin_to_junction(s0s, 1, bus)?;
            k.hold_branch(40.0, 104.0, bus, s1, k1, None, k1, "iec/coils/relay")?;
            let lamp_contact = k.put("iec/contacts/no", 100.0, 72.0, k1, false)?;
            let h1s = k.put("iec/signalling/lamp_green", 100.0, 104.0, h1, true)?;
            k.junction_to_pin(sup, lamp_contact, 0)?;
            k.wire((lamp_contact, 1), (h1s, 0))?;
            Ok(())
        },
    )
}

/// 24 V PLC input card: the supply, four inputs and the field devices that
/// drive them. Open ends: L+ and M supply, M of the card, the sensor supply.
fn plc_input_card() -> Result<MacroFile> {
    capture(
        "PLC input card",
        "Four digital inputs on a 24 V PLC card with a push button, stop button, key switch and float switch",
        Vec::new(),
        &["plc", "input", "digital input", "i/o", "24 V", "sensor"],
        |s| {
            let mut k = Kit::new(s);
            let a1 = k.dev("-A1", "PLC supply 24 V")?;
            let a2 = k.dev("-A2", "Digital input card, 4 channels")?;
            let s1 = k.dev("-S1", "Start")?;
            let s2 = k.dev("-S2", "Stop")?;
            let s3 = k.dev("-S3", "Key switch")?;
            let b1 = k.dev("-B1", "Level float switch")?;
            let ps = k.put("iec/plc/power_supply", 40.0, 40.0, a1, true)?;
            let rail = k.junction(40.0, 64.0)?;
            k.pin_to_junction(ps, 0, rail)?;
            let fields: [(&str, DeviceId); 4] = [
                ("iec/operators/pushbutton_no", s1),
                ("iec/operators/pushbutton_nc", s2),
                ("iec/operators/selector_no", s3),
                ("iec/sensors/float_no", b1),
            ];
            for (i, (key, dev)) in fields.into_iter().enumerate() {
                let x = 40.0 + 24.0 * i as f64;
                let field = k.put(key, x, 72.0, dev, true)?;
                let di = k.put("iec/plc/digital_input", x, 112.0, a2, i == 0)?;
                k.junction_to_pin(rail, field, 0)?;
                k.wire((field, 1), (di, 0))?;
            }
            Ok(())
        },
    )
}

/// Frequency converter between a breaker and the motor. Open ends: L1 L2 L3
/// above the breaker, converter PE and motor PE.
fn vfd_drive() -> Result<MacroFile> {
    capture(
        "Frequency converter drive",
        "Motor fed from a frequency converter behind a three-pole breaker",
        vec![parameter("power", Value::Float(7.5), "Motor power in kW")],
        &["vfd", "frequency converter", "inverter", "drive", "motor"],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Drive input breaker")?;
            let t1 = k.dev("-T1", "Frequency converter {power} kW")?;
            let m1 = k.dev("-M1", "Motor {power} kW")?;
            k.s.describe(t1, "power_kw", "{power}")?;
            k.s.describe(m1, "power_kw", "{power}")?;
            let f1s = k.put("iec/protection/mcb_3p", 40.0, 40.0, f1, true)?;
            let t1s = k.put("iec/drives/frequency_converter", 40.0, 72.0, t1, true)?;
            let m1s = k.put("iec/motors/motor_3ph", 40.0, 120.0, m1, true)?;
            for p in 0..3u16 {
                k.wire((f1s, 2 * p + 1), (t1s, p))?;
                k.wire((t1s, 4 + p), (m1s, p))?;
            }
            Ok(())
        },
    )
}

/// Soft starter with a bypass contactor. Open ends: L1 L2 L3 above the
/// fuses, the bypass coil, motor PE.
fn soft_starter() -> Result<MacroFile> {
    capture(
        "Soft starter with bypass",
        "Motor soft starter behind fuses with a bypass contactor across its power section",
        vec![parameter("power", Value::Float(15.0), "Motor power in kW")],
        &["soft starter", "motor", "starter", "bypass", "contactor"],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Main circuit fuses")?;
            let t1 = k.dev("-T1", "Soft starter {power} kW")?;
            let k1 = k.dev("-K1", "Bypass contactor")?;
            let m1 = k.dev("-M1", "Motor {power} kW")?;
            k.s.describe(t1, "power_kw", "{power}")?;
            k.s.describe(m1, "power_kw", "{power}")?;
            let f1s = k.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
            let t1s = k.put("iec/drives/soft_starter", 40.0, 72.0, t1, true)?;
            let k1m = k.put("iec/contactors/main_3p", 80.0, 80.0, k1, false)?;
            let m1s = k.put("iec/motors/motor_3ph", 40.0, 120.0, m1, true)?;
            for p in 0..3u16 {
                k.wire((f1s, 2 * p + 1), (t1s, p))?;
                k.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
                k.wire((t1s, 3 + p), (m1s, p))?;
                k.wire((k1m, 2 * p + 1), (m1s, p))?;
            }
            k.put("iec/coils/contactor", 130.0, 80.0, k1, true)?;
            Ok(())
        },
    )
}

/// Single-phase 230 V incomer with residual current protection, a 24 V
/// supply and a service socket. Open ends: L, N, PE above the incomer, the
/// 24 V and 0 V rails, the socket PE.
fn supply_230_24() -> Result<MacroFile> {
    capture(
        "Single-phase supply with 24 V",
        "230 V incomer with RCBO, a 24 V DC supply with output breaker and a service socket",
        vec![parameter(
            "voltage",
            Value::Int(24),
            "Output voltage in V DC",
        )],
        &[
            "supply",
            "230 V",
            "single phase",
            "24 V",
            "rcbo",
            "socket",
            "power supply",
        ],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Incomer RCBO")?;
            let g1 = k.dev("-G1", "Power supply {voltage} V DC")?;
            let f2 = k.dev("-F2", "Output breaker")?;
            let f3 = k.dev("-F3", "Socket breaker")?;
            let x1 = k.dev("-X1", "Service socket")?;
            k.s.describe(g1, "output_voltage_v", "{voltage}")?;
            let f1s = k.put("iec/protection/rcbo_2p", 40.0, 40.0, f1, true)?;
            let g1s = k.put("iec/supplies/power_supply_dc", 40.0, 88.0, g1, true)?;
            let f2s = k.put("iec/protection/mcb_1p", 40.0, 128.0, f2, true)?;
            let f3s = k.put("iec/protection/mcb_1p_n", 100.0, 88.0, f3, true)?;
            let x1s = k.put("iec/terminals/socket_outlet", 100.0, 128.0, x1, true)?;
            // The incomer's outputs 2 and N' feed both branches.
            let live = k.junction(40.0, 70.0)?;
            let neutral = k.junction(48.0, 74.0)?;
            k.pin_to_junction(f1s, 1, live)?;
            k.pin_to_junction(f1s, 3, neutral)?;
            k.junction_to_pin(live, g1s, 0)?;
            k.junction_to_pin(neutral, g1s, 1)?;
            k.junction_to_pin(live, f3s, 0)?;
            k.junction_to_pin(neutral, f3s, 2)?;
            k.wire((g1s, 3), (f2s, 0))?;
            k.wire((f3s, 1), (x1s, 0))?;
            k.wire((f3s, 3), (x1s, 1))?;
            Ok(())
        },
    )
}

/// DOL starter whose brake coil is released by the contactor. Open ends: as
/// the DOL starter, plus the brake coil return.
fn motor_with_brake() -> Result<MacroFile> {
    capture(
        "Motor with brake",
        "Direct-on-line motor starter whose holding brake is released through a contactor contact",
        vec![parameter("power", Value::Float(1.5), "Motor power in kW")],
        &["motor", "brake", "starter", "holding brake", "contactor"],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Main circuit fuses")?;
            let k1 = k.dev("-K1", "Motor contactor")?;
            let f2 = k.dev("-F2", "Overload relay")?;
            let m1 = k.dev("-M1", "Motor {power} kW")?;
            let y1 = k.dev("-Y1", "Motor brake")?;
            let f3 = k.dev("-F3", "Control circuit breaker")?;
            let s0 = k.dev("-S0", "Stop")?;
            let s1 = k.dev("-S1", "Start")?;
            k.s.describe(m1, "power_kw", "{power}")?;
            let f1s = k.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
            let k1m = k.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let f2s = k.put("iec/protection/overload_relay", 40.0, 104.0, f2, true)?;
            let m1s = k.put("iec/motors/motor_3ph", 40.0, 150.0, m1, true)?;
            for p in 0..3u16 {
                k.wire((f1s, 2 * p + 1), (k1m, 2 * p))?;
                k.wire((k1m, 2 * p + 1), (f2s, 2 * p))?;
                k.wire((f2s, 2 * p + 1), (m1s, p))?;
            }
            let f3s = k.put("iec/protection/mcb_1p", 100.0, 40.0, f3, true)?;
            let sup = k.junction(100.0, 64.0)?;
            k.pin_to_junction(f3s, 1, sup)?;
            let s0s = k.put("iec/operators/pushbutton_nc", 100.0, 72.0, s0, true)?;
            k.junction_to_pin(sup, s0s, 0)?;
            let f2a = k.put("iec/contacts/thermal_nc", 100.0, 96.0, f2, false)?;
            k.wire((s0s, 1), (f2a, 0))?;
            let bus = k.junction(100.0, 120.0)?;
            k.pin_to_junction(f2a, 1, bus)?;
            k.hold_branch(100.0, 128.0, bus, s1, k1, None, k1, "iec/coils/contactor")?;
            let release = k.put("iec/contacts/no", 160.0, 72.0, k1, false)?;
            let brake = k.put("iec/coils/brake", 160.0, 104.0, y1, true)?;
            k.junction_to_pin(sup, release, 0)?;
            k.wire((release, 1), (brake, 0))?;
            Ok(())
        },
    )
}

/// Contactor switched by a selector, with running and stopped lamps from its
/// auxiliary contacts. Open ends: L1 L2 L3 and the load below the main
/// contacts, the control supply above the breaker, N below coil and lamps.
fn contactor_indication() -> Result<MacroFile> {
    capture(
        "Contactor with indication",
        "Contactor switched by a selector, with a green running lamp and a red stopped lamp from its auxiliary contacts",
        Vec::new(),
        &[
            "contactor",
            "auxiliary",
            "lamp",
            "indication",
            "signal lamp",
            "selector",
        ],
        |s| {
            let mut k = Kit::new(s);
            let k1 = k.dev("-K1", "Contactor")?;
            let f1 = k.dev("-F1", "Control circuit breaker")?;
            let s1 = k.dev("-S1", "On/off selector")?;
            let h1 = k.dev("-H1", "Running lamp")?;
            let h2 = k.dev("-H2", "Stopped lamp")?;
            k.put("iec/contactors/main_3p", 40.0, 40.0, k1, false)?;
            let f1s = k.put("iec/protection/mcb_1p", 100.0, 40.0, f1, true)?;
            let sup = k.junction(100.0, 64.0)?;
            k.pin_to_junction(f1s, 1, sup)?;
            let s1s = k.put("iec/operators/selector_no", 100.0, 72.0, s1, true)?;
            k.junction_to_pin(sup, s1s, 0)?;
            let coil = k.put("iec/coils/contactor", 100.0, 104.0, k1, true)?;
            k.wire((s1s, 1), (coil, 0))?;
            let run = k.put("iec/contacts/no", 140.0, 72.0, k1, false)?;
            let stopped = k.put("iec/contacts/nc", 180.0, 72.0, k1, false)?;
            let green = k.put("iec/signalling/lamp_green", 140.0, 104.0, h1, true)?;
            let red = k.put("iec/signalling/lamp_red", 180.0, 104.0, h2, true)?;
            k.junction_to_pin(sup, run, 0)?;
            k.junction_to_pin(sup, stopped, 0)?;
            k.wire((run, 1), (green, 0))?;
            k.wire((stopped, 1), (red, 0))?;
            Ok(())
        },
    )
}

/// Crane hoist: up and down contactors, hold-to-run buttons, end limits,
/// interlock and a brake released while either contactor is in. Open ends:
/// L1 L2 L3 above the fuses, the control supply, N below coils and brake,
/// motor PE.
fn crane_hoist() -> Result<MacroFile> {
    capture(
        "Crane hoist up/down",
        "Hoist motor with up and down contactors, hold-to-run buttons, end limit switches, emergency stop and a brake released by either contactor",
        vec![parameter("power", Value::Float(5.5), "Motor power in kW")],
        &[
            "crane",
            "hoist",
            "lift",
            "up",
            "down",
            "limit switch",
            "brake",
            "reversing",
        ],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Main circuit fuses")?;
            let k1 = k.dev("-K1", "Hoist up contactor")?;
            let k2 = k.dev("-K2", "Hoist down contactor")?;
            let f2 = k.dev("-F2", "Overload relay")?;
            let m1 = k.dev("-M1", "Hoist motor {power} kW")?;
            let y1 = k.dev("-Y1", "Hoist brake")?;
            let f3 = k.dev("-F3", "Control circuit breaker")?;
            let s0 = k.dev("-S0", "Emergency stop")?;
            let s1 = k.dev("-S1", "Up")?;
            let s2 = k.dev("-S2", "Down")?;
            let b1 = k.dev("-B1", "Upper end limit")?;
            let b2 = k.dev("-B2", "Lower end limit")?;
            k.s.describe(m1, "power_kw", "{power}")?;
            k.reversing_power([f1, k1, k2, f2, m1], "iec/motors/motor_3ph")?;

            let f3s = k.put("iec/protection/mcb_1p", 140.0, 40.0, f3, true)?;
            let s0s = k.put("iec/operators/emergency_stop", 140.0, 64.0, s0, true)?;
            let f2a = k.put("iec/contacts/thermal_nc", 140.0, 88.0, f2, false)?;
            k.wire((f3s, 1), (s0s, 0))?;
            k.wire((s0s, 1), (f2a, 0))?;
            let bus = k.junction(140.0, 112.0)?;
            k.pin_to_junction(f2a, 1, bus)?;
            // Up: button, upper limit, down interlock, up coil.
            for (x, button, limit, other, coil) in
                [(160.0, s1, b1, k2, k1), (200.0, s2, b2, k1, k2)]
            {
                let bs = k.put("iec/operators/pushbutton_no", x, 128.0, button, true)?;
                let ls = k.put("iec/contacts/position_nc", x, 152.0, limit, true)?;
                let il = k.put("iec/contacts/nc", x, 176.0, other, false)?;
                let cs = k.put("iec/coils/contactor", x, 200.0, coil, true)?;
                k.junction_to_pin(bus, bs, 0)?;
                k.wire((bs, 1), (ls, 0))?;
                k.wire((ls, 1), (il, 0))?;
                k.wire((il, 1), (cs, 0))?;
            }
            // Brake: released while either contactor is in.
            let up_aux = k.put("iec/contacts/no", 240.0, 128.0, k1, false)?;
            let down_aux = k.put("iec/contacts/no", 256.0, 128.0, k2, false)?;
            k.junction_to_pin(bus, up_aux, 0)?;
            k.junction_to_pin(bus, down_aux, 0)?;
            let join = k.junction(240.0, 152.0)?;
            k.pin_to_junction(up_aux, 1, join)?;
            k.pin_to_junction(down_aux, 1, join)?;
            let brake = k.put("iec/coils/brake", 240.0, 164.0, y1, true)?;
            k.junction_to_pin(join, brake, 0)?;
            Ok(())
        },
    )
}

/// Pump on a motor protection breaker, run by hand or by a float switch.
/// Open ends: L1 L2 L3, the control supply, N below the coil, motor PE.
fn pump_float() -> Result<MacroFile> {
    capture(
        "Pump with float switch",
        "Pump motor on a motor protection breaker and contactor, started by hand or by a float switch",
        vec![parameter(
            "power",
            Value::Float(1.1),
            "Pump motor power in kW",
        )],
        &["pump", "float", "level", "motor", "hand", "auto", "water"],
        |s| {
            let mut k = Kit::new(s);
            let q1 = k.dev("-Q1", "Motor protection breaker")?;
            let k1 = k.dev("-K1", "Pump contactor")?;
            let m1 = k.dev("-M1", "Pump motor {power} kW")?;
            let f1 = k.dev("-F1", "Control circuit breaker")?;
            let s1 = k.dev("-S1", "Hand run")?;
            let b1 = k.dev("-B1", "Level float switch")?;
            k.s.describe(m1, "power_kw", "{power}")?;
            let q1s = k.put(
                "iec/protection/motor_protection_breaker",
                40.0,
                40.0,
                q1,
                true,
            )?;
            let k1m = k.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let m1s = k.put("iec/motors/motor_3ph", 40.0, 112.0, m1, true)?;
            k.three_phase(q1s, k1m)?;
            for p in 0..3u16 {
                k.wire((k1m, 2 * p + 1), (m1s, p))?;
            }
            let f1s = k.put("iec/protection/mcb_1p", 100.0, 40.0, f1, true)?;
            let sup = k.junction(100.0, 64.0)?;
            k.pin_to_junction(f1s, 1, sup)?;
            let hand = k.put("iec/operators/selector_no", 100.0, 72.0, s1, true)?;
            let float = k.put("iec/sensors/float_no", 124.0, 72.0, b1, true)?;
            k.junction_to_pin(sup, hand, 0)?;
            k.junction_to_pin(sup, float, 0)?;
            let join = k.junction(100.0, 96.0)?;
            k.pin_to_junction(hand, 1, join)?;
            k.pin_to_junction(float, 1, join)?;
            let coil = k.put("iec/coils/contactor", 100.0, 108.0, k1, true)?;
            k.junction_to_pin(join, coil, 0)?;
            Ok(())
        },
    )
}

/// Three-phase heater on a contactor behind a control thermostat and a
/// safety limit thermostat. Open ends: L1 L2 L3, the control supply, N below
/// the coil, the heater star point.
fn heater_thermostat() -> Result<MacroFile> {
    capture(
        "Heater with thermostat",
        "Three-phase heater switched by a thermostat with an independent high-limit thermostat",
        vec![parameter("power", Value::Float(6.0), "Heater power in kW")],
        &[
            "heater",
            "heating",
            "thermostat",
            "temperature",
            "limit",
            "contactor",
        ],
        |s| {
            let mut k = Kit::new(s);
            let f1 = k.dev("-F1", "Heater breaker")?;
            let k1 = k.dev("-K1", "Heater contactor")?;
            let e1 = k.dev("-E1", "Heater {power} kW")?;
            let f2 = k.dev("-F2", "Control circuit breaker")?;
            let b1 = k.dev("-B1", "Control thermostat")?;
            let b2 = k.dev("-B2", "High-limit thermostat")?;
            k.s.describe(e1, "power_kw", "{power}")?;
            let f1s = k.put("iec/protection/mcb_3p", 40.0, 40.0, f1, true)?;
            let k1m = k.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let e1s = k.put("iec/passive/heater_3ph", 40.0, 112.0, e1, true)?;
            k.three_phase(f1s, k1m)?;
            for p in 0..3u16 {
                k.wire((k1m, 2 * p + 1), (e1s, p))?;
            }
            let f2s = k.put("iec/protection/mcb_1p", 100.0, 40.0, f2, true)?;
            let t1 = k.put("iec/sensors/temperature_no", 100.0, 72.0, b1, true)?;
            let t2 = k.put("iec/sensors/temperature_nc", 100.0, 96.0, b2, true)?;
            let coil = k.put("iec/coils/contactor", 100.0, 120.0, k1, true)?;
            k.wire((f2s, 1), (t1, 0))?;
            k.wire((t1, 1), (t2, 0))?;
            k.wire((t2, 1), (coil, 0))?;
            Ok(())
        },
    )
}
