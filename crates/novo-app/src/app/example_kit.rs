//! Small drawing kit for the larger example projects (`example_mcc`, ...):
//! places library symbols by key, wires pins by name, and gives devices
//! parts. It goes through the same editor commands a user's clicks do
//! (`novo_editor::edit::place` and `draw_wire`), so the examples carry the
//! same cross references, terminals and potentials as hand-drawn pages.

use std::collections::BTreeMap;

use novo_core::{
    Device, DeviceId, DeviceTag, MountingPlate, Page, PageId, Placement, Point, PotentialId,
    Result as ModelResult, Rotation, SymbolId, SymbolInstance, SymbolRole, Terminal, Tx, Value,
    Wire, WireId,
};
use novo_editor::catalog::Catalog;
use novo_editor::edit::{PlaceOptions, draw_wire, place};
use novo_editor::geometry::Target;
use novo_layout::enclosure::{Door, Enclosure, Hinge};

/// A symbol and one of its pins, by the pin's name in the library.
pub(super) type Pin<'a> = (SymbolId, &'a str);

/// One end of a wire: a pin or a junction.
#[derive(Clone, Copy)]
pub(super) enum End<'a> {
    Pin(Pin<'a>),
    /// A pin by position in the symbol (first, second), for contacts and
    /// coils whose pin names depend on the contact numbers already used.
    Idx(SymbolId, u16),
    Junction(novo_core::JunctionId),
}

pub(super) fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("the example tags are valid")
}

/// Adds a drawn page filed under `structure` (`=MCC+CAB`).
pub(super) fn add_page(
    tx: &mut Tx<'_>,
    name: &str,
    title: &str,
    structure: &str,
) -> ModelResult<PageId> {
    let page = tx.add_page(name)?;
    tx.update::<Page>(page, |p| {
        p.title = title.into();
        p.structure = tag(structure);
    })?;
    Ok(page)
}

/// Drawing helper bound to one transaction and one current page.
pub(super) struct Kit<'a, 't> {
    pub tx: &'a mut Tx<'t>,
    pub page: PageId,
    catalog: Catalog,
}

impl<'a, 't> Kit<'a, 't> {
    pub fn new(tx: &'a mut Tx<'t>, page: PageId) -> Self {
        Kit {
            tx,
            page,
            catalog: Catalog::builtin(),
        }
    }

    /// Continues on another page.
    pub fn on(&mut self, page: PageId) {
        self.page = page;
    }

    /// Places `key` with its pin 0 at (`x`, `y`) mm. `device` is the full
    /// tag of the device it belongs to (`=MCC+CAB-K1`); the main function and
    /// the contacts of one device share the tag. An empty tag places a
    /// symbol that makes no device of its own choice (terminals, earth).
    pub fn put(&mut self, key: &str, at: (f64, f64), device: &str) -> SymbolId {
        let mut options = PlaceOptions::new();
        if !device.is_empty() {
            options.parent = Some(tag(device));
        }
        self.place_with(key, at, &options)
    }

    fn place_with(&mut self, key: &str, at: (f64, f64), options: &PlaceOptions) -> SymbolId {
        let symbol = self
            .catalog
            .get(key)
            .unwrap_or_else(|| panic!("{key} is not in the symbol library"));
        let placement = Placement {
            origin: Point::new(at.0, at.1),
            rotation: Rotation::R0,
            mirrored: false,
        };
        place(self.tx, self.page, symbol, placement, options)
            .unwrap_or_else(|e| panic!("placing {key} at {at:?}: {e}"))
    }

    /// A potential feed (`L1`, `N`, `+24V`) with its pin at (`x`, `y` + 8).
    pub fn feed(&mut self, potential: PotentialId, at: (f64, f64)) -> SymbolId {
        let mut options = PlaceOptions::new();
        options.potential = Some(potential);
        self.place_with("iec/connectivity/potential_feed", at, &options)
    }

    /// A potential feed drawn upside down: its pin is at the top, the wire
    /// leaves upward. For a potential defined at the output of a device above.
    pub fn feed_down(&mut self, potential: PotentialId, pin_at: (f64, f64)) -> SymbolId {
        let mut options = PlaceOptions::new();
        options.potential = Some(potential);
        let symbol = self
            .catalog
            .get("iec/connectivity/potential_feed")
            .expect("feed");
        let placement = Placement {
            origin: Point::new(pin_at.0, pin_at.1 + 8.0),
            rotation: Rotation::R180,
            mirrored: false,
        };
        place(self.tx, self.page, symbol, placement, &options).expect("feed placed")
    }

    /// A junction at `at`, to wire several pins to one point.
    pub fn junction(&mut self, at: (f64, f64)) -> novo_core::JunctionId {
        self.tx
            .add_junction(self.page, Point::new(at.0, at.1))
            .expect("junction")
    }

    /// A protective earth feed.
    pub fn feed_pe(&mut self, potential: PotentialId, at: (f64, f64)) -> SymbolId {
        let mut options = PlaceOptions::new();
        options.potential = Some(potential);
        self.place_with("iec/connectivity/potential_feed_pe", at, &options)
    }

    /// A terminal of `strip` named `name`. `key` is the terminal symbol
    /// (`iec/terminals/terminal`, `terminal_pe`, `terminal_n`).
    pub fn term(&mut self, strip: DeviceId, name: &str, key: &str, at: (f64, f64)) -> SymbolId {
        let mut options = PlaceOptions::new();
        options.strip = Some(strip);
        let symbol = self.place_with(key, at, &options);
        let terminals = match self.tx.project().get::<SymbolInstance>(symbol) {
            Some(SymbolInstance {
                role: SymbolRole::Terminal { terminals },
                ..
            }) => terminals.clone(),
            _ => Vec::new(),
        };
        if let Some(&t) = terminals.first() {
            self.tx
                .update::<Terminal>(t, |t| t.name = name.into())
                .expect("the terminal was just placed");
        }
        symbol
    }

    fn pin_index(&self, (symbol, name): Pin<'_>) -> u16 {
        let instance = self
            .tx
            .project()
            .get::<SymbolInstance>(symbol)
            .expect("the symbol was placed");
        let index = instance
            .pins
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| {
                let names: Vec<_> = instance.pins.iter().map(|p| p.name.as_str()).collect();
                panic!("{:?} has no pin {name}; it has {names:?}", instance.symbol)
            });
        u16::try_from(index).expect("few pins")
    }

    fn endpoint(&self, end: End<'_>) -> novo_core::Endpoint {
        match end {
            End::Pin(p) => novo_core::Endpoint::pin(p.0, self.pin_index(p)),
            End::Idx(s, i) => novo_core::Endpoint::pin(s, i),
            End::Junction(j) => novo_core::Endpoint::junction(j),
        }
    }

    /// Wires two ends, routed orthogonally through `corners` like a drawn
    /// wire. A route the editor refuses (it would run along another wire) is
    /// still a connection; the straight line is what gets stored then.
    pub fn link(&mut self, a: End<'_>, b: End<'_>, corners: &[(f64, f64)]) -> WireId {
        let (ea, eb) = (self.endpoint(a), self.endpoint(b));
        let corners: Vec<Point> = corners.iter().map(|&(x, y)| Point::new(x, y)).collect();
        let at = |kit: &Self, e| {
            kit.tx
                .project()
                .endpoint_position(e)
                .expect("the end has a position")
        };
        let (pa, pb) = (at(self, ea), at(self, eb));
        match draw_wire(
            self.tx,
            self.page,
            Target::Endpoint(ea, pa),
            &corners,
            Target::Endpoint(eb, pb),
        ) {
            Ok(wire) => wire,
            Err(_) => self.tx.connect(ea, eb).expect("both ends exist"),
        }
    }

    /// Wires two pins.
    pub fn wire(&mut self, a: Pin<'_>, b: Pin<'_>) -> WireId {
        self.link(End::Pin(a), End::Pin(b), &[])
    }

    /// [`Kit::wire`] through the given corners.
    pub fn wire_via(&mut self, a: Pin<'_>, b: Pin<'_>, corners: &[(f64, f64)]) -> WireId {
        self.link(End::Pin(a), End::Pin(b), corners)
    }

    /// [`Kit::wire`] with the conductor's cross-section in mm².
    pub fn wire_mm2(&mut self, a: Pin<'_>, b: Pin<'_>, mm2: f64) -> WireId {
        let wire = self.wire(a, b);
        self.set_mm2(wire, mm2);
        wire
    }

    /// Sets the cross-section of a drawn wire.
    pub fn set_mm2(&mut self, wire: WireId, mm2: f64) {
        self.tx
            .update::<Wire>(wire, |w| w.cross_section_mm2 = Some(mm2))
            .expect("the wire was just drawn");
    }

    /// Three wires between equal-numbered poles: `a` pins to `b` pins in
    /// order, all `mm2`.
    pub fn wire3(&mut self, a: (SymbolId, [&str; 3]), b: (SymbolId, [&str; 3]), mm2: f64) {
        for i in 0..3 {
            self.wire_mm2((a.0, a.1[i]), (b.0, b.1[i]), mm2);
        }
    }

    /// A cable `text` (`=MCC-W1`) whose cores `names` carry `wires` in order,
    /// all `mm2`, `length_m` long.
    pub fn cable(
        &mut self,
        text: &str,
        names: &[&str],
        wires: &[WireId],
        mm2: f64,
        length_m: f64,
        description: &str,
    ) -> ModelResult<novo_core::CableId> {
        let cores = names
            .iter()
            .map(|n| {
                let mut core = novo_core::CableCore::new(*n);
                core.cross_section_mm2 = Some(mm2);
                core
            })
            .collect();
        let cable = self.tx.add_cable(tag(text), cores)?;
        for (core, wire) in wires.iter().enumerate() {
            self.tx.assign_core(cable, core, Some(*wire))?;
        }
        self.tx.update::<novo_core::Cable>(cable, |c| {
            c.length_m = Some(length_m);
            c.description = description.into();
        })?;
        Ok(cable)
    }

    /// A device by tag, which must exist (the symbol that names it has been
    /// placed).
    pub fn device(&self, text: &str) -> DeviceId {
        self.tx
            .project()
            .devices_by_tag(&tag(text))
            .first()
            .copied()
            .unwrap_or_else(|| panic!("no device {text}"))
    }

    /// Sets a description on the device `text`.
    pub fn describe(&mut self, text: &str, description: &str) {
        let device = self.device(text);
        self.tx
            .update::<Device>(device, |d| d.description = description.into())
            .expect("the device exists");
    }

    /// Sets numeric properties (`rated_current_a`, `power_kw`, ...) on the
    /// device `text`.
    pub fn props(&mut self, text: &str, props: &[(&str, f64)]) {
        let device = self.device(text);
        self.tx
            .update::<Device>(device, |d| {
                for (key, value) in props {
                    d.properties.insert((*key).into(), Value::Float(*value));
                }
            })
            .expect("the device exists");
    }
}

/// An article not in the starter catalog. Planning values, marked as such:
/// there is no maker name to make an order number look verified.
#[derive(Clone, Copy)]
pub(super) struct Article {
    pub order: &'static str,
    pub description: &'static str,
    pub size: (f64, f64, f64),
    pub loss_w: f64,
    pub props: &'static [(&'static str, f64)],
}

/// Hands out one part per article and assigns it to devices. Articles in the
/// starter catalog are named by (maker, order number) only and get their data
/// from it when the example is finished (`give_parts_their_data`).
#[derive(Default)]
pub(super) struct Parts {
    made: BTreeMap<(String, String), novo_core::PartId>,
}

impl Parts {
    /// A catalog article.
    pub fn catalog(
        &mut self,
        tx: &mut Tx<'_>,
        device: DeviceId,
        maker: &str,
        order: &str,
    ) -> ModelResult<()> {
        let part = self.part(tx, maker, order)?;
        tx.assign_part(device, part)
    }

    /// A generic article with planning values.
    pub fn generic(
        &mut self,
        tx: &mut Tx<'_>,
        device: DeviceId,
        article: &Article,
    ) -> ModelResult<()> {
        let part = self.generic_part(tx, article)?;
        tx.assign_part(device, part)
    }

    /// The project part of a catalog article.
    pub fn catalog_part(
        &mut self,
        tx: &mut Tx<'_>,
        maker: &str,
        order: &str,
    ) -> ModelResult<novo_core::PartId> {
        self.part(tx, maker, order)
    }

    /// The project part of a generic article, made on first use.
    pub fn generic_part(
        &mut self,
        tx: &mut Tx<'_>,
        article: &Article,
    ) -> ModelResult<novo_core::PartId> {
        let fresh = !self
            .made
            .contains_key(&("Generic".to_owned(), article.order.to_owned()));
        let part = self.part(tx, "Generic", article.order)?;
        if fresh {
            tx.update::<novo_core::Part>(part, |p| {
                p.description = article.description.into();
                p.type_number = article.order.into();
                let (w, h, d) = article.size;
                if w > 0.0 {
                    for (key, value) in [("width_mm", w), ("height_mm", h), ("depth_mm", d)] {
                        p.properties.insert(key.into(), Value::Float(value));
                    }
                    p.properties
                        .insert("power_loss_w".into(), Value::Float(article.loss_w));
                    // A planning limit for the heat result: the 60 C most
                    // control gear is rated for in a cabinet.
                    p.properties
                        .insert("max_ambient_c".into(), Value::Float(60.0));
                    p.properties.insert(
                        novo_thermal::bridge::LOSS_SOURCE_PROPERTY.into(),
                        Value::Text("estimated".into()),
                    );
                }
                for (key, value) in article.props {
                    p.properties.insert((*key).into(), Value::Float(*value));
                }
            })?;
        }
        Ok(part)
    }

    fn part(
        &mut self,
        tx: &mut Tx<'_>,
        maker: &str,
        order: &str,
    ) -> ModelResult<novo_core::PartId> {
        let key = (maker.to_owned(), order.to_owned());
        if let Some(&part) = self.made.get(&key) {
            return Ok(part);
        }
        let part = tx.add_part(maker, order)?;
        self.made.insert(key, part);
        Ok(part)
    }
}

/// The footprint of a terminal strip: no single part, so its own size.
/// `blocks` of `pitch` mm plus two end covers; 0.3 W a block at rated
/// current, the starter catalog's estimate.
pub(super) fn size_strip(
    tx: &mut Tx<'_>,
    strip: DeviceId,
    blocks: usize,
    pitch: f64,
) -> ModelResult<()> {
    let n = blocks as f64;
    tx.update::<Device>(strip, |d| {
        for (key, value) in [
            ("width_mm", n * pitch + 2.0 * 2.2),
            ("height_mm", 47.5),
            ("depth_mm", 42.5),
            ("power_loss_w", 0.3 * n),
        ] {
            d.properties.insert(key.into(), Value::Float(value));
        }
        d.properties.insert(
            novo_thermal::bridge::LOSS_SOURCE_PROPERTY.into(),
            Value::Text("estimated".into()),
        );
    })
}

/// A cabinet with a mounting plate and a right-hinged door.
pub(super) fn enclosure(plate: (f64, f64), outside: (f64, f64, f64)) -> (MountingPlate, Enclosure) {
    (
        MountingPlate {
            width: plate.0,
            height: plate.1,
            margin: 10.0,
        },
        Enclosure {
            width: outside.0,
            height: outside.1,
            depth: outside.2,
            plate_setback: 20.0,
            door: Some(Door {
                hinge: Hinge::Right,
                max_angle: 110.0,
                inner_depth: 0.0,
            }),
            front_clearance: Some(1000.0),
            hinge_side_clearance: None,
        },
    )
}

/// What the finishing step of an example needs to know.
pub(super) struct Finish<'a> {
    pub name: &'a str,
    pub description: &'a str,
    /// The cabinet's location tag (`+CAB`).
    pub cabinet: &'a str,
    pub plate: MountingPlate,
    pub enclosure: Enclosure,
    pub supply_v: f64,
    pub short_circuit_ka: f64,
}

/// What every example does after its pages are drawn, in the order the
/// motor starter does it: files the pages by their tags, numbers the wires,
/// copies catalog data into the parts, plans the cabinet (rails, ducts and
/// places, the same as Place all) and states the supply.
pub(super) fn finish(tx: &mut Tx<'_>, finish: &Finish<'_>) -> ModelResult<()> {
    super::outline_tree::auto_file(tx)?;
    novo_numbering::number_new_wires(tx)?;
    super::example::give_parts_their_data(tx)?;
    let cab = super::example::prepare_cabinet(tx, finish.cabinet, finish.plate, &finish.enclosure)?;
    super::example::plan_cabinet(tx, cab)?;
    tx.set_info(|info| {
        info.name = finish.name.into();
        info.description = finish.description.into();
        info.properties.insert(
            novo_checks::calc_view::inputs::SUPPLY_VOLTAGE_V.into(),
            finish.supply_v.into(),
        );
        info.properties.insert(
            novo_checks::standards::keys::SUPPLY_SHORT_CIRCUIT_KA.into(),
            finish.short_circuit_ka.into(),
        );
    })
}

/// A vertical chain of two-pin elements (a control circuit rung): where the
/// next element goes and the end it hangs from.
pub(super) struct Col<'a> {
    pub x: f64,
    pub y: f64,
    pub last: End<'a>,
}

impl<'a, 't> Kit<'a, 't> {
    /// Starts a rung at `x` with a `potential` feed at the top.
    pub fn rung(&mut self, potential: PotentialId, x: f64) -> Col<'static> {
        let feed = self.feed(potential, (x, 24.0));
        Col {
            x,
            y: 40.0,
            last: End::Pin((feed, "1")),
        }
    }

    /// Hangs the two-pin element `key` of device `device` under the rung and
    /// continues from its second pin.
    pub fn series(&mut self, col: &mut Col<'_>, key: &str, device: &str) -> SymbolId {
        let s = self.put(key, (col.x, col.y), device);
        self.link(col.last, End::Idx(s, 0), &[]);
        col.last = End::Idx(s, 1);
        col.y += 24.0;
        s
    }

    /// A start button in parallel with the holding contact of `contactor`
    /// (the self-holding branch of a start/stop rung).
    pub fn hold(&mut self, col: &mut Col<'_>, button: &str, start: &str, contactor: &str) {
        self.either(col, button, start, "iec/contacts/no", contactor);
    }

    /// Two elements in parallel (an OR): `key_a` of device `dev_a` and
    /// `key_b` of device `dev_b`, side by side under the rung.
    pub fn either(
        &mut self,
        col: &mut Col<'_>,
        key_a: &str,
        dev_a: &str,
        key_b: &str,
        dev_b: &str,
    ) {
        let y = col.y;
        let top = self.junction((col.x, y));
        self.link(col.last, End::Junction(top), &[]);
        let a = self.put(key_a, (col.x, y + 8.0), dev_a);
        let b = self.put(key_b, (col.x + 16.0, y + 8.0), dev_b);
        self.link(End::Junction(top), End::Idx(a, 0), &[]);
        self.link(End::Junction(top), End::Idx(b, 0), &[]);
        let bottom = self.junction((col.x, y + 32.0));
        self.link(End::Idx(a, 1), End::Junction(bottom), &[]);
        self.link(End::Idx(b, 1), End::Junction(bottom), &[]);
        col.last = End::Junction(bottom);
        col.y = y + 40.0;
    }

    /// A terminal of `strip` named `name` in the rung.
    pub fn series_term(&mut self, col: &mut Col<'_>, strip: DeviceId, name: &str) -> SymbolId {
        let s = self.term(strip, name, "iec/terminals/terminal", (col.x, col.y));
        self.link(col.last, End::Idx(s, 0), &[]);
        col.last = End::Idx(s, 1);
        col.y += 24.0;
        s
    }

    /// Ends a rung on `potential` (a feed under the last element).
    pub fn close(&mut self, col: &Col<'_>, potential: PotentialId) {
        let feed = self.feed(potential, (col.x, col.y));
        self.link(col.last, End::Pin((feed, "1")), &[]);
    }
}

/// Gives every wire that has no cross-section `mm2` (the control wiring).
pub(super) fn fill_sections(tx: &mut Tx<'_>, mm2: f64) -> ModelResult<()> {
    let bare: Vec<WireId> = tx
        .project()
        .wires()
        .filter(|w| w.cross_section_mm2.is_none())
        .map(|w| w.id)
        .collect();
    for id in bare {
        tx.update::<Wire>(id, |w| w.cross_section_mm2 = Some(mm2))?;
    }
    Ok(())
}

/// Gives a terminal strip its footprint (`blocks` terminal blocks) and the
/// catalog blocks its feed-through, neutral and protective earth terminals
/// are made of.
pub(super) fn style_strip(
    tx: &mut Tx<'_>,
    parts: &mut Parts,
    strip: DeviceId,
    blocks: usize,
) -> ModelResult<()> {
    size_strip(tx, strip, blocks, 5.2)?;
    for (kind, order) in [
        (novo_core::TerminalKind::Feedthrough, "3044076"),
        (novo_core::TerminalKind::Neutral, "3044076"),
        (novo_core::TerminalKind::ProtectiveEarth, "3044089"),
    ] {
        let part = parts.catalog_part(tx, "Phoenix Contact", order)?;
        tx.update::<Device>(strip, |d| {
            d.properties.insert(
                novo_terminals::strip_part_key(kind),
                Value::Int(i64::try_from(part.0).expect("small id")),
            );
        })?;
    }
    Ok(())
}
