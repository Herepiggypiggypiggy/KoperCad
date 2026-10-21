//! A small power distribution board: the fixture for the single-line
//! diagram and the checks on it.
//!
//! ```text
//! supply - Q1 (63 A) +- Q2 (16 A) - M1 (12 A nameplate)
//!                    +- Q3 (10 A) - drive U1 - M2 (2.2 kW)
//!                    +- Q4 (16 A) - heater E1 (6 kW)
//! ```
//!
//! Every run is three wires with a cross-section and a length; the supply is
//! 400 V three-phase with 10 kA stated. A fixture, not a design: the
//! cross-sections are chosen so the whole board passes.

use std::collections::BTreeMap;

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, PageId, Pin, Placement, Point, Potential,
    PotentialKind, Result, SymbolId, SymbolInstance, SymbolRole, Tx, Wire, WireId,
};

fn at(x: f64) -> Placement {
    Placement::at(Point::new(x, 0.0))
}

fn pins(names: &[&str]) -> Vec<Pin> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, (i as f64) * 8.0, 0.0))
        .collect()
}

fn pole_pins() -> Vec<Pin> {
    pins(&["1", "2", "3", "4", "5", "6"])
}

/// The devices and wires of the distribution board.
#[derive(Debug, Clone)]
pub struct Board {
    pub q1: DeviceId,
    pub q2: DeviceId,
    pub q3: DeviceId,
    pub q4: DeviceId,
    pub drive: DeviceId,
    pub m1: DeviceId,
    pub m2: DeviceId,
    pub heater: DeviceId,
    /// Wire sets (one wire per phase), by name.
    pub wires: BTreeMap<String, [WireId; 3]>,
}

struct Builder<'a, 'b> {
    tx: &'a mut Tx<'b>,
    page: PageId,
    wires: BTreeMap<String, [WireId; 3]>,
}

impl Builder<'_, '_> {
    fn device(&mut self, tag: &str, description: &str) -> DeviceId {
        self.tx
            .add_device(DeviceTag::parse(tag).unwrap(), description)
            .unwrap()
    }

    fn symbol(&mut self, key: &str, x: f64, pins: Vec<Pin>, device: DeviceId) -> SymbolId {
        self.tx
            .place_symbol(
                self.page,
                key,
                at(x),
                pins,
                SymbolRole::Device { device, main: true },
            )
            .unwrap()
    }

    fn breaker(&mut self, tag: &str, x: f64, rated: f64) -> (DeviceId, SymbolId) {
        let d = self.device(tag, "Circuit breaker");
        let s = self.symbol("iec/protection/mcb_3p", x, pole_pins(), d);
        self.tx
            .update::<SymbolInstance>(s, |s| {
                s.pass_through = vec![vec![0, 1], vec![2, 3], vec![4, 5]]
            })
            .unwrap();
        self.tx
            .set_property(d, "rated_current_a", Some(rated.into()))
            .unwrap();
        (d, s)
    }

    /// Three wires from `from` pins to `to` pins, each with a section and a
    /// length.
    fn run(
        &mut self,
        name: &str,
        from: (SymbolId, [u16; 3]),
        to: (SymbolId, [u16; 3]),
        section: Option<f64>,
        length: Option<f64>,
    ) {
        let mut ids = Vec::new();
        for i in 0..3 {
            let w = self
                .tx
                .connect(
                    Endpoint::pin(from.0, from.1[i]),
                    Endpoint::pin(to.0, to.1[i]),
                )
                .unwrap();
            if let Some(s) = section {
                self.tx
                    .update::<Wire>(w, |w| w.cross_section_mm2 = Some(s))
                    .unwrap();
            }
            if let Some(l) = length {
                self.tx.set_property(w, "length_m", Some(l.into())).unwrap();
            }
            ids.push(w);
        }
        self.wires.insert(name.to_owned(), ids.try_into().unwrap());
    }
}

const LINE: [u16; 3] = [0, 2, 4];
const LOAD: [u16; 3] = [1, 3, 5];

/// supply - Q1 (63 A) +- Q2 (16 A) - M1 (12 A nameplate)
///                     +- Q3 (10 A) - drive - M2 (2.2 kW)
///                     +- Q4 (16 A) - heater (6 kW)
pub fn distribution_board(doc: &mut Document) -> Result<Board> {
    Ok(doc.transact("Distribution board", distribution_board_in)?.0)
}

/// [`distribution_board`] inside a transaction the caller owns.
pub fn distribution_board_in(tx: &mut Tx<'_>) -> Result<Board> {
    distribution_board_with(tx, 0)
}

/// [`distribution_board`] with `extra` more feeders on Q1 (a breaker `-Q10`,
/// `-Q11`, ... and a 5 A motor each), to make a branch wider than a sheet.
pub fn distribution_board_wide(doc: &mut Document, extra: usize) -> Result<Board> {
    Ok(doc
        .transact("Distribution board", |tx| {
            distribution_board_with(tx, extra)
        })?
        .0)
}

fn distribution_board_with(tx: &mut Tx<'_>, extra: usize) -> Result<Board> {
    {
        let page = tx.add_page("1")?;
        let mut pots = Vec::new();
        for name in ["L1", "L2", "L3"] {
            let p = tx.add_potential(name, PotentialKind::Line)?;
            tx.update::<Potential>(p, |p| p.voltage = Some(400.0))?;
            pots.push(p);
        }
        tx.set_info(|i| {
            i.name = "Board".into();
            i.properties
                .insert("supply.short_circuit_ka".into(), 10.0.into());
        })?;
        let mut b = Builder {
            tx,
            page,
            wires: BTreeMap::new(),
        };
        // The supply: one feed per phase on a pole of Q1.
        let feeds: Vec<SymbolId> = pots
            .iter()
            .enumerate()
            .map(|(i, p)| {
                b.tx.place_symbol(
                    page,
                    "iec/connectivity/potential_feed",
                    at(i as f64 * 8.0),
                    pins(&["1"]),
                    SymbolRole::PotentialFeed { potential: *p },
                )
                .unwrap()
            })
            .collect();
        let (q1, q1s) = b.breaker("=A1+CAB-Q1", 0.0, 63.0);
        let (q2, q2s) = b.breaker("=A1+CAB-Q2", 40.0, 16.0);
        let (q3, q3s) = b.breaker("=A1+CAB-Q3", 80.0, 10.0);
        let (q4, q4s) = b.breaker("=A1+CAB-Q4", 120.0, 16.0);
        // Supply to Q1.
        let mut ids = Vec::new();
        for i in 0..3 {
            let w =
                b.tx.connect(Endpoint::pin(feeds[i], 0), Endpoint::pin(q1s, LINE[i]))
                    .unwrap();
            b.tx.update::<Wire>(w, |w| w.cross_section_mm2 = Some(25.0))
                .unwrap();
            b.tx.set_property(w, "length_m", Some(10.0.into())).unwrap();
            ids.push(w);
        }
        b.wires.insert("supply".to_owned(), ids.try_into().unwrap());
        b.run("q2", (q1s, LOAD), (q2s, LINE), Some(16.0), Some(5.0));
        b.run("q3", (q1s, LOAD), (q3s, LINE), Some(16.0), Some(8.0));
        b.run("q4", (q1s, LOAD), (q4s, LINE), Some(16.0), Some(4.0));

        let m1 = b.device("=A1+FLD-M1", "Pump");
        let m1s = b.symbol("iec/motors/motor_3ph", 0.0, pins(&["U1", "V1", "W1"]), m1);
        b.tx.set_property(m1, "rated_current_a", Some(12.0.into()))
            .unwrap();
        b.run("m1", (q2s, LOAD), (m1s, [0, 1, 2]), Some(2.5), Some(30.0));

        let drive = b.device("=A1+CAB-U1", "Drive");
        let ds = b.symbol(
            "iec/drives/frequency_converter",
            40.0,
            pins(&["L1", "L2", "L3", "U", "V", "W"]),
            drive,
        );
        b.run(
            "drive_in",
            (q3s, LOAD),
            (ds, [0, 1, 2]),
            Some(2.5),
            Some(3.0),
        );
        let m2 = b.device("=A1+FLD-M2", "Fan");
        let m2s = b.symbol("iec/motors/motor_3ph", 40.0, pins(&["U1", "V1", "W1"]), m2);
        b.tx.set_property(m2, "power_kw", Some(2.2.into())).unwrap();
        b.run(
            "m2",
            (ds, [3, 4, 5]),
            (m2s, [0, 1, 2]),
            Some(1.5),
            Some(60.0),
        );

        let heater = b.device("=A1+FLD-E1", "Heater");
        let hs = b.symbol(
            "iec/passive/heater_3p",
            80.0,
            pins(&["1", "2", "3"]),
            heater,
        );
        b.tx.set_property(heater, "power_kw", Some(6.0.into()))
            .unwrap();
        b.run("e1", (q4s, LOAD), (hs, [0, 1, 2]), Some(2.5), Some(10.0));

        for i in 0..extra {
            let n = 10 + i;
            let (_, qs) = b.breaker(&format!("=A1+CAB-Q{n}"), 160.0 + 40.0 * i as f64, 10.0);
            b.run(
                &format!("x{n}"),
                (q1s, LOAD),
                (qs, LINE),
                Some(4.0),
                Some(5.0),
            );
            let m = b.device(&format!("=A1+FLD-M{n}"), "Motor");
            let ms = b.symbol(
                "iec/motors/motor_3ph",
                160.0 + 40.0 * i as f64,
                pins(&["U1", "V1", "W1"]),
                m,
            );
            b.tx.set_property(m, "rated_current_a", Some(5.0.into()))
                .unwrap();
            b.run(
                &format!("y{n}"),
                (qs, LOAD),
                (ms, [0, 1, 2]),
                Some(2.5),
                Some(10.0),
            );
        }

        Ok(Board {
            q1,
            q2,
            q3,
            q4,
            drive,
            m1,
            m2,
            heater,
            wires: b.wires,
        })
    }
}
