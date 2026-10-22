//! From the result of the reduced scenario back to the real devices.

use novo_thermal::{DeviceResult, HotSpot, HotSpotKind, Scenario, Status};

use super::build::Reduction;
use crate::cabinet::snapshot::apply_margins;
use crate::result::{CfdSnapshot, DeviceHeatPaths, MaterialKind};

/// Spread of the bands of a device estimated from its block, share of the
/// rise. Wider than the 12 to 20 % of a resolved device on purpose.
const MEMBER_BAND_SHARE: f64 = 0.35;
/// Film coefficient used when the block has none (no air next to it), W/m2K.
const FALLBACK_FILM_H: f64 = 5.0;

/// The snapshot of the reduced run, expressed for the original scenario:
/// one result per real device, the device materials renamed, the notes and
/// the accuracy statement extended.
pub fn expand(snap: &CfdSnapshot, red: &Reduction, original: &Scenario) -> CfdSnapshot {
    let mut out = snap.clone();
    let amb = original.ambient_c;
    let block_result = |id: u64| snap.devices.iter().find(|d| d.id == id);
    let block_paths = |id: u64| snap.heat_paths.iter().find(|d| d.id == id);

    let mut devices: Vec<DeviceResult> = Vec::new();
    let mut paths: Vec<DeviceHeatPaths> = Vec::new();
    for b in &red.blocks {
        let Some(br) = block_result(b.id) else {
            continue;
        };
        let block_loss: f64 = b.members.iter().map(|&i| original.devices[i].loss_w).sum();
        for &i in &b.members {
            let d = &original.devices[i];
            let mut r = br.clone();
            r.id = d.id;
            r.footprint = d.footprint;
            r.loss_w = d.loss_w;
            r.rated_max_ambient_c = d.rated_max_ambient_c;
            r.rated_max_surface_c = d.rated_max_surface_c;
            if b.members.len() > 1 {
                let h = if br.h_w_m2k > 0.5 {
                    br.h_w_m2k
                } else {
                    FALLBACK_FILM_H
                };
                let f = &d.footprint;
                let area = (f.w * f.h + 2.0 * (f.w + f.h) * d.height()) * 1e-6;
                let to_air = d.loss_w * (1.0 - br.plate_share);
                let rise = to_air / (h * area.max(1e-6));
                r.surface_c = br.air_c + rise;
                r.h_w_m2k = h;
                let spread = MEMBER_BAND_SHARE * (r.surface_c - amb).max(0.0);
                r.surface_band_c = (r.surface_c - spread, r.surface_c + spread);
                r.air_band_c = br.air_band_c;
            }
            apply_margins(&mut r);
            // A lumped device's temperature is an estimate built on its block's
            // air: it may be marginal, but it is never called over its limit.
            if b.members.len() > 1 && r.status == Status::Over {
                r.status = Status::Marginal;
            }
            devices.push(r);
            let share = if block_loss > 0.0 {
                d.loss_w / block_loss
            } else {
                0.0
            };
            if let Some(p) = block_paths(b.id) {
                paths.push(DeviceHeatPaths {
                    id: d.id,
                    total_w: d.loss_w,
                    conduction_w: p.conduction_w * share,
                    radiation_w: p.radiation_w * share,
                });
            }
        }
    }
    devices.sort_by_key(|d| original.devices.iter().position(|o| o.id == d.id));
    out.devices = devices;
    out.heat_paths = paths;

    // the materials of lumped blocks carry the lead member's id and say so
    for m in &mut out.materials {
        if let MaterialKind::Device(id) = m.kind
            && let Some(b) = red
                .blocks
                .iter()
                .find(|b| b.id == id && b.members.len() > 1)
        {
            let lead = original.devices[b.members[0]].id;
            m.kind = MaterialKind::Device(lead);
            m.name = format!("Lumped block of {} devices", b.members.len());
        }
    }

    // device hot spots again from the per-device results; air spots stay
    let lumped: Vec<u64> = red
        .blocks
        .iter()
        .filter(|b| b.members.len() > 1)
        .flat_map(|b| b.members.iter().map(|&i| original.devices[i].id))
        .collect();
    out.hot_spots.retain(|h| h.kind == HotSpotKind::Air);
    let mean_air = mean_air(snap);
    let mut ranked: Vec<&DeviceResult> = out
        .devices
        .iter()
        .filter(|d| matches!(d.status, Status::Over | Status::Marginal))
        .collect();
    ranked.sort_by(|a, b| {
        a.margin_k
            .unwrap_or(f64::MAX)
            .total_cmp(&b.margin_k.unwrap_or(f64::MAX))
    });
    let spots: Vec<HotSpot> = ranked
        .iter()
        .map(|d| {
            let f = d.footprint;
            HotSpot {
                kind: HotSpotKind::Device(d.id),
                x_mm: f.x + f.w / 2.0,
                y_mm: f.y + f.h / 2.0,
                temp_c: d.surface_c,
                above_mean_k: d.surface_c - mean_air,
                reason: if d.status == Status::Over {
                    "over its temperature limit".into()
                } else if lumped.contains(&d.id) && d.margin_k.is_some_and(|m| m < 0.0) {
                    "estimated over its limit from a lumped block; not resolved, run a finer level"
                        .into()
                } else {
                    "within its limit, but not at the pessimistic end of the estimate".into()
                },
            }
        })
        .collect();
    out.hot_spots.splice(0..0, spots);

    out.notes.splice(0..0, red.report.notes());
    out.accuracy.headline.push_str(" Reduced model.");
    out.accuracy.limits.insert(
        1,
        "Reduced model for a large cabinet: cold devices are lumped into blocks, thin ducts and \
         rails are widened to porous strips and edges are snapped to a pitch. The air temperature \
         round a lumped device is the block's, its housing temperature is estimated from its own \
         loss. Not compared with a full-resolution run of the same cabinet, nor with a \
         measurement."
            .into(),
    );
    out
}

/// Mean temperature of the air cells (volume weighted).
fn mean_air(s: &CfdSnapshot) -> f64 {
    let g = &s.grid;
    let (mut sum, mut vol) = (0.0, 0.0);
    for k in 0..g.nz {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let c = g.index(i, j, k);
                if s.material[c] == 0 {
                    let v = g.volume_m3(i, j, k);
                    sum += v * f64::from(s.temperature_c[c]);
                    vol += v;
                }
            }
        }
    }
    if vol > 0.0 { sum / vol } else { 0.0 }
}
