//! Builds the result structure from a solved state.

use novo_layout::Rect;

use crate::flow;
use crate::model::Scenario;
use crate::result::*;
use crate::setup::{self, Setup};
use crate::thermal::{self, State, Step};

pub fn grid_field(s: &Setup, values: Vec<f64>) -> Field {
    Field {
        grid: s.grid,
        values,
    }
}

pub fn assumptions(scn: &Scenario) -> Vec<Assumption> {
    let p = &scn.params;
    let fav = p.cooling_favourable();
    let unf = p.cooling_unfavourable();
    let a = |name, value, unit, lo: f64, hi: f64| Assumption {
        name,
        value,
        unit,
        low: lo.min(hi),
        high: lo.max(hi),
    };
    vec![
        a(
            "clutter drag",
            p.clutter_drag_per_m,
            "1/m",
            fav.clutter_drag_per_m,
            unf.clutter_drag_per_m,
        ),
        a(
            "unresolved mixing",
            p.mixing_m2s,
            "m2/s",
            fav.mixing_m2s,
            unf.mixing_m2s,
        ),
        a("plate to air", p.h_in, "W/m2K", fav.h_in, unf.h_in),
        a(
            "device to plate contact",
            p.plate_contact_h,
            "W/m2K",
            fav.plate_contact_h,
            unf.plate_contact_h,
        ),
        a(
            "radiation view factor",
            p.radiation_view_factor,
            "",
            fav.radiation_view_factor,
            unf.radiation_view_factor,
        ),
        a(
            "wall coefficient scale",
            p.wall_k_scale,
            "",
            fav.wall_k_scale,
            unf.wall_k_scale,
        ),
    ]
}

/// The result of one solved state, with bands equal to the central value and
/// status from the central estimate. The caller widens the bands.
pub fn build(
    scn: &Scenario,
    s: &Setup,
    st: &State,
    load: f64,
    step: Option<&Step>,
) -> ThermalResult {
    let grid = s.grid;
    let n = s.n();
    let (vx, vy) = flow::cell_velocity(s, &st.flow);
    let velocity = VectorField {
        grid,
        vx: vx.clone(),
        vy: vy.clone(),
    };

    // Air enthalpy flux relative to ambient, W/m.
    let rho_cp = s.air.rho_cp();
    let mut afx = vec![0.0; n];
    let mut afy = vec![0.0; n];
    for i in 0..n {
        let k = rho_cp * (st.t_air[i] - s.ambient) * s.depth * (1.0 - s.block[i]).max(0.0);
        afx[i] = k * vx[i];
        afy[i] = k * vy[i];
    }

    // Plate conduction flux, W/m.
    let kp = s.params.plate_k * s.params.plate_thickness_mm / 1000.0;
    let mut pfx = vec![0.0; n];
    let mut pfy = vec![0.0; n];
    for r in 0..s.ny {
        for c in 0..s.nx {
            let i = r * s.nx + c;
            if s.plate_idx[i].is_none() {
                continue;
            }
            let on = |j: usize| s.plate_idx[j].is_some();
            let grad = |lo: Option<usize>, hi: Option<usize>, ds: f64| -> f64 {
                match (lo, hi) {
                    (Some(a), Some(b)) => (st.t_plate[b] - st.t_plate[a]) / (2.0 * ds),
                    (Some(a), None) => (st.t_plate[i] - st.t_plate[a]) / ds,
                    (None, Some(b)) => (st.t_plate[b] - st.t_plate[i]) / ds,
                    (None, None) => 0.0,
                }
            };
            let west = (c > 0 && on(i - 1)).then(|| i - 1);
            let east = (c + 1 < s.nx && on(i + 1)).then(|| i + 1);
            let north = (r > 0 && on(i - s.nx)).then(|| i - s.nx);
            let south = (r + 1 < s.ny && on(i + s.nx)).then(|| i + s.nx);
            pfx[i] = -kp * grad(west, east, s.hx);
            pfy[i] = -kp * grad(north, south, s.hy);
        }
    }

    let mut plate_vals = st.t_air.clone();
    for &i in &s.plate_cells {
        plate_vals[i] = st.t_plate[i];
    }

    // Devices (bands and status are filled by the caller).
    let mut devices = Vec::new();
    for (k, d) in scn.devices.iter().enumerate() {
        let dc = &s.devices[k];
        let air_c: f64 = dc.air.iter().map(|&(i, w)| w * st.t_air[i]).sum();
        let surface_c = st.t_dev[k];
        devices.push(DeviceResult {
            id: d.id,
            footprint: d.footprint,
            loss_w: d.loss_w * load,
            surface_c,
            air_c,
            h_w_m2k: st.h_dev[k],
            plate_share: thermal::plate_share(s, st, k),
            surface_band_c: (surface_c, surface_c),
            air_band_c: (air_c, air_c),
            rated_max_ambient_c: d.rated_max_ambient_c,
            rated_max_surface_c: d.rated_max_surface_c,
            margin_k: None,
            margin_pessimistic_k: None,
            status: Status::NoLimit,
        });
    }
    for d in &mut devices {
        apply_margins(d);
    }

    // Balance.
    let f = thermal::fluxes(s, st, load, step);
    let out_total = f.faces.iter().sum::<f64>() + f.openings + f.storage;
    let residual = f.loss - out_total;
    let mean_air = st.t_air.iter().sum::<f64>() / n as f64;
    let max_air = st.t_air.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let balance = Balance {
        loss_w: f.loss,
        front_w: f.faces[setup::FRONT],
        rear_w: f.faces[setup::REAR],
        left_w: f.faces[setup::LEFT],
        right_w: f.faces[setup::RIGHT],
        top_w: f.faces[setup::TOP],
        bottom_w: f.faces[setup::BOTTOM],
        openings_w: f.openings,
        storage_w: f.storage,
        residual_w: residual,
        residual_rel: if f.loss > 0.0 { residual / f.loss } else { 0.0 },
        mean_air_c: mean_air,
        max_air_c: max_air,
        mean_rise_k: mean_air - s.ambient,
        iec_rise_k: if s.k_ae > 0.0 { f.loss / s.k_ae } else { 0.0 },
    };

    // Openings.
    let mut openings: Vec<OpeningFlow> = Vec::new();
    let mut keys: Vec<(usize, bool)> = s.ports.iter().map(|p| (p.index, p.is_fan)).collect();
    keys.sort();
    keys.dedup();
    for (index, is_fan) in keys {
        let mut q = 0.0;
        let mut area = 0.0;
        let mut p_sum = 0.0;
        let mut count = 0.0_f64;
        for (k, port) in s.ports.iter().enumerate() {
            if port.index == index && port.is_fan == is_fan {
                q -= st.flow.port_q[k];
                area += port.area_m2;
                p_sum += st.flow.p[port.cell];
                count += 1.0;
            }
        }
        openings.push(OpeningFlow {
            index,
            is_fan,
            flow_m3h: q * 3600.0,
            speed_ms: if area > 0.0 { q.abs() / area } else { 0.0 },
            pressure_pa: p_sum / count.max(1.0),
        });
    }

    let air_field = grid_field(s, st.t_air.clone());
    let (half, top) = profile(&air_field);
    let max_speed = velocity.max_magnitude();
    let mut notes = vec![NOTE.to_string()];
    if !st.converged && step.is_none() {
        notes.push("The solver did not fully settle; treat the numbers with extra care.".into());
    }
    if s.ports.is_empty() {
        notes.push("Closed enclosure: air circulates inside only.".into());
    }
    if s.jet_mixing_m2s > 0.0 {
        notes.push(
            "Fan case: this 2D model has no jet momentum. A mixing term stands in for the fan \
             jet, but 3D CFD of a test box still gave a mean air rise about 1.6 times higher \
             (air next to the device up to 3 times); treat the mean air temperature as a low \
             estimate. Heat balance and flow agree with the CFD."
                .into(),
        );
    }
    if scn
        .devices
        .iter()
        .any(|d| d.rated_max_ambient_c.is_none() && d.rated_max_surface_c.is_none())
    {
        notes.push("Devices without a temperature limit are not judged.".into());
    }

    ThermalResult {
        grid,
        ambient_c: s.ambient,
        air_temp: air_field,
        plate_temp: grid_field(s, plate_vals),
        on_plate: s.on_plate.clone(),
        velocity,
        plate_flux: VectorField {
            grid,
            vx: pfx,
            vy: pfy,
        },
        air_flux: VectorField {
            grid,
            vx: afx,
            vy: afy,
        },
        devices,
        hot_spots: Vec::new(),
        recommendations: Vec::new(),
        balance,
        openings,
        max_speed_ms: max_speed,
        air_at_half_height_c: half,
        air_at_top_c: top,
        assumptions: assumptions(scn),
        notes,
        iterations: st.iterations,
        converged: st.converged,
    }
}

/// Mean air temperature over the width at half height and at the top.
fn profile(field: &Field) -> (f64, f64) {
    let g = field.grid;
    let row_mean =
        |r: usize| -> f64 { (0..g.cols).map(|c| field.at(c, r)).sum::<f64>() / g.cols as f64 };
    let half = 0.5 * (row_mean(g.rows / 2) + row_mean((g.rows - 1) / 2));
    (half, row_mean(0))
}

/// Sets margin and status from the (possibly widened) bands.
pub fn apply_margins(d: &mut DeviceResult) {
    let margin = |amb: f64, surf: f64| -> Option<f64> {
        let a = d.rated_max_ambient_c.map(|m| m - amb);
        let b = d.rated_max_surface_c.map(|m| m - surf);
        match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    };
    d.margin_k = margin(d.air_c, d.surface_c);
    d.margin_pessimistic_k = margin(d.air_band_c.1, d.surface_band_c.1);
    d.status = match (d.margin_k, d.margin_pessimistic_k) {
        (Some(m), _) if m < 0.0 => Status::Over,
        (Some(_), Some(p)) if p < 0.0 => Status::Marginal,
        (Some(_), _) => Status::Ok,
        _ => Status::NoLimit,
    };
}

/// Ranked hot spots: devices at or over their limit first, then air pockets.
pub fn hot_spots(result: &ThermalResult) -> Vec<HotSpot> {
    let mean = result.balance.mean_air_c;
    let mut out: Vec<HotSpot> = Vec::new();
    let mut ranked: Vec<&DeviceResult> = result
        .devices
        .iter()
        .filter(|d| matches!(d.status, Status::Over | Status::Marginal))
        .collect();
    ranked.sort_by(|a, b| {
        a.margin_k
            .unwrap_or(f64::MAX)
            .total_cmp(&b.margin_k.unwrap_or(f64::MAX))
    });
    for d in ranked {
        let f: Rect = d.footprint;
        let reason = match d.status {
            Status::Over => "over its temperature limit".to_string(),
            _ => "within its limit, but not at the pessimistic end of the estimate".to_string(),
        };
        out.push(HotSpot {
            kind: HotSpotKind::Device(d.id),
            x_mm: f.x + f.w / 2.0,
            y_mm: f.y + f.h / 2.0,
            temp_c: d.surface_c,
            above_mean_k: d.surface_c - mean,
            reason,
        });
    }

    // Air pockets: local maxima well above the mean, spaced apart.
    let field = &result.air_temp;
    let g = field.grid;
    let mut peaks: Vec<(f64, usize, usize)> = Vec::new();
    for r in 0..g.rows {
        for c in 0..g.cols {
            let v = field.at(c, r);
            if v - mean < 2.0 {
                continue;
            }
            let mut is_max = true;
            'win: for dr in -2i64..=2 {
                for dc in -2i64..=2 {
                    let (rr, cc) = (r as i64 + dr, c as i64 + dc);
                    if rr < 0 || cc < 0 || rr >= g.rows as i64 || cc >= g.cols as i64 {
                        continue;
                    }
                    if field.at(cc as usize, rr as usize) > v {
                        is_max = false;
                        break 'win;
                    }
                }
            }
            if is_max {
                peaks.push((v, c, r));
            }
        }
    }
    peaks.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut taken: Vec<(f64, f64)> = Vec::new();
    for (v, c, r) in peaks {
        let (x, y) = g.centre(c, r);
        if taken.iter().any(|t| (t.0 - x).hypot(t.1 - y) < 120.0) {
            continue;
        }
        taken.push((x, y));
        let near = result.devices.iter().find(|d| {
            let f = d.footprint;
            x >= f.x - 40.0 && x <= f.right() + 40.0 && y >= f.y - 40.0 && y <= f.bottom() + 40.0
        });
        let reason = if y < 0.15 * g.height_mm {
            "hot air collects under the top".to_string()
        } else if let Some(d) = near {
            format!("heat from device {} is not carried away", d.id)
        } else {
            "air hardly moves here".to_string()
        };
        out.push(HotSpot {
            kind: HotSpotKind::Air,
            x_mm: x,
            y_mm: y,
            temp_c: v,
            above_mean_k: v - mean,
            reason,
        });
        if out.len() >= 8 {
            break;
        }
    }
    out
}
