//! Project to AutomationML (IEC 62714, CAEX 2.15).
//!
//! The plant becomes one instance hierarchy: the project, its plant sections
//! (`=A1`), its locations (`+CAB1`), and in them the devices, terminal strips
//! (terminals are the strip's interfaces) and cables (cores are child
//! elements). Connectivity is exported as nets: a net element with one
//! interface per member pin and one internal link per pin, so a reader gets
//! the netlist, junctions included, without any page geometry.
//!
//! Standard AutomationML roles and the standard `Frame` attribute (position in
//! millimetres) are used where they exist; KoperCAD's own roles live in the
//! `KoperCADRoleClassLib` that is written into the file, so the file is
//! valid on its own and a reader that does not know them still sees the
//! structure.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    CabinetPlacement, Device, DeviceId, DeviceKind, DeviceTag, Direction, Endpoint, PageId,
    Project, SymbolId, SymbolRole, TerminalId, Value,
};

use super::caex::{El, guid, write};
use crate::report::Report;

/// Library and role names shared by the writer and the reader.
pub const ROLE_LIB: &str = "KoperCADRoleClassLib";
pub const INTERFACE_LIB: &str = "KoperCADInterfaceClassLib";

/// What the file says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmlOptions {
    /// `YYYY-MM-DDTHH:MM:SS`; callers pass the clock so output is reproducible.
    pub created: String,
    pub version: String,
}

impl Default for AmlOptions {
    fn default() -> Self {
        AmlOptions {
            created: "1970-01-01T00:00:00".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// The finished file and what it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct AmlExport {
    pub text: String,
    pub report: Report,
}

fn role(name: &str) -> El {
    El::new("RoleRequirements").attr("RefBaseRoleClassPath", format!("{ROLE_LIB}/{name}"))
}

fn string(name: &str, value: &str) -> El {
    El::attribute(name, "xs:string", value)
}

fn value_attribute(name: &str, value: &Value) -> El {
    match value {
        Value::Bool(b) => El::attribute(name, "xs:boolean", b.to_string()),
        Value::Int(i) => El::attribute(name, "xs:long", i.to_string()),
        Value::Float(f) => El::attribute(name, "xs:double", crate::sheets::number_text(*f)),
        Value::Text(t) => string(name, t),
    }
}

fn properties(props: &BTreeMap<String, Value>) -> Option<El> {
    if props.is_empty() {
        return None;
    }
    let mut group = El::new("Attribute")
        .attr("Name", "Properties")
        .attr("AttributeDataType", "xs:string");
    for (k, v) in props {
        group.push(value_attribute(k, v));
    }
    Some(group)
}

/// The standard AutomationML `Frame`: where an object sits, in millimetres.
fn frame(x: f64, y: f64, z: f64, rz_deg: f64) -> El {
    let n = crate::sheets::number_text;
    let mut f = El::new("Attribute")
        .attr("Name", "Frame")
        .attr("RefSemantic", "AutomationMLBaseAttributeType/Frame");
    for (name, v) in [
        ("x", x),
        ("y", y),
        ("z", z),
        ("rx", 0.0),
        ("ry", 0.0),
        ("rz", rz_deg),
    ] {
        f.push(
            El::attribute(name, "xs:double", n(v))
                .attr("Unit", if name.starts_with('r') { "deg" } else { "mm" }),
        );
    }
    f
}

/// The node a tag hangs under: its location (`+CAB1`), else its function
/// (`=A1`), else the project itself (empty). Locations come first because
/// that is how a panel builder looks for a device: in the cabinet.
fn node_key(tag: &DeviceTag) -> String {
    let aspect = |function: bool| {
        if function {
            DeviceTag {
                function: tag.function.clone(),
                ..DeviceTag::default()
            }
        } else {
            DeviceTag {
                location: tag.location.clone(),
                ..DeviceTag::default()
            }
        }
        .to_string()
    };
    let location = aspect(false);
    if location.is_empty() {
        aspect(true)
    } else {
        location
    }
}

#[derive(Default)]
struct Tree {
    nodes: BTreeMap<String, Vec<El>>,
    structures: BTreeMap<String, El>,
    /// Structure records whose node key is taken by another record.
    extra: Vec<El>,
}

impl Tree {
    fn add(&mut self, tag: &DeviceTag, el: El) {
        self.nodes.entry(node_key(tag)).or_default().push(el);
    }

    fn structure(&mut self, tag: &DeviceTag, el: El) {
        match self.structures.entry(node_key(tag)) {
            std::collections::btree_map::Entry::Vacant(v) => {
                v.insert(el);
            }
            std::collections::btree_map::Entry::Occupied(_) => self.extra.push(el),
        }
    }

    fn into_children(mut self) -> Vec<El> {
        let mut keys: BTreeSet<String> = self.nodes.keys().cloned().collect();
        keys.extend(self.structures.keys().cloned());
        let mut out = Vec::new();
        for key in keys {
            let els = self.nodes.remove(&key).unwrap_or_default();
            if key.is_empty() {
                out.extend(els);
                continue;
            }
            let mut node = self
                .structures
                .remove(&key)
                .unwrap_or_else(|| structure_el(&key, "", false, None));
            node.children.extend(els);
            out.push(node);
        }
        out.append(&mut self.extra);
        out
    }
}

fn structure_el(name: &str, description: &str, enclosure: bool, plate: Option<(f64, f64)>) -> El {
    let function = name.starts_with('=');
    let mut el = El::new("InternalElement")
        .attr("ID", guid("structure", name))
        .attr("Name", name)
        .child(string("Tag", name));
    if !description.is_empty() {
        el.push(string("Description", description));
    }
    if enclosure {
        el.push(El::attribute("Enclosure", "xs:boolean", "true"));
    } else if !function {
        // Said outright: a reader takes a bare location for a panel.
        el.push(El::attribute("Enclosure", "xs:boolean", "false"));
    }
    if let Some((w, h)) = plate {
        let n = crate::sheets::number_text;
        el.push(El::attribute("PlateWidth", "xs:double", n(w)).attr("Unit", "mm"));
        el.push(El::attribute("PlateHeight", "xs:double", n(h)).attr("Unit", "mm"));
    }
    el.push(role(if function {
        "PlantSection"
    } else if enclosure {
        "Enclosure"
    } else {
        "Location"
    }));
    el
}

/// The device word the drawing profile gives a library symbol, for tools
/// that do not know KoperCAD's own `Symbol` key.
fn kind_of(key: &str) -> &'static str {
    let id = key.trim_start_matches("iec/");
    match id.split('/').next().unwrap_or_default() {
        "coils" => "coil",
        "contacts" if id.contains("thermal") => "thermal_switch",
        "contacts" => "contact",
        "contactors" => "power_poles",
        "motors" => "motor",
        "signalling" => "lamp",
        "earth" => "earth",
        "terminals" if id.contains("plug") || id.contains("socket") => "plug",
        "terminals" => "terminal",
        "connectivity" if id.ends_with("interruption_point") => "interruption",
        "connectivity" => "pin",
        _ => "generic",
    }
}

fn face(d: Direction) -> &'static str {
    match d {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

/// The label a page goes by in the file: its structure and name.
fn page_label(page: &novo_core::Page) -> String {
    if page.structure.is_empty() {
        page.name.clone()
    } else {
        format!("{}/{}", page.structure, page.name)
    }
}

/// The AutomationML file of a project.
pub fn export_project(project: &Project, options: &AmlOptions) -> AmlExport {
    let mut report = Report::default();
    let mut tree = Tree::default();

    for s in project.structures() {
        let plate = s.plate.map(|p| (p.width, p.height));
        let mut el = structure_el(&s.tag.to_string(), &s.description, s.enclosure, plate);
        if let Some(p) = properties(&s.properties) {
            el.push(p);
        }
        tree.structure(&s.tag, el);
    }

    // Interfaces: terminals on their strips, pins as the nets need them.
    let mut interfaces: BTreeMap<DeviceId, Vec<El>> = BTreeMap::new();
    let mut terminal_ids: BTreeMap<TerminalId, String> = BTreeMap::new();
    for strip in project
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip)
    {
        for t in project.terminals_of_strip(strip.id) {
            let id = guid("terminal", &t.id.raw().to_string());
            terminal_ids.insert(t.id, id.clone());
            let mut el = El::new("ExternalInterface")
                .attr("ID", id)
                .attr("Name", t.name.clone())
                .attr("RefBaseClassPath", format!("{INTERFACE_LIB}/Terminal"));
            el.push(El::attribute("Level", "xs:int", t.level.to_string()));
            if let Some(j) = t.jumper {
                el.push(El::attribute("Jumper", "xs:int", j.to_string()));
            }
            el.push(string("Kind", &format!("{:?}", t.kind)));
            if let Some(part) = t.part.and_then(|p| project.part(p)) {
                el.push(string("OrderNumber", &part.order_number));
            }
            interfaces.entry(strip.id).or_default().push(el);
        }
        report.moved("terminals", project.terminals_of_strip(strip.id).len());
    }

    // Drawn symbols: one element per symbol under its device or strip (an
    // interruption point under its page), with the pins where they sit on the
    // page. The file's y runs up from the bottom of the sheet.
    let mut symbol_els: BTreeMap<DeviceId, Vec<El>> = BTreeMap::new();
    let mut page_symbols: BTreeMap<PageId, Vec<El>> = BTreeMap::new();
    let mut drawn_pin: BTreeMap<(SymbolId, u16), String> = BTreeMap::new();
    let (mut drawn, mut multi_level, mut undrawn) = (0usize, 0usize, 0usize);
    for sym in project.symbols() {
        let Some(page) = project.page(sym.page) else {
            continue;
        };
        let height = page.format.size_mm().1;
        let sid = guid("symbol", &sym.id.raw().to_string());
        let mut el = El::new("InternalElement")
            .attr("ID", sid.clone())
            .attr("Name", sym.symbol.0.clone());
        el.push(string("Page", &page_label(page)));
        el.push(string("Kind", kind_of(sym.symbol.0.as_str())));
        el.push(string("Symbol", sym.symbol.0.as_str()));
        el.push(frame(
            sym.placement.origin.x,
            height - sym.placement.origin.y,
            0.0,
            0.0,
        ));
        let owner = match &sym.role {
            SymbolRole::Device { device, .. } => Some(*device),
            SymbolRole::Terminal { terminals } => {
                if terminals.len() > 1 {
                    multi_level += 1;
                }
                let first = terminals.first().and_then(|t| project.terminal(*t));
                if let Some(t) = first {
                    el.push(string("Terminal", &t.name));
                }
                first.map(|t| t.strip)
            }
            SymbolRole::Interruption { name } => {
                el.push(string("Net", name));
                None
            }
            // A free symbol (an earth, a lamp) has no owner but is drawn.
            SymbolRole::Unassigned => None,
            _ => {
                undrawn += 1;
                continue;
            }
        };
        for (i, pin) in sym.pins.iter().enumerate() {
            let at = sym.pin_position(i as u16).unwrap_or(sym.placement.origin);
            let id = guid("symbolpin", &format!("{}:{i}", sym.id.raw()));
            drawn_pin.insert((sym.id, i as u16), id.clone());
            let n = crate::sheets::number_text;
            let mut pe = El::new("ExternalInterface")
                .attr("ID", id)
                .attr("Name", pin.name.clone());
            pe.push(El::attribute(
                "X",
                "xs:double",
                n(at.x - sym.placement.origin.x),
            ));
            // Page y runs down, the file's up.
            pe.push(El::attribute(
                "Y",
                "xs:double",
                n(-(at.y - sym.placement.origin.y)),
            ));
            if let Some(d) = pin.direction {
                pe.push(string("Faces", face(sym.placement.apply_direction(d))));
            }
            el.push(pe);
        }
        el.push(role("Symbol"));
        drawn += 1;
        match owner {
            Some(d) => symbol_els.entry(d).or_default().push(el),
            None => page_symbols.entry(sym.page).or_default().push(el),
        }
    }
    report.moved("drawn symbols", drawn);
    report.lost(
        "drawn symbols without a device or strip",
        undrawn,
        "a symbol assigned to nothing has no element to hang under",
    );
    report.lost(
        "terminal levels",
        multi_level,
        "a multi-level terminal symbol is written with its first terminal",
    );

    // Nets: member pins, resolved to device pins and terminals.
    let netlist = project.netlist();
    let mut pin_ids: BTreeMap<(DeviceId, String), String> = BTreeMap::new();
    let mut net_els: Vec<El> = Vec::new();
    let mut links: Vec<El> = Vec::new();
    let mut unresolved = 0usize;
    for net in netlist.nets() {
        let mut members: BTreeSet<(String, String)> = BTreeSet::new();
        // The drawn pins each member stands for.
        let mut drawn_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut interruptions: BTreeSet<(String, String)> = BTreeSet::new();
        for end in &net.endpoints {
            let Endpoint::Pin { symbol, pin } = *end else {
                continue;
            };
            let Some(sym) = project.symbol(symbol) else {
                continue;
            };
            let Some(name) = sym.pins.get(usize::from(pin)).map(|p| p.name.clone()) else {
                continue;
            };
            let on_page = drawn_pin.get(&(symbol, pin)).cloned();
            match &sym.role {
                SymbolRole::Device { device, .. } => {
                    let id = pin_ids
                        .entry((*device, name.clone()))
                        .or_insert_with(|| guid("pin", &format!("{}:{name}", device.raw())))
                        .clone();
                    let label = project
                        .pin_designation(novo_core::PinRef::new(symbol, pin))
                        .unwrap_or_else(|| name.clone());
                    if let Some(d) = on_page {
                        drawn_of.entry(id.clone()).or_default().insert(d);
                    }
                    members.insert((id, label));
                }
                SymbolRole::Terminal { .. } => {
                    if let Some(t) = sym.terminal_of_pin(pin)
                        && let Some(id) = terminal_ids.get(&t)
                    {
                        let label = project.terminal_designation(t).unwrap_or_default();
                        if let Some(d) = on_page {
                            drawn_of.entry(id.clone()).or_default().insert(d);
                        }
                        members.insert((id.clone(), label));
                    } else {
                        unresolved += 1;
                    }
                }
                // An interruption point is a drawn pin of its own: the net
                // point links to it so a reader sees the page-to-page
                // continuation as drawn and adds none of its own.
                SymbolRole::Interruption { name } => {
                    if let Some(d) = on_page {
                        interruptions.insert((d, name.clone()));
                    }
                }
                _ => unresolved += 1,
            }
        }
        if members.len() < 2 {
            continue;
        }
        for (d, name) in interruptions {
            members.insert((d, name));
        }
        let potential = net
            .potential()
            .and_then(|p| project.potential(p))
            .map(|p| p.name.clone());
        let name = potential
            .clone()
            .unwrap_or_else(|| format!("N{}", net.id.0 + 1));
        let net_id = guid("net", &net.id.0.to_string());
        let mut el = El::new("InternalElement")
            .attr("ID", net_id.clone())
            .attr("Name", name);
        if let Some(p) = &potential {
            el.push(string("Potential", p));
        }
        let numbers: Vec<String> = net
            .wires
            .iter()
            .filter_map(|w| project.wire(*w).and_then(|w| w.number.clone()))
            .collect();
        if !numbers.is_empty() {
            el.push(string("WireNumbers", &numbers.join(", ")));
        }
        el.push(role("Net"));
        for (n, (member, label)) in members.iter().enumerate() {
            let point = guid("netpoint", &format!("{}:{n}", net.id.0));
            el.push(
                El::new("ExternalInterface")
                    .attr("ID", point.clone())
                    .attr("Name", label.clone())
                    .attr("RefBaseClassPath", format!("{INTERFACE_LIB}/NetPoint")),
            );
            links.push(
                El::new("InternalLink")
                    .attr("Name", label.clone())
                    .attr("RefPartnerSideA", member.clone())
                    .attr("RefPartnerSideB", point.clone()),
            );
            for d in drawn_of.get(member).into_iter().flatten() {
                links.push(
                    El::new("InternalLink")
                        .attr("Name", label.clone())
                        .attr("RefPartnerSideA", d.clone())
                        .attr("RefPartnerSideB", point.clone()),
                );
            }
        }
        net_els.push(el);
    }
    report.moved("nets", net_els.len());
    report.lost(
        "connection points",
        unresolved,
        "interruption points and unassigned symbols have no device to attach to",
    );

    // Devices.
    let placements: BTreeMap<DeviceId, &CabinetPlacement> = project
        .all::<CabinetPlacement>()
        .map(|p| (p.device, p))
        .collect();
    let mut devices = 0usize;
    for d in project.devices() {
        let mut el = device_el(project, d, &placements);
        for (key, id) in &pin_ids {
            if key.0 == d.id {
                el.push(
                    El::new("ExternalInterface")
                        .attr("ID", id.clone())
                        .attr("Name", key.1.clone())
                        .attr("RefBaseClassPath", format!("{INTERFACE_LIB}/Pin")),
                );
            }
        }
        for i in interfaces.remove(&d.id).unwrap_or_default() {
            el.push(i);
        }
        for sy in symbol_els.remove(&d.id).unwrap_or_default() {
            el.push(sy);
        }
        tree.add(&d.tag, el);
        devices += 1;
    }
    report.moved("devices", devices);

    // Cables and their cores.
    let mut cables = 0usize;
    for c in project.cables() {
        let mut el = El::new("InternalElement")
            .attr("ID", guid("cable", &c.id.raw().to_string()))
            .attr("Name", c.tag.to_string())
            .child(string("Tag", &c.tag.to_string()));
        if !c.description.is_empty() {
            el.push(string("Description", &c.description));
        }
        if let Some(l) = c.length_m {
            el.push(
                El::attribute("Length", "xs:double", crate::sheets::number_text(l))
                    .attr("Unit", "m"),
            );
        }
        if let Some(part) = c.part.and_then(|p| project.part(p)) {
            el.push(string("Manufacturer", &part.manufacturer));
            el.push(string("OrderNumber", &part.order_number));
        }
        if let Some(p) = properties(&c.properties) {
            el.push(p);
        }
        el.push(role("Cable"));
        for (n, core) in c.cores.iter().enumerate() {
            let mut ce = El::new("InternalElement")
                .attr("ID", guid("core", &format!("{}:{n}", c.id.raw())))
                .attr("Name", core.name.clone());
            if let Some(mm2) = core.cross_section_mm2 {
                ce.push(
                    El::attribute("CrossSection", "xs:double", crate::sheets::number_text(mm2))
                        .attr("Unit", "mm2"),
                );
            }
            if core.shield {
                ce.push(El::attribute("Shield", "xs:boolean", "true"));
            }
            if let Some(w) = core.wire.and_then(|w| project.wire(w))
                && let Some(number) = &w.number
            {
                ce.push(string("WireNumber", number));
            }
            ce.push(role("CableCore"));
            el.push(ce);
        }
        tree.add(&c.tag, el);
        cables += 1;
    }
    report.moved("cables", cables);

    let structures = project.structures().count();
    report.moved("locations and sections", structures);

    // Assemble.
    let name = if project.info().name.is_empty() {
        "Project".to_string()
    } else {
        project.info().name.clone()
    };
    let mut root = El::new("InternalElement")
        .attr("ID", guid("project", &name))
        .attr("Name", name.clone());
    if !project.info().description.is_empty() {
        root.push(string("Description", &project.info().description));
    }
    root.push(role("Project"));
    for c in tree.into_children() {
        root.push(c);
    }
    let mut page_group = El::new("InternalElement")
        .attr("ID", guid("pages", &name))
        .attr("Name", "Pages");
    page_group.push(role("PageGroup"));
    let mut pages = 0usize;
    for page in project.pages() {
        let mut el = El::new("InternalElement")
            .attr("ID", guid("page", &page.id.raw().to_string()))
            .attr("Name", page_label(page));
        if !page.title.is_empty() {
            el.push(string("Title", &page.title));
        }
        el.push(role("Page"));
        for sy in page_symbols.remove(&page.id).unwrap_or_default() {
            el.push(sy);
        }
        page_group.push(el);
        pages += 1;
    }
    if pages > 0 {
        root.push(page_group);
    }
    report.moved("pages", pages);
    if !net_els.is_empty() {
        let mut group = El::new("InternalElement")
            .attr("ID", guid("nets", &name))
            .attr("Name", "Nets");
        group.push(role("NetGroup"));
        for n in net_els {
            group.push(n);
        }
        root.push(group);
    }
    for l in links {
        root.push(l);
    }

    report.lost(
        "page graphics",
        project.pages().count(),
        "pages, their titles and the placed symbols are written; texts, lines and frames are not yet",
    );
    report.lost(
        "wire routes",
        project.wires().count(),
        "wires are exported as nets, without their drawn route",
    );
    report.note("Devices keep their tag, description, article and properties; placed devices carry a Frame.");

    let mut file = El::new("CAEXFile")
        .attr("FileName", format!("{name}.aml"))
        .attr("SchemaVersion", "2.15")
        .attr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
        .attr("xsi:noNamespaceSchemaLocation", "CAEX_ClassModel_V2.15.xsd");
    file.push(
        El::new("AdditionalInformation").child(
            El::new("WriterHeader")
                .child(El::new("WriterName").text("KoperCAD"))
                .child(El::new("WriterID").text("KoperVonk.KoperCAD"))
                .child(El::new("WriterVendor").text("KoperVonk"))
                .child(
                    El::new("WriterVendorURL")
                        .text("https://github.com/Herepiggypiggypiggy/kopercad"),
                )
                .child(El::new("WriterVersion").text(options.version.clone()))
                .child(El::new("WriterRelease").text(options.version.clone()))
                .child(El::new("LastWritingDateTime").text(options.created.clone()))
                .child(El::new("WriterProjectTitle").text(name.clone()))
                .child(El::new("WriterProjectID").text(guid("project", &name))),
        ),
    );
    file.push(El::new("InstanceHierarchy").attr("Name", name).child(root));
    file.push(interface_lib());
    file.push(role_lib());
    file.push(base_role_lib());

    let mut text = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    write(&file, 0, &mut text);
    AmlExport { text, report }
}

fn device_el(
    project: &Project,
    d: &Device,
    placements: &BTreeMap<DeviceId, &CabinetPlacement>,
) -> El {
    let mut el = El::new("InternalElement")
        .attr("ID", guid("device", &d.id.raw().to_string()))
        .attr("Name", d.tag.to_string())
        .child(string("Tag", &d.tag.to_string()));
    if !d.description.is_empty() {
        el.push(string("Description", &d.description));
    }
    let parts: Vec<_> = d.parts.iter().filter_map(|p| project.part(*p)).collect();
    if let Some(p) = parts.first() {
        el.push(string("Manufacturer", &p.manufacturer));
        el.push(string("OrderNumber", &p.order_number));
        if !p.type_number.is_empty() {
            el.push(string("TypeNumber", &p.type_number));
        }
        if !p.description.is_empty() {
            el.push(string("ArticleDescription", &p.description));
        }
    }
    if parts.len() > 1 {
        let mut extra = El::new("Attribute")
            .attr("Name", "AdditionalArticles")
            .attr("AttributeDataType", "xs:string");
        for (n, p) in parts.iter().enumerate().skip(1) {
            extra.push(
                El::new("Attribute")
                    .attr("Name", format!("Article{}", n + 1))
                    .attr("AttributeDataType", "xs:string")
                    .child(string("Manufacturer", &p.manufacturer))
                    .child(string("OrderNumber", &p.order_number)),
            );
        }
        el.push(extra);
    }
    if let Some(p) = properties(&d.properties) {
        el.push(p);
    }
    if let Some(pl) = placements.get(&d.id) {
        let rz = if pl.rotated { 90.0 } else { 0.0 };
        el.push(frame(pl.x, pl.y, 0.0, rz));
    }
    el.push(role(match d.kind {
        DeviceKind::TerminalStrip => "TerminalStrip",
        DeviceKind::Connector => "Connector",
        DeviceKind::Plc => "Plc",
        DeviceKind::General => "Device",
    }));
    el
}

fn class(tag: &'static str, name: &str, base: Option<&str>) -> El {
    let mut e = El::new(tag).attr("Name", name);
    if let Some(b) = base {
        e = e.attr("RefBaseClassPath", b);
    }
    e
}

fn base_role_lib() -> El {
    let mut base = class("RoleClass", "AutomationMLBaseRole", None);
    for r in [
        "Group",
        "Facet",
        "Resource",
        "Product",
        "Process",
        "Structure",
        "PropertySet",
    ] {
        base.push(class("RoleClass", r, None));
    }
    El::new("RoleClassLib")
        .attr("Name", "AutomationMLBaseRoleClassLib")
        .child(El::new("Description").text("Standard AutomationML base roles"))
        .child(El::new("Version").text("2.10.0"))
        .child(base)
}

fn role_lib() -> El {
    let base = "AutomationMLBaseRoleClassLib/AutomationMLBaseRole";
    let mut lib = El::new("RoleClassLib")
        .attr("Name", ROLE_LIB)
        .child(El::new("Description").text("KoperCAD electrical roles"))
        .child(El::new("Version").text("1.0.0"));
    for (name, parent) in [
        ("Project", "Structure"),
        ("PlantSection", "Structure"),
        ("Location", "Structure"),
        ("Enclosure", "Structure"),
        ("Device", "Resource"),
        ("TerminalStrip", "Resource"),
        ("Connector", "Resource"),
        ("Plc", "Resource"),
        ("Cable", "Resource"),
        ("CableCore", "Resource"),
        ("NetGroup", "Group"),
        ("Net", "Group"),
    ] {
        lib.push(class("RoleClass", name, Some(&format!("{base}/{parent}"))));
    }
    lib
}

fn interface_lib() -> El {
    El::new("InterfaceClassLib")
        .attr("Name", INTERFACE_LIB)
        .child(El::new("Description").text("KoperCAD connection points"))
        .child(El::new("Version").text("1.0.0"))
        .child(class("InterfaceClass", "Terminal", None))
        .child(class("InterfaceClass", "Pin", None))
        .child(class("InterfaceClass", "NetPoint", None))
}
