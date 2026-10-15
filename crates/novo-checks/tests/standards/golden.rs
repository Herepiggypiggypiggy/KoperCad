//! A small machine that meets every rule of every standard with the data
//! stated, so each test can break one thing and watch one rule change.
//!
//! 400 V three-phase supply, switch-disconnector, time-delay fuses, contactor,
//! overload relay and a 2.2 kW motor; 24 V dc control with an emergency stop,
//! start and stop buttons and a green lamp.

use novo_checks::standards::{self, ComplianceReport, RuleResult, RuleStatus, StandardSet};
use novo_core::*;

pub fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// Everything a test may want to edit.
pub struct Golden {
    pub doc: Document,
    pub page: PageId,
    pub q1: DeviceId,
    pub f1: DeviceId,
    pub k1: DeviceId,
    pub f2: DeviceId,
    pub m1: DeviceId,
    pub f3: DeviceId,
    pub s0: DeviceId,
    pub s1: DeviceId,
    pub s2: DeviceId,
    pub h1: DeviceId,
    pub q1_sym: SymbolId,
    pub f1_sym: SymbolId,
    pub k1_sym: SymbolId,
    pub f2_sym: SymbolId,
    pub m1_sym: SymbolId,
    pub f3_sym: SymbolId,
    pub s0_sym: SymbolId,
    pub s1_sym: SymbolId,
    pub s2_sym: SymbolId,
    pub coil_sym: SymbolId,
    pub h1_sym: SymbolId,
    pub l1: PotentialId,
    pub l_plus: PotentialId,
    pub l_minus: PotentialId,
    pub pe: PotentialId,
    /// The wires of the main circuit (L1, L2, L3), in order.
    pub power_wires: Vec<WireId>,
    pub pe_wire: WireId,
    pub control_wires: Vec<WireId>,
}

pub fn set(tx: &mut Tx<'_>, id: impl Into<AnyId>, key: &str, value: impl Into<Value>) {
    tx.set_property(id.into(), key, Some(value.into())).unwrap();
}

pub fn pins(names: &[&str]) -> Vec<Pin> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
        .collect()
}

fn role(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

pub struct Draw<'a, 'b> {
    pub tx: &'a mut Tx<'b>,
    pub page: PageId,
    x: f64,
}

impl<'a, 'b> Draw<'a, 'b> {
    pub fn new(tx: &'a mut Tx<'b>, page: PageId) -> Self {
        Draw { tx, page, x: 10.0 }
    }

    /// A symbol of `device` with named pins and the poles that pass a
    /// potential, placed further right each time.
    pub fn sym(
        &mut self,
        key: &str,
        device: DeviceId,
        main: bool,
        names: &[&str],
        poles: &[&[u16]],
        bridges: &[&[u16]],
    ) -> SymbolId {
        self.x += 40.0;
        let id = self
            .tx
            .place_symbol(
                self.page,
                key,
                Placement::at(Point::new(self.x, 40.0)),
                pins(names),
                role(device, main),
            )
            .unwrap();
        let poles: Vec<Vec<u16>> = poles.iter().map(|p| p.to_vec()).collect();
        let bridges: Vec<Vec<u16>> = bridges.iter().map(|p| p.to_vec()).collect();
        self.tx
            .update::<SymbolInstance>(id, |s| {
                s.pass_through = poles;
                s.bridges = bridges;
            })
            .unwrap();
        id
    }

    pub fn three_pole(&mut self, key: &str, device: DeviceId, main: bool) -> SymbolId {
        self.sym(
            key,
            device,
            main,
            &["1", "2", "3", "4", "5", "6"],
            &[&[0, 1], &[2, 3], &[4, 5]],
            &[],
        )
    }

    pub fn two_pole(&mut self, key: &str, device: DeviceId, main: bool, a: &str, b: &str) -> SymbolId {
        self.sym(key, device, main, &[a, b], &[&[0, 1]], &[])
    }

    pub fn feed(&mut self, potential: PotentialId) -> SymbolId {
        self.x += 40.0;
        self.tx
            .place_symbol(
                self.page,
                "iec/connectivity/potential_feed",
                Placement::at(Point::new(self.x, 10.0)),
                vec![Pin::new("1", 0.0, 8.0)],
                SymbolRole::PotentialFeed { potential },
            )
            .unwrap()
    }

    pub fn wire(&mut self, a: SymbolId, pa: u16, b: SymbolId, pb: u16) -> WireId {
        self.tx
            .connect(Endpoint::pin(a, pa), Endpoint::pin(b, pb))
            .unwrap()
    }

    pub fn style(&mut self, wire: WireId, colour: &str, mm2: f64) {
        self.tx
            .update::<Wire>(wire, |w| {
                w.color = Some(colour.to_owned());
                w.cross_section_mm2 = Some(mm2);
            })
            .unwrap();
    }
}

pub fn golden() -> Golden {
    let mut doc = Document::default();
    let g = doc
        .transact("Build the golden machine", |tx| {
            tx.set_info(|i| i.name = "Golden machine".into())?;
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let l2 = tx.add_potential("L2", PotentialKind::Line)?;
            let l3 = tx.add_potential("L3", PotentialKind::Line)?;
            let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
            let l_plus = tx.add_potential("L+", PotentialKind::DcPositive)?;
            let l_minus = tx.add_potential("L-", PotentialKind::DcNegative)?;
            for p in [l1, l2, l3] {
                tx.update::<Potential>(p, |p| p.voltage = Some(400.0))?;
            }
            tx.update::<Potential>(l_plus, |p| p.voltage = Some(24.0))?;

            let q1 = tx.add_device(tag("=A1+CAB-Q1"), "Main switch")?;
            let f1 = tx.add_device(tag("=A1+CAB-F1"), "Motor fuses")?;
            let k1 = tx.add_device(tag("=A1+CAB-K1"), "Motor contactor")?;
            let f2 = tx.add_device(tag("=A1+CAB-F2"), "Overload relay")?;
            let m1 = tx.add_device(tag("=A1+FLD-M1"), "Pump motor")?;
            let f3 = tx.add_device(tag("=A1+CAB-F3"), "Control breaker")?;
            let s0 = tx.add_device(tag("=A1+CAB-S0"), "Emergency stop")?;
            let s1 = tx.add_device(tag("=A1+CAB-S1"), "Start")?;
            let s2 = tx.add_device(tag("=A1+CAB-S2"), "Stop")?;
            let h1 = tx.add_device(tag("=A1+CAB-H1"), "Running")?;

            // Facts of the machine, as a maker's data or the designer states them.
            set(tx, q1, "isolating", true);
            set(tx, q1, "lockable", true);
            set(tx, q1, RATED_CURRENT_A, 25.0);
            set(tx, q1, "sccr_ka", 65.0);
            set(tx, f1, RATED_CURRENT_A, 10.0);
            set(tx, f1, "protection_kind", "time_delay_fuse");
            set(tx, f1, "sccr_ka", 100.0);
            set(tx, f1, "current_limiting", true);
            set(tx, f1, "let_through_ka", 2.0);
            set(tx, k1, "sccr_ka", 5.0);
            set(tx, f2, "sccr_ka", 5.0);
            set(tx, f2, "setting_a", 5.5);
            set(tx, m1, "flc_a", 5.0);
            set(tx, m1, "service_factor", 1.15);
            set(tx, f3, RATED_CURRENT_A, 2.0);
            set(tx, s0, "function", "emergency stop");
            set(tx, s0, "actuator_colour", "RD");
            set(tx, s0, "background_colour", "YE");
            set(tx, s0, "stop_category", 0i64);
            set(tx, s1, "function", "start");
            set(tx, s1, "actuator_colour", "GN");
            set(tx, s2, "function", "stop");
            set(tx, s2, "actuator_colour", "RD");
            set(tx, h1, "function", "normal");

            let mut d = Draw::new(tx, page);
            let feeds = [d.feed(l1), d.feed(l2), d.feed(l3)];
            let feed_pe = d.feed(pe);
            let q1_sym = d.three_pole("iec/switches/switch_disconnector_3p", q1, true);
            let f1_sym = d.three_pole("iec/protection/fuse_3p", f1, true);
            let k1_sym = d.three_pole("iec/contactors/main_3p", k1, false);
            let f2_sym = d.sym(
                "iec/protection/overload_relay",
                f2,
                true,
                &["1", "2", "3", "4", "5", "6"],
                &[],
                &[&[0, 1], &[2, 3], &[4, 5]],
            );
            let m1_sym = d.sym(
                "iec/motors/motor_3ph",
                m1,
                true,
                &["U1", "V1", "W1", "PE"],
                &[],
                &[],
            );
            let mut power_wires = Vec::new();
            for phase in 0..3u16 {
                let (top, bottom) = (phase * 2, phase * 2 + 1);
                power_wires.push(d.wire(feeds[usize::from(phase)], 0, q1_sym, top));
                power_wires.push(d.wire(q1_sym, bottom, f1_sym, top));
                power_wires.push(d.wire(f1_sym, bottom, k1_sym, top));
                power_wires.push(d.wire(k1_sym, bottom, f2_sym, top));
                power_wires.push(d.wire(f2_sym, bottom, m1_sym, phase));
            }
            let pe_wire = d.wire(feed_pe, 0, m1_sym, 3);
            for w in &power_wires {
                d.style(*w, "BK", 2.5);
            }
            d.style(pe_wire, "GNYE", 2.5);

            // Control: L+ -> F3 -> S0 -> S2 -> S1 -> coil -> L-, lamp across.
            let feed_p = d.feed(l_plus);
            let feed_m = d.feed(l_minus);
            let f3_sym = d.two_pole("iec/protection/mcb_1p", f3, true, "1", "2");
            let s0_sym = d.two_pole("iec/operators/emergency_stop", s0, true, "11", "12");
            let s2_sym = d.two_pole("iec/operators/pushbutton_nc", s2, true, "11", "12");
            let s1_sym = d.two_pole("iec/operators/pushbutton_no", s1, true, "13", "14");
            let coil_sym = d.sym("iec/coils/contactor", k1, true, &["A1", "A2"], &[], &[]);
            let h1_sym = d.sym("iec/signalling/lamp_green", h1, true, &["X1", "X2"], &[], &[]);
            let control_wires = vec![
                d.wire(feed_p, 0, f3_sym, 0),
                d.wire(f3_sym, 1, s0_sym, 0),
                d.wire(s0_sym, 1, s2_sym, 0),
                d.wire(s2_sym, 1, s1_sym, 0),
                d.wire(s1_sym, 1, coil_sym, 0),
                d.wire(s1_sym, 1, h1_sym, 0),
                d.wire(coil_sym, 1, feed_m, 0),
                d.wire(h1_sym, 1, feed_m, 0),
            ];
            for w in &control_wires {
                d.style(*w, "BU", 1.0);
            }

            // Machine data.
            standards::set_info_property(tx, "machine.rated_voltage_v", Some(400i64.into()))?;
            standards::set_info_property(tx, "machine.phases", Some(3i64.into()))?;
            standards::set_info_property(tx, "machine.frequency_hz", Some(50i64.into()))?;
            standards::set_info_property(tx, "machine.full_load_current_a", Some(5.0.into()))?;
            standards::set_info_property(tx, "supply.short_circuit_ka", Some(10.0.into()))?;
            standards::set_info_property(tx, "panel.sccr_ka", Some(65.0.into()))?;
            // NFPA 79 has no dc control limit built in: the engineer enters
            // it from their own copy.
            standards::set_info_property(
                tx,
                "standards.nfpa79.max_control_dc_v",
                Some(250.0.into()),
            )?;
            standards::select(tx, &StandardSet::ALL)?;

            Ok(Golden {
                doc: Document::default(),
                page,
                q1,
                f1,
                k1,
                f2,
                m1,
                f3,
                s0,
                s1,
                s2,
                h1,
                q1_sym,
                f1_sym,
                k1_sym,
                f2_sym,
                m1_sym,
                f3_sym,
                s0_sym,
                s1_sym,
                s2_sym,
                coil_sym,
                h1_sym,
                l1,
                l_plus,
                l_minus,
                pe,
                power_wires,
                pe_wire,
                control_wires,
            })
        })
        .unwrap();
    Golden { doc, ..g.0 }
}

impl Golden {
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Tx<'_>, &Golden) -> Result<R>) -> R {
        // The closure only reads the handles; clone them out of the borrow.
        let handles = self.handles();
        self.doc
            .transact("edit", |tx| f(tx, &handles))
            .map(|(r, _)| r)
            .unwrap()
    }

    fn handles(&self) -> Golden {
        Golden {
            doc: Document::default(),
            page: self.page,
            q1: self.q1,
            f1: self.f1,
            k1: self.k1,
            f2: self.f2,
            m1: self.m1,
            f3: self.f3,
            s0: self.s0,
            s1: self.s1,
            s2: self.s2,
            h1: self.h1,
            q1_sym: self.q1_sym,
            f1_sym: self.f1_sym,
            k1_sym: self.k1_sym,
            f2_sym: self.f2_sym,
            m1_sym: self.m1_sym,
            f3_sym: self.f3_sym,
            s0_sym: self.s0_sym,
            s1_sym: self.s1_sym,
            s2_sym: self.s2_sym,
            coil_sym: self.coil_sym,
            h1_sym: self.h1_sym,
            l1: self.l1,
            l_plus: self.l_plus,
            l_minus: self.l_minus,
            pe: self.pe,
            power_wires: self.power_wires.clone(),
            pe_wire: self.pe_wire,
            control_wires: self.control_wires.clone(),
        }
    }

    pub fn project(&self) -> &Project {
        self.doc.project()
    }

    pub fn report(&self) -> ComplianceReport {
        standards::compliance(self.project())
    }

    pub fn result(&self, id: &str) -> RuleResult {
        self.report()
            .results
            .into_iter()
            .find(|r| r.rule.id == id)
            .unwrap_or_else(|| panic!("no result for {id}"))
    }

    pub fn status(&self, id: &str) -> RuleStatus {
        self.result(id).status
    }

    /// Re-keys a symbol, e.g. to turn a switch-disconnector into a fuse.
    pub fn rekey(&mut self, symbol: SymbolId, key: &str) {
        let key = key.to_owned();
        self.edit(|tx, _| tx.update::<SymbolInstance>(symbol, |s| s.symbol = SymbolKey(key)));
    }

    pub fn prop(&mut self, id: impl Into<AnyId>, key: &str, value: impl Into<Value>) {
        let (id, key, value) = (id.into(), key.to_owned(), value.into());
        self.edit(|tx, _| tx.set_property(id, key, Some(value)));
    }

    pub fn clear(&mut self, id: impl Into<AnyId>, key: &str) {
        let (id, key) = (id.into(), key.to_owned());
        self.edit(|tx, _| tx.set_property(id, key, None));
    }

    pub fn project_prop(&mut self, key: &str, value: Option<Value>) {
        let key = key.to_owned();
        self.edit(|tx, _| standards::set_info_property(tx, &key, value));
    }

    pub fn style(&mut self, wire: WireId, colour: Option<&str>, mm2: Option<f64>) {
        let colour = colour.map(str::to_owned);
        self.edit(|tx, _| {
            tx.update::<Wire>(wire, |w| {
                w.color = colour;
                w.cross_section_mm2 = mm2;
            })
        });
    }
}

impl Golden {
    /// A neutral net with one wire of the given colour, to a lamp.
    pub fn add_neutral(&mut self, colour: Option<&str>) -> WireId {
        let page = self.page;
        self.edit(|tx, _| {
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let h = tx.add_device(tag("=A1+CAB-H2"), "Socket lamp")?;
            let mut d = Draw::new(tx, page);
            let feed = d.feed(n);
            let lamp = d.sym("iec/signalling/lamp", h, true, &["X1", "X2"], &[], &[]);
            let w = d.wire(feed, 0, lamp, 1);
            if let Some(c) = colour {
                d.style(w, c, 1.0);
            }
            Ok(w)
        })
    }
}
