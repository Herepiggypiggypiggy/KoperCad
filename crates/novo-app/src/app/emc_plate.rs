//! The EMC view on the Cabinet's 2D mounting plate: the same picture as the
//! 3D view (`emc_view`), drawn flat over the plate. Wires in their class
//! colour, keep-clear bands, an exposure map, numbered hot spots.

use eframe::egui;
use novo_canvas::Viewport;
use novo_view3d::emc::{self, Class, EmcLayers, Picture, field};

use super::emc_view::style;

fn color(c: [u8; 3], a: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}

/// What the plate view draws.
pub(super) struct Shown<'a> {
    pub(super) pic: &'a Picture,
    /// The exposure map of `pic`.
    pub(super) field: Option<&'a emc::Field>,
    pub(super) layers: &'a EmcLayers,
    pub(super) selected: Option<u32>,
}

/// Paints the picture over the plate. `rect` is the canvas widget's screen
/// rectangle. Returns the hot spot whose badge was clicked.
pub(super) fn draw(
    ui: &egui::Ui,
    rect: egui::Rect,
    view: &Viewport,
    shown: &Shown,
    dark: bool,
) -> Option<u32> {
    let Shown {
        pic,
        field,
        layers,
        selected,
    } = *shown;
    let st = style(dark);
    let pal = crate::theme::ui_palette();
    let painter = ui.painter_at(rect);
    let to_screen = |x: f32, y: f32| {
        let s = view.world_to_screen(novo_canvas::vec2(x, y));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    let px_per_mm = (to_screen(1.0, 0.0).x - to_screen(0.0, 0.0).x).abs();

    if layers.risk
        && let Some(f) = field
    {
        for j in 0..f.ny {
            for i in 0..f.nx {
                let v = f.values[j * f.nx + i];
                let a = (((v - 0.04) / 0.5).clamp(0.0, 1.0) * 150.0).round() as u8;
                if a == 0 {
                    continue;
                }
                let (x, y) = (
                    f.origin[0] + i as f32 * f.cell,
                    f.origin[1] + j as f32 * f.cell,
                );
                let cell =
                    egui::Rect::from_two_pos(to_screen(x, y), to_screen(x + f.cell, y + f.cell));
                painter.rect_filled(cell, 0.0, color(st.risk(v), a));
            }
        }
    }
    if layers.sources {
        for n in pic.nodes.iter().filter(|n| n.role == emc::Role::Source) {
            let c = n.centre();
            let reach = field::reach_mm(n.level) * 1.6;
            let colour = st.colour(Class::Motor);
            for k in 1..=3u8 {
                let r = reach * f32::from(k) / 3.0 * px_per_mm;
                painter.circle_filled(to_screen(c[0], c[1]), r, color(colour, 5 + 4 * (4 - k)));
            }
        }
    }
    if layers.halos {
        for r in pic.runs.iter().filter(|r| r.halo_mm > 0.0) {
            let pts: Vec<egui::Pos2> = r.path.iter().map(|p| to_screen(p[0], p[1])).collect();
            let c = st.colour(r.class);
            for (k, a) in [(1.0, 9u8), (0.66, 13), (0.33, 18)] {
                painter.add(egui::Shape::line(
                    pts.clone(),
                    egui::Stroke::new(2.0 * r.halo_mm * k * px_per_mm, color(c, a)),
                ));
            }
        }
    }
    if layers.classes {
        for r in &pic.runs {
            let pts: Vec<egui::Pos2> = r.path.iter().map(|p| to_screen(p[0], p[1])).collect();
            let c = st.colour(r.class);
            painter.add(egui::Shape::line(
                pts.clone(),
                egui::Stroke::new(2.5, color(c, 255)),
            ));
            if r.shielded {
                painter.add(egui::Shape::line(pts, egui::Stroke::new(1.0, pal.panel)));
            }
        }
    }
    if layers.loops {
        for l in &pic.loops {
            let pts: Vec<egui::Pos2> = l.outline.iter().map(|p| to_screen(p[0], p[1])).collect();
            painter.add(egui::Shape::convex_polygon(
                pts,
                color(st.risk(l.risk), 50),
                egui::Stroke::new(1.0, color(st.risk(l.risk), 200)),
            ));
        }
    }
    let mut picked = None;
    if layers.spots {
        let font = egui::FontId::proportional(12.0);
        let mut spots: Vec<_> = pic.hotspots.iter().collect();
        spots.sort_by(|a, b| a.risk.total_cmp(&b.risk));
        let mut placed: Vec<egui::Rect> = Vec::new();
        for h in spots {
            let p = to_screen(h.at[0], h.at[1]);
            if !rect.contains(p) {
                continue;
            }
            let fill = st.risk(0.55 + 0.45 * h.risk);
            let on = selected == Some(h.id);
            let text = emc::label(h);
            let galley = painter.layout_no_wrap(text, font.clone(), super::emc_view::text_on(fill));
            let size = galley.size() + egui::vec2(12.0, 6.0);
            // A badge that would sit on another one steps down, with a stalk
            // back to its point.
            let mut chip = egui::Rect::from_center_size(p, size);
            for _ in 0..12 {
                if !placed.iter().any(|o| o.expand(2.0).intersects(chip)) {
                    break;
                }
                chip = chip.translate(egui::vec2(0.0, size.y + 3.0));
            }
            placed.push(chip);
            if chip.center() != p {
                painter.line_segment(
                    [p, chip.center_top()],
                    egui::Stroke::new(1.5, color(fill, 230)),
                );
            }
            painter.circle_stroke(
                p,
                (18.0 + 22.0 * h.risk) * px_per_mm.max(0.2),
                egui::Stroke::new(1.5, color(fill, 220)),
            );
            painter.rect(
                chip,
                size.y * 0.5,
                color(fill, 255),
                egui::Stroke::new(
                    if on { 2.5 } else { 1.5 },
                    if on { pal.accent } else { pal.panel },
                ),
                egui::StrokeKind::Outside,
            );
            painter.galley(
                chip.min + egui::vec2(6.0, 3.0),
                galley,
                super::emc_view::text_on(fill),
            );
            let r = ui.interact(
                chip,
                egui::Id::new(("emc_plate_badge", h.id)),
                egui::Sense::click(),
            );
            if r.clicked() {
                picked = Some(h.id);
            }
            r.on_hover_text(format!("{}: {}", emc::label(h), h.title));
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use crate::actions::Action;
    use crate::app::gate1_tests::Drive;

    #[test]
    fn the_cabinet_shows_the_emc_picture_of_the_example() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::OpenEmcExample);
        d.frame();
        d.app
            .switch_workspace(crate::app::workspace::Workspace::Cabinet);
        d.frame();
        d.app.cabinet.emc.on = true;
        d.frame();
        d.frame();
        let pic = d.app.cabinet.emc.picture().expect("a picture of the plate");
        assert!(!pic.runs.is_empty(), "wires are classed");
        assert!(!pic.nodes.is_empty(), "devices have roles");
    }

    #[test]
    fn wires_with_no_duct_route_are_counted_and_not_drawn() {
        use crate::app::cabinet_routes::RouteLine;
        use crate::app::emc_view::EmcState;

        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        crate::app::route_all_tests::wired_cabinet(&mut d);
        let project = d.app.backend.document().unwrap().project();
        let id = project.structures().find(|s| s.enclosure).unwrap().id;
        let mut emc = EmcState::default();
        emc.sync_plate(project, id, || {
            vec![RouteLine {
                wire: 1,
                points: vec![(60.0, 60.0), (60.0, 250.0)],
                free_air: true,
                length_mm: 190.0,
            }]
        });
        assert_eq!(emc.left_out, 1);
        let pic = emc.picture().expect("a picture of the plate");
        assert!(pic.runs.is_empty(), "no run is drawn for it");
    }
}
