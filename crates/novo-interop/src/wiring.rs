//! Wiring plan to DXF: the mounting plate with its ducts (and how full they
//! are), the devices and every wire's route, with a table of the wires and
//! their cut lengths beside it.
//!
//! Units are millimetres, 1:1 with the plate. The plate's top left corner is
//! the origin and y points up as DXF wants, so a point at (x, y) on the plate
//! is at (x, -y) in the file. Layers: PLATE, RAILS, DUCTS, DEVICES, ROUTES
//! (wires through ducts), FREEAIR (wires no duct reaches), TEXT, TABLE.

use novo_core::{Device, DeviceId, Project, Rail, RailKind, StructureId, Wire};
use novo_routing::Kind;
use novo_routing::model::route_project;

use crate::dxf::{Drawing, Entity, EntityKind, HAlign, Layer, Point, TextAlign, VAlign};

const TEXT_MM: f64 = 4.0;
const TABLE_GAP_MM: f64 = 40.0;
const ROW_MM: f64 = 7.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, -y)
}

fn text(layer: &str, x: f64, y: f64, height: f64, text: String, h: HAlign, v: VAlign) -> Entity {
    Entity::new(
        layer,
        EntityKind::Text {
            pos: pt(x, y),
            height,
            rotation_deg: 0.0,
            text,
            align: TextAlign { h, v },
        },
    )
}

fn rect(layer: &str, x: f64, y: f64, w: f64, h: f64) -> Entity {
    Entity::new(
        layer,
        EntityKind::Polyline {
            points: vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)],
            closed: true,
        },
    )
}

/// The wiring plan of an enclosure's plate, or `None` when it has none.
pub fn wiring_dxf(project: &Project, structure: StructureId) -> Option<Drawing> {
    let layout = novo_layout::model::load(project, structure)?;
    let mut d = Drawing::default();
    for (name, colour) in [
        ("PLATE", 7),
        ("RAILS", 5),
        ("DUCTS", 3),
        ("DEVICES", 8),
        ("ROUTES", 4),
        ("FREEAIR", 1),
        ("TEXT", 7),
        ("TABLE", 7),
    ] {
        d.layers.push(Layer::new(name, colour));
    }
    let (pw, ph) = (layout.plate.width, layout.plate.height);
    d.push(rect("PLATE", 0.0, 0.0, pw, ph));
    let m = layout.plate.margin;
    d.push(rect("PLATE", m, m, pw - 2.0 * m, ph - 2.0 * m));

    let routed = route_project(project);
    let plan = routed
        .plans
        .iter()
        .find(|(s, _)| *s == structure)
        .map(|(_, p)| p);

    for rail in project.all::<Rail>().filter(|r| r.structure == structure) {
        let (w, h) = if rail.vertical {
            (rail.width.max(0.0), rail.length)
        } else {
            (rail.length, rail.width.max(0.0))
        };
        match rail.kind {
            RailKind::Duct => {
                // The duct's line runs down its middle.
                let (x, y) = if rail.vertical {
                    (rail.x - w / 2.0, rail.y)
                } else {
                    (rail.x, rail.y - h / 2.0)
                };
                d.push(rect("DUCTS", x, y, w, h));
                let fill = plan
                    .and_then(|p| p.fill.iter().find(|f| f.duct == rail.id.raw()))
                    .filter(|f| f.capacity_mm2 > 0.0 && !f.jobs.is_empty());
                if let Some(f) = fill {
                    d.push(text(
                        "TEXT",
                        x + w / 2.0,
                        y + h / 2.0,
                        TEXT_MM,
                        format!("{:.0}%", f.ratio * 100.0),
                        HAlign::Center,
                        VAlign::Middle,
                    ));
                }
            }
            _ => {
                let (x1, y1) = if rail.vertical {
                    (rail.x, rail.y + rail.length)
                } else {
                    (rail.x + rail.length, rail.y)
                };
                d.push(Entity::new(
                    "RAILS",
                    EntityKind::Line {
                        a: pt(rail.x, rail.y),
                        b: pt(x1, y1),
                    },
                ));
            }
        }
    }

    for p in &layout.placements {
        let f = p.footprint();
        d.push(rect("DEVICES", f.x, f.y, f.w, f.h));
        let tag = project
            .get::<Device>(DeviceId::from(p.id))
            .and_then(|dev| dev.tag.device_code().map(str::to_owned))
            .unwrap_or_default();
        d.push(text(
            "DEVICES",
            f.x + f.w / 2.0,
            f.y + f.h / 2.0,
            TEXT_MM,
            tag,
            HAlign::Center,
            VAlign::Middle,
        ));
    }

    let wires: std::collections::HashMap<u64, &Wire> =
        project.wires().map(|w| (w.id.raw(), w)).collect();
    let mut table: Vec<(String, f64, &'static str)> = Vec::new();
    for route in plan.map(|p| p.routes.as_slice()).unwrap_or_default() {
        let layer = if route.kind == Kind::FreeAir {
            "FREEAIR"
        } else {
            "ROUTES"
        };
        d.push(Entity::new(
            layer,
            EntityKind::Polyline {
                points: route.points.iter().map(|p| pt(p.x, p.y)).collect(),
                closed: false,
            },
        ));
        let wire = wires.get(&route.job);
        let label = wire
            .and_then(|w| w.number.clone())
            .unwrap_or_else(|| format!("#{}", route.job));
        let ends = wire
            .and_then(|w| Some((w.from.as_pin()?, w.to.as_pin()?)))
            .map(|(a, b)| {
                format!(
                    "{} > {}",
                    project.pin_designation(a).unwrap_or_default(),
                    project.pin_designation(b).unwrap_or_default()
                )
            })
            .unwrap_or_default();
        let length = route.length_mm.ceil().max(1.0);
        // The label sits on the longest straight piece.
        if let Some(w) = route
            .points
            .windows(2)
            .max_by(|a, b| a[0].manhattan(&a[1]).total_cmp(&b[0].manhattan(&b[1])))
        {
            d.push(text(
                layer,
                (w[0].x + w[1].x) / 2.0,
                (w[0].y + w[1].y) / 2.0,
                TEXT_MM * 0.75,
                label.clone(),
                HAlign::Center,
                VAlign::Bottom,
            ));
        }
        let basis = match route.kind {
            Kind::FreeAir => "free air",
            Kind::Ducted => "routed",
        };
        table.push((format!("{label}  {ends}"), length, basis));
    }

    // The wire table to the right of the plate, longest first.
    table.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let x = pw + TABLE_GAP_MM;
    d.push(text(
        "TABLE",
        x,
        0.0,
        TEXT_MM * 1.25,
        "Wire        From > To        Cut length mm   Basis".into(),
        HAlign::Left,
        VAlign::Top,
    ));
    for (i, (name, length, basis)) in table.iter().enumerate() {
        d.push(text(
            "TABLE",
            x,
            (i as f64 + 1.5) * ROW_MM,
            TEXT_MM,
            format!("{name}   {length:.0}   {basis}"),
            HAlign::Left,
            VAlign::Top,
        ));
    }
    Some(d)
}
