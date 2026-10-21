//! A big synthetic project for scale measurements.
//!
//! One *feeder* is a motor starter drawn over two pages (main and control
//! circuit, the Gate 1 circuit with its own tags): about 9 devices, 27 wires,
//! a cable and four terminals. `feeders` of them make a project of `2 *
//! feeders` pages. Feeders are grouped into cabinets (`per_cabinet` each) with
//! one terminal strip per cabinet, a mounting plate, a rail and every device
//! placed, so the cabinet, 3D and heat paths see load too.
//!
//! The geometry is deliberately plain: this is a load generator, not a
//! design. Part data is catalogue-like (a few dozen distinct parts, shared by
//! thousands of devices), and the numbers are repeatable for a given spec.

use novo_core::{
    CableCore, Device, DeviceId, DeviceTag, Document, Endpoint, MountingPlate, PageId, Pin,
    Placement, Point, PotentialId, PotentialKind, RailKind, Result, StructureId, SymbolId,
    SymbolInstance, SymbolRole, Terminal, TerminalKind, Tx, Value,
};

/// How big the project is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigSpec {
    /// Motor starters; the project has `2 * feeders` pages.
    pub feeders: usize,
    /// Feeders per cabinet (each cabinet gets a plate, a rail and a strip).
    pub per_cabinet: usize,
}

impl BigSpec {
    /// 20 feeders: 40 pages, a typical machine.
    pub const SMALL: BigSpec = BigSpec {
        feeders: 20,
        per_cabinet: 10,
    };
    /// 100 feeders: 200 pages.
    pub const MEDIUM: BigSpec = BigSpec {
        feeders: 100,
        per_cabinet: 10,
    };
    /// 300 feeders: 600 pages, about 2,700 devices and 8,000 wires.
    pub const LARGE: BigSpec = BigSpec {
        feeders: 300,
        per_cabinet: 10,
    };

    pub fn pages(&self) -> usize {
        self.feeders * 2
    }
}

/// What the generator made, for the benchmark to aim at.
#[derive(Debug, Clone)]
pub struct Big {
    pub pages: Vec<PageId>,
    pub cabinets: Vec<StructureId>,
    pub devices: Vec<DeviceId>,
    /// A symbol on the middle page, for single-edit timings.
    pub probe: SymbolId,
    /// A device tag that exists once, for search timings.
    pub probe_tag: String,
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("generated tags are valid")
}

fn at(x: f64, y: f64) -> Placement {
    Placement::at(Point::new(x, y))
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

fn role(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

struct Potentials {
    l: [PotentialId; 3],
    pe: PotentialId,
    ctl_l: PotentialId,
    n: PotentialId,
}

/// Catalogue entries cycled through by the generated devices: (manufacturer,
/// order number, width, height, depth in mm, loss in W).
const PARTS: [(&str, &str, f64, f64, f64, f64); 7] = [
    ("Siemens", "3NW7033", 54.0, 80.0, 60.0, 1.5),
    ("Siemens", "3RT2016-1AP01", 45.0, 85.0, 90.0, 2.5),
    ("Siemens", "3RU2116-1FB0", 45.0, 85.0, 90.0, 4.0),
    ("Siemens", "1LE1003-0EB42-2AA4", 120.0, 120.0, 200.0, 0.0),
    ("Siemens", "5SY6106-7", 18.0, 90.0, 70.0, 1.0),
    ("Siemens", "3SU1100-0AB20-1CA0", 22.0, 22.0, 40.0, 0.0),
    ("Siemens", "3SU1100-0AB40-1BA0", 22.0, 22.0, 40.0, 0.0),
];

/// Builds the project as one undo step.
pub fn big_project(doc: &mut Document, spec: BigSpec) -> Result<Big> {
    Ok(doc
        .transact("Generate big project", |tx| build(tx, spec))?
        .0)
}

fn build(tx: &mut Tx<'_>, spec: BigSpec) -> Result<Big> {
    tx.set_info(|i| i.name = format!("Big project, {} feeders", spec.feeders))?;
    let pot = Potentials {
        l: [
            tx.add_potential("L1", PotentialKind::Line)?,
            tx.add_potential("L2", PotentialKind::Line)?,
            tx.add_potential("L3", PotentialKind::Line)?,
        ],
        pe: tx.add_potential("PE", PotentialKind::ProtectiveEarth)?,
        ctl_l: tx.add_potential("L", PotentialKind::Line)?,
        n: tx.add_potential("N", PotentialKind::Neutral)?,
    };
    let mut parts = Vec::new();
    for (maker, order, w, h, d, loss) in PARTS {
        let part = tx.add_part(maker, order)?;
        for (k, v) in [
            ("width_mm", w),
            ("height_mm", h),
            ("depth_mm", d),
            ("power_loss_w", loss),
        ] {
            tx.set_property(part, k, Some(Value::Float(v)))?;
        }
        parts.push(part);
    }
    let terminal_part = tx.add_part("Phoenix Contact", "3044076")?;
    let cable_part = tx.add_part("Lapp", "1119304")?;

    let per_cabinet = spec.per_cabinet.max(1);
    let cabinet_count = spec.feeders.div_ceil(per_cabinet);
    let mut cabinets = Vec::new();
    let mut strips = Vec::new();
    for c in 0..cabinet_count {
        let cab = tx.add_structure(tag(&format!("+CAB{}", c + 1)), "Generated cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 1600.0,
                height: 1900.0,
                margin: 10.0,
            }),
        )?;
        novo_layout::model::set_enclosure(
            tx,
            cab,
            &novo_layout::enclosure::Enclosure {
                width: 1700.0,
                height: 2000.0,
                depth: 400.0,
                plate_setback: 0.0,
                door: None,
                front_clearance: None,
                hinge_side_clearance: None,
            },
        )?;
        for row in 0..8 {
            tx.add_rail(
                cab,
                50.0,
                150.0 + 230.0 * f64::from(row),
                1500.0,
                RailKind::DinRail,
            )?;
        }
        strips.push(tx.add_terminal_strip(tag(&format!("=A1+CAB{}-X1", c + 1)))?);
        cabinets.push(cab);
    }

    let mut pages = Vec::new();
    let mut devices = Vec::new();
    let mut probe = None;
    let mut probe_tag = String::new();
    for i in 0..spec.feeders {
        let c = i / per_cabinet;
        let n = i + 1;
        let main_page = tx.add_page(format!("{}", 2 * n - 1))?;
        let ctl_page = tx.add_page(format!("{}", 2 * n))?;
        tx.update::<novo_core::Page>(main_page, |p| p.title = format!("Feeder {n} main"))?;
        tx.update::<novo_core::Page>(ctl_page, |p| p.title = format!("Feeder {n} control"))?;
        pages.extend([main_page, ctl_page]);

        let cab_tag = format!("=A1+CAB{}", c + 1);
        let mut dev = |tx: &mut Tx<'_>, letter: &str, number: usize, part: usize, text: &str| {
            let d = tx.add_device(tag(&format!("{cab_tag}-{letter}{number}")), text)?;
            tx.assign_part(d, parts[part])?;
            devices.push(d);
            Ok::<_, novo_core::ModelError>(d)
        };
        let f1 = dev(tx, "F", 3 * n - 2, 0, "Feeder fuses")?;
        let k1 = dev(tx, "K", n, 1, "Motor contactor")?;
        let f2 = dev(tx, "F", 3 * n - 1, 2, "Overload relay")?;
        let m1 = dev(tx, "M", n, 3, "Motor")?;
        let f3 = dev(tx, "F", 3 * n, 4, "Control breaker")?;
        let s0 = dev(tx, "S", 2 * n - 1, 5, "Stop")?;
        let s1 = dev(tx, "S", 2 * n, 6, "Start")?;
        tx.set_property(m1, "power_kw", Some(Value::Float(2.2)))?;
        if i == spec.feeders / 2 {
            probe_tag = tx
                .project()
                .get::<Device>(k1)
                .map(|d| d.tag.to_string())
                .unwrap_or_default();
        }

        // Cabinet layout: devices left to right on the rails, 10 per row.
        let slot = i % per_cabinet;
        let (cab_id, x0) = (cabinets[c], 60.0 + 150.0 * slot as f64);
        for (row, d) in [f1, k1, f2, f3, s0, s1].into_iter().enumerate() {
            let rail_y = 150.0 + 230.0 * row as f64 + 30.0;
            novo_layout::model::place_device(tx, cab_id, d.raw(), x0, rail_y, false)?;
        }

        // ----- main page -------------------------------------------------
        let feed = |tx: &mut Tx<'_>, page, potential, x| {
            tx.place_symbol(
                page,
                "iec/connectivity/potential_feed",
                at(x, 20.0),
                pins(&[("1", 0.0, 8.0)]),
                SymbolRole::PotentialFeed { potential },
            )
        };
        let feeds = [
            feed(tx, main_page, pot.l[0], 40.0)?,
            feed(tx, main_page, pot.l[1], 48.0)?,
            feed(tx, main_page, pot.l[2], 56.0)?,
        ];
        let feed_pe = feed(tx, main_page, pot.pe, 72.0)?;
        let pole = |tx: &mut Tx<'_>, key, y, d| {
            tx.place_symbol(main_page, key, at(40.0, y), three_pole(), role(d, true))
        };
        let f1_sym = pole(tx, "iec/protection/fuse_3p", 40.0, f1)?;
        let k1_main = tx.place_symbol(
            main_page,
            "iec/contactors/main_3p",
            at(40.0, 72.0),
            three_pole(),
            role(k1, false),
        )?;
        let f2_main = pole(tx, "iec/protection/overload_relay", 104.0, f2)?;
        tx.update::<SymbolInstance>(f2_main, |s| {
            s.bridges = vec![vec![0, 1], vec![2, 3], vec![4, 5]]
        })?;

        let mut x_syms = Vec::new();
        for (j, (name, x)) in [("1", 40.0), ("2", 48.0), ("3", 56.0), ("PE", 72.0)]
            .into_iter()
            .enumerate()
        {
            let terminal = tx.add_terminal(strips[c], format!("{n}.{name}"))?;
            if j == 3 {
                tx.update::<Terminal>(terminal, |t| t.kind = TerminalKind::ProtectiveEarth)?;
            }
            let key = if j == 3 {
                "iec/terminals/terminal_pe"
            } else {
                "iec/terminals/terminal"
            };
            x_syms.push(tx.place_symbol(
                main_page,
                key,
                at(x, 136.0),
                pins(&[("1", 0.0, 0.0), ("2", 0.0, 8.0)]),
                SymbolRole::Terminal {
                    terminals: vec![terminal],
                },
            )?);
        }
        let m1_sym = tx.place_symbol(
            main_page,
            "iec/motors/motor_3ph",
            at(40.0, 180.0),
            pins(&[
                ("U1", 0.0, 0.0),
                ("V1", 8.0, 0.0),
                ("W1", 16.0, 0.0),
                ("PE", 24.0, 0.0),
            ]),
            role(m1, true),
        )?;
        for phase in 0..3u16 {
            let (top, bottom) = (phase * 2, phase * 2 + 1);
            let p = usize::from(phase);
            tx.connect(Endpoint::pin(feeds[p], 0), Endpoint::pin(f1_sym, top))?;
            tx.connect(Endpoint::pin(f1_sym, bottom), Endpoint::pin(k1_main, top))?;
            tx.connect(Endpoint::pin(k1_main, bottom), Endpoint::pin(f2_main, top))?;
            tx.connect(Endpoint::pin(f2_main, bottom), Endpoint::pin(x_syms[p], 0))?;
        }
        tx.connect(Endpoint::pin(feed_pe, 0), Endpoint::pin(x_syms[3], 0))?;
        let mut motor_wires = Vec::new();
        for (j, terminal) in x_syms.iter().enumerate() {
            let pin = u16::try_from(j).expect("four pins");
            motor_wires.push(tx.connect(Endpoint::pin(*terminal, 1), Endpoint::pin(m1_sym, pin))?);
        }
        let mut cores = Vec::new();
        for name in ["BK", "BN", "GY", "GNYE"] {
            let mut core = CableCore::new(name);
            core.cross_section_mm2 = Some(1.5);
            cores.push(core);
        }
        let w1 = tx.add_cable(tag(&format!("=A1-W{n}")), cores)?;
        for (core, wire) in motor_wires.iter().enumerate() {
            tx.assign_core(w1, core, Some(*wire))?;
        }
        tx.update::<novo_core::Cable>(w1, |c| {
            c.part = Some(cable_part);
            c.length_m = Some(12.0);
        })?;

        // ----- control page ----------------------------------------------
        let feed_l = feed(tx, ctl_page, pot.ctl_l, 40.0)?;
        let single = |tx: &mut Tx<'_>, key, x, y, pair: [&str; 2], d, main| {
            tx.place_symbol(
                ctl_page,
                key,
                at(x, y),
                two_pin(pair[0], pair[1]),
                role(d, main),
            )
        };
        let f3_sym = single(
            tx,
            "iec/protection/mcb_1p",
            40.0,
            40.0,
            ["1", "2"],
            f3,
            true,
        )?;
        let s0_sym = single(
            tx,
            "iec/operators/pushbutton_nc",
            40.0,
            72.0,
            ["11", "12"],
            s0,
            true,
        )?;
        let s1_sym = single(
            tx,
            "iec/operators/pushbutton_no",
            40.0,
            104.0,
            ["13", "14"],
            s1,
            true,
        )?;
        let k1_aux = single(tx, "iec/contacts/no", 56.0, 104.0, ["13", "14"], k1, false)?;
        let f2_aux = single(
            tx,
            "iec/contacts/thermal_nc",
            40.0,
            136.0,
            ["95", "96"],
            f2,
            false,
        )?;
        let k1_coil = single(tx, "iec/coils/relay", 40.0, 168.0, ["A1", "A2"], k1, true)?;
        let feed_n = tx.place_symbol(
            ctl_page,
            "iec/connectivity/potential_feed",
            at(40.0, 192.0),
            pins(&[("1", 0.0, 8.0)]),
            SymbolRole::PotentialFeed { potential: pot.n },
        )?;
        tx.connect(Endpoint::pin(feed_l, 0), Endpoint::pin(f3_sym, 0))?;
        tx.connect(Endpoint::pin(f3_sym, 1), Endpoint::pin(s0_sym, 0))?;
        let split = tx.add_junction(ctl_page, Point::new(40.0, 96.0))?;
        tx.connect(Endpoint::pin(s0_sym, 1), Endpoint::junction(split))?;
        tx.connect(Endpoint::junction(split), Endpoint::pin(s1_sym, 0))?;
        tx.connect(Endpoint::junction(split), Endpoint::pin(k1_aux, 0))?;
        let join = tx.add_junction(ctl_page, Point::new(40.0, 128.0))?;
        tx.connect(Endpoint::pin(s1_sym, 1), Endpoint::junction(join))?;
        tx.connect(Endpoint::pin(k1_aux, 1), Endpoint::junction(join))?;
        tx.connect(Endpoint::junction(join), Endpoint::pin(f2_aux, 0))?;
        tx.connect(Endpoint::pin(f2_aux, 1), Endpoint::pin(k1_coil, 0))?;
        tx.connect(Endpoint::pin(k1_coil, 1), Endpoint::pin(feed_n, 0))?;

        let single_pole = vec![vec![0, 1]];
        let three = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
        for (symbol, groups) in [
            (f1_sym, &three),
            (k1_main, &three),
            (f3_sym, &single_pole),
            (s0_sym, &single_pole),
            (s1_sym, &single_pole),
            (k1_aux, &single_pole),
            (f2_aux, &single_pole),
        ] {
            tx.update::<SymbolInstance>(symbol, |s| s.pass_through = groups.clone())?;
        }
        if i == spec.feeders / 2 {
            probe = Some(k1_coil);
        }
    }
    // Every strip uses the same terminal block part.
    for strip in &strips {
        tx.update::<Device>(*strip, |d| {
            d.properties.insert(
                "terminal_part.feedthrough".into(),
                Value::Int(i64::try_from(terminal_part.0).expect("small id")),
            );
        })?;
    }
    Ok(Big {
        pages,
        cabinets,
        devices,
        probe: probe.expect("at least one feeder"),
        probe_tag,
    })
}
