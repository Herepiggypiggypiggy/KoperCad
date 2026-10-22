//! The interim EMC picture: what the EMC view shows until the rule engine
//! (`novo-emc`) is connected. It reads the routed wires and the placed
//! devices of one plate, guesses a cable class for each wire from the tag
//! letters of its ends and its colour, and measures what geometry can say:
//! where an aggressor and a victim run side by side, which noisy and
//! sensitive parts stand close, and the area each pair of go and return
//! wires encloses.
//!
//! Everything it says is level `Index` or `Rule` and the picture carries
//! `Provenance::Stub`, so the view says that the classes are guesses. The
//! distances it asks for are defaults, not a standard's figures.

use std::collections::BTreeMap;

use novo_view3d::emc::{
    Bond, Class, Fix, Hotspot, Kind, Level, LoopSheet, Node, Picture, Provenance, Quality, Role,
    Run,
};

/// What a device is, from the symbol that draws its main function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// Frequency converter, soft starter.
    Drive,
    /// DC power supply, DC/DC converter.
    Supply,
    Transformer,
    Plc,
    Sensor,
    Motor,
    /// Contactors, relays, coils.
    Switching,
    Protection,
    Terminal,
    Other,
}

/// The family of a library symbol key (`iec/drives/frequency_converter`).
pub(crate) fn family_of(key: &str) -> Family {
    let k = key.to_ascii_lowercase();
    let part = |p: &str| k.contains(&format!("/{p}/"));
    if part("drives") {
        Family::Drive
    } else if part("supplies") {
        Family::Supply
    } else if part("transformers") {
        if k.contains("emc_filter") {
            Family::Other
        } else {
            Family::Transformer
        }
    } else if part("plc") {
        Family::Plc
    } else if part("sensors") {
        Family::Sensor
    } else if part("motors") {
        Family::Motor
    } else if part("coils") || part("contactors") {
        Family::Switching
    } else if part("protection") || part("switches") {
        Family::Protection
    } else if part("terminals") {
        Family::Terminal
    } else {
        Family::Other
    }
}

/// A placed device, as the stub needs it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DeviceIn {
    pub id: u64,
    pub tag: String,
    pub family: Family,
    /// x, y, width, height on the plate, mm.
    pub rect: [f32; 4],
    pub height: f32,
}

/// One end of a wire.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EndIn {
    pub device: Option<u64>,
    pub family: Family,
    /// Library key of the symbol the pin is on.
    pub key: String,
    /// Designation of the pin (`U`, `L+`, `1`).
    pub pin: String,
}

/// A routed wire, as the stub needs it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WireIn {
    pub id: u64,
    pub label: String,
    pub ends: (Option<EndIn>, Option<EndIn>),
    pub colour: Option<String>,
    pub section_mm2: Option<f64>,
    /// Plate x, y and height, mm.
    pub path: Vec<[f32; 3]>,
}

impl WireIn {
    fn devices(&self) -> (Option<u64>, Option<u64>) {
        (
            self.ends.0.as_ref().and_then(|e| e.device),
            self.ends.1.as_ref().and_then(|e| e.device),
        )
    }
}

/// Distance asked between an aggressor and a victim run, mm. A default of
/// this stub: the engine states the real figure per pair.
pub(crate) fn halo_mm(class: Class) -> f32 {
    match class {
        Class::Motor => 80.0,
        Class::Mains => 40.0,
        _ => 0.0,
    }
}

/// How much of the asked distance a victim class needs, 0 to 1.
fn victim_weight(class: Class) -> f32 {
    match class {
        Class::Analog => 1.0,
        Class::Safety => 1.0,
        Class::Bus => 0.9,
        Class::Control => 0.45,
        _ => 0.0,
    }
}

fn aggressor_weight(r: &Run) -> f32 {
    let base = match r.class {
        Class::Motor => 1.0,
        Class::Mains => 0.6,
        _ => 0.0,
    };
    if r.shielded { base * 0.3 } else { base }
}

fn role_of(f: Family) -> (Role, f32, &'static str) {
    match f {
        Family::Drive => (Role::Source, 1.0, "frequency drive"),
        Family::Supply => (Role::Source, 0.45, "switching power supply"),
        Family::Transformer => (Role::Source, 0.4, "transformer"),
        Family::Switching => (Role::Source, 0.25, "contactor or relay"),
        Family::Plc => (Role::Victim, 0.8, "controller"),
        Family::Sensor => (Role::Victim, 0.9, "sensor"),
        _ => (Role::Neutral, 0.0, ""),
    }
}

fn colour_class(colour: &str) -> Option<Class> {
    let c = colour.to_ascii_lowercase();
    if (c.contains("green") && c.contains("yellow")) || c.contains("gnye") || c == "gn/ye" {
        Some(Class::Earth)
    } else if c.contains("blue") || c.contains("red") || c == "bu" || c == "rd" {
        Some(Class::Control)
    } else {
        None
    }
}

fn is_output_pin(pin: &str) -> bool {
    matches!(
        pin.to_ascii_uppercase().as_str(),
        "U" | "V" | "W" | "U1" | "V1" | "W1" | "T1" | "T2" | "T3" | "PE2"
    )
}

fn is_analog_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    ["analog_input", "analog_output", "pt100", "transmitter"]
        .iter()
        .any(|p| k.contains(p))
}

/// The class of one wire and the words for why.
pub(crate) fn classify(w: &WireIn) -> (Class, bool, String) {
    let ends: Vec<&EndIn> = [&w.ends.0, &w.ends.1].into_iter().flatten().collect();
    let colour = w.colour.as_deref().and_then(colour_class);
    if colour == Some(Class::Earth) {
        return (
            Class::Earth,
            false,
            "green-yellow wire: protective conductor".into(),
        );
    }
    if ends
        .iter()
        .any(|e| e.family == Family::Drive && is_output_pin(&e.pin))
    {
        return (
            Class::Motor,
            false,
            "leaves a drive output: motor cable".into(),
        );
    }
    if ends.iter().any(|e| is_analog_key(&e.key)) {
        return (
            Class::Analog,
            false,
            "ends on an analog input or sensor".into(),
        );
    }
    if ends
        .iter()
        .any(|e| e.key.contains("/network/") || (e.family == Family::Plc && e.pin.starts_with('P')))
    {
        return (Class::Bus, false, "ends on a network port".into());
    }
    if ends
        .iter()
        .any(|e| e.family == Family::Supply && matches!(e.pin.as_str(), "L+" | "M" | "+" | "-"))
    {
        return (Class::Control, false, "24 V supply output".into());
    }
    if let Some(c) = colour {
        return (c, false, "blue or red wire: DC control".into());
    }
    if let Some(s) = w.section_mm2.filter(|s| *s >= 2.5) {
        return (Class::Mains, false, format!("{s} mm²: load current"));
    }
    let has = |f: Family| ends.iter().any(|e| e.family == f);
    if has(Family::Drive) || has(Family::Motor) || has(Family::Protection) || has(Family::Supply) {
        return (
            Class::Mains,
            false,
            "ends on a load, a drive or a protective device".into(),
        );
    }
    if has(Family::Switching) && !has(Family::Plc) && !has(Family::Sensor) {
        return (Class::Mains, false, "contactor wiring".into());
    }
    (Class::Control, false, "control wiring".into())
}

fn seg_len(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// Points along a path every `step` mm, each with the direction of its
/// segment (unit, plate plane).
fn sample(path: &[[f32; 3]], step: f32) -> Vec<([f32; 3], [f32; 2])> {
    let mut out = Vec::new();
    let mut carry = 0.0f32;
    for w in path.windows(2) {
        let len = seg_len(w[0], w[1]);
        if len < 1e-3 {
            continue;
        }
        let dir = [(w[1][0] - w[0][0]) / len, (w[1][1] - w[0][1]) / len];
        let mut at = carry;
        while at < len {
            let t = at / len;
            out.push((
                [
                    w[0][0] + (w[1][0] - w[0][0]) * t,
                    w[0][1] + (w[1][1] - w[0][1]) * t,
                    w[0][2] + (w[1][2] - w[0][2]) * t,
                ],
                dir,
            ));
            at += step;
        }
        carry = at - len;
    }
    out
}

/// Closest plate distance from `p` to a path, with the direction of the
/// segment it is closest to.
fn nearest(p: [f32; 3], path: &[[f32; 3]]) -> Option<(f32, [f32; 2])> {
    let mut best: Option<(f32, [f32; 2])> = None;
    for w in path.windows(2) {
        let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-6 {
            continue;
        }
        let t = (((p[0] - w[0][0]) * dx + (p[1] - w[0][1]) * dy) / len2).clamp(0.0, 1.0);
        let (qx, qy) = (w[0][0] + t * dx, w[0][1] + t * dy);
        let d = ((p[0] - qx).powi(2) + (p[1] - qy).powi(2)).sqrt();
        if best.is_none_or(|(b, _)| d < b) {
            let l = len2.sqrt();
            best = Some((d, [dx / l, dy / l]));
        }
    }
    best
}

/// A stretch where one run lies close beside another.
#[derive(Debug, Clone, PartialEq)]
struct Beside {
    length_mm: f32,
    closest_mm: f32,
    span: Vec<[f32; 3]>,
}

/// The longest stretch of `a` that runs within `within` mm of `b` and about
/// along it (within roughly 37 degrees); a crossing is not a parallel run.
fn beside(a: &[[f32; 3]], b: &[[f32; 3]], within: f32) -> Option<Beside> {
    const STEP: f32 = 10.0;
    let mut best: Option<Beside> = None;
    let mut cur: Vec<[f32; 3]> = Vec::new();
    let mut cur_min = f32::INFINITY;
    let close = |cur: &mut Vec<[f32; 3]>, cur_min: &mut f32, best: &mut Option<Beside>| {
        if cur.len() >= 2 {
            let len = (cur.len() - 1) as f32 * STEP;
            if best.as_ref().is_none_or(|b| len > b.length_mm) {
                *best = Some(Beside {
                    length_mm: len,
                    closest_mm: *cur_min,
                    span: cur.clone(),
                });
            }
        }
        cur.clear();
        *cur_min = f32::INFINITY;
    };
    for (p, dir) in sample(a, STEP) {
        match nearest(p, b) {
            Some((d, bdir)) if d <= within && (dir[0] * bdir[0] + dir[1] * bdir[1]).abs() > 0.8 => {
                cur.push(p);
                cur_min = cur_min.min(d);
            }
            _ => close(&mut cur, &mut cur_min, &mut best),
        }
    }
    close(&mut cur, &mut cur_min, &mut best);
    best
}

/// Index of a parallel run, 0 to 1: grows with the length side by side and
/// with how far inside the asked distance the runs lie.
pub(crate) fn parallel_risk(length_mm: f32, closest_mm: f32, asked_mm: f32, weight: f32) -> f32 {
    let k = (length_mm / 150.0) * (asked_mm / (closest_mm + asked_mm * 0.5));
    (1.0 - (-0.8 * k * weight).exp()).clamp(0.0, 1.0)
}

fn rect_gap(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dx = (a[0] - (b[0] + b[2])).max(b[0] - (a[0] + a[2])).max(0.0);
    let dy = (a[1] - (b[1] + b[3])).max(b[1] - (a[1] + a[3])).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn mid(rect: [f32; 4]) -> [f32; 2] {
    [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5]
}

fn middle_of(span: &[[f32; 3]]) -> [f32; 3] {
    span[span.len() / 2]
}

fn shoelace_cm2(points: &[[f32; 3]]) -> f32 {
    let mut s = 0.0;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s.abs() * 0.5 / 100.0
}

/// Loops of the project: every pair of wires between the same two devices
/// encloses the area between them (go along one, back along the other).
fn loops(wires: &[WireIn], runs: &[Run]) -> Vec<LoopSheet> {
    let mut by_pair: BTreeMap<(u64, u64), Vec<&WireIn>> = BTreeMap::new();
    for w in wires {
        if let (Some(a), Some(b)) = w.devices() {
            by_pair.entry((a.min(b), a.max(b))).or_default().push(w);
        }
    }
    let mut out = Vec::new();
    for ws in by_pair.values() {
        // Wires of one class only; a power and a control wire are no pair.
        let class_of = |w: &WireIn| runs.iter().find(|r| r.id == w.id).map(|r| r.class);
        for i in 0..ws.len() {
            for j in i + 1..ws.len() {
                let (a, b) = (ws[i], ws[j]);
                if class_of(a) != class_of(b)
                    || matches!(class_of(a), Some(Class::Earth) | None)
                    || a.path.len() < 2
                    || b.path.len() < 2
                {
                    continue;
                }
                // Both wires leave the first device: join a's path to the
                // reverse of b's, starting from the end nearer a's start.
                let mut second = b.path.clone();
                let d_same = seg_len(a.path[0], b.path[0]);
                let d_flip = seg_len(a.path[0], *b.path.last().unwrap_or(&b.path[0]));
                if d_flip < d_same {
                    second.reverse();
                }
                second.reverse();
                let mut outline = a.path.clone();
                outline.extend(second);
                let area = shoelace_cm2(&outline);
                if area < 1.0 {
                    continue;
                }
                let class = class_of(a).unwrap_or(Class::Control);
                let limit = match class {
                    Class::Analog | Class::Bus => 15.0,
                    Class::Safety => 30.0,
                    _ => 60.0,
                };
                out.push(LoopSheet {
                    label: format!("{} + {}", a.label, b.label),
                    outline,
                    area_cm2: area,
                    risk: (area / (area + limit)).clamp(0.0, 1.0),
                });
            }
        }
    }
    out.sort_by(|a, b| b.area_cm2.total_cmp(&a.area_cm2));
    out.truncate(12);
    out
}

fn advice(kind: Kind, aggressor: &Run, victim: &Run) -> Vec<Fix> {
    match kind {
        Kind::Parallel => vec![
            Fix {
                key: "separate-duct".into(),
                label: format!("Put {} in another duct", victim.label),
                detail: "Run the sensitive wire in a duct of its own, away from the noisy run.".into(),
                applicable: false,
            },
            Fix {
                key: "cross-90".into(),
                label: "Cross at a right angle".into(),
                detail: "Where the runs must meet, cross them at about 90 degrees: the coupling is least.".into(),
                applicable: false,
            },
            Fix {
                key: "shield".into(),
                label: format!("Shield {}", aggressor.label),
                detail: "A screened cable terminated all round at both ends keeps most of the noise in.".into(),
                applicable: false,
            },
        ],
        _ => Vec::new(),
    }
}

/// Builds the picture of one plate.
pub(crate) fn build(plate: [f32; 2], devices: &[DeviceIn], wires: &[WireIn]) -> Picture {
    let mut pic = Picture::empty(plate, Provenance::Stub);
    for d in devices {
        let (role, level, what) = role_of(d.family);
        pic.nodes.push(Node {
            device: d.id,
            tag: d.tag.clone(),
            role,
            rect: d.rect,
            height: d.height,
            level,
            what: what.into(),
        });
    }
    for w in wires {
        if w.path.len() < 2 {
            continue;
        }
        let (class, shielded, why) = classify(w);
        pic.runs.push(Run {
            id: w.id,
            label: w.label.clone(),
            class,
            shielded,
            path: w.path.clone(),
            halo_mm: halo_mm(class),
            class_reason: why,
            overridden: false,
        });
        if class == Class::Earth {
            pic.bonds.push(Bond {
                label: w.label.clone(),
                path: w.path.clone(),
                quality: Quality::Good,
            });
        }
    }
    let mut spots: Vec<Hotspot> = Vec::new();
    // Aggressor runs beside victim runs.
    for a in pic.runs.iter().filter(|r| r.halo_mm > 0.0) {
        for v in pic
            .runs
            .iter()
            .filter(|r| victim_weight(r.class) > 0.0 && r.id != a.id)
        {
            let asked = a.halo_mm * victim_weight(v.class);
            let Some(b) = beside(&a.path, &v.path, asked) else {
                continue;
            };
            if b.length_mm < 40.0 {
                continue;
            }
            let risk = parallel_risk(b.length_mm, b.closest_mm, asked, aggressor_weight(a));
            if risk < 0.12 {
                continue;
            }
            let at = middle_of(&b.span);
            spots.push(Hotspot {
                id: 0,
                kind: Kind::Parallel,
                level: Level::Index,
                at: [at[0], at[1], at[2] + 14.0],
                span: b.span.clone(),
                risk,
                title: format!("{} beside {}", a.class.short(), v.class.short()),
                why: format!(
                    "{} ({}) and {} ({}) run side by side. The noisy wire couples into the sensitive one more the longer they run together and the closer they are.",
                    a.label,
                    a.class.name().to_lowercase(),
                    v.label,
                    v.class.name().to_lowercase()
                ),
                numbers: vec![
                    ("Side by side".into(), format!("{:.0} mm", b.length_mm)),
                    ("Closest".into(), format!("{:.0} mm", b.closest_mm)),
                    ("Distance asked".into(), format!("{asked:.0} mm (default)")),
                    ("Index".into(), format!("{risk:.2}")),
                ],
                runs: vec![a.id, v.id],
                devices: Vec::new(),
                rule: "EMC-02".into(),
                source: String::new(),
                band: None,
                assumptions: Vec::new(),
                fixes: advice(Kind::Parallel, a, v),
            });
        }
    }
    // Noisy parts near sensitive parts.
    for s in pic
        .nodes
        .iter()
        .filter(|n| n.role == Role::Source && n.level >= 0.4)
    {
        for v in pic.nodes.iter().filter(|n| n.role == Role::Victim) {
            let gap = rect_gap(s.rect, v.rect);
            let reach = 60.0 + 140.0 * s.level;
            if gap >= reach {
                continue;
            }
            let risk = (s.level * v.level * (1.0 - gap / reach)).clamp(0.0, 1.0);
            if risk < 0.12 {
                continue;
            }
            let (a, b) = (mid(s.rect), mid(v.rect));
            let at = [
                (a[0] + b[0]) * 0.5,
                (a[1] + b[1]) * 0.5,
                s.height.max(v.height) + 10.0,
            ];
            spots.push(Hotspot {
                id: 0,
                kind: Kind::Proximity,
                level: Level::Index,
                at,
                span: Vec::new(),
                risk,
                title: format!("{} near {}", s.tag, v.tag),
                why: format!(
                    "{} ({}) stands close to {} ({}). The nearer a sensitive part sits to a noisy one, the more it is leaned on.",
                    s.tag, s.what, v.tag, v.what
                ),
                numbers: vec![
                    ("Gap".into(), format!("{gap:.0} mm")),
                    ("Keep-clear (default)".into(), format!("{reach:.0} mm")),
                    ("Index".into(), format!("{risk:.2}")),
                ],
                runs: Vec::new(),
                devices: vec![s.device, v.device],
                rule: "EMC-17".into(),
                source: String::new(),
                band: None,
                assumptions: Vec::new(),
                fixes: vec![Fix {
                    key: "move-victim".into(),
                    label: format!("Move {} away from {}", v.tag, s.tag),
                    detail: "Placing it farther along the plate, or behind a bonded metal divider, lowers the exposure.".into(),
                    applicable: false,
                }],
            });
        }
    }
    // Merge findings of one pair of classes that lie close together (a cable
    // of three wires makes one finding, not three).
    spots.sort_by(|a, b| b.risk.total_cmp(&a.risk));
    let mut kept: Vec<Hotspot> = Vec::new();
    for s in spots {
        let near = kept.iter_mut().find(|k| {
            k.kind == s.kind
                && ((k.at[0] - s.at[0]).powi(2) + (k.at[1] - s.at[1]).powi(2)).sqrt() < 60.0
        });
        match near {
            Some(k) => {
                for r in s.runs {
                    if !k.runs.contains(&r) {
                        k.runs.push(r);
                    }
                }
            }
            None => kept.push(s),
        }
    }
    kept.truncate(12);
    for (i, h) in kept.iter_mut().enumerate() {
        h.id = i as u32 + 1;
    }
    pic.hotspots = kept;
    pic.loops = loops(wires, &pic.runs);
    pic.assumptions = vec![
        "Cable classes are guessed from device tag letters and wire colour".into(),
        "Keep-clear distances are defaults: 80 mm for motor cables, 40 mm for mains, scaled by victim".into(),
        "No currents, frequencies or shield data are used".into(),
    ];
    pic
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: u64, family: Family, rect: [f32; 4]) -> DeviceIn {
        DeviceIn {
            id,
            tag: format!("-D{id}"),
            family,
            rect,
            height: 60.0,
        }
    }

    fn end(device: u64, family: Family, key: &str, pin: &str) -> Option<EndIn> {
        Some(EndIn {
            device: Some(device),
            family,
            key: key.into(),
            pin: pin.into(),
        })
    }

    fn drive_end(device: u64) -> Option<EndIn> {
        end(device, Family::Drive, "iec/drives/frequency_converter", "U")
    }

    fn ai_end(device: u64) -> Option<EndIn> {
        end(device, Family::Plc, "iec/plc/analog_input", "1")
    }

    fn wire(id: u64, ends: (Option<EndIn>, Option<EndIn>), path: &[[f32; 3]]) -> WireIn {
        WireIn {
            id,
            label: format!("w{id}"),
            ends,
            colour: None,
            section_mm2: None,
            path: path.to_vec(),
        }
    }

    fn drive_and_sensor() -> (Vec<DeviceIn>, Vec<WireIn>) {
        let devices = vec![
            dev(1, Family::Drive, [50.0, 50.0, 100.0, 120.0]),
            dev(2, Family::Terminal, [50.0, 400.0, 60.0, 60.0]),
            dev(3, Family::Terminal, [300.0, 400.0, 40.0, 40.0]),
            dev(4, Family::Plc, [300.0, 50.0, 100.0, 100.0]),
        ];
        let wires = vec![
            // drive output to terminals: down the plate
            wire(
                10,
                (
                    drive_end(1),
                    end(2, Family::Terminal, "iec/terminals/terminal", "1"),
                ),
                &[[100.0, 170.0, 10.0], [100.0, 400.0, 10.0]],
            ),
            // analog input to terminals: beside the motor cable for 200 mm, 20 mm off
            wire(
                11,
                (
                    end(3, Family::Terminal, "iec/terminals/terminal", "1"),
                    ai_end(4),
                ),
                &[
                    [120.0, 400.0, 10.0],
                    [120.0, 200.0, 10.0],
                    [300.0, 200.0, 10.0],
                    [300.0, 150.0, 10.0],
                ],
            ),
        ];
        (devices, wires)
    }

    #[test]
    fn knows_the_family_of_a_library_symbol() {
        assert_eq!(family_of("iec/drives/frequency_converter"), Family::Drive);
        assert_eq!(family_of("iec/supplies/power_supply_dc"), Family::Supply);
        assert_eq!(family_of("iec/plc/cpu"), Family::Plc);
        assert_eq!(family_of("iec/sensors/pt100"), Family::Sensor);
        assert_eq!(family_of("iec/transformers/emc_filter"), Family::Other);
        assert_eq!(
            family_of("iec/transformers/transformer_1ph"),
            Family::Transformer
        );
        assert_eq!(family_of("nfpa/motors/motor_3ph"), Family::Motor);
        assert_eq!(family_of("iec/coils/relay"), Family::Switching);
    }

    #[test]
    fn guesses_classes_from_the_ends() {
        let (d, w) = drive_and_sensor();
        let pic = build([500.0, 600.0], &d, &w);
        assert_eq!(pic.run(10).unwrap().class, Class::Motor);
        assert_eq!(pic.run(11).unwrap().class, Class::Analog);
        assert_eq!(pic.run(10).unwrap().halo_mm, 80.0);
        assert_eq!(pic.provenance, Provenance::Stub);
        assert!(pic.is_sane());
    }

    #[test]
    fn a_drive_input_is_mains_and_the_supply_output_is_control() {
        let input = wire(
            1,
            (
                end(1, Family::Drive, "iec/drives/frequency_converter", "L1"),
                None,
            ),
            &[[0.0; 3], [10.0, 0.0, 0.0]],
        );
        let out = wire(
            2,
            (
                end(2, Family::Supply, "iec/supplies/power_supply_dc", "L+"),
                end(3, Family::Plc, "iec/plc/cpu", "L+"),
            ),
            &[[0.0; 3], [10.0, 0.0, 0.0]],
        );
        assert_eq!(classify(&input).0, Class::Mains);
        assert_eq!(classify(&out).0, Class::Control);
        let port = wire(
            3,
            (end(3, Family::Plc, "iec/plc/cpu", "P1"), None),
            &[[0.0; 3], [10.0, 0.0, 0.0]],
        );
        assert_eq!(classify(&port).0, Class::Bus);
    }

    #[test]
    fn finds_the_run_side_by_side_and_measures_it() {
        let (d, w) = drive_and_sensor();
        let pic = build([500.0, 600.0], &d, &w);
        let spot = pic
            .hotspots
            .iter()
            .find(|h| h.kind == Kind::Parallel)
            .expect("a parallel finding");
        assert_eq!(spot.runs, vec![10, 11]);
        assert!(spot.risk > 0.4, "{}", spot.risk);
        let along = spot
            .numbers
            .iter()
            .find(|(n, _)| n == "Side by side")
            .unwrap();
        let mm: f32 = along.1.trim_end_matches(" mm").parse().unwrap();
        assert!((140.0..=240.0).contains(&mm), "{mm}");
        assert!(
            spot.numbers
                .iter()
                .any(|(n, v)| n == "Closest" && v == "20 mm")
        );
        assert_eq!(spot.id, 1);
    }

    #[test]
    fn a_crossing_is_not_a_parallel_run() {
        let devices = vec![
            dev(1, Family::Drive, [0.0; 4]),
            dev(3, Family::Terminal, [0.0; 4]),
            dev(4, Family::Plc, [0.0; 4]),
        ];
        let wires = vec![
            wire(
                10,
                (drive_end(1), None),
                &[[100.0, 0.0, 10.0], [100.0, 300.0, 10.0]],
            ),
            wire(
                11,
                (
                    end(3, Family::Terminal, "iec/terminals/terminal", "1"),
                    ai_end(4),
                ),
                &[[0.0, 150.0, 10.0], [300.0, 150.0, 10.0]],
            ),
        ];
        let pic = build([500.0, 500.0], &devices, &wires);
        assert!(pic.hotspots.iter().all(|h| h.kind != Kind::Parallel));
    }

    #[test]
    fn farther_apart_is_less_risky_than_closer() {
        let near = parallel_risk(200.0, 20.0, 80.0, 1.0);
        let far = parallel_risk(200.0, 70.0, 80.0, 1.0);
        let short = parallel_risk(50.0, 20.0, 80.0, 1.0);
        assert!(near > far && near > short);
        assert!(parallel_risk(200.0, 20.0, 80.0, 0.3) < near);
        assert!((0.0..=1.0).contains(&near));
    }

    #[test]
    fn a_noisy_part_beside_a_sensitive_one_is_a_finding_and_far_away_is_not() {
        let close = vec![
            dev(1, Family::Drive, [0.0, 0.0, 100.0, 100.0]),
            dev(2, Family::Plc, [120.0, 0.0, 80.0, 80.0]),
        ];
        let far = vec![
            dev(1, Family::Drive, [0.0, 0.0, 100.0, 100.0]),
            dev(2, Family::Plc, [700.0, 0.0, 80.0, 80.0]),
        ];
        let a = build([900.0, 400.0], &close, &[]);
        let b = build([900.0, 400.0], &far, &[]);
        assert!(
            a.hotspots
                .iter()
                .any(|h| h.kind == Kind::Proximity && h.devices == vec![1, 2])
        );
        assert!(b.hotspots.is_empty());
    }

    #[test]
    fn earth_wires_become_bonds_and_are_not_aggressors_or_victims() {
        let devices = vec![
            dev(1, Family::Protection, [0.0; 4]),
            dev(2, Family::Terminal, [0.0; 4]),
        ];
        let mut w = wire(5, (None, None), &[[0.0, 0.0, 5.0], [100.0, 0.0, 5.0]]);
        w.colour = Some("GNYE".into());
        let pic = build([300.0, 300.0], &devices, &[w]);
        assert_eq!(pic.run(5).unwrap().class, Class::Earth);
        assert_eq!(pic.bonds.len(), 1);
        assert_eq!(pic.run(5).unwrap().halo_mm, 0.0);
    }

    #[test]
    fn a_go_and_return_pair_far_apart_encloses_a_big_loop() {
        let devices = vec![
            dev(1, Family::Supply, [0.0; 4]),
            dev(2, Family::Plc, [0.0; 4]),
        ];
        let ends = || {
            (
                end(1, Family::Supply, "iec/supplies/power_supply_dc", "L+"),
                end(2, Family::Plc, "iec/plc/cpu", "L+"),
            )
        };
        let go = wire(20, ends(), &[[0.0, 0.0, 5.0], [200.0, 0.0, 5.0]]);
        let back = wire(21, ends(), &[[0.0, 100.0, 5.0], [200.0, 100.0, 5.0]]);
        let pic = build([300.0, 300.0], &devices, &[go, back]);
        let l = pic.loops.first().expect("a loop");
        // 200 mm x 100 mm = 200 cm2
        assert!((l.area_cm2 - 200.0).abs() < 1.0, "{}", l.area_cm2);
        assert!(l.risk > 0.5);
    }

    #[test]
    fn hot_spots_are_numbered_from_one_worst_first() {
        let (d, w) = drive_and_sensor();
        let pic = build([500.0, 600.0], &d, &w);
        let ids: Vec<u32> = pic.hotspots.iter().map(|h| h.id).collect();
        assert_eq!(ids, (1..=ids.len() as u32).collect::<Vec<_>>());
        let risks: Vec<f32> = pic.hotspots.iter().map(|h| h.risk).collect();
        assert!(risks.windows(2).all(|p| p[0] >= p[1]));
    }
}
