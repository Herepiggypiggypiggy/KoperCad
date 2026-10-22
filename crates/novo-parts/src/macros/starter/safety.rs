use novo_core::{Endpoint, Result, SymbolId};

use super::{MacroFile, Sheet, capture};

/// Every macro of this batch, in the order they are listed.
pub(super) fn all() -> Vec<fn() -> Result<MacroFile>> {
    vec![safety_estop_relay, plc_network]
}

/// A wire between two pins that leaves its ends along the pins and bends at
/// the given corners.
fn wire_via(
    s: &mut Sheet<'_, '_>,
    a: (SymbolId, u16),
    b: (SymbolId, u16),
    corners: &[(f64, f64)],
) -> Result<()> {
    s.wire_via(Endpoint::pin(a.0, a.1), Endpoint::pin(b.0, b.1), corners)
}

/// Two-channel emergency stop on a safety relay with a reset button and two
/// contactor coils. Open ends: +24 V to A1, 13, 23 and the reset button, 0 V
/// below the coils and A2.
fn safety_estop_relay() -> Result<MacroFile> {
    capture(
        "Emergency stop with safety relay",
        "Two-channel emergency stop monitored by a safety relay with a reset button and two redundant contactor coils",
        Vec::new(),
        &[
            "safety",
            "safety relay",
            "emergency stop",
            "e-stop",
            "reset",
            "two channel",
            "redundant",
            "contactor",
        ],
        |s| {
            let s1 = s.device("-S1", "Emergency stop, 2 channels")?;
            let s2 = s.device("-S2", "Reset")?;
            let k1 = s.device("-K1", "Safety relay")?;
            let k2 = s.device("-K2", "Safety contactor 1")?;
            let k3 = s.device("-K3", "Safety contactor 2")?;
            // Relay pins: 0 A1, 1 S34, 2 S11, 3 S12, 4 S21, 5 S22,
            // then A2, 13, 14, 23, 24, 33, 34, 41, 42.
            let relay = s.put("iec/safety/relay", 80.0, 88.0, k1, true)?;
            // Pins: 0 11, 1 12, 2 21, 3 22.
            let estop = s.put("iec/safety/emergency_stop_2ch", 176.0, 48.0, s1, true)?;
            let reset = s.put("iec/operators/pushbutton_no", 96.0, 48.0, s2, true)?;
            s.wire((reset, 1), (relay, 1))?;
            // Channel 1: out at S11, back at S12.
            wire_via(s, (relay, 2), (estop, 0), &[(112.0, 32.0), (176.0, 32.0)])?;
            wire_via(s, (estop, 1), (relay, 3), &[(176.0, 72.0), (120.0, 72.0)])?;
            // Channel 2: out at S21, back at S22.
            wire_via(s, (relay, 4), (estop, 2), &[(128.0, 24.0), (184.0, 24.0)])?;
            wire_via(s, (estop, 3), (relay, 5), &[(184.0, 76.0), (136.0, 76.0)])?;
            // Each safety output feeds one contactor coil.
            let c1 = s.put("iec/coils/contactor", 104.0, 144.0, k2, true)?;
            let c2 = s.put("iec/coils/contactor", 136.0, 144.0, k3, true)?;
            s.wire((relay, 8), (c1, 0))?;
            s.wire((relay, 10), (c2, 0))?;
            Ok(())
        },
    )
}

/// PLC CPU with an Ethernet switch, a drive and an operator panel on the
/// network. Open ends: the 24 V supplies of the CPU and the switch, PE, and
/// the spare switch ports.
fn plc_network() -> Result<MacroFile> {
    capture(
        "PLC network with switch",
        "PLC CPU on an Ethernet switch with a frequency converter and an operator panel as network devices",
        Vec::new(),
        &[
            "plc", "network", "ethernet", "switch", "profinet", "hmi", "drive", "fieldbus",
        ],
        |s| {
            let a1 = s.device("-A1", "PLC CPU")?;
            let a2 = s.device("-A2", "Ethernet switch, 5 ports")?;
            let t1 = s.device("-T1", "Frequency converter, network port")?;
            let a3 = s.device("-A3", "Operator panel, network port")?;
            // CPU pins: 0 L+, 1 M, 2 PE, 3 P1, 4 P2, 5 A, 6 B, 7 SH.
            let cpu = s.put("iec/plc/cpu", 40.0, 40.0, a1, true)?;
            // Switch pins: 0 P1, 1 P2, 2 P3, 3 P4, 4 P5, 5 L+, 6 M.
            let sw = s.put("iec/network/ethernet_switch_5", 136.0, 48.0, a2, true)?;
            let drive = s.put("iec/network/port_ethernet_2p", 136.0, 104.0, t1, true)?;
            let hmi = s.put("iec/network/port_ethernet", 176.0, 104.0, a3, true)?;
            wire_via(
                s,
                (cpu, 3),
                (sw, 0),
                &[(88.0, 52.0), (88.0, 36.0), (136.0, 36.0)],
            )?;
            s.wire((sw, 3), (drive, 0))?;
            wire_via(s, (sw, 4), (hmi, 0), &[(144.0, 88.0), (176.0, 88.0)])?;
            Ok(())
        },
    )
}
