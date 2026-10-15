//! STEP export of the cabinet (ISO 10303-21, application protocol 214).
//!
//! The 3D scene of an enclosure (shell, mounting plate, rails, ducts,
//! devices, door) becomes one STEP assembly: a product per item, named after
//! it (`-K1`, `Mounting plate`), positioned with its pose and coloured like
//! the 3D view. A mechanical colleague opens it in any CAD program and gets
//! the cabinet as a tree of named solids, not a pile of faces.
//!
//! Every item is written as exact boundary-representation solids (planar
//! faces, straight edges), never as a triangle mesh. Boxes are one solid;
//! rails and ducts are the few boxes their section is made of. What the
//! format cannot carry here is counted in the [`Report`]: wires, maker shapes
//! (a part's own geometry is written as its bounding box) and an open door
//! (written closed).
//!
//! World axes are the 3D view's: x to the right, y up, z out of the mounting
//! plate towards the door, millimetres.
//!
//! The file is deterministic: the same scene and options give the same bytes.

use std::collections::HashMap;
use std::fmt::Write as _;

use glam::{Quat, Vec3};
use novo_core::{Project, StructureId};
use novo_view3d::build::{self, Options};
use novo_view3d::mesh::profile_boxes;
use novo_view3d::{Item, ItemKind, Scene, Shape};

use crate::report::Report;

/// What the file says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOptions {
    /// The assembly's name, for example the enclosure tag `+CAB1`.
    pub name: String,
    /// `YYYY-MM-DDTHH:MM:SS`, written in the header. Callers pass the clock;
    /// the engine never reads it, so output stays reproducible.
    pub created: String,
    pub author: String,
}

impl Default for StepOptions {
    fn default() -> Self {
        StepOptions {
            name: "Cabinet".into(),
            created: "1970-01-01T00:00:00".into(),
            author: String::new(),
        }
    }
}

/// The finished file and what it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct StepExport {
    pub text: String,
    pub report: Report,
}

/// The STEP file of one enclosure of the project. `None` while the
/// enclosure has no mounting plate (nothing to model yet).
pub fn export_enclosure(
    project: &Project,
    id: StructureId,
    options: &Options,
    step: &StepOptions,
) -> Option<StepExport> {
    let scene = build::model::scene(project, id, options)?;
    Some(export_scene(&scene, step))
}

/// Property of an enclosure (`Structure`) holding its depth in millimetres.
pub const DEPTH_KEY: &str = "depth_mm";

/// One enclosure's file.
#[derive(Debug, Clone, PartialEq)]
pub struct EnclosureStep {
    pub structure: StructureId,
    /// The enclosure's tag, for example `+CAB1`.
    pub tag: String,
    pub export: StepExport,
}

/// A STEP file for every enclosure that has a mounting plate, in tag order.
/// The depth comes from the enclosure's `depth_mm` property and is
/// 250 mm when it has none (the 3D view's default).
pub fn export_all(project: &Project, base: &StepOptions) -> Vec<EnclosureStep> {
    let mut out = Vec::new();
    for s in project
        .structures()
        .filter(|s| s.enclosure && s.plate.is_some())
    {
        let mut options = Options::default();
        let depth = match s.properties.get(DEPTH_KEY) {
            Some(novo_core::Value::Float(v)) => Some(*v),
            Some(novo_core::Value::Int(v)) => Some(*v as f64),
            _ => None,
        };
        if let Some(d) = depth.filter(|d| *d > 0.0) {
            options.shell.depth = d;
        }
        let tag = s.tag.to_string();
        let step = StepOptions {
            name: tag.clone(),
            ..base.clone()
        };
        if let Some(export) = export_enclosure(project, s.id, &options, &step) {
            out.push(EnclosureStep {
                structure: s.id,
                tag,
                export,
            });
        }
    }
    out.sort_by(|a, b| a.tag.cmp(&b.tag));
    out
}

/// The STEP file of a scene.
pub fn export_scene(scene: &Scene, options: &StepOptions) -> StepExport {
    let mut report = Report::default();
    let mut w = Writer::new();
    let ctx = w.context();

    let mut products: Vec<Product> = Vec::new();
    let mut part_shapes = 0usize;
    let mut skipped = 0usize;
    for item in &scene.items {
        let boxes = item_boxes(item);
        if boxes.is_empty() {
            skipped += 1;
            continue;
        }
        if matches!(item.shape, Shape::Part(_)) {
            part_shapes += 1;
        }
        products.push(w.item_product(item, &boxes, &ctx));
    }

    // The root assembly: an origin and one placement per child.
    let root_origin = w.axis(Vec3::ZERO, Vec3::Z, Vec3::X);
    let mut root_items = vec![root_origin];
    let root = w.product(
        "assembly",
        &options.name,
        &mut |w, _| {
            let rep = format!(
                "SHAPE_REPRESENTATION('{}',(#ROOTITEMS),#{})",
                esc(&options.name),
                ctx.geometry
            );
            w.raw_rep(&rep)
        },
        &ctx,
    );
    let mut usages = Vec::new();
    for (n, p) in products.iter().enumerate() {
        let pose = w.axis(p.pose.0, p.pose.1, p.pose.2);
        root_items.push(pose);
        usages.push(w.usage(&root, p, pose, n + 1));
    }
    let _ = usages;
    // The root representation's item list is known only now.
    let list = root_items
        .iter()
        .map(|i| format!("#{i}"))
        .collect::<Vec<_>>()
        .join(",");
    w.patch(root.rep, "#ROOTITEMS", &list);

    w.styles(&products, &ctx);

    report.moved("solids", products.iter().map(|p| p.solids).sum());
    report.moved("items", products.len());
    report.lost(
        "wires",
        scene.wires.len(),
        "wires and cables are not modelled as solids yet",
    );
    report.lost(
        "maker shapes",
        part_shapes,
        "a part's own geometry is written as its bounding box",
    );
    report.lost("empty items", skipped, "they have no thickness");
    if scene.groups.iter().any(|g| g.angle.abs() > 1e-6) {
        report.note("The door is written closed.");
    }
    if scene.items.iter().any(|i| i.id.kind == ItemKind::Enclosure) {
        report.note(format!(
            "The enclosure is drawn as a housing with {HOUSING_WALL_MM} mm walls and an open front; its real wall thickness is not in the model."
        ));
    }
    report.note("Millimetres; z points out of the mounting plate towards the door.");

    StepExport {
        text: w.finish(options),
        report,
    }
}

/// A local box as (min, max); the boxes an item is made of.
fn item_boxes(item: &Item) -> Vec<(Vec3, Vec3)> {
    let s = item.size;
    if s.x <= 0.0 || s.y <= 0.0 || s.z <= 0.0 {
        return Vec::new();
    }
    if item.id.kind == ItemKind::Enclosure && item.shape == Shape::Box {
        return housing(s);
    }
    match item.shape {
        Shape::Profile(profile) => profile_boxes(profile, s.y, s.z)
            .into_iter()
            .map(|(a, b)| {
                (
                    Vec3::new(a.x * s.x, a.y, a.z),
                    Vec3::new(b.x * s.x, b.y, b.z),
                )
            })
            .collect(),
        _ => vec![(
            Vec3::new(-s.x / 2.0, -s.y / 2.0, 0.0),
            Vec3::new(s.x / 2.0, s.y / 2.0, s.z),
        )],
    }
}

/// Thickness of the housing walls. The model has no sheet thickness yet.
pub const HOUSING_WALL_MM: f32 = 2.0;

/// The enclosure as a sheet-metal housing instead of a solid block: the
/// shell item is the interior (as wide and high as the plate, from the plate
/// to the door), so the walls go around it and a back wall behind the plate.
/// The front stays open for the door.
fn housing(s: Vec3) -> Vec<(Vec3, Vec3)> {
    let t = HOUSING_WALL_MM;
    let (hx, hy) = (s.x / 2.0, s.y / 2.0);
    let back = -PLATE_BEHIND_MM - t;
    vec![
        // Back wall, behind the mounting plate.
        (
            Vec3::new(-hx - t, -hy - t, back),
            Vec3::new(hx + t, hy + t, back + t),
        ),
        // Left and right walls, full height.
        (
            Vec3::new(-hx - t, -hy - t, back + t),
            Vec3::new(-hx, hy + t, s.z),
        ),
        (
            Vec3::new(hx, -hy - t, back + t),
            Vec3::new(hx + t, hy + t, s.z),
        ),
        // Bottom and top walls between them.
        (Vec3::new(-hx, -hy - t, back + t), Vec3::new(hx, -hy, s.z)),
        (Vec3::new(-hx, hy, back + t), Vec3::new(hx, hy + t, s.z)),
    ]
}

/// How far the mounting plate reaches behind the plate surface.
const PLATE_BEHIND_MM: f32 = build::PLATE_THICKNESS_MM as f32;

/// STEP real: always with a point, never an exponent, no float noise.
fn real(v: f32) -> String {
    real64(f64::from(v))
}

fn real64(v: f64) -> String {
    let r = (v * 1e6).round() / 1e6;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r}");
    if s.contains('.') { s } else { format!("{s}.") }
}

/// A STEP string body: quotes doubled, non-ASCII as `\X2\....\X0\`.
fn esc(text: &str) -> String {
    let mut out = String::new();
    let mut wide = String::new();
    let flush = |out: &mut String, wide: &mut String| {
        if !wide.is_empty() {
            out.push_str("\\X2\\");
            out.push_str(wide);
            out.push_str("\\X0\\");
            wide.clear();
        }
    };
    for c in text.chars() {
        if c.is_ascii() && !c.is_control() {
            flush(&mut out, &mut wide);
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                c => out.push(c),
            }
        } else if (c as u32) <= 0xFFFF {
            let _ = write!(wide, "{:04X}", c as u32);
        } else {
            // Outside the BMP: STEP's X4 form.
            flush(&mut out, &mut wide);
            let _ = write!(out, "\\X4\\{:08X}\\X0\\", c as u32);
        }
    }
    flush(&mut out, &mut wide);
    out
}

/// Snaps float noise from a rotation to exact 0 and ±1.
fn clean(v: Vec3) -> Vec3 {
    let f = |x: f32| {
        if x.abs() < 1e-5 {
            0.0
        } else if (x.abs() - 1.0).abs() < 1e-5 {
            x.signum()
        } else {
            x
        }
    };
    Vec3::new(f(v.x), f(v.y), f(v.z)).normalize_or(Vec3::Z)
}

fn default_color(item: &Item) -> ([u8; 3], bool) {
    if let Some(t) = item.tint {
        return (t, item.translucent);
    }
    match item.id.kind {
        ItemKind::Enclosure => ([200, 205, 210], true),
        ItemKind::Plate => ([232, 156, 40], false),
        ItemKind::Rail => ([190, 192, 196], false),
        ItemKind::Duct => ([150, 156, 164], false),
        ItemKind::Door => ([170, 190, 205], true),
        ItemKind::Device => ([86, 108, 134], item.translucent),
        _ => ([140, 140, 140], item.translucent),
    }
}

fn item_name(item: &Item) -> String {
    if let Some(l) = &item.label {
        return l.to_string();
    }
    let kind = match item.id.kind {
        ItemKind::Enclosure => "Enclosure",
        ItemKind::Plate => "Mounting plate",
        ItemKind::Rail => "DIN rail",
        ItemKind::Duct => "Wiring duct",
        ItemKind::Device => "Device",
        ItemKind::Door => "Door",
        ItemKind::Wire => "Wire",
        _ => "Item",
    };
    match item.id.kind {
        ItemKind::Enclosure | ItemKind::Plate | ItemKind::Door => kind.to_string(),
        _ => format!("{kind} {}", item.id.raw),
    }
}

/// Handles to what the shared preamble defined.
struct Context {
    app: u32,
    pctx: u32,
    pdctx: u32,
    geometry: u32,
}

/// One written item: where it sits, how it is styled.
struct Product {
    definition: u32,
    rep: u32,
    /// Location, axis and reference direction of the pose.
    pose: (Vec3, Vec3, Vec3),
    solid_ids: Vec<u32>,
    color: [u8; 3],
    transparent: bool,
    solids: usize,
    child_origin: u32,
}

struct Root {
    definition: u32,
    rep: u32,
}

struct Writer {
    lines: Vec<String>,
    patches: Vec<(u32, String, String)>,
}

impl Writer {
    fn new() -> Self {
        Writer {
            lines: Vec::new(),
            patches: Vec::new(),
        }
    }

    /// Adds `#n=body;` and returns n.
    fn add(&mut self, body: impl Into<String>) -> u32 {
        self.lines.push(body.into());
        self.lines.len() as u32
    }

    fn raw_rep(&mut self, body: &str) -> u32 {
        self.add(body)
    }

    fn patch(&mut self, id: u32, from: &str, to: &str) {
        self.patches.push((id, from.into(), to.into()));
    }

    fn point(&mut self, p: Vec3) -> u32 {
        self.add(format!(
            "CARTESIAN_POINT('',({},{},{}))",
            real(p.x),
            real(p.y),
            real(p.z)
        ))
    }

    fn direction(&mut self, d: Vec3) -> u32 {
        self.add(format!(
            "DIRECTION('',({},{},{}))",
            real64(f64::from(d.x)),
            real64(f64::from(d.y)),
            real64(f64::from(d.z))
        ))
    }

    fn axis(&mut self, location: Vec3, axis: Vec3, reference: Vec3) -> u32 {
        let p = self.point(location);
        let a = self.direction(axis);
        let r = self.direction(reference);
        self.add(format!("AXIS2_PLACEMENT_3D('',#{p},#{a},#{r})"))
    }

    /// Application, product contexts, units and the geometric context.
    fn context(&mut self) -> Context {
        let app = self.add("APPLICATION_CONTEXT('automotive design')");
        self.add(format!(
            "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})"
        ));
        let pctx = self.add(format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
        let pdctx = self.add(format!(
            "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
        ));
        let len = self.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))");
        let ang = self.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))");
        let sol = self.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())");
        let unc = self.add(format!(
            "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-05),#{len},'distance_accuracy_value','confusion accuracy')"
        ));
        let geometry = self.add(format!(
            "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{unc}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{len},#{ang},#{sol}))REPRESENTATION_CONTEXT('Context','3D'))"
        ));
        Context {
            app,
            pctx,
            pdctx,
            geometry,
        }
    }

    /// PRODUCT through PRODUCT_DEFINITION_SHAPE and the link to its
    /// representation (made by `make_rep`).
    fn product(
        &mut self,
        id: &str,
        name: &str,
        make_rep: &mut dyn FnMut(&mut Writer, u32) -> u32,
        ctx: &Context,
    ) -> Root {
        let product = self.add(format!(
            "PRODUCT('{}','{}','',(#{}))",
            esc(id),
            esc(name),
            ctx.pctx
        ));
        let formation = self.add(format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
        let definition = self.add(format!(
            "PRODUCT_DEFINITION('design','',#{formation},#{})",
            ctx.pdctx
        ));
        let shape = self.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
        let rep = make_rep(self, definition);
        self.add(format!("SHAPE_DEFINITION_REPRESENTATION(#{shape},#{rep})"));
        self.add(format!(
            "PRODUCT_RELATED_PRODUCT_CATEGORY('part','',(#{product}))"
        ));
        Root { definition, rep }
    }

    fn item_product(&mut self, item: &Item, boxes: &[(Vec3, Vec3)], ctx: &Context) -> Product {
        let name = item_name(item);
        let id = format!("{:?}-{}", item.id.kind, item.id.raw);
        let origin = self.axis(Vec3::ZERO, Vec3::Z, Vec3::X);
        let solid_ids: Vec<u32> = boxes
            .iter()
            .map(|&(min, max)| self.box_solid(&name, min, max))
            .collect();
        let mut reps = vec![origin];
        reps.extend(&solid_ids);
        let list = reps
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let geometry = ctx.geometry;
        let root = self.product(
            &id,
            &name,
            &mut |w, _| {
                w.add(format!(
                    "ADVANCED_BREP_SHAPE_REPRESENTATION('{}',({list}),#{geometry})",
                    esc(&name)
                ))
            },
            ctx,
        );
        let q: Quat = item.pose.rotation;
        let (color, transparent) = default_color(item);
        Product {
            definition: root.definition,
            rep: root.rep,
            pose: (
                item.pose.translation,
                clean(q * Vec3::Z),
                clean(q * Vec3::X),
            ),
            solids: solid_ids.len(),
            solid_ids,
            color,
            transparent,
            child_origin: origin,
        }
    }

    /// One axis-aligned box in the item's local frame.
    fn box_solid(&mut self, name: &str, min: Vec3, max: Vec3) -> u32 {
        let corner = |i: usize| {
            Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            )
        };
        let vertices: Vec<u32> = (0..8)
            .map(|i| {
                let p = self.point(corner(i));
                self.add(format!("VERTEX_POINT('',#{p})"))
            })
            .collect();
        let mut edges: HashMap<(usize, usize), u32> = HashMap::new();
        let mut faces = Vec::new();
        // (axis, positive side): the face is the plane at that side.
        for (axis, positive) in [
            (0, false),
            (0, true),
            (1, false),
            (1, true),
            (2, false),
            (2, true),
        ] {
            let bit = 1usize << axis;
            let on: Vec<usize> = (0..8).filter(|i| (i & bit != 0) == positive).collect();
            // Corner order around the face, then flipped if it would run
            // clockwise seen from outside.
            let mut loop_ = [on[0], on[1], on[3], on[2]];
            let mut normal = Vec3::ZERO;
            normal[axis] = if positive { 1.0 } else { -1.0 };
            let (a, b, c) = (corner(loop_[0]), corner(loop_[1]), corner(loop_[2]));
            if (b - a).cross(c - b).dot(normal) < 0.0 {
                loop_.reverse();
            }
            let mut oriented = Vec::new();
            for k in 0..4 {
                let (from, to) = (loop_[k], loop_[(k + 1) % 4]);
                let key = (from.min(to), from.max(to));
                let edge = *edges.entry(key).or_insert_with(|| {
                    let (p, q) = (corner(key.0), corner(key.1));
                    let dir = (q - p).normalize();
                    let pt = self.point(p);
                    let d = self.direction(dir);
                    let v = self.add(format!("VECTOR('',#{d},{})", real((q - p).length())));
                    let line = self.add(format!("LINE('',#{pt},#{v})"));
                    self.add(format!(
                        "EDGE_CURVE('',#{},#{},#{line},.T.)",
                        vertices[key.0], vertices[key.1]
                    ))
                });
                let forward = from < to;
                oriented.push(self.add(format!(
                    "ORIENTED_EDGE('',*,*,#{edge},{})",
                    if forward { ".T." } else { ".F." }
                )));
            }
            let list = oriented
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let lp = self.add(format!("EDGE_LOOP('',({list}))"));
            let bound = self.add(format!("FACE_OUTER_BOUND('',#{lp},.T.)"));
            let reference = if axis == 0 { Vec3::Y } else { Vec3::X };
            let placement = self.axis(corner(on[0]), normal, reference);
            let plane = self.add(format!("PLANE('',#{placement})"));
            faces.push(self.add(format!("ADVANCED_FACE('',(#{bound}),#{plane},.T.)")));
        }
        let list = faces
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let shell = self.add(format!("CLOSED_SHELL('',({list}))"));
        self.add(format!("MANIFOLD_SOLID_BREP('{}',#{shell})", esc(name)))
    }

    /// Places `child` in `root` at the pose axis.
    fn usage(&mut self, root: &Root, child: &Product, pose: u32, n: usize) -> u32 {
        let nauo = self.add(format!(
            "NEXT_ASSEMBLY_USAGE_OCCURRENCE('{n}','','',#{},#{},$)",
            root.definition, child.definition
        ));
        let shape = self.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{nauo})"));
        let idt = self.add(format!(
            "ITEM_DEFINED_TRANSFORMATION('','',#{},#{pose})",
            child.child_origin
        ));
        let rel = self.add(format!(
            "(REPRESENTATION_RELATIONSHIP('','',#{},#{})REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{idt})SHAPE_REPRESENTATION_RELATIONSHIP())",
            child.rep, root.rep
        ));
        self.add(format!(
            "CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{rel},#{shape})"
        ))
    }

    /// Colours (and see-through for shell and door) on every solid.
    fn styles(&mut self, products: &[Product], ctx: &Context) {
        let mut cache: HashMap<([u8; 3], bool), u32> = HashMap::new();
        let mut styled = Vec::new();
        for p in products {
            let style = *cache.entry((p.color, p.transparent)).or_insert_with(|| {
                let [r, g, b] = p.color.map(|c| format!("{:.4}", f64::from(c) / 255.0));
                let c = self.add(format!("COLOUR_RGB('',{r},{g},{b})"));
                let fill = self.add(format!("FILL_AREA_STYLE_COLOUR('',#{c})"));
                let area = self.add(format!("FILL_AREA_STYLE('',(#{fill}))"));
                let usage = self.add(format!("SURFACE_STYLE_FILL_AREA(#{area})"));
                let mut styles = format!("#{usage}");
                if p.transparent {
                    let t = self.add("SURFACE_STYLE_TRANSPARENT(0.65)");
                    let _ = write!(styles, ",#{t}");
                }
                let side = self.add(format!("SURFACE_SIDE_STYLE('',({styles}))"));
                let su = self.add(format!("SURFACE_STYLE_USAGE(.BOTH.,#{side})"));
                self.add(format!("PRESENTATION_STYLE_ASSIGNMENT((#{su}))"))
            });
            for s in &p.solid_ids {
                styled.push(self.add(format!("STYLED_ITEM('color',(#{style}),#{s})")));
            }
        }
        if !styled.is_empty() {
            let list = styled
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(",");
            self.add(format!(
                "MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',({list}),#{})",
                ctx.geometry
            ));
        }
        let _ = ctx.app;
    }

    fn finish(mut self, options: &StepOptions) -> String {
        for (id, from, to) in std::mem::take(&mut self.patches) {
            let line = &mut self.lines[id as usize - 1];
            *line = line.replace(&from, &to);
        }
        let mut out = String::new();
        out.push_str("ISO-10303-21;\nHEADER;\n");
        let _ = writeln!(out, "FILE_DESCRIPTION(('KoperCAD cabinet export'),'2;1');");
        let _ = writeln!(
            out,
            "FILE_NAME('{}.stp','{}',('{}'),('KoperVonk'),'KoperCAD','KoperCAD','');",
            esc(&options.name),
            esc(&options.created),
            esc(&options.author)
        );
        out.push_str(
            "FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n",
        );
        for (i, line) in self.lines.iter().enumerate() {
            let _ = writeln!(out, "#{}={};", i + 1, line);
        }
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        out
    }
}

/// `YYYY-MM-DDTHH:MM:SS` of a Unix time in UTC, for [`StepOptions::created`].
pub fn iso_utc(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reals_always_have_a_point_and_no_noise() {
        assert_eq!(real(7.5), "7.5");
        assert_eq!(real(3.0), "3.");
        assert_eq!(real(0.1), "0.1");
        assert_eq!(real(-0.0), "0.");
        assert_eq!(real64(1e-9), "0.");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(esc("O'Neil"), "O''Neil");
        assert_eq!(esc("Kast Ø1"), "Kast \\X2\\00D8\\X0\\1");
        assert_eq!(esc("a\\b"), "a\\\\b");
    }

    #[test]
    fn dates_are_utc() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00");
        assert_eq!(iso_utc(1_792_000_000), "2026-10-14T17:46:40");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00");
    }
}
