//! Reads a project into an [`Input`], runs the engine, applies fixes.
//!
//! Facts the drawing cannot hold yet live in `Properties` (decision of the
//! EMC plan: properties first, typed fields later):
//!
//! | Key | On | Meaning |
//! | --- | --- | --- |
//! | `emc.check` | project | the EMC rules run with the design checks |
//! | `emc.class` | wire, cable | `motor`, `power`, `control`, `analog`, `fieldbus`, `safety`, `earth` |
//! | `emc.shield_a`, `emc.shield_b` | cable | shield end: `360`, `pigtail:<mm>`, `open` |
//! | `emc.twisted` | cable | the cores are twisted pairs |
//! | `emc.leaves` | wire | the wire leaves the cabinet |
//! | `emc.role` | device | `drive`, `filter`, `sensitive`, `noisy` |
//! | `emc.filter` | device, part | `built-in` or `external`: a drive's mains filter |
//! | `emc.filter_for` | filter device | tag of the drive it serves |
//! | `emc.bonded` | device | case bonded to the plate (false is flagged) |
//! | `emc.finish` | enclosure | `galvanised`, `bare`, `painted` |
//! | `emc.divider` | duct | a metal divider between classes |
//! | `emc.<limit>` | project | thresholds of [`Params`] (`emc.min_parallel_mm` ...) |

use std::collections::{BTreeMap, HashMap};

use novo_core::{
    CabinetPlacement, Cable, CableCore, Device, DeviceKind, DeviceTag, Endpoint, NetId, Netlist,
    PinRef, PotentialKind, Project, Properties, Rail, RailKind, Result, Structure, SymbolRole,
    Tx, Value,
};

use crate::class::{EmcClass, Shield, Termination};
use crate::input::{
    Device as EmcDevice, Duct, FilterState, Finish, Input, Origin, Params, Plate, Role, Run,
};
use crate::report::{Fix, Report};

/// Project property that turns the EMC rules on.
pub const CHECK_KEY: &str = "emc.check";

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Text(t) => t.trim().replace(',', ".").parse().ok(),
        Value::Bool(_) => None,
    }
}

fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Int(i) => Some(*i != 0),
        Value::Text(t) => match t.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" | "ja" => Some(true),
            "false" | "no" | "0" | "nee" => Some(false),
            _ => None,
        },
        Value::Float(_) => None,
    }
}

fn text<'a>(props: &'a Properties, key: &str) -> Option<&'a str> {
    match props.get(key)? {
        Value::Text(t) => Some(t.trim()),
        _ => None,
    }
}

/// Whether the project asked for the EMC rules.
pub fn is_enabled(project: &Project) -> bool {
    project
        .info()
        .properties
        .get(CHECK_KEY)
        .and_then(as_bool)
        .unwrap_or(false)
}

/// Turns the EMC rules on or off (one undo step with the caller's transaction).
pub fn set_enabled(tx: &mut Tx<'_>, on: bool) -> Result<()> {
    tx.set_info(|info| {
        if on {
            info.properties.insert(CHECK_KEY.to_owned(), Value::Bool(true));
        } else {
            info.properties.remove(CHECK_KEY);
        }
    })
}

/// The thresholds, with the project's overrides.
pub fn params_of(project: &Project) -> Params {
    let props = &project.info().properties;
    let num = |name: &str, default: f64| {
        props
            .get(&format!("emc.{name}"))
            .and_then(as_f64)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(default)
    };
    let d = Params::default();
    let mut user_set: Vec<&'static str> = Vec::new();
    for name in ["min_parallel_mm", "max_loop_cm2", "filter_max_mm", "sensitive_min_mm", "pigtail_max_mm", "frequency_hz", "didt_a_per_us", "wire_height_mm"] {
        if props.get(&format!("emc.{name}")).and_then(as_f64).is_some_and(|v| v.is_finite() && v > 0.0) {
            user_set.push(name);
        }
    }
    let mut distance_override = Vec::new();
    for a in EmcClass::ALL {
        for b in EmcClass::ALL {
            let key = format!("emc.distance.{}.{}_mm", a.id(), b.id());
            if let Some(v) = props.get(&key).and_then(as_f64).filter(|v| *v >= 0.0) {
                distance_override.push((a, b, v));
            }
        }
    }
    Params {
        min_parallel_mm: num("min_parallel_mm", d.min_parallel_mm),
        max_loop_cm2: num("max_loop_cm2", d.max_loop_cm2),
        filter_max_mm: num("filter_max_mm", d.filter_max_mm),
        sensitive_min_mm: num("sensitive_min_mm", d.sensitive_min_mm),
        pigtail_max_mm: num("pigtail_max_mm", d.pigtail_max_mm),
        frequency_hz: num("frequency_hz", d.frequency_hz),
        didt_a_per_us: num("didt_a_per_us", d.didt_a_per_us),
        wire_height_mm: num("wire_height_mm", d.wire_height_mm),
        distance_override,
        user_set,
    }
}

/// The last two parts of a symbol library key: `("drives", "frequency_converter")`.
fn key_parts(key: &str) -> (&str, &str) {
    let mut it = key.rsplit('/');
    let name = it.next().unwrap_or_default();
    (it.next().unwrap_or_default(), name)
}

fn pin_name(project: &Project, pin: PinRef) -> String {
    project
        .symbol(pin.symbol)
        .and_then(|s| s.pins.get(usize::from(pin.pin)))
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

fn pin_label(project: &Project, pin: PinRef) -> String {
    let device = novo_routing::model::device_of_pin(project, pin)
        .and_then(|d| project.device(d))
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    format!("{device}:{}", pin_name(project, pin))
}

fn device_prop<'a>(project: &'a Project, device: &'a Device, key: &str) -> Option<&'a Value> {
    device.properties.get(key).or_else(|| {
        device
            .parts
            .iter()
            .filter_map(|p| project.part(*p))
            .find_map(|p| p.properties.get(key))
    })
}

/// How a net was classed.
#[derive(Clone)]
struct NetClass {
    class: EmcClass,
    origin: Origin,
    why: String,
    drive: Option<u64>,
}

fn is_drive_output(pin: &str) -> bool {
    matches!(
        pin.to_ascii_uppercase().as_str(),
        "U" | "V" | "W" | "U2" | "V2" | "W2" | "T1" | "T2" | "T3" | "2T1" | "4T2" | "6T3" | "MOTOR"
    )
}

fn classify_net(project: &Project, netlist: &Netlist, net: NetId) -> NetClass {
    let n = netlist.net(net);
    let mut motor_drive: Option<u64> = None;
    let mut safety = false;
    let mut fieldbus = false;
    let mut analog = false;
    let mut power_device = false;
    for e in &n.endpoints {
        let Endpoint::Pin { symbol, pin } = e else { continue };
        let Some(sym) = project.symbol(*symbol) else { continue };
        let (group, name) = key_parts(&sym.symbol.0);
        let pname = sym.pins.get(usize::from(*pin)).map(|p| p.name.as_str()).unwrap_or("");
        match group {
            "drives" if name == "frequency_converter" && is_drive_output(pname) => {
                motor_drive = novo_routing::model::device_of_pin(project, PinRef::new(*symbol, *pin))
                    .map(|d| d.raw())
                    .or(Some(0));
            }
            "safety" => safety = true,
            "operators" if name.starts_with("emergency_stop") || name.starts_with("pullcord") => {
                safety = true
            }
            "network" => fieldbus = true,
            "plc" if name.starts_with("analog") => analog = true,
            "sensors" if matches!(name, "pt100" | "transmitter" | "encoder") => analog = true,
            "motors" => power_device = true,
            "passive" if name.starts_with("heater") => power_device = true,
            "contactors"
                if matches!(pname, "1" | "2" | "3" | "4" | "5" | "6" | "L1" | "L2" | "L3" | "T1" | "T2" | "T3") =>
            {
                power_device = true
            }
            _ => {}
        }
        if let SymbolRole::Device { device, .. } = &sym.role
            && let Some(d) = project.device(*device)
            && let Some(sig) = text(&d.properties, "signal").or_else(|| {
                d.parts
                    .iter()
                    .filter_map(|p| project.part(*p))
                    .find_map(|p| text(&p.properties, "signal"))
            })
            && sig.eq_ignore_ascii_case("analog")
        {
            analog = true;
        }
    }
    let mk = |class, origin, why: &str| NetClass { class, origin, why: why.to_owned(), drive: None };
    if let Some(d) = motor_drive {
        return NetClass {
            drive: (d != 0).then_some(d),
            ..mk(EmcClass::Motor, Origin::Derived, "on a drive's motor output")
        };
    }
    if safety {
        return mk(EmcClass::Safety, Origin::Derived, "on a safety device");
    }
    if fieldbus {
        return mk(EmcClass::Fieldbus, Origin::Derived, "on a network port");
    }
    if analog {
        return mk(EmcClass::Analog, Origin::Derived, "on an analog channel or sensor");
    }
    let mut max_v: Option<f64> = None;
    let mut kinds: Vec<PotentialKind> = Vec::new();
    for pid in &n.potentials {
        if let Some(p) = project.get::<novo_core::Potential>(*pid) {
            kinds.push(p.kind);
            if let Some(v) = p.voltage {
                max_v = Some(max_v.map_or(v, |m: f64| m.max(v)));
            }
        }
    }
    if !kinds.is_empty() && kinds.iter().all(|k| *k == PotentialKind::ProtectiveEarth) {
        return mk(EmcClass::Earth, Origin::Derived, "protective earth");
    }
    if let Some(v) = max_v {
        return if v >= 50.0 {
            mk(EmcClass::Power, Origin::Derived, "a potential of 50 V or more")
        } else {
            mk(EmcClass::Control, Origin::Derived, "a potential under 50 V")
        };
    }
    if kinds.iter().any(|k| matches!(k, PotentialKind::Line | PotentialKind::Pen | PotentialKind::Neutral)) {
        return mk(EmcClass::Power, Origin::Derived, "a mains potential");
    }
    if power_device {
        return mk(EmcClass::Power, Origin::Derived, "on a motor, heater or contactor power pole");
    }
    mk(
        EmcClass::Control,
        Origin::Guessed,
        "no voltage and no power device on it: assumed control",
    )
}

fn shield_end(cable: &Cable, key: &str) -> Termination {
    match text(&cable.properties, key) {
        Some(t) => {
            let t = t.to_ascii_lowercase();
            if matches!(t.as_str(), "360" | "full" | "all-round") {
                Termination::Full360
            } else if matches!(t.as_str(), "open" | "none") {
                Termination::Open
            } else if let Some(mm) = t.strip_prefix("pigtail:").and_then(|n| n.trim().parse().ok()) {
                Termination::Pigtail(mm)
            } else if t == "pigtail" {
                Termination::Pigtail(100.0)
            } else {
                Termination::Unknown
            }
        }
        None => Termination::Unknown,
    }
}

fn finish_of(s: &Structure) -> Option<Finish> {
    match text(&s.properties, "emc.finish")?.to_ascii_lowercase().as_str() {
        "painted" | "lacquered" | "powder" | "powder-coated" => Some(Finish::Painted),
        "galvanised" | "galvanized" | "bare" | "zinc" | "stainless" | "conductive" => {
            Some(Finish::Conductive)
        }
        _ => None,
    }
}

fn role_of(project: &Project, device: &Device) -> Role {
    if let Some(r) = text(&device.properties, "emc.role") {
        return match r.to_ascii_lowercase().as_str() {
            "drive" => Role::Drive,
            "filter" => Role::Filter,
            "sensitive" => Role::Sensitive,
            "noisy" => Role::Noisy,
            _ => Role::Other,
        };
    }
    if device.kind == DeviceKind::Plc {
        return Role::Sensitive;
    }
    for s in project.symbols() {
        let SymbolRole::Device { device: d, .. } = &s.role else { continue };
        if *d != device.id {
            continue;
        }
        match key_parts(&s.symbol.0) {
            ("drives", "frequency_converter") => return Role::Drive,
            ("transformers", "emc_filter") => return Role::Filter,
            ("plc", name) if name.starts_with("analog") || name == "cpu" => return Role::Sensitive,
            ("supplies", _) => return Role::Noisy,
            _ => {}
        }
    }
    Role::Other
}

/// Builds the engine's input from the project.
pub fn input_of(project: &Project, netlist: &Netlist) -> Input {
    let params = params_of(project);
    let placements: HashMap<_, _> = project.all::<CabinetPlacement>().map(|p| (p.device, p)).collect();
    let started = !placements.is_empty();

    // Plates and ducts.
    let mut plates = Vec::new();
    let mut ducts: Vec<Duct> = Vec::new();
    let mut tag_of: HashMap<u64, DeviceTag> = HashMap::new();
    for s in project.structures().filter(|s| s.enclosure) {
        tag_of.insert(s.id.raw(), s.tag.clone());
        plates.push(Plate { id: s.id.raw(), tag: s.tag.to_string(), finish: finish_of(s) });
    }
    if started {
        let dividers: HashMap<u64, bool> = project
            .all::<Rail>()
            .filter(|r| r.kind == RailKind::Duct)
            .map(|r| (r.id.raw(), r.properties.get("emc.divider").and_then(as_bool).unwrap_or(false)))
            .collect();
        for p in &plates {
            let structure = novo_core::StructureId::from(p.id);
            for d in novo_routing::model::ducts(project, structure) {
                ducts.push(Duct {
                    id: d.id,
                    a: (d.a.x, d.a.y),
                    b: (d.b.x, d.b.y),
                    width_mm: d.width,
                    divider: dividers.get(&d.id).copied().unwrap_or(false),
                });
            }
        }
    }
    let routed = started.then(|| novo_routing::model::route_project(project));

    // Devices that matter for EMC.
    let mut devices = Vec::new();
    for d in project.devices() {
        let role = role_of(project, d);
        if role == Role::Other {
            continue;
        }
        let place = placements.get(&d.id);
        let rect = place.map(|pl| {
            let size = novo_layout::model::device_size(project, d);
            let (w, h) = if pl.rotated { (size.height, size.width) } else { (size.width, size.height) };
            (pl.x, pl.y, w, h)
        });
        let filter = match device_prop(project, d, "emc.filter") {
            Some(Value::Text(t)) => {
                let t = t.trim().to_ascii_lowercase();
                if matches!(t.as_str(), "built-in" | "builtin" | "internal" | "c1" | "c2") {
                    FilterState::Built
                } else if matches!(t.as_str(), "external" | "fitted" | "yes" | "true") {
                    FilterState::Declared
                } else {
                    FilterState::Unknown
                }
            }
            Some(Value::Bool(true)) => FilterState::Built,
            _ => FilterState::Unknown,
        };
        devices.push(EmcDevice {
            id: d.id.raw(),
            tag: d.tag.to_string(),
            role,
            plate: place.map(|pl| pl.structure.raw()),
            rect,
            filter,
            filter_for: text(&d.properties, "emc.filter_for").map(str::to_owned),
            bonded: d.properties.get("emc.bonded").and_then(as_bool),
        });
    }

    // Runs.
    let mut net_class: HashMap<NetId, NetClass> = HashMap::new();
    let mut runs = Vec::new();
    for w in project.wires() {
        let net = netlist.net_of_wire(w);
        let pins = (w.from.as_pin(), w.to.as_pin());
        let name = match (&w.number, pins) {
            (Some(n), _) if !n.is_empty() => n.clone(),
            (_, (Some(a), Some(b))) => format!("{} to {}", pin_label(project, a), pin_label(project, b)),
            (_, (Some(p), None)) | (_, (None, Some(p))) => pin_label(project, p),
            _ => format!("wire {}", w.id.raw()),
        };
        let cable_id = project.cable_of_wire(w.id);
        let cable = cable_id.and_then(|c| project.cable(c));
        let declared = text(&w.properties, "emc.class")
            .or_else(|| cable.and_then(|c| text(&c.properties, "emc.class")))
            .and_then(EmcClass::parse);
        let nc = net
            .map(|n| net_class.entry(n).or_insert_with(|| classify_net(project, netlist, n)).clone())
            .unwrap_or(NetClass {
                class: EmcClass::Control,
                origin: Origin::Guessed,
                why: "open at both ends: assumed control".to_owned(),
                drive: None,
            });
        let (class, origin, why) = match declared {
            Some(c) => (c, Origin::Declared, "set by the user".to_owned()),
            None => (nc.class, nc.origin, nc.why.clone()),
        };
        let shield = cable.filter(|c| c.cores.iter().any(|k| k.shield)).map(|c| Shield {
            a: shield_end(c, "emc.shield_a"),
            b: shield_end(c, "emc.shield_b"),
        });
        let device_of = |p: Option<PinRef>| {
            p.and_then(|p| novo_routing::model::device_of_pin(project, p))
        };
        let (da, db) = (device_of(pins.0), device_of(pins.1));
        let plate = [da, db]
            .into_iter()
            .flatten()
            .find_map(|d| placements.get(&d))
            .map(|pl| pl.structure);
        let route = routed.as_ref().and_then(|r| r.routes.get(&w.id.raw()));
        let diameter = novo_routing::model::wire_diameter_mm(novo_routing::model::wire_section_mm2(project, w))
            .unwrap_or(novo_routing::DEFAULT_WIRE_DIAMETER_MM);
        let leaves = w.properties.get("emc.leaves").and_then(as_bool).unwrap_or_else(|| {
            let Some(pl) = plate.and_then(|s| tag_of.get(&s.raw())) else { return false };
            [da, db].into_iter().flatten().filter_map(|d| project.device(d)).any(|d| {
                !d.tag.location.is_empty() && !pl.location.is_empty() && d.tag.location != pl.location
            })
        });
        runs.push(Run {
            id: w.id.raw(),
            name,
            class,
            origin,
            class_reason: why,
            cable: cable_id.map(|c| c.raw()),
            shield,
            twisted: cable.and_then(|c| c.properties.get("emc.twisted")).and_then(as_bool).unwrap_or(false),
            net: net.map(|n| n.0),
            from: da.map(|d| d.raw()),
            to: db.map(|d| d.raw()),
            plate: plate.map(|s| s.raw()),
            path: route.map(|r| r.points.iter().map(|p| (p.x, p.y)).collect()).unwrap_or_default(),
            ducts: route.map(|r| r.ducts.clone()).unwrap_or_default(),
            diameter_mm: diameter,
            drive: nc.drive.filter(|_| declared.is_none() || class == EmcClass::Motor),
            leaves_cabinet: leaves,
        });
    }
    Input { runs, ducts, devices, plates, params }
}

/// Runs the engine on a project.
pub fn analyse_project(project: &Project) -> Report {
    analyse_with(project, &project.netlist())
}

/// Runs the engine with a netlist that is current for the project.
pub fn analyse_with(project: &Project, netlist: &Netlist) -> Report {
    crate::analyse(&input_of(project, netlist))
}

/// The picture of a project: classes, ribbons, loop sheets, zones and
/// hotspots, ready to draw.
pub fn picture_of(project: &Project) -> crate::Picture {
    let netlist = project.netlist();
    let input = input_of(project, &netlist);
    let report = crate::analyse(&input);
    crate::picture(&input, &report)
}

/// The classes of all runs, for a legend or a view.
pub fn class_counts(input: &Input) -> BTreeMap<EmcClass, usize> {
    let mut m = BTreeMap::new();
    for r in &input.runs {
        *m.entry(r.class).or_default() += 1;
    }
    m
}

/// Applies a fix as one command in the caller's transaction. Returns a
/// sentence saying what was done. Fixes that are done by hand return an error.
pub fn apply(tx: &mut Tx<'_>, fix: &Fix) -> Result<String> {
    use novo_core::ModelError;
    match fix {
        Fix::DeclareClass { wire, class } => {
            let id = novo_core::WireId::from(*wire);
            tx.set_property(id, "emc.class", Some(Value::Text(class.id().to_owned())))?;
            Ok(format!("Wire marked as {}", class.label()))
        }
        Fix::AddShield { cable } => {
            let id = novo_core::CableId::from(*cable);
            tx.update::<Cable>(id, |c| {
                if !c.cores.iter().any(|k| k.shield) {
                    let mut core = CableCore::new("SH");
                    core.shield = true;
                    c.cores.push(core);
                }
            })?;
            Ok("Shield core added to the cable; set both ends to all round".to_owned())
        }
        Fix::Terminate360 { cable } => {
            let id = novo_core::CableId::from(*cable);
            for key in ["emc.shield_a", "emc.shield_b"] {
                tx.set_property(id, key, Some(Value::Text("360".to_owned())))?;
            }
            Ok("Both shield ends set to all round (a requirement for the build)".to_owned())
        }
        Fix::AddFilter { drive } => {
            let drive_id = novo_core::DeviceId::from(*drive);
            let d = tx
                .project()
                .device(drive_id)
                .ok_or(ModelError::NotFound(drive_id.into()))?
                .clone();
            let n = tx.project().next_tag_number(&d.tag, "Z");
            let tag = DeviceTag {
                function: d.tag.function.clone(),
                location: d.tag.location.clone(),
                product: vec![format!("Z{n}")],
            };
            let filter = tx.add_device(tag.clone(), format!("EMC filter for {}", d.tag))?;
            tx.set_property(filter, "emc.role", Some(Value::Text("filter".to_owned())))?;
            tx.set_property(filter, "emc.filter_for", Some(Value::Text(d.tag.to_string())))?;
            Ok(format!("Filter {tag} added for {}: place it next to the drive", d.tag))
        }
        other => Err(ModelError::Format(format!("{other:?} is done by hand"))),
    }
}
