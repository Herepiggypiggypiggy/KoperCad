//! Assembly order: the steps to build one cabinet from its layout, in the
//! order the workshop does them: rails, ducts, devices, wires, labels.
//!
//! Pure geometry and ordering, no model access. The caller names the
//! devices and hands in the wire jobs; the order is a function of the layout
//! alone, so the same layout always gives the same list.
//!
//! Rules (see `docs/design/assembly-checklist.md`):
//! - Rails are marked R1.. and ducts D1.. by position, top to bottom then
//!   left to right, the marks the plate drawing and the checklist use.
//! - A device is mounted after the rail it clips on. Devices follow the rail
//!   order, along the rail; devices on the bare plate come last, in reading
//!   order, so the builder works across the plate and never reaches over
//!   finished work.
//! - Wires go in after every device is mounted, labels last. A label depends
//!   only on its own device.

use crate::Placement;
use crate::state::CabinetLayout;

/// The part of the build a step belongs to, in build order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    Rails,
    Ducts,
    Devices,
    Wires,
    Labels,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Rails => "Rails",
            Phase::Ducts => "Ducts",
            Phase::Devices => "Devices",
            Phase::Wires => "Wires",
            Phase::Labels => "Labels",
        }
    }
}

/// A wire to run, as the wire list names it.
#[derive(Debug, Clone, PartialEq)]
pub struct WireJob {
    /// Wire number or other short name.
    pub name: String,
    /// What to do: ends and length, already formatted.
    pub text: String,
}

/// One step of the build.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// 1-based position in the list.
    pub number: usize,
    pub phase: Phase,
    /// The model id the step is about (rail, duct or device), if any.
    pub subject: Option<u64>,
    /// R1, D1 or the device name; empty for a wire.
    pub mark: String,
    pub text: String,
    /// Numbers of the steps that must be done before this one.
    pub after: Vec<usize>,
}

fn mm(v: f64) -> String {
    crate::drill::mm(v)
}

fn along(p: &Placement, rail_vertical: bool) -> f64 {
    if rail_vertical { p.y } else { p.x }
}

/// The build steps of a cabinet. `name` gives the printed name of a device
/// id (its tag).
pub fn steps(layout: &CabinetLayout, name: &dyn Fn(u64) -> String, wires: &[WireJob]) -> Vec<Step> {
    let mut out: Vec<Step> = Vec::new();
    let mut push = |phase, subject, mark: String, text: String, after: Vec<usize>| {
        let number = out.len() + 1;
        out.push(Step {
            number,
            phase,
            subject,
            mark,
            text,
            after,
        });
        number
    };

    let mut rails: Vec<&crate::Rail> = layout.rails.iter().collect();
    rails.sort_by(|a, b| (a.y, a.x, a.id).partial_cmp(&(b.y, b.x, b.id)).unwrap());
    let mut rail_step = std::collections::BTreeMap::new();
    let mut rail_mark = std::collections::BTreeMap::new();
    for (i, r) in rails.iter().enumerate() {
        let mark = format!("R{}", i + 1);
        let n = push(
            Phase::Rails,
            Some(r.id),
            mark.clone(),
            format!(
                "Cut {} to {} mm and fit it {} at x {}, y {}",
                if r.vertical {
                    "a vertical DIN rail"
                } else {
                    "a DIN rail"
                },
                mm(r.length),
                if r.vertical { "upright" } else { "level" },
                mm(r.x),
                mm(r.y)
            ),
            vec![],
        );
        rail_step.insert(r.id, n);
        rail_mark.insert(r.id, (mark, r.vertical));
    }

    let mut ducts: Vec<&crate::Duct> = layout.ducts.iter().collect();
    ducts.sort_by(|a, b| (a.y, a.x, a.id).partial_cmp(&(b.y, b.x, b.id)).unwrap());
    for (i, d) in ducts.iter().enumerate() {
        push(
            Phase::Ducts,
            Some(d.id),
            format!("D{}", i + 1),
            format!(
                "Cut a {} mm wide wiring duct to {} mm and fit it {} at x {}, y {}",
                mm(d.width),
                mm(d.length),
                if d.vertical { "upright" } else { "level" },
                mm(d.x),
                mm(d.y)
            ),
            vec![],
        );
    }

    // Devices: rail by rail in mark order, along the rail; then the bare
    // plate in reading order.
    let mut device_step = std::collections::BTreeMap::new();
    let mount =
        |push: &mut dyn FnMut(Phase, Option<u64>, String, String, Vec<usize>) -> usize,
         p: &Placement,
         text: String,
         after: Vec<usize>| { push(Phase::Devices, Some(p.id), name(p.id), text, after) };
    for r in &rails {
        let (mark, vertical) = &rail_mark[&r.id];
        let mut on: Vec<&Placement> = layout
            .placements
            .iter()
            .filter(|p| p.rail == Some(r.id))
            .collect();
        on.sort_by(|a, b| {
            (along(a, *vertical), a.id)
                .partial_cmp(&(along(b, *vertical), b.id))
                .unwrap()
        });
        for p in on {
            let offset = along(p, *vertical) - if *vertical { r.y } else { r.x };
            let n = mount(
                &mut push,
                p,
                format!(
                    "Clip {} on {mark}, {} mm from its start{}",
                    name(p.id),
                    mm(offset),
                    if p.rotated { ", turned" } else { "" }
                ),
                vec![rail_step[&r.id]],
            );
            device_step.insert(p.id, n);
        }
    }
    let mut bare: Vec<&Placement> = layout
        .placements
        .iter()
        .filter(|p| p.rail.is_none_or(|r| !rail_step.contains_key(&r)))
        .collect();
    bare.sort_by(|a, b| (a.y, a.x, a.id).partial_cmp(&(b.y, b.x, b.id)).unwrap());
    for p in bare {
        let n = mount(
            &mut push,
            p,
            format!(
                "Screw {} to the plate at x {}, y {}{}",
                name(p.id),
                mm(p.x),
                mm(p.y),
                if p.rotated { ", turned" } else { "" }
            ),
            vec![],
        );
        device_step.insert(p.id, n);
    }

    let all_devices: Vec<usize> = device_step.values().copied().collect::<Vec<_>>();
    let mut all_devices = all_devices;
    all_devices.sort_unstable();
    for w in wires {
        push(
            Phase::Wires,
            None,
            w.name.clone(),
            w.text.clone(),
            all_devices.clone(),
        );
    }

    // Labels in the order the devices were mounted.
    let mut mounted: Vec<(&u64, &usize)> = device_step.iter().collect();
    mounted.sort_by_key(|(_, n)| **n);
    for (id, n) in mounted {
        push(
            Phase::Labels,
            Some(*id),
            name(*id),
            format!("Fit the label {}", name(*id)),
            vec![*n],
        );
    }
    out
}

/// What is wrong with an order, as sentences; empty when it is sound. Every
/// step must come after what it depends on, numbers must run 1..n, and the
/// phases must not go backwards.
pub fn order_problems(steps: &[Step]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        if s.number != i + 1 {
            out.push(format!("Step {} is numbered {}", i + 1, s.number));
        }
        for a in &s.after {
            if *a >= s.number {
                out.push(format!("Step {} comes before step {a} it needs", s.number));
            }
        }
        if i > 0 && steps[i - 1].phase > s.phase {
            out.push(format!(
                "Step {} ({}) comes after a later phase",
                s.number,
                s.phase.name()
            ));
        }
    }
    out
}
