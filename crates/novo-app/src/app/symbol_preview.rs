//! A picture of one placed symbol: the Relations list of the inspector
//! shows it beside each place a device is drawn, so a contact reads as a
//! contact and not as a row of text. The geometry is a pure function of
//! the model and the symbol art ([`symbol_preview`]); [`paint`] only fits
//! it into a rectangle.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use novo_canvas::model::{self, PlaceholderArt, SymbolArt};
use novo_canvas::{Aabb, Primitive, Shape, Vec2};
use novo_core::{Project, SymbolId};

/// The symbol as drawn on its page (mm), with its pins named.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SymbolPreview {
    pub shapes: Vec<Primitive>,
    /// Pin positions on the page and their designations.
    pub pins: Vec<(Vec2, String)>,
    pub bounds: Aabb,
}

/// The preview of a placed symbol, or `None` when it is not in the project.
pub(super) fn symbol_preview(
    project: &Project,
    art: &dyn SymbolArt,
    id: SymbolId,
) -> Option<SymbolPreview> {
    let sym = project.symbol(id)?;
    let local = art.artwork(sym).unwrap_or_else(|| {
        PlaceholderArt::draw(
            sym,
            novo_canvas::Stroke::world(0.35, novo_canvas::Color::BLACK),
        )
    });
    let place = model::placement(&sym.placement);
    let shapes: Vec<Primitive> = local.iter().map(|p| p.transformed(&place)).collect();
    let pins: Vec<(Vec2, String)> = (0..sym.pins.len())
        .filter_map(|i| {
            let at = sym.pin_position(i as u16)?;
            Some((model::vec(at), sym.pins[i].name.clone()))
        })
        .collect();
    let bounds = shapes
        .iter()
        .fold(Aabb::EMPTY, |b, p| b.union(&p.shape.bounds()))
        .union(&Aabb::from_points(pins.iter().map(|(p, _)| *p)));
    Some(SymbolPreview {
        shapes,
        pins,
        bounds,
    })
}

/// Scale (points per mm) and offset that fit `bounds` into `size` with
/// `margin`, centred; a symbol thinner than 4 mm is fitted as if it were.
pub(super) fn fit(bounds: &Aabb, size: egui::Vec2, margin: f32) -> Option<(f32, egui::Vec2)> {
    if bounds.is_empty() || size.x <= 2.0 * margin || size.y <= 2.0 * margin {
        return None;
    }
    let extent = bounds.size().max(Vec2::splat(4.0));
    let scale = ((size.x - 2.0 * margin) / extent.x).min((size.y - 2.0 * margin) / extent.y);
    let c = bounds.center();
    Some((
        scale,
        egui::vec2(size.x / 2.0 - c.x * scale, size.y / 2.0 - c.y * scale),
    ))
}

/// Paints the preview into `rect`: the symbol in the text colour, its pins
/// as dots with their designations when there is room. `main` outlines the
/// card in the accent colour (the coil).
pub(super) fn paint(ui: &egui::Ui, rect: Rect, preview: &SymbolPreview, main: bool) {
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 4.0, visuals.extreme_bg_color);
    let border = if main {
        Stroke::new(1.0, visuals.selection.stroke.color)
    } else {
        visuals.widgets.noninteractive.bg_stroke
    };
    painter.rect_stroke(rect, 4.0, border, egui::StrokeKind::Inside);
    let Some((scale, offset)) = fit(&preview.bounds, rect.size(), 8.0) else {
        return;
    };
    let to = |p: Vec2| {
        Pos2::new(
            rect.min.x + p.x * scale + offset.x,
            rect.min.y + p.y * scale + offset.y,
        )
    };
    let ink = visuals.text_color();
    let line = Stroke::new(1.3, ink);
    for prim in &preview.shapes {
        let fill = prim.fill.map(|_| ink);
        match &prim.shape {
            Shape::Line { a, b } => {
                painter.line_segment([to(*a), to(*b)], line);
            }
            Shape::Polyline { points, closed } => {
                painter.add(egui::Shape::Path(egui::epaint::PathShape {
                    points: points.iter().map(|p| to(*p)).collect(),
                    closed: *closed,
                    fill: fill.filter(|_| *closed).unwrap_or(Color32::TRANSPARENT),
                    stroke: line.into(),
                }));
            }
            Shape::Rect { min, max } => {
                let r = Rect::from_two_pos(to(*min), to(*max));
                if let Some(f) = fill {
                    painter.rect_filled(r, 0.0, f);
                } else {
                    painter.rect_stroke(r, 0.0, line, egui::StrokeKind::Middle);
                }
            }
            Shape::Circle { center, radius } => {
                let c = to(*center);
                match fill {
                    Some(f) => painter.circle_filled(c, radius * scale, f),
                    None => painter.circle_stroke(c, radius * scale, line),
                };
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let pts: Vec<Pos2> = (0..=20)
                    .map(|i| {
                        let a = start + sweep * i as f32 / 20.0;
                        to(*center + Vec2::new(a.cos(), a.sin()) * *radius)
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, line));
            }
        }
    }
    let accent = visuals.selection.stroke.color;
    let names = preview.pins.len() <= 6 && rect.width() >= 48.0;
    for (at, name) in &preview.pins {
        let p = to(*at);
        painter.circle_filled(p, 1.8, accent);
        if names {
            let right = p.x < rect.center().x;
            painter.text(
                p + egui::vec2(if right { -4.0 } else { 4.0 }, 0.0),
                if right {
                    egui::Align2::RIGHT_CENTER
                } else {
                    egui::Align2::LEFT_CENTER
                },
                name,
                egui::FontId::proportional(8.5),
                visuals.weak_text_color(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{DeviceTag, Document, Pin, Placement, Point, SymbolRole};
    use novo_symbols::{Library, Pack};

    fn contact() -> (Document, SymbolId) {
        let mut doc = Document::default();
        let id = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
                tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(Point::new(50.0, 40.0)),
                    vec![Pin::new("13", 0.0, -8.0), Pin::new("14", 0.0, 8.0)],
                    SymbolRole::Device {
                        device: k1,
                        main: false,
                    },
                )
            })
            .unwrap()
            .0;
        (doc, id)
    }

    fn art() -> novo_editor::art::LibraryArt {
        novo_editor::art::LibraryArt::new(
            Library::shared(Pack::Iec),
            novo_canvas::Stroke::world(0.35, novo_canvas::Color::BLACK),
        )
    }

    #[test]
    fn preview_is_the_symbol_where_it_is_placed_with_its_pins() {
        let (doc, id) = contact();
        let p = symbol_preview(doc.project(), &art(), id).unwrap();
        assert!(!p.shapes.is_empty());
        assert_eq!(
            p.pins,
            [
                (Vec2::new(50.0, 32.0), "13".to_owned()),
                (Vec2::new(50.0, 48.0), "14".to_owned())
            ]
        );
        for (pin, _) in &p.pins {
            assert!(p.bounds.contains_point(*pin));
        }
        assert!(symbol_preview(doc.project(), &art(), SymbolId(u64::MAX)).is_none());
    }

    #[test]
    fn fit_centres_and_keeps_the_aspect_ratio() {
        let b = Aabb::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 20.0));
        let (scale, offset) = fit(&b, egui::vec2(60.0, 60.0), 5.0).unwrap();
        assert_eq!(scale, 2.5);
        // The box centre lands on the rect centre.
        assert_eq!(5.0 * scale + offset.x, 30.0);
        assert_eq!(10.0 * scale + offset.y, 30.0);
        assert!(fit(&Aabb::EMPTY, egui::vec2(60.0, 60.0), 5.0).is_none());
        assert!(fit(&b, egui::vec2(8.0, 8.0), 5.0).is_none());
    }

    /// The preview paints real geometry in a headless egui run.
    #[test]
    fn paints_the_symbol_headlessly() {
        let (doc, id) = contact();
        let preview = symbol_preview(doc.project(), &art(), id).unwrap();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::click());
            paint(ui, rect, &preview, true);
        });
        out.textures_delta.clear();
        let shapes = out.shapes.len();
        let meshes = ctx.tessellate(out.shapes, out.pixels_per_point);
        assert!(shapes > 4, "{shapes} shapes");
        let vertices: usize = meshes
            .iter()
            .map(|m| match &m.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                _ => 0,
            })
            .sum();
        assert!(vertices > 40, "{vertices} vertices");
    }
}
