//! The hot-spot list of the 3D Heat strip: devices ordered by how close they
//! are to a limit, and the pockets of hot air the solver found. A device row
//! jumps to the device in the view.

use novo_cfd::CfdSnapshot;
use novo_thermal::{HotSpotKind, Status};

/// One device in the list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Row {
    pub id: u64,
    pub tag: String,
    pub surface_c: f64,
    pub loss_w: f64,
    pub status: Status,
    /// Kelvin left to the tightest limit; negative is over.
    pub margin_k: Option<f64>,
    /// The same at the pessimistic end of the uncertainty band.
    pub margin_pessimistic_k: Option<f64>,
}

impl Row {
    /// One line: tag, surface temperature and the margin when a limit is known.
    pub(crate) fn text(&self) -> String {
        let margin = match (self.status, self.margin_k) {
            (Status::Over, Some(m)) => format!(", {:.0} K over its limit", -m),
            (Status::Marginal, Some(m)) => match self.margin_pessimistic_k {
                Some(p) if p < 0.0 => format!(
                    ", {m:.0} K below its limit, but the uncertainty band reaches {:.0} K over",
                    -p
                ),
                _ => format!(", {m:.0} K below its limit"),
            },
            (_, Some(m)) => format!(", {m:.0} K below its limit"),
            _ => String::new(),
        };
        format!(
            "{}  {:.0} °C, {:.1} W{margin}",
            self.tag, self.surface_c, self.loss_w
        )
    }
}

fn rank(s: Status) -> u8 {
    match s {
        Status::Over => 0,
        Status::Marginal => 1,
        Status::Ok => 2,
        Status::NoLimit => 3,
    }
}

/// The `limit` hottest devices: over their limit first, then marginal, then
/// by surface temperature. Devices that lose no heat are left out.
pub(crate) fn rows(snap: &CfdSnapshot, tag_of: &dyn Fn(u64) -> String, limit: usize) -> Vec<Row> {
    let mut out: Vec<Row> = snap
        .devices
        .iter()
        .filter(|d| d.loss_w > 0.0)
        .map(|d| Row {
            id: d.id,
            tag: tag_of(d.id),
            surface_c: d.surface_c,
            loss_w: d.loss_w,
            status: d.status,
            margin_k: d.margin_k,
            margin_pessimistic_k: d.margin_pessimistic_k,
        })
        .collect();
    out.sort_by(|a, b| {
        rank(a.status)
            .cmp(&rank(b.status))
            .then(b.surface_c.total_cmp(&a.surface_c))
    });
    out.truncate(limit);
    out
}

/// The solver's pockets of hot air as text lines, hottest first.
pub(crate) fn air_pockets(snap: &CfdSnapshot) -> Vec<String> {
    let mut v: Vec<_> = snap
        .hot_spots
        .iter()
        .filter(|h| h.kind == HotSpotKind::Air)
        .collect();
    v.sort_by(|a, b| b.temp_c.total_cmp(&a.temp_c));
    v.iter()
        .map(|h| {
            format!(
                "{:.0} °C, {:.0} K above the mean air: {}",
                h.temp_c, h.above_mean_k, h.reason
            )
        })
        .collect()
}

/// The menu on the strip. Returns the device the user picked.
pub(crate) fn menu(
    ui: &mut eframe::egui::Ui,
    snap: &CfdSnapshot,
    tag_of: &dyn Fn(u64) -> String,
) -> Option<u64> {
    let mut jump = None;
    ui.menu_button("Hot spots", |ui| {
        let rows = rows(snap, tag_of, 10);
        if rows.is_empty() {
            ui.label("No device in the result gives off heat.");
        }
        for r in &rows {
            if ui
                .button(r.text())
                .on_hover_text("Select the device and bring it into view")
                .clicked()
            {
                jump = Some(r.id);
                ui.close();
            }
        }
        let air = air_pockets(snap);
        if !air.is_empty() {
            ui.separator();
            ui.label(eframe::egui::RichText::new("Hot air").small());
            for a in air {
                ui.label(eframe::egui::RichText::new(a).small());
            }
        }
    });
    jump
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::snapshot;
    use novo_layout::Rect;
    use novo_thermal::{DeviceResult, HotSpot};

    fn device(
        id: u64,
        surface: f64,
        loss: f64,
        status: Status,
        margin: Option<f64>,
    ) -> DeviceResult {
        DeviceResult {
            id,
            footprint: Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            loss_w: loss,
            surface_c: surface,
            air_c: surface - 5.0,
            h_w_m2k: 8.0,
            plate_share: 0.2,
            surface_band_c: (surface, surface),
            air_band_c: (surface, surface),
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            margin_k: margin,
            margin_pessimistic_k: margin,
            status,
        }
    }

    #[test]
    fn over_the_limit_comes_first_then_the_hottest() {
        let mut snap = snapshot();
        snap.devices = vec![
            device(1, 80.0, 5.0, Status::Ok, Some(20.0)),
            device(2, 60.0, 5.0, Status::Over, Some(-4.0)),
            device(3, 90.0, 0.0, Status::Ok, None),
            device(4, 70.0, 5.0, Status::Ok, Some(30.0)),
        ];
        let tag = |id: u64| format!("-Q{id}");
        let r = rows(&snap, &tag, 10);
        assert_eq!(r.iter().map(|r| r.id).collect::<Vec<_>>(), [2, 1, 4]);
        assert!(
            r[0].text().contains("4 K over its limit"),
            "{}",
            r[0].text()
        );
        assert!(r[1].text().contains("20 K below"));
        assert_eq!(rows(&snap, &tag, 2).len(), 2);
    }

    #[test]
    fn marginal_says_where_the_uncertainty_band_reaches() {
        let mut snap = snapshot();
        let mut d = device(5, 75.0, 5.0, Status::Marginal, Some(3.0));
        d.margin_pessimistic_k = Some(-2.0);
        snap.devices = vec![d];
        let r = rows(&snap, &|id| format!("-Q{id}"), 10);
        let t = r[0].text();
        assert!(
            t.contains("3 K below its limit, but the uncertainty band reaches 2 K over"),
            "{t}"
        );
    }

    #[test]
    fn hot_air_is_listed_hottest_first() {
        let mut snap = snapshot();
        let spot = |t: f64, why: &str| HotSpot {
            kind: HotSpotKind::Air,
            x_mm: 0.0,
            y_mm: 0.0,
            temp_c: t,
            above_mean_k: 9.0,
            reason: why.into(),
        };
        snap.hot_spots = vec![spot(40.0, "a"), spot(55.0, "b")];
        let a = air_pockets(&snap);
        assert!(a[0].contains("55 °C") && a[1].contains("40 °C"));
    }
}
