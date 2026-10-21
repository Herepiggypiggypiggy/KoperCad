//! A big synthetic project for scale measurements.
//!
//! A *feeder* is one circuit drawn over two pages. Unless `uniform` is set,
//! feeders come in five kinds, so the project is not one drawing repeated:
//!
//! * motor starters (three of five): fuses, contactor, overload relay,
//!   terminals, motor and a cable; the motor power (0.37 to 11 kW) picks the
//!   parts, the cable cross-section and the cable length (5 to 80 m);
//! * three-phase heater groups: breaker, contactor, heater and a thermostat
//!   loop;
//! * I/O loops: 4 to 12 sensors on a digital input module and as many valves
//!   on a digital output module, with terminals in between.
//!
//! Cabinets differ in size and in how many feeders they hold. The catalogue is
//! about 45 distinct parts with different sizes and losses. All of it comes
//! from a seeded generator, so a given spec makes the same project every time.
//!
//! The geometry is deliberately plain: this is a load generator, not a design.
//! Wire lengths come from where the devices sit on the plates, so they vary with
//! the cabinet and the slot.

use novo_core::{
    CableCore, Device, DeviceId, DeviceTag, Document, Endpoint, MountingPlate, PageId, PartId, Pin,
    Placement, Point, PotentialId, PotentialKind, RailKind, Result, StructureId, SymbolId,
    SymbolInstance, SymbolRole, Terminal, TerminalKind, Tx, Value,
};

/// How big the project is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigSpec {
    /// Circuits; the project has `2 * feeders` pages.
    pub feeders: usize,
    /// Feeders per cabinet on average (each cabinet gets a plate, rails and a
    /// strip; the real count varies by up to 3 per cabinet).
    pub per_cabinet: usize,
    /// Every feeder the same motor starter, as the first version of the
    /// generator made them: the best case for caches and a fair baseline.
    pub uniform: bool,
    /// Seed of the pseudo-random choices.
    pub seed: u64,
}

impl BigSpec {
    /// 20 feeders: 40 pages, a typical machine.
    pub const SMALL: BigSpec = BigSpec::new(20);
    /// 100 feeders: 200 pages.
    pub const MEDIUM: BigSpec = BigSpec::new(100);
    /// 300 feeders: 600 pages.
    pub const LARGE: BigSpec = BigSpec::new(300);

    pub const fn new(feeders: usize) -> BigSpec {
        BigSpec {
            feeders,
            per_cabinet: 10,
            uniform: false,
            seed: 1,
        }
    }

    pub const fn uniform(self) -> BigSpec {
        BigSpec {
            uniform: true,
            ..self
        }
    }

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

/// Small deterministic generator (splitmix64): no dependency, same numbers on
/// every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }
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
    plus24: PotentialId,
    zero24: PotentialId,
}

/// Motor sizes: kW, contactor/overload current class, cable mm2.
const MOTORS: [(f64, usize, f64); 8] = [
    (0.37, 0, 1.5),
    (0.75, 0, 1.5),
    (1.5, 1, 1.5),
    (2.2, 1, 1.5),
    (4.0, 2, 2.5),
    (5.5, 3, 4.0),
    (7.5, 3, 6.0),
    (11.0, 4, 10.0),
];

/// Part ladders: (order-number stem, width, height, depth, loss) per size
/// class, smallest first.
struct Ladders {
    fuse: Vec<PartId>,
    contactor: Vec<PartId>,
    overload: Vec<PartId>,
    motor: Vec<PartId>,
    mcb1: Vec<PartId>,
    mcb3: Vec<PartId>,
    button: Vec<PartId>,
    heater: Vec<PartId>,
    sensor: PartId,
    valve: PartId,
    di: PartId,
    dout: PartId,
    terminal: PartId,
    cable: Vec<PartId>,
}

fn part(
    tx: &mut Tx<'_>,
    maker: &str,
    order: String,
    size: (f64, f64, f64),
    loss: f64,
) -> Result<PartId> {
    let part = tx.add_part(maker, order)?;
    for (k, v) in [
        ("width_mm", size.0),
        ("height_mm", size.1),
        ("depth_mm", size.2),
        ("power_loss_w", loss),
    ] {
        tx.set_property(part, k, Some(Value::Float(v)))?;
    }
    Ok(part)
}

fn ladders(tx: &mut Tx<'_>) -> Result<Ladders> {
    let mut fuse = Vec::new();
    let mut contactor = Vec::new();
    let mut overload = Vec::new();
    let mut motor = Vec::new();
    let mut mcb1 = Vec::new();
    let mut mcb3 = Vec::new();
    let mut button = Vec::new();
    let mut heater = Vec::new();
    for (i, amps) in [6, 10, 16, 25, 40].into_iter().enumerate() {
        let f = i as f64;
        fuse.push(part(
            tx,
            "Siemens",
            format!("3NW70{amps:02}"),
            (45.0 + 9.0 * f, 80.0, 60.0),
            1.5 + 0.8 * f,
        )?);
        contactor.push(part(
            tx,
            "Siemens",
            format!("3RT20{amps:02}-1AP01"),
            (45.0 + 5.0 * f, 85.0 + 4.0 * f, 90.0 + 6.0 * f),
            2.5 + 1.4 * f,
        )?);
        overload.push(part(
            tx,
            "Siemens",
            format!("3RU21{amps:02}-1FB0"),
            (45.0 + 5.0 * f, 85.0 + 3.0 * f, 90.0),
            3.0 + 1.5 * f,
        )?);
        motor.push(part(
            tx,
            "Siemens",
            format!("1LE1003-0E{amps:02}"),
            (110.0 + 20.0 * f, 110.0 + 20.0 * f, 200.0 + 30.0 * f),
            0.0,
        )?);
        mcb3.push(part(
            tx,
            "Siemens",
            format!("5SY63{amps:02}-7"),
            (54.0, 90.0, 70.0),
            3.0 + 1.0 * f,
        )?);
        heater.push(part(
            tx,
            "Heatrex",
            format!("HX-{}", 500 * (i + 1)),
            (140.0, 100.0 + 20.0 * f, 80.0),
            0.0,
        )?);
    }
    for (i, amps) in [2, 4, 6, 10, 16].into_iter().enumerate() {
        mcb1.push(part(
            tx,
            "Siemens",
            format!("5SY61{amps:02}-7"),
            (18.0, 90.0, 70.0),
            0.8 + 0.3 * i as f64,
        )?);
    }
    for (i, kind) in [
        "3SU1100-0AB20-1CA0",
        "3SU1100-0AB40-1BA0",
        "3SU1100-1HB20-1CA0",
    ]
    .into_iter()
    .enumerate()
    {
        button.push(part(
            tx,
            "Siemens",
            kind.into(),
            (22.0, 22.0, 40.0 + 5.0 * i as f64),
            0.0,
        )?);
    }
    let terminal = tx.add_part("Phoenix Contact", "3044076")?;
    let cable: Vec<PartId> = ["1119304", "1119305", "1119306", "1119307"]
        .into_iter()
        .map(|o| tx.add_part("Lapp", o))
        .collect::<Result<_>>()?;
    Ok(Ladders {
        fuse,
        contactor,
        overload,
        motor,
        mcb1,
        mcb3,
        button,
        heater,
        sensor: part(
            tx,
            "Pepperl+Fuchs",
            "NBB4-12GM50".into(),
            (14.0, 14.0, 50.0),
            0.1,
        )?,
        valve: part(tx, "Festo", "MFH-5-1/4".into(), (30.0, 60.0, 40.0), 0.0)?,
        di: part(
            tx,
            "Siemens",
            "6ES7521-1BL00".into(),
            (35.0, 147.0, 129.0),
            3.0,
        )?,
        dout: part(
            tx,
            "Siemens",
            "6ES7522-1BL01".into(),
            (35.0, 147.0, 129.0),
            4.5,
        )?,
        terminal,
        cable,
    })
}

/// Everything one feeder needs from the project.
struct Ctx<'a> {
    pot: &'a Potentials,
    parts: &'a Ladders,
    cab_tag: String,
    cab: StructureId,
    strip: DeviceId,
    /// Slot on the plate, and the plate width available per slot.
    slot: usize,
    pitch: f64,
    /// Rails on the plate; devices go on rail `row % rows`.
    rows: usize,
    devices: &'a mut Vec<DeviceId>,
    /// Tags are numbered by the feeder number.
    n: usize,
    pages: [PageId; 2],
}

fn feed(tx: &mut Tx<'_>, page: PageId, potential: PotentialId, x: f64, y: f64) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/connectivity/potential_feed",
        at(x, y),
        pins(&[("1", 0.0, 8.0)]),
        SymbolRole::PotentialFeed { potential },
    )
}

fn terminal_symbol(
    tx: &mut Tx<'_>,
    page: PageId,
    strip: DeviceId,
    name: &str,
    x: f64,
    y: f64,
) -> Result<SymbolId> {
    let terminal = tx.add_terminal(strip, name)?;
    tx.place_symbol(
        page,
        "iec/terminals/terminal",
        at(x, y),
        pins(&[("1", 0.0, 0.0), ("2", 0.0, 8.0)]),
        SymbolRole::Terminal {
            terminals: vec![terminal],
        },
    )
}

impl Ctx<'_> {
    /// Adds a device with its part and puts it on a rail of the cabinet.
    fn device(
        &mut self,
        tx: &mut Tx<'_>,
        letter: &str,
        number: usize,
        part: PartId,
        row: usize,
        text: &str,
    ) -> Result<DeviceId> {
        let d = tx.add_device(tag(&format!("{}-{letter}{number}", self.cab_tag)), text)?;
        tx.assign_part(d, part)?;
        self.devices.push(d);
        novo_layout::model::place_device(
            tx,
            self.cab,
            d.raw(),
            60.0 + self.pitch * self.slot as f64,
            150.0 + 230.0 * (row % self.rows) as f64 + 30.0,
            false,
        )?;
        Ok(d)
    }
}

/// Bridges (switched poles) of a symbol, as the symbol library says.
fn pass_through(tx: &mut Tx<'_>, symbol: SymbolId, poles: usize) -> Result<()> {
    let groups: Vec<Vec<u16>> = (0..poles)
        .map(|p| vec![2 * p as u16, 2 * p as u16 + 1])
        .collect();
    tx.update::<SymbolInstance>(symbol, |s| s.pass_through = groups)
}

/// Motor starter: the Gate 1 circuit with parts, cable and length by size.
fn motor_starter(
    tx: &mut Tx<'_>,
    c: &mut Ctx<'_>,
    rng: &mut Rng,
    uniform: bool,
) -> Result<SymbolId> {
    let (kw, class, mm2) = if uniform {
        MOTORS[3]
    } else {
        MOTORS[rng.below(MOTORS.len())]
    };
    let length_m = if uniform {
        12.0
    } else {
        rng.range(5, 80) as f64
    };
    let n = c.n;
    let p = c.parts;
    let (main_page, ctl_page) = (c.pages[0], c.pages[1]);
    let f1 = c.device(tx, "F", 3 * n - 2, p.fuse[class], 0, "Feeder fuses")?;
    let k1 = c.device(tx, "K", n, p.contactor[class], 1, "Motor contactor")?;
    let f2 = c.device(tx, "F", 3 * n - 1, p.overload[class], 2, "Overload relay")?;
    let m1 = c.device(tx, "M", n, p.motor[class], 6, "Motor")?;
    let f3 = c.device(tx, "F", 3 * n, p.mcb1[rng.below(3)], 3, "Control breaker")?;
    let s0 = c.device(tx, "S", 2 * n - 1, p.button[0], 4, "Stop")?;
    let s1 = c.device(tx, "S", 2 * n, p.button[1], 5, "Start")?;
    tx.set_property(m1, "power_kw", Some(Value::Float(kw)))?;

    let pot = c.pot;
    let feeds = [
        feed(tx, main_page, pot.l[0], 40.0, 20.0)?,
        feed(tx, main_page, pot.l[1], 48.0, 20.0)?,
        feed(tx, main_page, pot.l[2], 56.0, 20.0)?,
    ];
    let feed_pe = feed(tx, main_page, pot.pe, 72.0, 20.0)?;
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
        let terminal = tx.add_terminal(c.strip, format!("{n}.{name}"))?;
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
        let q = usize::from(phase);
        tx.connect(Endpoint::pin(feeds[q], 0), Endpoint::pin(f1_sym, top))?;
        tx.connect(Endpoint::pin(f1_sym, bottom), Endpoint::pin(k1_main, top))?;
        tx.connect(Endpoint::pin(k1_main, bottom), Endpoint::pin(f2_main, top))?;
        tx.connect(Endpoint::pin(f2_main, bottom), Endpoint::pin(x_syms[q], 0))?;
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
        core.cross_section_mm2 = Some(mm2);
        cores.push(core);
    }
    let w1 = tx.add_cable(tag(&format!("=A1-W{n}")), cores)?;
    for (core, wire) in motor_wires.iter().enumerate() {
        tx.assign_core(w1, core, Some(*wire))?;
    }
    let cable_part = p.cable[class.min(p.cable.len() - 1)];
    tx.update::<novo_core::Cable>(w1, |cable| {
        cable.part = Some(cable_part);
        cable.length_m = Some(length_m);
    })?;

    // Control page.
    let feed_l = feed(tx, ctl_page, pot.ctl_l, 40.0, 20.0)?;
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
    let feed_n = feed(tx, ctl_page, pot.n, 40.0, 192.0)?;
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
    for (symbol, poles) in [
        (f1_sym, 3),
        (k1_main, 3),
        (f3_sym, 1),
        (s0_sym, 1),
        (s1_sym, 1),
        (k1_aux, 1),
        (f2_aux, 1),
    ] {
        pass_through(tx, symbol, poles)?;
    }
    Ok(k1_coil)
}

/// Three-phase heater group: breaker, contactor, heater; a thermostat closes
/// the contactor coil.
fn heater_group(tx: &mut Tx<'_>, c: &mut Ctx<'_>, rng: &mut Rng) -> Result<SymbolId> {
    let class = rng.below(5);
    let n = c.n;
    let p = c.parts;
    let (main_page, ctl_page) = (c.pages[0], c.pages[1]);
    let q1 = c.device(tx, "Q", n, p.mcb3[class], 0, "Heater breaker")?;
    let k1 = c.device(tx, "K", n, p.contactor[class.min(3)], 1, "Heater contactor")?;
    let e1 = c.device(tx, "E", n, p.heater[class], 5, "Heater")?;
    let f3 = c.device(tx, "F", 3 * n, p.mcb1[rng.below(3)], 3, "Control breaker")?;
    let b1 = c.device(tx, "B", n, p.sensor, 4, "Thermostat")?;
    let pot = c.pot;
    let feeds = [
        feed(tx, main_page, pot.l[0], 40.0, 20.0)?,
        feed(tx, main_page, pot.l[1], 48.0, 20.0)?,
        feed(tx, main_page, pot.l[2], 56.0, 20.0)?,
    ];
    let q_sym = tx.place_symbol(
        main_page,
        "iec/protection/mcb_3p",
        at(40.0, 40.0),
        three_pole(),
        role(q1, true),
    )?;
    let k_main = tx.place_symbol(
        main_page,
        "iec/contactors/main_3p",
        at(40.0, 72.0),
        three_pole(),
        role(k1, false),
    )?;
    let e_sym = tx.place_symbol(
        main_page,
        "iec/passive/heater_3ph",
        at(40.0, 104.0),
        pins(&[("1", 0.0, 0.0), ("3", 8.0, 0.0), ("5", 16.0, 0.0)]),
        role(e1, true),
    )?;
    for phase in 0..3u16 {
        let (top, bottom) = (phase * 2, phase * 2 + 1);
        let i = usize::from(phase);
        tx.connect(Endpoint::pin(feeds[i], 0), Endpoint::pin(q_sym, top))?;
        tx.connect(Endpoint::pin(q_sym, bottom), Endpoint::pin(k_main, top))?;
        tx.connect(Endpoint::pin(k_main, bottom), Endpoint::pin(e_sym, phase))?;
    }
    pass_through(tx, q_sym, 3)?;
    pass_through(tx, k_main, 3)?;

    let feed_l = feed(tx, ctl_page, pot.ctl_l, 40.0, 20.0)?;
    let f3_sym = tx.place_symbol(
        ctl_page,
        "iec/protection/mcb_1p",
        at(40.0, 40.0),
        two_pin("1", "2"),
        role(f3, true),
    )?;
    let b_sym = tx.place_symbol(
        ctl_page,
        "iec/contacts/no",
        at(40.0, 72.0),
        two_pin("13", "14"),
        role(b1, true),
    )?;
    let coil = tx.place_symbol(
        ctl_page,
        "iec/coils/relay",
        at(40.0, 104.0),
        two_pin("A1", "A2"),
        role(k1, true),
    )?;
    let feed_n = feed(tx, ctl_page, pot.n, 40.0, 128.0)?;
    tx.connect(Endpoint::pin(feed_l, 0), Endpoint::pin(f3_sym, 0))?;
    tx.connect(Endpoint::pin(f3_sym, 1), Endpoint::pin(b_sym, 0))?;
    tx.connect(Endpoint::pin(b_sym, 1), Endpoint::pin(coil, 0))?;
    tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(feed_n, 0))?;
    pass_through(tx, f3_sym, 1)?;
    pass_through(tx, b_sym, 1)?;
    Ok(coil)
}

/// I/O loop: sensors on a digital input module, valves on an output module,
/// terminals on the strip in between.
fn io_loop(tx: &mut Tx<'_>, c: &mut Ctx<'_>, rng: &mut Rng) -> Result<SymbolId> {
    let channels = rng.range(4, 12);
    let n = c.n;
    let p = c.parts;
    let (in_page, out_page) = (c.pages[0], c.pages[1]);
    let pot = c.pot;
    let di = c.device(tx, "A", 2 * n - 1, p.di, 7, "Digital input module")?;
    let dout = c.device(tx, "A", 2 * n, p.dout, 7, "Digital output module")?;
    let module_pins = |prefix: &str| -> Vec<Pin> {
        (0..channels)
            .map(|k| Pin::new(format!("{prefix}{k}"), 8.0 * k as f64, 0.0))
            .collect()
    };
    let di_sym = tx.place_symbol(
        in_page,
        "iec/plc/digital_input",
        at(40.0, 100.0),
        module_pins("I"),
        role(di, true),
    )?;
    let do_sym = tx.place_symbol(
        out_page,
        "iec/plc/digital_output",
        at(40.0, 40.0),
        module_pins("Q"),
        role(dout, true),
    )?;
    let plus_in = feed(tx, in_page, pot.plus24, 20.0, 20.0)?;
    let zero_out = feed(tx, out_page, pot.zero24, 20.0, 160.0)?;
    let mut last_sensor = None;
    for k in 0..channels {
        let x = 40.0 + 8.0 * k as f64;
        let sensor = c.device(tx, "B", 1000 * n + k, p.sensor, 4, "Proximity sensor")?;
        let valve = c.device(tx, "Y", 1000 * n + k, p.valve, 5, "Valve")?;
        let t_in = terminal_symbol(tx, in_page, c.strip, &format!("{n}.I{k}"), x, 60.0)?;
        let b_sym = tx.place_symbol(
            in_page,
            "iec/contacts/no",
            at(x, 20.0),
            two_pin("1", "2"),
            role(sensor, true),
        )?;
        tx.connect(Endpoint::pin(plus_in, 0), Endpoint::pin(b_sym, 0))?;
        tx.connect(Endpoint::pin(b_sym, 1), Endpoint::pin(t_in, 0))?;
        tx.connect(Endpoint::pin(t_in, 1), Endpoint::pin(di_sym, k as u16))?;
        let t_out = terminal_symbol(tx, out_page, c.strip, &format!("{n}.Q{k}"), x, 80.0)?;
        let y_sym = tx.place_symbol(
            out_page,
            "iec/coils/solenoid_valve",
            at(x, 120.0),
            two_pin("A1", "A2"),
            role(valve, true),
        )?;
        tx.connect(Endpoint::pin(do_sym, k as u16), Endpoint::pin(t_out, 0))?;
        tx.connect(Endpoint::pin(t_out, 1), Endpoint::pin(y_sym, 0))?;
        tx.connect(Endpoint::pin(y_sym, 1), Endpoint::pin(zero_out, 0))?;
        last_sensor = Some(b_sym);
    }
    Ok(last_sensor.expect("at least four channels"))
}

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
        plus24: tx.add_potential("+24V", PotentialKind::DcPositive)?,
        zero24: tx.add_potential("0V", PotentialKind::DcNegative)?,
    };
    let parts = ladders(tx)?;
    let mut rng = Rng(spec.seed);

    // Cabinets differ in how many feeders they hold and so in size.
    let per_cabinet = spec.per_cabinet.max(1);
    let mut cabinets = Vec::new();
    let mut strips = Vec::new();
    let mut slots_of = Vec::new();
    let mut planned = 0;
    while planned < spec.feeders {
        let c = cabinets.len();
        let slots = if spec.uniform {
            per_cabinet
        } else {
            (per_cabinet + c % 4).saturating_sub(1).max(2)
        };
        let pitch = if spec.uniform {
            150.0
        } else {
            130.0 + 10.0 * (c % 4) as f64
        };
        let plate_w = 120.0 + pitch * slots as f64 + 60.0;
        let plate_h = if spec.uniform {
            1900.0
        } else {
            [1400.0, 1900.0, 2100.0][c % 3]
        };
        let cab = tx.add_structure(tag(&format!("+CAB{}", c + 1)), "Generated cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: plate_w,
                height: plate_h,
                margin: 10.0,
            }),
        )?;
        novo_layout::model::set_enclosure(
            tx,
            cab,
            &novo_layout::enclosure::Enclosure {
                width: plate_w + 100.0,
                height: plate_h + 100.0,
                depth: [300.0, 400.0, 500.0][c % 3],
                plate_setback: 0.0,
                door: None,
                front_clearance: None,
                hinge_side_clearance: None,
            },
        )?;
        // As many rails as the plate holds (the tall devices need 230 mm).
        let rows = ((plate_h - 250.0) / 230.0).floor() as usize;
        for row in 0..rows {
            tx.add_rail(
                cab,
                50.0,
                150.0 + 230.0 * row as f64,
                plate_w - 100.0,
                RailKind::DinRail,
            )?;
        }
        strips.push(tx.add_terminal_strip(tag(&format!("=A1+CAB{}-X1", c + 1)))?);
        cabinets.push(cab);
        slots_of.push((slots, pitch, rows));
        planned += slots;
    }

    let mut pages = Vec::new();
    let mut devices = Vec::new();
    let mut probe = None;
    let mut probe_tag = String::new();
    let (mut c, mut slot) = (0, 0);
    for i in 0..spec.feeders {
        if slot >= slots_of[c].0 {
            c += 1;
            slot = 0;
        }
        let n = i + 1;
        let main_page = tx.add_page(format!("{}", 2 * n - 1))?;
        let ctl_page = tx.add_page(format!("{}", 2 * n))?;
        tx.update::<novo_core::Page>(main_page, |p| p.title = format!("Circuit {n} main"))?;
        tx.update::<novo_core::Page>(ctl_page, |p| p.title = format!("Circuit {n} control"))?;
        pages.extend([main_page, ctl_page]);
        let mut ctx = Ctx {
            pot: &pot,
            parts: &parts,
            cab_tag: format!("=A1+CAB{}", c + 1),
            cab: cabinets[c],
            strip: strips[c],
            slot,
            pitch: slots_of[c].1,
            rows: slots_of[c].2,
            devices: &mut devices,
            n,
            pages: [main_page, ctl_page],
        };
        let before = ctx.devices.len();
        let kind = if spec.uniform { 0 } else { rng.below(5) };
        let marker = match kind {
            0..=2 => motor_starter(tx, &mut ctx, &mut rng, spec.uniform)?,
            3 => heater_group(tx, &mut ctx, &mut rng)?,
            _ => io_loop(tx, &mut ctx, &mut rng)?,
        };
        if i == spec.feeders / 2 {
            probe = Some(marker);
            // The first device the feeder added carries a tag that exists once.
            probe_tag = tx
                .project()
                .get::<Device>(devices[before])
                .map(|d| d.tag.to_string())
                .unwrap_or_default();
        }
        slot += 1;
    }
    // Every strip uses the same terminal block part.
    for strip in &strips {
        tx.update::<Device>(*strip, |d| {
            d.properties.insert(
                "terminal_part.feedthrough".into(),
                Value::Int(i64::try_from(parts.terminal.0).expect("small id")),
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
