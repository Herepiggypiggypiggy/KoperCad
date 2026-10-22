//! The second batch of starter macros: supply, reversing, drives, brakes,
//! crane hoist, pump and heater circuits. Pins, bridges and pass-through
//! paths come from the symbol library, so a macro cannot drift from its
//! symbols. Where several wires meet, a bus of junctions keeps each wire on
//! its own stretch of line.

use novo_core::{DeviceId, Endpoint, Result, SymbolId, Value};

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
        three_phase_supply,
    ]
}

type Pin = (SymbolId, u16);

/// Wires three phases between two three-pole symbols: `from` pins
/// 1, 3, 5 of the lower side to `to` pins 0, 2, 4 of the upper side.
fn three_phase(s: &mut Sheet<'_, '_>, from: SymbolId, to: SymbolId) -> Result<()> {
    for p in 0..3u16 {
        s.wire((from, 2 * p + 1), (to, 2 * p))?;
    }
    Ok(())
}

/// A start button with a self-holding contact and, optionally, an interlock
/// contact of the opposite direction, then the coil.
struct Branch {
    /// The pins that hang from the feeding bus.
    tops: [Pin; 2],
    coil: SymbolId,
}

/// Draws a start button in parallel with a self-holding contact at (`x`,
/// `y`), then the optional interlock and the coil. The pins that take the
/// supply are returned for the caller's bus.
#[allow(clippy::too_many_arguments)]
fn hold_branch(
    s: &mut Sheet<'_, '_>,
    x: f64,
    y: f64,
    start: DeviceId,
    hold: DeviceId,
    interlock: Option<DeviceId>,
    coil: DeviceId,
    coil_key: &str,
) -> Result<Branch> {
    let s1 = s.put("iec/operators/pushbutton_no", x, y, start, true)?;
    let k_hold = s.put("iec/contacts/no", x + 16.0, y, hold, false)?;
    let mut tail_y = y + 32.0;
    let mut upper: Option<Pin> = None;
    if let Some(dev) = interlock {
        let il = s.put("iec/contacts/nc", x, tail_y, dev, false)?;
        upper = Some((il, 1));
        tail_y += 24.0;
    }
    let c = s.put(coil_key, x, tail_y, coil, true)?;
    let mut join = vec![(s1, 1), (k_hold, 1)];
    match upper {
        Some(from) => {
            join.push((from.0, 0));
            s.bus(y + 24.0, &join)?;
            s.wire(from, (c, 0))?;
        }
        None => {
            join.push((c, 0));
            s.bus(y + 24.0, &join)?;
        }
    }
    Ok(Branch {
        tops: [(s1, 0), (k_hold, 0)],
        coil: c,
    })
}

/// Fuses, two contactors and an overload relay feeding a motor, the second
/// contactor swapping L1 and L3 to reverse. The phases fan out and in
/// through junctions at staggered heights, so no two wires share a stretch.
/// Returns the motor.
fn reversing_power(
    s: &mut Sheet<'_, '_>,
    devices: [DeviceId; 5],
    motor_key: &str,
) -> Result<SymbolId> {
    let [f1, k1, k2, f2, m1] = devices;
    let f1s = s.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
    let k1m = s.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
    let k2m = s.put("iec/contactors/main_3p", 80.0, 72.0, k2, false)?;
    let f2s = s.put("iec/protection/overload_relay", 40.0, 104.0, f2, true)?;
    let m1s = s.put(motor_key, 40.0, 150.0, m1, true)?;
    let x = |p: u16| 40.0 + 8.0 * f64::from(p);
    for p in 0..3u16 {
        let i = usize::from(p);
        // In: the fuse pole feeds forward straight and reverse swapped.
        let up = s.junction(x(p), [60.0, 64.0, 68.0][2 - i])?;
        s.tx.connect(Endpoint::pin(f1s, 2 * p + 1), Endpoint::junction(up))?;
        s.tx.connect(Endpoint::junction(up), Endpoint::pin(k1m, 2 * p))?;
        s.tx.connect(Endpoint::junction(up), Endpoint::pin(k2m, 2 * (2 - p)))?;
        // Out: both contactors end on the same overload pole.
        let down = s.junction(x(p), [92.0, 96.0, 100.0][i])?;
        s.tx.connect(Endpoint::pin(k1m, 2 * p + 1), Endpoint::junction(down))?;
        s.tx.connect(Endpoint::junction(down), Endpoint::pin(f2s, 2 * p))?;
        s.tx.connect(Endpoint::pin(k2m, 2 * p + 1), Endpoint::junction(down))?;
        s.wire((f2s, 2 * p + 1), (m1s, p))?;
    }
    Ok(m1s)
}

/// Reversing starter: forward and reverse contactors, electrical interlock,
/// separate start buttons. Open points: L1 L2 L3 and PE above the fuses,
/// the control supply above the breaker, N below both coils.
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
            let f1 = s.device("-F1", "Main circuit fuses")?;
            let k1 = s.device("-K1", "Forward contactor")?;
            let k2 = s.device("-K2", "Reverse contactor")?;
            let f2 = s.device("-F2", "Overload relay")?;
            let m1 = s.device("-M1", "Motor {power} kW")?;
            let f3 = s.device("-F3", "Control circuit breaker")?;
            let s0 = s.device("-S0", "Stop")?;
            let s1 = s.device("-S1", "Forward")?;
            let s2 = s.device("-S2", "Reverse")?;
            s.describe(m1, "power_kw", "{power}")?;
            let m1s = reversing_power(s, [f1, k1, k2, f2, m1], "iec/motors/motor_3ph")?;
            s.pe_entry(104.0, 40.0, (m1s, 3))?;

            let f3s = s.put("iec/protection/mcb_1p", 140.0, 40.0, f3, true)?;
            let s0s = s.put("iec/operators/pushbutton_nc", 140.0, 64.0, s0, true)?;
            let f2a = s.put("iec/contacts/thermal_nc", 140.0, 88.0, f2, false)?;
            s.wire((f3s, 1), (s0s, 0))?;
            s.wire((s0s, 1), (f2a, 0))?;
            let fwd = hold_branch(s, 160.0, 128.0, s1, k1, Some(k2), k1, "iec/coils/contactor")?;
            let rev = hold_branch(s, 208.0, 128.0, s2, k2, Some(k1), k2, "iec/coils/contactor")?;
            s.bus(
                112.0,
                &[(f2a, 1), fwd.tops[0], fwd.tops[1], rev.tops[0], rev.tops[1]],
            )?;
            s.bus(216.0, &[(fwd.coil, 1), (rev.coil, 1)])?;
            Ok(())
        },
    )
}

/// Start/stop latch on a relay with a run lamp. Open points: supply above
/// the breaker, N below the relay and the lamp rail.
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
            let f1 = s.device("-F1", "Control circuit breaker")?;
            let s0 = s.device("-S0", "Stop")?;
            let s1 = s.device("-S1", "Start")?;
            let k1 = s.device("-K1", "Latching relay")?;
            let h1 = s.device("-H1", "Run lamp")?;
            let f1s = s.put("iec/protection/mcb_1p", 40.0, 40.0, f1, true)?;
            let s0s = s.put("iec/operators/pushbutton_nc", 40.0, 72.0, s0, true)?;
            let lamp_contact = s.put("iec/contacts/no", 100.0, 72.0, k1, false)?;
            s.number(lamp_contact, &["23", "24"])?;
            let h1s = s.put("iec/signalling/lamp_green", 100.0, 104.0, h1, true)?;
            s.bus(64.0, &[(f1s, 1), (s0s, 0), (lamp_contact, 0)])?;
            s.wire((lamp_contact, 1), (h1s, 0))?;
            let branch = hold_branch(s, 40.0, 104.0, s1, k1, None, k1, "iec/coils/relay")?;
            s.bus(96.0, &[(s0s, 1), branch.tops[0], branch.tops[1]])?;
            s.bus(168.0, &[(branch.coil, 1), (h1s, 1)])?;
            Ok(())
        },
    )
}

/// 24 V PLC input card: the supply, four inputs and the field devices that
/// drive them. Open points: L+ and M above the card supply.
fn plc_input_card() -> Result<MacroFile> {
    capture(
        "PLC input card",
        "Four digital inputs on a 24 V PLC card with a push button, stop button, key switch and float switch",
        Vec::new(),
        &["plc", "input", "digital input", "i/o", "24 V", "sensor"],
        |s| {
            let a1 = s.device("-A1", "PLC supply 24 V")?;
            let a2 = s.device("-A2", "Digital input card, 4 channels")?;
            let s1 = s.device("-S1", "Start")?;
            let s2 = s.device("-S2", "Stop")?;
            let s3 = s.device("-S3", "Key switch")?;
            let b1 = s.device("-B1", "Level float switch")?;
            let ps = s.put("iec/plc/power_supply", 40.0, 40.0, a1, true)?;
            let fields: [(&str, DeviceId); 4] = [
                ("iec/operators/pushbutton_no", s1),
                ("iec/operators/pushbutton_nc", s2),
                ("iec/operators/selector_no", s3),
                ("iec/sensors/float_no", b1),
            ];
            let mut taps: Vec<Pin> = Vec::new();
            for (i, (key, dev)) in fields.into_iter().enumerate() {
                let x = 40.0 + 24.0 * i as f64;
                let field = s.put(key, x, 72.0, dev, true)?;
                let di = s.put("iec/plc/digital_input", x, 112.0, a2, i == 0)?;
                s.wire((field, 1), (di, 0))?;
                taps.push((field, 0));
            }
            // The sensor supply is the card's L+, taken round the card.
            let rail = s.bus(64.0, &taps)?;
            s.wire_via(
                Endpoint::pin(ps, 0),
                Endpoint::junction(rail[0]),
                &[(40.0, 34.0), (16.0, 34.0), (16.0, 64.0)],
            )?;
            Ok(())
        },
    )
}

/// Frequency converter between a breaker and the motor. Open points: L1 L2
/// L3 above the breaker, PE above the converter.
fn vfd_drive() -> Result<MacroFile> {
    capture(
        "Frequency converter drive",
        "Motor fed from a frequency converter behind a three-pole breaker",
        vec![parameter("power", Value::Float(7.5), "Motor power in kW")],
        &["vfd", "frequency converter", "inverter", "drive", "motor"],
        |s| {
            let f1 = s.device("-F1", "Drive input breaker")?;
            let t1 = s.device("-T1", "Frequency converter {power} kW")?;
            let m1 = s.device("-M1", "Motor {power} kW")?;
            s.describe(t1, "power_kw", "{power}")?;
            s.describe(m1, "power_kw", "{power}")?;
            let f1s = s.put("iec/protection/mcb_3p", 40.0, 40.0, f1, true)?;
            let t1s = s.put("iec/drives/frequency_converter", 40.0, 72.0, t1, true)?;
            let m1s = s.put("iec/motors/motor_3ph", 40.0, 120.0, m1, true)?;
            for p in 0..3u16 {
                s.wire((f1s, 2 * p + 1), (t1s, p))?;
                s.wire((t1s, 4 + p), (m1s, p))?;
            }
            // Protective earth: through the converter's PE to the motor.
            s.pe_entry(64.0, 40.0, (t1s, 3))?;
            s.wire((t1s, 7), (m1s, 3))?;
            Ok(())
        },
    )
}

/// Soft starter with a bypass contactor. Open points: L1 L2 L3 and PE above
/// the fuses, the bypass coil's two ends.
fn soft_starter() -> Result<MacroFile> {
    capture(
        "Soft starter with bypass",
        "Motor soft starter behind fuses with a bypass contactor across its power section",
        vec![parameter("power", Value::Float(15.0), "Motor power in kW")],
        &["soft starter", "motor", "starter", "bypass", "contactor"],
        |s| {
            let f1 = s.device("-F1", "Main circuit fuses")?;
            let t1 = s.device("-T1", "Soft starter {power} kW")?;
            let k1 = s.device("-K1", "Bypass contactor")?;
            let m1 = s.device("-M1", "Motor {power} kW")?;
            s.describe(t1, "power_kw", "{power}")?;
            s.describe(m1, "power_kw", "{power}")?;
            let f1s = s.put("iec/protection/fuse_3p", 40.0, 40.0, f1, true)?;
            let t1s = s.put("iec/drives/soft_starter", 40.0, 72.0, t1, true)?;
            let k1m = s.put("iec/contactors/main_3p", 80.0, 80.0, k1, false)?;
            let m1s = s.put("iec/motors/motor_3ph", 40.0, 120.0, m1, true)?;
            let x = |p: u16| 40.0 + 8.0 * f64::from(p);
            for p in 0..3u16 {
                let i = usize::from(p);
                // In: the fuse pole feeds the soft starter and the bypass.
                let up = s.junction(x(p), [68.0, 64.0, 60.0][i])?;
                s.tx.connect(Endpoint::pin(f1s, 2 * p + 1), Endpoint::junction(up))?;
                s.tx.connect(Endpoint::junction(up), Endpoint::pin(t1s, p))?;
                s.tx.connect(Endpoint::junction(up), Endpoint::pin(k1m, 2 * p))?;
                // Out: the soft starter and the bypass meet on the motor.
                let down = s.junction(x(p), [100.0, 104.0, 108.0][i])?;
                s.tx.connect(Endpoint::pin(t1s, 3 + p), Endpoint::junction(down))?;
                s.tx.connect(Endpoint::junction(down), Endpoint::pin(m1s, p))?;
                s.tx.connect(Endpoint::pin(k1m, 2 * p + 1), Endpoint::junction(down))?;
            }
            s.pe_entry(64.0, 40.0, (m1s, 3))?;
            s.put("iec/coils/contactor", 130.0, 80.0, k1, true)?;
            Ok(())
        },
    )
}

/// Single-phase 230 V incomer with residual current protection, a 24 V
/// supply and a service socket. Open points: L and N above the incomer, PE
/// above the supply and above the socket, the 24 V and 0 V rails below.
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
            let f1 = s.device("-F1", "Incomer RCBO")?;
            let g1 = s.device("-G1", "Power supply {voltage} V DC")?;
            let f2 = s.device("-F2", "Output breaker")?;
            let f3 = s.device("-F3", "Socket breaker")?;
            let x1 = s.device("-X1", "Service socket")?;
            s.describe(g1, "output_voltage_v", "{voltage}")?;
            let f1s = s.put("iec/protection/rcbo_2p", 40.0, 40.0, f1, true)?;
            let g1s = s.put("iec/supplies/power_supply_dc", 40.0, 88.0, g1, true)?;
            let f2s = s.put("iec/protection/mcb_1p", 40.0, 128.0, f2, true)?;
            let f3s = s.put("iec/protection/mcb_1p_n", 100.0, 88.0, f3, true)?;
            let x1s = s.put("iec/terminals/socket_outlet", 100.0, 128.0, x1, true)?;
            // The incomer's outputs 2 and N' feed both branches.
            s.bus(72.0, &[(f1s, 1), (g1s, 0), (f3s, 0)])?;
            s.bus(76.0, &[(f1s, 3), (g1s, 1), (f3s, 2)])?;
            s.wire((g1s, 3), (f2s, 0))?;
            s.wire((f3s, 1), (x1s, 0))?;
            s.wire((f3s, 3), (x1s, 1))?;
            s.pe_entry(64.0, 40.0, (g1s, 2))?;
            s.pe_entry(124.0, 88.0, (x1s, 2))?;
            Ok(())
        },
    )
}

/// DOL starter whose brake coil is released by the contactor. Open points:
/// as the DOL starter.
fn motor_with_brake() -> Result<MacroFile> {
    capture(
        "Motor with brake",
        "Direct-on-line motor starter whose holding brake is released through a contactor contact",
        vec![parameter("power", Value::Float(1.5), "Motor power in kW")],
        &["motor", "brake", "starter", "holding brake", "contactor"],
        |s| {
            let f1 = s.device("-F1", "Main circuit fuses")?;
            let k1 = s.device("-K1", "Motor contactor")?;
            let f2 = s.device("-F2", "Overload relay")?;
            let m1 = s.device("-M1", "Motor {power} kW")?;
            let y1 = s.device("-Y1", "Motor brake")?;
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

            let f3s = s.put("iec/protection/mcb_1p", 100.0, 40.0, f3, true)?;
            let s0s = s.put("iec/operators/pushbutton_nc", 100.0, 72.0, s0, true)?;
            let f2a = s.put("iec/contacts/thermal_nc", 100.0, 96.0, f2, false)?;
            let release = s.put("iec/contacts/no", 160.0, 72.0, k1, false)?;
            s.number(release, &["23", "24"])?;
            let brake = s.put("iec/coils/brake", 160.0, 104.0, y1, true)?;
            s.bus(64.0, &[(f3s, 1), (s0s, 0), (release, 0)])?;
            s.wire((s0s, 1), (f2a, 0))?;
            s.wire((release, 1), (brake, 0))?;
            let branch = hold_branch(s, 100.0, 128.0, s1, k1, None, k1, "iec/coils/contactor")?;
            s.bus(120.0, &[(f2a, 1), branch.tops[0], branch.tops[1]])?;
            s.bus(192.0, &[(branch.coil, 1), (brake, 1)])?;
            Ok(())
        },
    )
}

/// Contactor switched by a selector, with running and stopped lamps from its
/// auxiliary contacts. Open points: L1 L2 L3 and the load below the main
/// contacts, the control supply above the breaker, N below the rail.
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
            let k1 = s.device("-K1", "Contactor")?;
            let f1 = s.device("-F1", "Control circuit breaker")?;
            let s1 = s.device("-S1", "On/off selector")?;
            let h1 = s.device("-H1", "Running lamp")?;
            let h2 = s.device("-H2", "Stopped lamp")?;
            s.put("iec/contactors/main_3p", 40.0, 40.0, k1, false)?;
            let f1s = s.put("iec/protection/mcb_1p", 100.0, 40.0, f1, true)?;
            let s1s = s.put("iec/operators/selector_no", 100.0, 72.0, s1, true)?;
            let coil = s.put("iec/coils/contactor", 100.0, 104.0, k1, true)?;
            s.wire((s1s, 1), (coil, 0))?;
            let run = s.put("iec/contacts/no", 140.0, 72.0, k1, false)?;
            let stopped = s.put("iec/contacts/nc", 180.0, 72.0, k1, false)?;
            let green = s.put("iec/signalling/lamp_green", 140.0, 104.0, h1, true)?;
            let red = s.put("iec/signalling/lamp_red", 180.0, 104.0, h2, true)?;
            s.bus(64.0, &[(f1s, 1), (s1s, 0), (run, 0), (stopped, 0)])?;
            s.wire((run, 1), (green, 0))?;
            s.wire((stopped, 1), (red, 0))?;
            s.bus(136.0, &[(coil, 1), (green, 1), (red, 1)])?;
            Ok(())
        },
    )
}

/// Crane hoist: up and down contactors, hold-to-run buttons, end limits,
/// interlock and a brake released while either contactor is in. Open points:
/// L1 L2 L3 and PE above the fuses, the control supply, N below the rail.
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
            let f1 = s.device("-F1", "Main circuit fuses")?;
            let k1 = s.device("-K1", "Hoist up contactor")?;
            let k2 = s.device("-K2", "Hoist down contactor")?;
            let f2 = s.device("-F2", "Overload relay")?;
            let m1 = s.device("-M1", "Hoist motor {power} kW")?;
            let y1 = s.device("-Y1", "Hoist brake")?;
            let f3 = s.device("-F3", "Control circuit breaker")?;
            let s0 = s.device("-S0", "Emergency stop")?;
            let s1 = s.device("-S1", "Up")?;
            let s2 = s.device("-S2", "Down")?;
            let b1 = s.device("-B1", "Upper end limit")?;
            let b2 = s.device("-B2", "Lower end limit")?;
            s.describe(m1, "power_kw", "{power}")?;
            let m1s = reversing_power(s, [f1, k1, k2, f2, m1], "iec/motors/motor_3ph")?;
            s.pe_entry(104.0, 40.0, (m1s, 3))?;

            let f3s = s.put("iec/protection/mcb_1p", 140.0, 40.0, f3, true)?;
            let s0s = s.put("iec/operators/emergency_stop", 140.0, 64.0, s0, true)?;
            let f2a = s.put("iec/contacts/thermal_nc", 140.0, 88.0, f2, false)?;
            s.wire((f3s, 1), (s0s, 0))?;
            s.wire((s0s, 1), (f2a, 0))?;
            let mut bus: Vec<Pin> = vec![(f2a, 1)];
            let mut returns: Vec<Pin> = Vec::new();
            // Up: button, upper limit, down interlock, up coil; and down.
            for (x, button, limit, other, coil) in
                [(160.0, s1, b1, k2, k1), (200.0, s2, b2, k1, k2)]
            {
                let bs = s.put("iec/operators/pushbutton_no", x, 128.0, button, true)?;
                let ls = s.put("iec/contacts/position_nc", x, 152.0, limit, true)?;
                let il = s.put("iec/contacts/nc", x, 176.0, other, false)?;
                let cs = s.put("iec/coils/contactor", x, 200.0, coil, true)?;
                bus.push((bs, 0));
                s.wire((bs, 1), (ls, 0))?;
                s.wire((ls, 1), (il, 0))?;
                s.wire((il, 1), (cs, 0))?;
                returns.push((cs, 1));
            }
            // Brake: released while either contactor is in.
            let up_aux = s.put("iec/contacts/no", 240.0, 128.0, k1, false)?;
            let down_aux = s.put("iec/contacts/no", 256.0, 128.0, k2, false)?;
            let brake = s.put("iec/coils/brake", 240.0, 164.0, y1, true)?;
            bus.extend([(up_aux, 0), (down_aux, 0)]);
            s.bus(152.0, &[(up_aux, 1), (down_aux, 1), (brake, 0)])?;
            s.bus(112.0, &bus)?;
            returns.push((brake, 1));
            s.bus(232.0, &returns)?;
            Ok(())
        },
    )
}

/// Pump on a motor protection breaker, run by hand or by a float switch.
/// Open points: L1 L2 L3 and PE above the breaker, the control supply, N
/// below the coil.
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
            let q1 = s.device("-Q1", "Motor protection breaker")?;
            let k1 = s.device("-K1", "Pump contactor")?;
            let m1 = s.device("-M1", "Pump motor {power} kW")?;
            let f1 = s.device("-F1", "Control circuit breaker")?;
            let s1 = s.device("-S1", "Hand run")?;
            let b1 = s.device("-B1", "Level float switch")?;
            s.describe(m1, "power_kw", "{power}")?;
            let q1s = s.put(
                "iec/protection/motor_protection_breaker",
                40.0,
                40.0,
                q1,
                true,
            )?;
            let k1m = s.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let m1s = s.put("iec/motors/motor_3ph", 40.0, 112.0, m1, true)?;
            three_phase(s, q1s, k1m)?;
            for p in 0..3u16 {
                s.wire((k1m, 2 * p + 1), (m1s, p))?;
            }
            s.pe_entry(64.0, 40.0, (m1s, 3))?;
            let f1s = s.put("iec/protection/mcb_1p", 100.0, 40.0, f1, true)?;
            let hand = s.put("iec/operators/selector_no", 100.0, 72.0, s1, true)?;
            let float = s.put("iec/sensors/float_no", 124.0, 72.0, b1, true)?;
            let coil = s.put("iec/coils/contactor", 100.0, 108.0, k1, true)?;
            s.bus(64.0, &[(f1s, 1), (hand, 0), (float, 0)])?;
            s.bus(96.0, &[(hand, 1), (float, 1), (coil, 0)])?;
            Ok(())
        },
    )
}

/// Three-phase heater on a contactor behind a control thermostat and a
/// safety limit thermostat. Open points: L1 L2 L3, the control supply, N
/// below the coil.
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
            let f1 = s.device("-F1", "Heater breaker")?;
            let k1 = s.device("-K1", "Heater contactor")?;
            let e1 = s.device("-E1", "Heater {power} kW")?;
            let f2 = s.device("-F2", "Control circuit breaker")?;
            let b1 = s.device("-B1", "Control thermostat")?;
            let b2 = s.device("-B2", "High-limit thermostat")?;
            s.describe(e1, "power_kw", "{power}")?;
            let f1s = s.put("iec/protection/mcb_3p", 40.0, 40.0, f1, true)?;
            let k1m = s.put("iec/contactors/main_3p", 40.0, 72.0, k1, false)?;
            let e1s = s.put("iec/passive/heater_3ph", 40.0, 112.0, e1, true)?;
            three_phase(s, f1s, k1m)?;
            for p in 0..3u16 {
                s.wire((k1m, 2 * p + 1), (e1s, p))?;
            }
            let f2s = s.put("iec/protection/mcb_1p", 100.0, 40.0, f2, true)?;
            let t1 = s.put("iec/sensors/temperature_no", 100.0, 72.0, b1, true)?;
            let t2 = s.put("iec/sensors/temperature_nc", 100.0, 96.0, b2, true)?;
            let coil = s.put("iec/coils/contactor", 100.0, 120.0, k1, true)?;
            s.wire((f2s, 1), (t1, 0))?;
            s.wire((t1, 1), (t2, 0))?;
            s.wire((t2, 1), (coil, 0))?;
            Ok(())
        },
    )
}

/// Three-phase incomer: main switch, an outgoing three-pole breaker for the
/// power circuits, a one-pole breaker for the control supply, and the N and
/// PE terminals. Place it above the starters and wire its outputs to their
/// L1 L2 L3, PE, control supply and N points. Open points: L1 L2 L3, N and
/// PE above (the supply), the outputs below.
fn three_phase_supply() -> Result<MacroFile> {
    capture(
        "Three-phase supply",
        "Three-phase incomer with main switch, outgoing breaker, control circuit breaker, and N and PE terminal symbols",
        Vec::new(),
        &[
            "supply",
            "incomer",
            "main switch",
            "three phase",
            "400 V",
            "breaker",
            "isolator",
            "pe",
        ],
        |s| {
            let q1 = s.device("-Q1", "Main switch")?;
            let f1 = s.device("-F1", "Power circuit breaker")?;
            let f2 = s.device("-F2", "Control circuit breaker")?;
            let q1s = s.put("iec/switches/main_switch_3p", 40.0, 40.0, q1, true)?;
            let f1s = s.put("iec/protection/mcb_3p", 40.0, 72.0, f1, true)?;
            let f2s = s.put("iec/protection/mcb_1p", 96.0, 72.0, f2, true)?;
            // The switch feeds the power breaker; a tap on L1 feeds the
            // control breaker.
            s.wire((q1s, 3), (f1s, 2))?;
            s.wire((q1s, 5), (f1s, 4))?;
            s.bus(64.0, &[(q1s, 1), (f1s, 0), (f2s, 0)])?;
            // N and PE pass through their terminals; the outputs below the
            // terminals go to the loads.
            s.put_plain("iec/terminals/terminal_pe", 64.0, 40.0)?;
            s.put_plain("iec/terminals/terminal_n", 104.0, 40.0)?;
            Ok(())
        },
    )
}
