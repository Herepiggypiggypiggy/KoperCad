//! The neutral drawing the DXF reader produces and the writer consumes.

/// A point in drawing units (millimetres once [`Drawing::scaled_to_mm`] ran).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

/// A layer. `color` is the AutoCAD colour index (1 red ... 7 white/black).
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub color: u16,
    /// Off or frozen in the source; its entities are still read.
    pub hidden: bool,
}

impl Layer {
    pub fn new(name: impl Into<String>, color: u16) -> Self {
        Layer {
            name: name.into(),
            color,
            hidden: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Baseline,
    Bottom,
    Middle,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextAlign {
    pub h: HAlign,
    pub v: VAlign,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityKind {
    Line {
        a: Point,
        b: Point,
    },
    Polyline {
        points: Vec<Point>,
        closed: bool,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    /// Counter-clockwise from `start_deg` to `end_deg`.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    /// One line of text; `pos` is the alignment point.
    Text {
        pos: Point,
        height: f64,
        rotation_deg: f64,
        text: String,
        align: TextAlign,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub layer: String,
    /// Colour override (AutoCAD index); `None` is "by layer".
    pub color: Option<u16>,
    pub kind: EntityKind,
}

impl Entity {
    pub fn new(layer: impl Into<String>, kind: EntityKind) -> Self {
        Entity {
            layer: layer.into(),
            color: None,
            kind,
        }
    }
}

/// A flat drawing: layers and entities, blocks already exploded.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    /// Millimetres per drawing unit (1.0 for a drawing in millimetres).
    pub mm_per_unit: f64,
    pub layers: Vec<Layer>,
    pub entities: Vec<Entity>,
}

impl Default for Drawing {
    fn default() -> Self {
        Drawing {
            mm_per_unit: 1.0,
            layers: Vec::new(),
            entities: Vec::new(),
        }
    }
}

impl Drawing {
    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Adds a layer unless one of that name exists.
    pub fn ensure_layer(&mut self, name: &str, color: u16) {
        if self.layer(name).is_none() {
            self.layers.push(Layer::new(name, color));
        }
    }

    /// Adds an entity, creating its layer (colour 7) when missing.
    pub fn push(&mut self, entity: Entity) {
        self.ensure_layer(&entity.layer.clone(), 7);
        self.entities.push(entity);
    }

    /// The same drawing with every length multiplied by `mm_per_unit`, so
    /// the result is in millimetres (`mm_per_unit` 1.0).
    pub fn scaled_to_mm(&self) -> Drawing {
        let k = self.mm_per_unit;
        let p = |p: Point| Point::new(p.x * k, p.y * k);
        let entities = self
            .entities
            .iter()
            .map(|e| {
                let kind = match &e.kind {
                    EntityKind::Line { a, b } => EntityKind::Line { a: p(*a), b: p(*b) },
                    EntityKind::Polyline { points, closed } => EntityKind::Polyline {
                        points: points.iter().map(|q| p(*q)).collect(),
                        closed: *closed,
                    },
                    EntityKind::Circle { center, radius } => EntityKind::Circle {
                        center: p(*center),
                        radius: radius * k,
                    },
                    EntityKind::Arc {
                        center,
                        radius,
                        start_deg,
                        end_deg,
                    } => EntityKind::Arc {
                        center: p(*center),
                        radius: radius * k,
                        start_deg: *start_deg,
                        end_deg: *end_deg,
                    },
                    EntityKind::Text {
                        pos,
                        height,
                        rotation_deg,
                        text,
                        align,
                    } => EntityKind::Text {
                        pos: p(*pos),
                        height: height * k,
                        rotation_deg: *rotation_deg,
                        text: text.clone(),
                        align: *align,
                    },
                };
                Entity {
                    layer: e.layer.clone(),
                    color: e.color,
                    kind,
                }
            })
            .collect();
        Drawing {
            mm_per_unit: 1.0,
            layers: self.layers.clone(),
            entities,
        }
    }

    /// Smallest box around all entities (circles and arcs by their full
    /// circle), or `None` for an empty drawing. Text counts by position.
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut min = Point::new(f64::MAX, f64::MAX);
        let mut max = Point::new(f64::MIN, f64::MIN);
        let mut any = false;
        let mut add = |x: f64, y: f64| {
            any = true;
            min.x = min.x.min(x);
            min.y = min.y.min(y);
            max.x = max.x.max(x);
            max.y = max.y.max(y);
        };
        for e in &self.entities {
            match &e.kind {
                EntityKind::Line { a, b } => {
                    add(a.x, a.y);
                    add(b.x, b.y);
                }
                EntityKind::Polyline { points, .. } => {
                    points.iter().for_each(|q| add(q.x, q.y));
                }
                EntityKind::Circle { center, radius } | EntityKind::Arc { center, radius, .. } => {
                    add(center.x - radius, center.y - radius);
                    add(center.x + radius, center.y + radius);
                }
                EntityKind::Text { pos, .. } => add(pos.x, pos.y),
            }
        }
        any.then_some((min, max))
    }
}
