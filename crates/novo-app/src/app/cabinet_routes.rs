//! Wire routes on the Cabinet canvas (WS-16): every routed wire as a line
//! over the plate and every duct tinted by how full it is, with the fill as a
//! percentage so colour is never the only cue. The numbers come from
//! novo-routing; this file only turns them into lines and rectangles.

use eframe::egui;
use novo_canvas::Viewport;
use novo_core::{Project, Rail, RailKind, StructureId};
use novo_routing::Kind;
use novo_routing::model::route_project;
use novo_theme::tokens;

/// One routed wire, as a polyline in plate millimetres.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RouteLine {
    /// The wire's id.
    pub(super) wire: u64,
    pub(super) points: Vec<(f32, f32)>,
    /// No duct reaches both ends: drawn dashed and red.
    pub(super) free_air: bool,
    pub(super) length_mm: f64,
}

/// A duct and how full it is.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DuctFill {
    pub(super) rail: u64,
    /// Top left and size on the plate, in mm.
    pub(super) rect: (f32, f32, f32, f32),
    /// Used over allowed fill, 0 for a duct nothing runs in; `None` when the
    /// duct has no cross-section yet.
    pub(super) ratio: Option<f64>,
    pub(super) wires: usize,
}

/// Everything drawn over the plate for the routes.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Overlay {
    pub(super) routes: Vec<RouteLine>,
    pub(super) ducts: Vec<DuctFill>,
}

impl Overlay {
    /// One line for the header strip: routed wires, those in free air, the
    /// fullest duct.
    pub(super) fn summary(&self) -> String {
        let free = self.routes.iter().filter(|r| r.free_air).count();
        let mut text = format!(
            "{} {} routed",
            self.routes.len(),
            if self.routes.len() == 1 {
                "wire"
            } else {
                "wires"
            }
        );
        if free > 0 {
            text.push_str(&format!(", {free} in free air"));
        }
        let fullest = self
            .ducts
            .iter()
            .filter_map(|d| d.ratio)
            .fold(None, |m: Option<f64>, r| Some(m.map_or(r, |m| m.max(r))));
        if let Some(r) = fullest {
            text.push_str(&format!(", fullest duct {:.0}%", r * 100.0));
        }
        text
    }
}

/// Routes the wires of `structure`'s devices and reads the result as an
/// overlay. Empty when no wire has both ends placed on this plate.
pub(super) fn overlay(project: &Project, structure: StructureId) -> Overlay {
    let routed = route_project(project);
    let Some((_, plan)) = routed.plans.iter().find(|(s, _)| *s == structure) else {
        return Overlay::default();
    };
    let routes = plan
        .routes
        .iter()
        .map(|r| RouteLine {
            wire: r.job,
            points: r.points.iter().map(|p| (p.x as f32, p.y as f32)).collect(),
            free_air: r.kind == Kind::FreeAir,
            length_mm: r.length_mm,
        })
        .collect();
    let ducts = project
        .all::<Rail>()
        .filter(|r| r.structure == structure && r.kind == RailKind::Duct)
        .map(|r| {
            let fill = plan.fill.iter().find(|f| f.duct == r.id.raw());
            let sized = r.width > 0.0 && r.height > 0.0;
            let (w, h) = if r.vertical {
                (r.width, r.length)
            } else {
                (r.length, r.width)
            };
            // The duct's line runs down its middle.
            let (x, y) = if r.vertical {
                (r.x - r.width / 2.0, r.y)
            } else {
                (r.x, r.y - r.width / 2.0)
            };
            DuctFill {
                rail: r.id.raw(),
                rect: (x as f32, y as f32, w as f32, h as f32),
                ratio: sized.then(|| fill.map_or(0.0, |f| f.ratio)),
                wires: fill.map_or(0, |f| f.jobs.len()),
            }
        })
        .collect();
    Overlay { routes, ducts }
}

/// Green up to 80% of the allowed fill, amber to 100%, red above.
fn tint(ratio: f64) -> egui::Color32 {
    if ratio > 1.0 {
        tokens::ERROR
    } else if ratio > 0.8 {
        tokens::WARNING
    } else {
        tokens::SUCCESS
    }
}

/// The overlay of the panel shown, rebuilt only when the project changed or
/// another panel is shown: routing walks every wire, which is too much work
/// for every frame.
#[derive(Default)]
pub(super) struct OverlayCache {
    key: Option<(u64, StructureId)>,
    overlay: Overlay,
}

impl OverlayCache {
    pub(super) fn get(&mut self, project: &Project, structure: StructureId) -> &Overlay {
        let key = (project.revision(), structure);
        if self.key != Some(key) {
            self.overlay = overlay(project, structure);
            self.key = Some(key);
        }
        &self.overlay
    }
}

/// A fill label needs this much room on screen (points), else it is left
/// off and the tint alone shows the duct.
const LABEL_MIN: egui::Vec2 = egui::vec2(30.0, 14.0);

fn fits(area: egui::Rect) -> bool {
    area.width() >= LABEL_MIN.x && area.height() >= LABEL_MIN.y
}

/// Paints the overlay. `rect` is the canvas widget's screen rectangle.
pub(super) fn draw(painter: &egui::Painter, rect: egui::Rect, view: &Viewport, overlay: &Overlay) {
    let to_screen = |x: f32, y: f32| {
        let s = view.world_to_screen(novo_canvas::vec2(x, y));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    for d in &overlay.ducts {
        let (x, y, w, h) = d.rect;
        let area = egui::Rect::from_two_pos(to_screen(x, y), to_screen(x + w, y + h));
        match d.ratio {
            Some(ratio) => {
                let c = tint(ratio);
                painter.rect_filled(area, 0.0, c.gamma_multiply(0.25));
                if d.wires > 0 && fits(area) {
                    painter.text(
                        area.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{:.0}%", ratio * 100.0),
                        egui::FontId::proportional(tokens::FONT_SMALL),
                        c,
                    );
                }
            }
            None => {
                painter.rect_filled(area, 0.0, tokens::TEXT_MUTED.gamma_multiply(0.2));
                if d.wires > 0 && fits(area) {
                    painter.text(
                        area.center(),
                        egui::Align2::CENTER_CENTER,
                        "no size",
                        egui::FontId::proportional(tokens::FONT_SMALL),
                        tokens::TEXT_MUTED,
                    );
                }
            }
        }
    }
    for r in &overlay.routes {
        let points: Vec<egui::Pos2> = r.points.iter().map(|&(x, y)| to_screen(x, y)).collect();
        if r.free_air {
            let stroke = egui::Stroke::new(1.5, tokens::ERROR);
            painter.extend(egui::Shape::dashed_line(&points, stroke, 6.0, 4.0));
        } else {
            let stroke = egui::Stroke::new(1.5, tokens::INFO);
            painter.add(egui::Shape::line(points, stroke));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{
        DeviceTag, Direction, MountingPlate, Pin, PinRef, Placement, Point, SymbolRole, Value,
    };

    /// Two devices on a plate joined by a wire, with a vertical duct of the
    /// given size between them (none when `duct` is `None`).
    fn cabinet(duct: Option<(f64, f64)>) -> (Project, StructureId) {
        let mut p = Project::new("t");
        let (cab, _) = p
            .transact("setup", |tx| {
                let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
                tx.set_plate(
                    cab,
                    Some(MountingPlate {
                        width: 400.0,
                        height: 400.0,
                        margin: 10.0,
                    }),
                )?;
                if let Some((w, h)) = duct {
                    let d = tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
                    tx.update::<Rail>(d, |r| r.vertical = true)?;
                    tx.set_rail_size(d, 200.0, w, h)?;
                }
                let part = tx.add_part("ACME", "C1")?;
                tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
                tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
                let page = tx.add_page("1")?;
                let mut pins = Vec::new();
                for (tag, y, dir) in [
                    ("=A1+CAB1-K1", 50.0, Direction::Down),
                    ("=A1+CAB1-K2", 250.0, Direction::Up),
                ] {
                    let d = tx.add_device(DeviceTag::parse(tag)?, "")?;
                    tx.assign_part(d, part)?;
                    tx.place_device(d, cab, Point::new(50.0, y), false, None)?;
                    let symbol = tx.place_symbol(
                        page,
                        "iec/contacts/no",
                        Placement::at(Point::new(0.0, 0.0)),
                        vec![Pin::new("1", 0.0, 0.0).with_direction(dir)],
                        SymbolRole::Device {
                            device: d,
                            main: true,
                        },
                    )?;
                    pins.push(PinRef::new(symbol, 0));
                }
                tx.connect(pins[0], pins[1])?;
                Ok(cab)
            })
            .unwrap();
        (p, cab)
    }

    #[test]
    fn a_wire_through_a_duct_is_a_route_and_a_fill() {
        let (p, cab) = cabinet(Some((40.0, 60.0)));
        let o = overlay(&p, cab);
        assert_eq!(o.routes.len(), 1);
        assert!(!o.routes[0].free_air);
        assert!(o.routes[0].points.len() >= 2);
        assert_eq!(o.ducts.len(), 1);
        let ratio = o.ducts[0].ratio.unwrap();
        assert!(ratio > 0.0 && ratio < 0.1, "{ratio}");
        assert_eq!(o.ducts[0].wires, 1);
        // The duct line is x=150, 40 mm wide: its rectangle spans 130..170.
        assert_eq!(o.ducts[0].rect, (130.0, 100.0, 40.0, 200.0));
        assert!(o.summary().starts_with("1 wire routed, fullest duct"));
    }

    #[test]
    fn without_a_duct_the_wire_is_drawn_as_free_air() {
        let (p, cab) = cabinet(None);
        let o = overlay(&p, cab);
        assert_eq!(o.routes.len(), 1);
        assert!(o.routes[0].free_air);
        assert!(o.ducts.is_empty());
        assert_eq!(o.summary(), "1 wire routed, 1 in free air");
    }

    #[test]
    fn a_duct_without_a_size_has_no_ratio() {
        let (p, cab) = cabinet(Some((0.0, 0.0)));
        let o = overlay(&p, cab);
        assert_eq!(o.ducts[0].ratio, None);
        assert_eq!(o.ducts[0].wires, 1);
    }

    #[test]
    fn a_plate_nothing_is_routed_on_has_an_empty_overlay() {
        let (p, _) = cabinet(None);
        assert_eq!(overlay(&p, StructureId::from(9999)), Overlay::default());
    }

    #[test]
    fn the_cache_rebuilds_only_when_the_project_or_panel_changes() {
        let (mut p, cab) = cabinet(Some((40.0, 60.0)));
        let mut cache = OverlayCache::default();
        assert_eq!(cache.get(&p, cab).routes.len(), 1);
        // Poison the stored copy: an unchanged project must not rebuild it.
        cache.overlay.routes.clear();
        assert!(cache.get(&p, cab).routes.is_empty());
        // Any edit bumps the revision and the overlay is built again.
        p.transact("edit", |tx| tx.add_page("2")).unwrap();
        assert_eq!(cache.get(&p, cab).routes.len(), 1);
        // Another panel is another key.
        assert!(cache.get(&p, StructureId::from(9999)).routes.is_empty());
    }
}
