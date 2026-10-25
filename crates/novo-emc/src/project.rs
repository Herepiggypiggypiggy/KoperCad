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
//! | `emc.current_a` | wire, cable | RMS current in amperes for the magnetic field estimate (else the rated current of a device at an end) |
//! | `emc.phase_deg` | wire | phase of that current in degrees (else read from the pin: U, V, W / L1, L2, L3, or the core order of a cable) |
//! | `emc.field_hz`, `emc.field_slice_mm`, `emc.field_limit_ut`, `emc.induced_limit_mv`, `emc.twist_factor` | project | settings of [`FieldParams`]; `emc.plate_image` (`true` or `false`) says whether the plate counts as a conducting plane |

use std::collections::{BTreeMap, HashMap};

use novo_core::{
    CabinetPlacement, Cable, CableCore, Device, DeviceKind, DeviceTag, Endpoint, NetId, Netlist,
    PinRef, PotentialKind, Project, Properties, Rail, RailKind, Result, Structure, SymbolRole, Tx,
    Value,
};

use crate::class::{EmcClass, Shield, Termination};
use crate::field::{Current, FieldParams, FieldSetup};
use crate::input::{
    Device as EmcDevice, Duct, FilterState, Finish, Input, Origin, Params, Plate, Role, Run,
};
use crate::report::{Fix, Report, Subject};

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
            info.properties
                .insert(CHECK_KEY.to_owned(), Value::Bool(true));
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
    for name in [
        "min_parallel_mm",
        "max_loop_cm2",
        "filter_max_mm",
        "sensitive_min_mm",
        "pigtail_max_mm",
        "frequency_hz",
        "didt_a_per_us",
        "wire_height_mm",
    ] {
        if props
            .get(&format!("emc.{name}"))
            .and_then(as_f64)
            .is_some_and(|v| v.is_finite() && v > 0.0)
        {
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

/// The settings of the magnetic field estimate, with the project's overrides.
pub fn field_params_of(project: &Project) -> FieldParams {
    let props = &project.info().properties;
    let d = FieldParams::default();
    let num = |name: &'static str, default: f64, user: &mut Vec<&'static str>| match props
        .get(&format!("emc.{name}"))
        .and_then(as_f64)
        .filter(|v| v.is_finite() && *v > 0.0)
    {
        Some(v) => {
            user.push(name);
            v
        }
        None => default,
    };
    let mut user_set = Vec::new();
    let plate_image = props.get("emc.plate_image").and_then(as_bool);
    if plate_image.is_some() {
        user_set.push("plate_image");
    }
    FieldParams {
        hz: num("field_hz", d.hz, &mut user_set),
        slice_mm: num("field_slice_mm", d.slice_mm, &mut user_set),
        limit_ut: num("field_limit_ut", d.limit_ut, &mut user_set),
        induced_limit_mv: num("induced_limit_mv", d.induced_limit_mv, &mut user_set),
        twist_factor: num("twist_factor", d.twist_factor, &mut user_set).min(1.0),
        plate_image,
        user_set,
    }
}

/// The phase in degrees a pin name says, for the three phases of a drive
/// output, a contactor pole or a mains terminal.
fn phase_of_pin(name: &str) -> Option<f64> {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    match n.as_str() {
        "U" | "U1" | "U2" | "L1" | "T1" | "1" | "2" | "1L1" | "2T1" => Some(0.0),
        "V" | "V1" | "V2" | "L2" | "T2" | "3" | "4" | "3L2" | "4T2" => Some(-120.0),
        "W" | "W1" | "W2" | "L3" | "T3" | "5" | "6" | "5L3" | "6T3" => Some(120.0),
        _ => None,
    }
}

/// The currents the magnetic field estimate uses: the user's `emc.current_a`
/// on a wire or its cable, else, for motor and power wires, the smaller
/// rated current of the devices at the two ends. Phases come from
/// `emc.phase_deg`, the pin names, a DC potential or the core order of a
/// cable; a wire with none of these is taken alone (worst case).
pub fn currents_of(project: &Project, netlist: &Netlist, input: &Input) -> Vec<Current> {
    let mut out: Vec<Current> = Vec::new();
    let mut cables: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for run in &input.runs {
        let Some(w) = project.wire(novo_core::WireId::from(run.id)) else {
            continue;
        };
        let cable = run
            .cable
            .and_then(|c| project.cable(novo_core::CableId::from(c)));
        let declared = w
            .properties
            .get("emc.current_a")
            .or_else(|| cable.and_then(|c| c.properties.get("emc.current_a")))
            .and_then(as_f64)
            .filter(|v| v.is_finite() && *v >= 0.0);
        let (amps, source, is_declared) = match declared {
            Some(a) => (a, "stated on the wire (emc.current_a)".to_owned(), true),
            None if matches!(run.class, EmcClass::Motor | EmcClass::Power) => {
                let ends: Vec<(f64, String)> = [run.from, run.to]
                    .into_iter()
                    .flatten()
                    .filter_map(|d| {
                        let id = novo_core::DeviceId::from(d);
                        let a = project.rated_current_a(id).filter(|a| *a > 0.0)?;
                        Some((a, project.device(id)?.tag.to_string()))
                    })
                    .collect();
                let Some((a, tag)) = ends.into_iter().min_by(|x, y| x.0.total_cmp(&y.0)) else {
                    continue;
                };
                (
                    a,
                    format!("rated current of {tag} (planning value: the smaller rated current)"),
                    false,
                )
            }
            None => continue,
        };
        let phase = w
            .properties
            .get("emc.phase_deg")
            .and_then(as_f64)
            .filter(|v| v.is_finite())
            .map(|p| (p, "stated on the wire (emc.phase_deg)"))
            .or_else(|| {
                [w.from.as_pin(), w.to.as_pin()]
                    .into_iter()
                    .flatten()
                    .find_map(|p| phase_of_pin(&pin_name(project, p)))
                    .map(|p| (p, "from the pin names"))
            })
            .or_else(|| {
                let n = run.net?;
                let potentials = &netlist.net(NetId(n)).potentials;
                potentials.iter().find_map(|pid| {
                    match project.get::<novo_core::Potential>(*pid)?.kind {
                        PotentialKind::DcPositive => Some((0.0, "DC positive")),
                        PotentialKind::DcNegative => Some((180.0, "DC negative: the return")),
                        _ => None,
                    }
                })
            });
        let idx = out.len();
        if let Some(c) = run.cable {
            cables.entry(c).or_default().push(idx);
        }
        out.push(Current {
            run: run.id,
            amps,
            phase_deg: phase.map_or(0.0, |p| p.0),
            source: match phase {
                Some((_, how)) => format!("{source}; phase {how}"),
                None => format!("{source}; phase not known, taken as a lone conductor"),
            },
            declared: is_declared,
            phase_known: phase.is_some(),
        });
    }
    // Cores of one cable with no phase of their own: go and return, or a
    // balanced three-phase set, in the order of the wires.
    for members in cables.values() {
        let open: Vec<usize> = members
            .iter()
            .copied()
            .filter(|i| !out[*i].phase_known)
            .collect();
        let set: &[f64] = match open.len() {
            2 => &[0.0, 180.0],
            3 => &[0.0, -120.0, 120.0],
            _ => continue,
        };
        for (i, ph) in open.iter().zip(set) {
            out[*i].phase_deg = *ph;
            out[*i].phase_known = true;
            out[*i].source = format!(
                "{}; phase assumed from the core order of the cable ({} cores)",
                out[*i].source.split("; phase").next().unwrap_or_default(),
                set.len()
            );
        }
    }
    out
}

/// Currents and settings of the magnetic field estimate for a project.
pub fn field_setup_of(project: &Project, netlist: &Netlist, input: &Input) -> FieldSetup {
    FieldSetup {
        currents: currents_of(project, netlist, input),
        params: field_params_of(project),
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
        let Endpoint::Pin { symbol, pin } = e else {
            continue;
        };
        let Some(sym) = project.symbol(*symbol) else {
            continue;
        };
        let (group, name) = key_parts(&sym.symbol.0);
        let pname = sym
            .pins
            .get(usize::from(*pin))
            .map(|p| p.name.as_str())
            .unwrap_or("");
        match group {
            "drives" if name == "frequency_converter" && is_drive_output(pname) => {
                motor_drive =
                    novo_routing::model::device_of_pin(project, PinRef::new(*symbol, *pin))
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
                if matches!(
                    pname,
                    "1" | "2" | "3" | "4" | "5" | "6" | "L1" | "L2" | "L3" | "T1" | "T2" | "T3"
                ) =>
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
    let mk = |class, origin, why: &str| NetClass {
        class,
        origin,
        why: why.to_owned(),
        drive: None,
    };
    if let Some(d) = motor_drive {
        return NetClass {
            drive: (d != 0).then_some(d),
            ..mk(
                EmcClass::Motor,
                Origin::Derived,
                "on a drive's motor output",
            )
        };
    }
    if safety {
        return mk(EmcClass::Safety, Origin::Derived, "on a safety device");
    }
    if fieldbus {
        return mk(EmcClass::Fieldbus, Origin::Derived, "on a network port");
    }
    if analog {
        return mk(
            EmcClass::Analog,
            Origin::Derived,
            "on an analog channel or sensor",
        );
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
            mk(
                EmcClass::Power,
                Origin::Derived,
                "a potential of 50 V or more",
            )
        } else {
            mk(EmcClass::Control, Origin::Derived, "a potential under 50 V")
        };
    }
    if kinds.iter().any(|k| {
        matches!(
            k,
            PotentialKind::Line | PotentialKind::Pen | PotentialKind::Neutral
        )
    }) {
        return mk(EmcClass::Power, Origin::Derived, "a mains potential");
    }
    if power_device {
        return mk(
            EmcClass::Power,
            Origin::Derived,
            "on a motor, heater or contactor power pole",
        );
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
            } else if let Some(mm) = t
                .strip_prefix("pigtail:")
                .and_then(|n| n.trim().parse().ok())
            {
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
    match text(&s.properties, "emc.finish")?
        .to_ascii_lowercase()
        .as_str()
    {
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
        let SymbolRole::Device { device: d, .. } = &s.role else {
            continue;
        };
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
    let placements: HashMap<_, _> = project
        .all::<CabinetPlacement>()
        .map(|p| (p.device, p))
        .collect();
    let started = !placements.is_empty();

    // Plates and ducts.
    let mut plates = Vec::new();
    let mut ducts: Vec<Duct> = Vec::new();
    let mut tag_of: HashMap<u64, DeviceTag> = HashMap::new();
    for s in project.structures().filter(|s| s.enclosure) {
        tag_of.insert(s.id.raw(), s.tag.clone());
        plates.push(Plate {
            id: s.id.raw(),
            tag: s.tag.to_string(),
            finish: finish_of(s),
        });
    }
    if started {
        let dividers: HashMap<u64, bool> = project
            .all::<Rail>()
            .filter(|r| r.kind == RailKind::Duct)
            .map(|r| {
                (
                    r.id.raw(),
                    r.properties
                        .get("emc.divider")
                        .and_then(as_bool)
                        .unwrap_or(false),
                )
            })
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
            let (w, h) = if pl.rotated {
                (size.height, size.width)
            } else {
                (size.width, size.height)
            };
            (pl.x, pl.y, w, h)
        });
        let filter = match device_prop(project, d, "emc.filter") {
            Some(Value::Text(t)) => {
                let t = t.trim().to_ascii_lowercase();
                if matches!(
                    t.as_str(),
                    "built-in" | "builtin" | "internal" | "c1" | "c2"
                ) {
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
            height_mm: novo_layout::model::device_depth(project, d).filter(|h| *h > 0.0),
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
            (_, (Some(a), Some(b))) => {
                format!("{} to {}", pin_label(project, a), pin_label(project, b))
            }
            (_, (Some(p), None)) | (_, (None, Some(p))) => pin_label(project, p),
            _ => format!("wire {}", w.id.raw()),
        };
        let cable_id = project.cable_of_wire(w.id);
        let cable = cable_id.and_then(|c| project.cable(c));
        let declared = text(&w.properties, "emc.class")
            .or_else(|| cable.and_then(|c| text(&c.properties, "emc.class")))
            .and_then(EmcClass::parse);
        let nc = net
            .map(|n| {
                net_class
                    .entry(n)
                    .or_insert_with(|| classify_net(project, netlist, n))
                    .clone()
            })
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
        let shield = cable
            .filter(|c| c.cores.iter().any(|k| k.shield))
            .map(|c| Shield {
                a: shield_end(c, "emc.shield_a"),
                b: shield_end(c, "emc.shield_b"),
            });
        let device_of =
            |p: Option<PinRef>| p.and_then(|p| novo_routing::model::device_of_pin(project, p));
        let (da, db) = (device_of(pins.0), device_of(pins.1));
        let plate = [da, db]
            .into_iter()
            .flatten()
            .find_map(|d| placements.get(&d))
            .map(|pl| pl.structure);
        let route = routed.as_ref().and_then(|r| r.routes.get(&w.id.raw()));
        let diameter = novo_routing::model::wire_diameter_mm(
            novo_routing::model::wire_section_mm2(project, w),
        )
        .unwrap_or(novo_routing::DEFAULT_WIRE_DIAMETER_MM);
        let leaves = w
            .properties
            .get("emc.leaves")
            .and_then(as_bool)
            .unwrap_or_else(|| {
                let Some(pl) = plate.and_then(|s| tag_of.get(&s.raw())) else {
                    return false;
                };
                [da, db]
                    .into_iter()
                    .flatten()
                    .filter_map(|d| project.device(d))
                    .any(|d| {
                        !d.tag.location.is_empty()
                            && !pl.location.is_empty()
                            && d.tag.location != pl.location
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
            twisted: cable
                .and_then(|c| c.properties.get("emc.twisted"))
                .and_then(as_bool)
                .unwrap_or(false),
            net: net.map(|n| n.0),
            from: da.map(|d| d.raw()),
            to: db.map(|d| d.raw()),
            plate: plate.map(|s| s.raw()),
            path: route
                .map(|r| r.points.iter().map(|p| (p.x, p.y)).collect())
                .unwrap_or_default(),
            ducts: route.map(|r| r.ducts.clone()).unwrap_or_default(),
            diameter_mm: diameter,
            drive: nc
                .drive
                .filter(|_| declared.is_none() || class == EmcClass::Motor),
            leaves_cabinet: leaves,
        });
    }
    Input {
        runs,
        ducts,
        devices,
        plates,
        params,
    }
}

/// Runs the engine on a project.
pub fn analyse_project(project: &Project) -> Report {
    analyse_with(project, &project.netlist())
}

/// Runs the engine with a netlist that is current for the project.
pub fn analyse_with(project: &Project, netlist: &Netlist) -> Report {
    let input = input_of(project, netlist);
    let field = field_setup_of(project, netlist, &input);
    crate::analyse_with_field(&input, Some(&field))
}

/// The picture of a project: classes, ribbons, loop sheets, zones and
/// hotspots, ready to draw.
pub fn picture_of(project: &Project) -> crate::Picture {
    let netlist = project.netlist();
    let input = input_of(project, &netlist);
    let field = field_setup_of(project, &netlist, &input);
    let report = crate::analyse_with_field(&input, Some(&field));
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
            tx.set_property(
                filter,
                "emc.filter_for",
                Some(Value::Text(d.tag.to_string())),
            )?;
            Ok(format!(
                "Filter {tag} added for {}: place it next to the drive",
                d.tag
            ))
        }
        Fix::BarePlate { plate } => {
            let id = novo_core::StructureId::from(*plate);
            tx.set_property(id, "emc.finish", Some(Value::Text("bare".to_owned())))?;
            Ok("Plate finish stated as bare: scrape the mounting area bare or use a galvanised plate (a requirement for the build)".to_owned())
        }
        Fix::BondDevice { device } => {
            let id = novo_core::DeviceId::from(*device);
            tx.set_property(id, "emc.bonded", Some(Value::Bool(true)))?;
            Ok("Device stated as bonded to the plate: fit the strap or make metal contact (a requirement for the build)".to_owned())
        }
        Fix::AvoidDuct { wire, duct } => {
            let id = novo_core::WireId::from(*wire);
            let w = tx
                .project()
                .get::<novo_core::Wire>(id)
                .ok_or(ModelError::NotFound(id.into()))?
                .clone();
            let mut ducts = novo_routing::model::avoided_ducts(&w.properties);
            if !ducts.contains(duct) {
                ducts.push(*duct);
            }
            let list: Vec<String> = ducts.iter().map(u64::to_string).collect();
            tx.set_property(
                id,
                novo_routing::model::AVOID_DUCTS,
                Some(Value::Text(list.join(","))),
            )?;
            Ok("Wire asked to stay out of the duct: it takes another duct only if one reaches both of its ends (the router never leaves a wire unrouted for it). Its route and cut length can change: print the wire list and cut list again".to_owned())
        }
        Fix::MoveCloser { device, to, max_mm } => {
            move_device(tx, *device, Some(*to), crate::mover::Goal::Within(*max_mm))
        }
        Fix::MoveAway {
            device,
            from,
            min_mm,
        } => move_device(tx, *device, Some(*from), crate::mover::Goal::Away(*min_mm)),
        Fix::MoveOutOfField { device, below_ut } => {
            move_device(tx, *device, None, crate::mover::Goal::Field(*below_ut))
        }
        Fix::KeepClear { wire, other, to_mm } => {
            let id = novo_core::WireId::from(*wire);
            let w = tx
                .project()
                .get::<novo_core::Wire>(id)
                .ok_or(ModelError::NotFound(id.into()))?
                .clone();
            let mut list = novo_routing::model::keep_clear(&w.properties);
            list.retain(|(o, _)| o != other);
            list.push((*other, to_mm.max(1.0)));
            let text: Vec<String> = list.iter().map(|(o, mm)| format!("{o}:{mm}")).collect();
            tx.set_property(
                id,
                novo_routing::model::KEEP_CLEAR,
                Some(Value::Text(text.join(","))),
            )?;
            // The router keeps the clearance only when ducts allow it: say so
            // instead of storing a request that changes nothing.
            let routed = novo_routing::model::route_project(tx.project());
            let unmet = routed.plans.iter().flat_map(|(_, p)| &p.problems).any(|p| {
                matches!(
                    p,
                    novo_routing::Problem::ClearanceUnmet { job, other: o, .. }
                        if (*job == *wire && o == other) || (*job == *other && o == wire)
                )
            });
            if unmet {
                // The most the plate's ducts can be apart, so the designer sees what is possible.
                let mut best = 0.0_f64;
                for (structure, _) in &routed.plans {
                    let ducts = novo_routing::model::ducts(tx.project(), *structure);
                    let net = novo_routing::model::network(tx.project(), *structure);
                    for x in &ducts {
                        for y in &ducts {
                            best = best.max(net.duct_gap(x.id, y.id).unwrap_or(0.0));
                        }
                    }
                }
                return Err(ModelError::Format(format!(
                    "No ducts keep the two wires {} mm apart: the ducts of the plate are at most {} mm apart edge to edge, and the stubs to the terminals are not counted. Move a duct or the devices by hand",
                    to_mm.round() as i64,
                    best.round() as i64
                )));
            }
            Ok(format!(
                "Wires asked to run in ducts at least {} mm apart, measured edge to edge between ducts (the stubs to the terminals are not counted). Their routes and cut lengths can change: print the wire list and cut list again",
                to_mm.round() as i64
            ))
        }
        Fix::ShortenPigtail { cable, to_mm } => {
            let id = novo_core::CableId::from(*cable);
            let c = tx
                .project()
                .get::<Cable>(id)
                .ok_or(ModelError::NotFound(id.into()))?
                .clone();
            let mut done = 0;
            for key in ["emc.shield_a", "emc.shield_b"] {
                if matches!(shield_end(&c, key), Termination::Pigtail(len) if len > *to_mm) {
                    tx.set_property(
                        id,
                        key,
                        Some(Value::Text(format!("pigtail:{}", to_mm.round() as i64))),
                    )?;
                    done += 1;
                }
            }
            if done == 0 {
                return Err(ModelError::Format(
                    "no shield end is a long pigtail".to_owned(),
                ));
            }
            Ok(format!(
                "{done} shield end{} set to a pigtail of {} mm at most (a requirement for the build)",
                if done == 1 { "" } else { "s" },
                to_mm.round() as i64
            ))
        }
        other => Err(ModelError::Format(format!("{other:?} is done by hand"))),
    }
}

fn move_device(
    tx: &mut Tx<'_>,
    device: u64,
    other: Option<u64>,
    goal: crate::mover::Goal,
) -> Result<String> {
    let spot = crate::mover::find_spot(tx.project(), device, other, goal)?;
    let rotated = tx
        .project()
        .all::<CabinetPlacement>()
        .find(|p| p.device.raw() == device)
        .is_some_and(|p| p.rotated);
    let rail_of = |project: &Project| {
        novo_layout::model::load(project, spot.structure).and_then(|l| {
            l.placements
                .iter()
                .find(|p| p.id == device)
                .and_then(|p| p.rail)
        })
    };
    let was_on_rail = rail_of(tx.project());
    novo_layout::model::place_device(tx, spot.structure, device, spot.x, spot.y, rotated)?;
    let rail_note = match (was_on_rail, rail_of(tx.project())) {
        (Some(_), None) => ". The device no longer stands on its DIN rail: check how it is fixed",
        _ => "",
    };
    let there = match (goal, spot.field_ut) {
        (crate::mover::Goal::Field(limit), Some(f)) => format!(
            "the modelled field there is about {f:.1} \u{b5}T, the planning value {limit:.1} \u{b5}T"
        ),
        _ => format!("gap {} mm there", spot.gap_mm.round() as i64),
    };
    Ok(format!(
        "New position: the nearest free spot that meets the rule, {} mm from where the device stands ({there}). Only the plate layout rules and the EMC rules were checked: not heat, airflow, the door swing or the length of the wires to the new spot. Wires are routed again, so cut lengths change, and the mounting holes are drilled anew: print the cut list and drilling plan again{rail_note}",
        spot.moved_mm.round() as i64
    ))
}

/// A finding that stays after the fix but is less severe.
#[derive(Debug, Clone, PartialEq)]
pub struct Eased {
    pub rule: &'static str,
    pub from: crate::report::Severity,
    pub to: crate::report::Severity,
    /// The finding's message after the fix.
    pub after: String,
}

/// What a fix would change, worked out on a copy of the project: the
/// findings it clears, any it brings, and the risk score of each wire it
/// moves. The project itself is not touched.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// What applying it does, in a sentence (as `apply` returns it).
    pub said: String,
    pub findings_before: usize,
    pub findings_after: usize,
    /// Rule and message of each finding the fix clears.
    pub cleared: Vec<(&'static str, String)>,
    /// Rule and message of each finding that is new after it.
    pub added: Vec<(&'static str, String)>,
    /// Each finding that stays but is less severe after the fix, such as a
    /// warning that becomes a note.
    pub eased: Vec<Eased>,
    /// Wires and devices whose score changes: subject, name, before, after.
    pub scores: Vec<(Subject, String, u32, u32)>,
}

fn finding_key(f: &crate::report::Finding) -> String {
    format!("{}{:?}", f.rule, f.subjects)
}

/// Previews `fix` on `project`. Errors when the fix is done by hand.
pub fn preview(project: &Project, fix: &Fix) -> Result<Preview> {
    preview_on(project, fix).map(|(p, _)| p)
}

/// Like [`preview`], also returning the copy with the fix applied, so a
/// view can draw the project as it would be.
pub fn preview_on(project: &Project, fix: &Fix) -> Result<(Preview, Project)> {
    let before = analyse_project(project);
    let mut copy = project.clone();
    let mut said = String::new();
    copy.transact("EMC preview", |tx| {
        said = apply(tx, fix)?;
        Ok(())
    })?;
    let after = analyse_project(&copy);
    let keys = |r: &Report| -> BTreeMap<String, (&'static str, String)> {
        r.findings
            .iter()
            .map(|f| (finding_key(f), (f.rule, f.message.clone())))
            .collect()
    };
    let (kb, ka) = (keys(&before), keys(&after));
    let severity = |r: &Report| -> BTreeMap<String, crate::report::Severity> {
        r.findings
            .iter()
            .map(|f| (finding_key(f), f.severity))
            .collect()
    };
    let (sb, sa) = (severity(&before), severity(&after));
    let eased: Vec<Eased> = sb
        .iter()
        .filter_map(|(k, v)| {
            let a = *sa.get(k).filter(|a| *a > v)?;
            let (rule, _) = kb.get(k)?;
            let (_, after) = ka.get(k)?;
            Some(Eased {
                rule,
                from: *v,
                to: a,
                after: after.clone(),
            })
        })
        .collect();
    let cleared = kb
        .iter()
        .filter(|(k, _)| !ka.contains_key(*k))
        .map(|(_, v)| v.clone())
        .collect();
    let added = ka
        .iter()
        .filter(|(k, _)| !kb.contains_key(*k))
        .map(|(_, v)| v.clone())
        .collect();
    let name_of = |input: &Input, s: Subject| match s {
        Subject::Run(id) => input
            .runs
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.name.clone()),
        Subject::Device(id) => input
            .devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.tag.clone()),
        _ => None,
    };
    let input = input_of(&copy, &copy.netlist());
    let mut scores = Vec::new();
    let all = before.runs.iter().chain(before.devices.iter());
    for b in all {
        let a = after
            .runs
            .iter()
            .chain(after.devices.iter())
            .find(|s| s.subject == b.subject)
            .map_or(0, |s| s.points);
        if a != b.points
            && let Some(name) = name_of(&input, b.subject)
        {
            scores.push((b.subject, name, b.points, a));
        }
    }
    let preview = Preview {
        said,
        findings_before: before.findings.len(),
        findings_after: after.findings.len(),
        cleared,
        added,
        eased,
        scores,
    };
    Ok((preview, copy))
}
