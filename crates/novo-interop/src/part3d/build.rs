//! STEP file to a triangle soup in millimetres: finds the solids (following
//! mapped items of assemblies), tessellates each distinct solid once and
//! places it for every use.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::brep::Reader;
use super::math::*;
use super::parse::{Entity, Param, ParseError, StepFile};
use super::tess::{Outcome, Soup, tessellate};

/// What went into a soup, for the cache and for tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StepStats {
    /// Distinct solids or shells tessellated.
    pub solids: u32,
    /// Placements of those (assemblies reuse a solid many times).
    pub placements: u32,
    pub faces: u32,
    /// Faces triangulated flat from their boundary (B-spline, swept).
    pub approximated_faces: u32,
    pub skipped_faces: u32,
    /// Triangles before simplification.
    pub raw_triangles: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    #[error("not a STEP file")]
    NotStep,
    #[error("the STEP file has no solid or surface geometry KoperCAD can read")]
    NoGeometry,
    #[error("the STEP file is too large or too detailed for a quick 3D preview")]
    TooComplex,
    #[error("cancelled")]
    Cancelled,
    #[error("the STEP file took too long to read")]
    TimedOut,
}

impl From<ParseError> for BuildError {
    fn from(_: ParseError) -> Self {
        BuildError::NotStep
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StepOptions {
    /// Chord tolerance for curves and curved faces, in millimetres.
    pub chord_tol_mm: f64,
    /// Faces to tessellate at most (counted over distinct solids).
    pub max_faces: usize,
    /// Placements of solids at most.
    pub max_placements: usize,
    /// Give up (with [`BuildError::TimedOut`]) at this moment.
    pub deadline: Option<Instant>,
}

impl StepOptions {
    fn interrupted(&self, cancel: &AtomicBool) -> Option<BuildError> {
        if cancel.load(Ordering::Relaxed) {
            Some(BuildError::Cancelled)
        } else if self.deadline.is_some_and(|d| Instant::now() >= d) {
            Some(BuildError::TimedOut)
        } else {
            None
        }
    }
}

impl Default for StepOptions {
    fn default() -> Self {
        StepOptions {
            chord_tol_mm: 0.05,
            max_faces: 150_000,
            max_placements: 5_000,
            deadline: None,
        }
    }
}

const SOLIDS: [&str; 6] = [
    "MANIFOLD_SOLID_BREP",
    "BREP_WITH_VOIDS",
    "FACETED_BREP",
    "SHELL_BASED_SURFACE_MODEL",
    "TESSELLATED_SOLID",
    "TESSELLATED_SHELL",
];

const REPS: [&str; 6] = [
    "SHAPE_REPRESENTATION",
    "ADVANCED_BREP_SHAPE_REPRESENTATION",
    "FACETED_BREP_SHAPE_REPRESENTATION",
    "MANIFOLD_SURFACE_SHAPE_REPRESENTATION",
    "TESSELLATED_SHAPE_REPRESENTATION",
    "GEOMETRICALLY_BOUNDED_SURFACE_SHAPE_REPRESENTATION",
];

fn is_solid(e: &Entity) -> bool {
    SOLIDS.contains(&e.name())
}

/// Builds the soup of a STEP file's text.
pub fn step_soup(
    text: &str,
    opts: &StepOptions,
    cancel: &AtomicBool,
) -> Result<(Soup, StepStats), BuildError> {
    let file = StepFile::parse(text)?;
    let mut reader = Reader::new(&file, opts.chord_tol_mm);
    let placed = placed_solids(&file, &reader, opts);
    if placed.is_empty() {
        return Err(BuildError::NoGeometry);
    }
    let mut stats = StepStats::default();
    let mut soups: HashMap<u32, Soup> = HashMap::new();
    let mut total = Soup::default();
    for (solid, xf) in &placed {
        if let Some(e) = opts.interrupted(cancel) {
            return Err(e);
        }
        if !soups.contains_key(solid) {
            let s = solid_soup(&file, &mut reader, *solid, opts, cancel, &mut stats)?;
            soups.insert(*solid, s);
            stats.solids += 1;
        }
        total.append(&soups[solid], xf);
        stats.placements += 1;
        if total.tris.len() > 4_000_000 {
            return Err(BuildError::TooComplex);
        }
    }
    stats.raw_triangles = total.tris.len() as u32;
    if total.tris.is_empty() {
        return Err(BuildError::NoGeometry);
    }
    Ok((total, stats))
}

/// Solids with the transform that places each use, in millimetres.
fn placed_solids(file: &StepFile, reader: &Reader, opts: &StepOptions) -> Vec<(u32, Xf)> {
    let all_solids = || -> Vec<u32> {
        let mut v: Vec<u32> = SOLIDS
            .iter()
            .flat_map(|n| file.ids_of(n).iter().copied())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    // Representations used as the body of a representation map are only
    // reached through a mapped item.
    let mut mapped: HashSet<u32> = HashSet::new();
    for &m in file.ids_of("REPRESENTATION_MAP") {
        if let Some(r) = file
            .entity(m)
            .and_then(|e| e.args().get(1).and_then(Param::as_ref))
        {
            mapped.insert(r);
        }
    }
    let rels = relations(file, reader);
    let children: HashSet<u32> = rels.values().flatten().map(|c| c.0).collect();
    let mut reps: Vec<u32> = REPS
        .iter()
        .flat_map(|n| file.ids_of(n).iter().copied())
        .filter(|r| !mapped.contains(r) && !children.contains(r))
        .collect();
    reps.sort_unstable();
    reps.dedup();
    let mut out = Vec::new();
    for r in reps {
        walk(
            file,
            reader,
            &rels,
            r,
            Xf::IDENTITY,
            0,
            &mut out,
            opts.max_placements,
        );
    }
    if out.is_empty() {
        out = all_solids()
            .into_iter()
            .map(|s| (s, Xf::IDENTITY))
            .collect();
    }
    out
}

/// Parent representation to its components: `(component rep, transform)`.
type Relations = HashMap<u32, Vec<(u32, Xf)>>;

/// Component placements of assemblies written as representation
/// relationships with an item-defined transformation (AP203/AP214).
///
/// `ITEM_DEFINED_TRANSFORMATION(item_1, item_2)` places the component's
/// axis (item 1) at an axis of the assembly (item 2). Files disagree on
/// which of the two representations is listed first, so the assembly is
/// the one that holds item 2 (item 1 is often one axis shared by all).
fn relations(file: &StepFile, reader: &Reader) -> Relations {
    let mut out: Relations = HashMap::new();
    for &id in file.ids_of("REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION") {
        let Some(e) = file.entity(id) else { continue };
        let Some(rr) = e.part("REPRESENTATION_RELATIONSHIP") else {
            continue;
        };
        let (Some(r1), Some(r2)) = (
            rr.get(2).and_then(Param::as_ref),
            rr.get(3).and_then(Param::as_ref),
        ) else {
            continue;
        };
        let Some(tr) = e
            .part("REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION")
            .and_then(|a| a.first())
            .and_then(Param::as_ref)
            .and_then(|t| file.entity(t))
        else {
            continue;
        };
        let a = tr.args();
        let (Some(i1), Some(i2)) = (
            a.get(2).and_then(Param::as_ref),
            a.get(3).and_then(Param::as_ref),
        ) else {
            continue;
        };
        let (Some(f1), Some(f2)) = (reader.frame(i1), reader.frame(i2)) else {
            continue;
        };
        let holds = |rep: u32, item: u32| {
            file.entity(rep)
                .and_then(|e| e.args().get(1).map(Param::refs))
                .is_some_and(|items| items.contains(&item))
        };
        let (parent, child) = if holds(r1, i2) && !holds(r2, i2) {
            (r1, r2)
        } else {
            (r2, r1)
        };
        let xf = Xf::from_frame(&f2).after(&Xf::from_frame(&f1).inverse());
        out.entry(parent).or_default().push((child, xf));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn walk(
    file: &StepFile,
    reader: &Reader,
    rels: &Relations,
    rep: u32,
    xf: Xf,
    depth: u8,
    out: &mut Vec<(u32, Xf)>,
    cap: usize,
) {
    if depth > 8 || out.len() >= cap {
        return;
    }
    let Some(e) = file.entity(rep) else { return };
    for (child, local) in rels.get(&rep).into_iter().flatten() {
        walk(
            file,
            reader,
            rels,
            *child,
            xf.after(local),
            depth + 1,
            out,
            cap,
        );
    }
    let Some(items) = e.args().get(1) else { return };
    for item in items.refs() {
        let Some(ie) = file.entity(item) else {
            continue;
        };
        if is_solid(&ie) {
            out.push((item, xf));
        } else if ie.name() == "MAPPED_ITEM" {
            let a = ie.args();
            let map = a
                .get(1)
                .and_then(Param::as_ref)
                .and_then(|m| file.entity(m));
            let target = a
                .get(2)
                .and_then(Param::as_ref)
                .and_then(|t| reader.frame(t));
            let (Some(map), Some(target)) = (map, target) else {
                continue;
            };
            let ma = map.args();
            let origin = ma
                .first()
                .and_then(Param::as_ref)
                .and_then(|o| reader.frame(o))
                .unwrap_or(Frame::IDENTITY);
            let Some(body) = ma.get(1).and_then(Param::as_ref) else {
                continue;
            };
            let local = Xf::from_frame(&target).after(&Xf::from_frame(&origin).inverse());
            walk(
                file,
                reader,
                rels,
                body,
                xf.after(&local),
                depth + 1,
                out,
                cap,
            );
        }
    }
}

fn solid_soup(
    file: &StepFile,
    reader: &mut Reader,
    solid: u32,
    opts: &StepOptions,
    cancel: &AtomicBool,
    stats: &mut StepStats,
) -> Result<Soup, BuildError> {
    let mut soup = Soup::default();
    let Some(e) = file.entity(solid) else {
        return Ok(soup);
    };
    let shells: Vec<u32> = match e.name() {
        "SHELL_BASED_SURFACE_MODEL" => e.args().get(1).map(Param::refs).unwrap_or_default(),
        _ => e
            .args()
            .get(1)
            .and_then(Param::as_ref)
            .into_iter()
            .collect(),
    };
    let tessellated = matches!(e.name(), "TESSELLATED_SOLID" | "TESSELLATED_SHELL");
    let face_ids: Vec<u32> = if tessellated {
        e.args().get(1).map(Param::refs).unwrap_or_default()
    } else {
        shells
            .iter()
            .filter_map(|&s| file.entity(s))
            .flat_map(|sh| sh.args().get(1).map(Param::refs).unwrap_or_default())
            .collect()
    };
    for f in face_ids {
        if let Some(e) = opts.interrupted(cancel) {
            return Err(e);
        }
        if stats.faces as usize >= opts.max_faces {
            return Err(BuildError::TooComplex);
        }
        stats.faces += 1;
        if tessellated {
            match triangulated_face(file, reader.scale, f) {
                Some(s) => soup.append(&s, &Xf::IDENTITY),
                None => stats.skipped_faces += 1,
            }
            continue;
        }
        let Some(face) = reader.face(f) else {
            stats.skipped_faces += 1;
            continue;
        };
        let (s, outcome) = tessellate(&face, opts.chord_tol_mm);
        match outcome {
            Outcome::Exact => {}
            Outcome::Approx => stats.approximated_faces += 1,
            Outcome::Skipped => stats.skipped_faces += 1,
        }
        soup.append(&s, &Xf::IDENTITY);
    }
    Ok(soup)
}

/// `TRIANGULATED_FACE(name, coordinates, pnmax, normals, link, pnindex, triangles)`
fn triangulated_face(file: &StepFile, scale: f64, id: u32) -> Option<Soup> {
    let e = file.entity(id)?;
    if e.name() != "TRIANGULATED_FACE" {
        return None;
    }
    let a = e.args();
    let cl = file.entity(a.get(1)?.as_ref()?)?;
    let coords = cl.args().get(2)?.as_list()?;
    let pos: Vec<V3> = coords
        .iter()
        .filter_map(|p| {
            let c: Vec<f64> = p.as_list()?.iter().filter_map(Param::as_f64).collect();
            (c.len() == 3).then(|| [c[0] * scale, c[1] * scale, c[2] * scale])
        })
        .collect();
    let pnindex: Vec<i64> = a
        .get(5)
        .and_then(Param::as_list)
        .map(|l| l.iter().filter_map(Param::as_int).collect())
        .unwrap_or_default();
    let map = |k: i64| -> Option<u32> {
        let i = if pnindex.is_empty() {
            k
        } else {
            *pnindex.get(usize::try_from(k - 1).ok()?)?
        };
        let i = usize::try_from(i - 1).ok()?;
        (i < pos.len()).then_some(i as u32)
    };
    let mut tris = Vec::new();
    for t in a.get(6)?.as_list()? {
        let v: Vec<i64> = t.as_list()?.iter().filter_map(Param::as_int).collect();
        if v.len() == 3
            && let (Some(x), Some(y), Some(z)) = (map(v[0]), map(v[1]), map(v[2]))
        {
            tris.push([x, y, z]);
        }
    }
    Some(Soup { pos, tris })
}

#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) fn block_step() -> String {
        super::tests::block_step(super::tests::MM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 10 x 20 x 30 mm block as an AP214-style manifold solid.
    pub(crate) fn block_step(unit: &str) -> String {
        let mut s = String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n");
        s.push_str(unit);
        s.push_str("\n#9=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#90))REPRESENTATION_CONTEXT('',''));\n");
        // Vertices 8..=15 are the box corners; faces use planes.
        let corners = [
            (0, 0, 0),
            (10, 0, 0),
            (10, 20, 0),
            (0, 20, 0),
            (0, 0, 30),
            (10, 0, 30),
            (10, 20, 30),
            (0, 20, 30),
        ];
        let mut n = 100;
        let mut pt = vec![];
        let mut vx = vec![];
        for c in corners {
            s.push_str(&format!(
                "#{n}=CARTESIAN_POINT('',({}.,{}.,{}.));\n",
                c.0, c.1, c.2
            ));
            pt.push(n);
            n += 1;
            s.push_str(&format!("#{n}=VERTEX_POINT('',#{});\n", n - 1));
            vx.push(n);
            n += 1;
        }
        // 12 edges as straight lines.
        let edges = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        let mut ec = vec![];
        for (a, b) in edges {
            s.push_str(&format!("#{n}=DIRECTION('',(1.,0.,0.));\n"));
            s.push_str(&format!("#{}=VECTOR('',#{n},1.);\n", n + 1));
            s.push_str(&format!("#{}=LINE('',#{},#{});\n", n + 2, pt[a], n + 1));
            s.push_str(&format!(
                "#{}=EDGE_CURVE('',#{},#{},#{},.T.);\n",
                n + 3,
                vx[a],
                vx[b],
                n + 2
            ));
            ec.push(n + 3);
            n += 4;
        }
        // Faces as (edge index, forward) loops, counter-clockwise from outside.
        type Loop = [(usize, bool); 4];
        type V = (f64, f64, f64);
        let faces: [(Loop, V, V, V); 6] = [
            (
                [(3, false), (2, false), (1, false), (0, false)],
                (0., 0., 0.),
                (0., 0., -1.),
                (1., 0., 0.),
            ),
            (
                [(4, true), (5, true), (6, true), (7, true)],
                (0., 0., 30.),
                (0., 0., 1.),
                (1., 0., 0.),
            ),
            (
                [(0, true), (9, true), (4, false), (8, false)],
                (0., 0., 0.),
                (0., -1., 0.),
                (1., 0., 0.),
            ),
            (
                [(1, true), (10, true), (5, false), (9, false)],
                (10., 0., 0.),
                (1., 0., 0.),
                (0., 1., 0.),
            ),
            (
                [(2, true), (11, true), (6, false), (10, false)],
                (0., 20., 0.),
                (0., 1., 0.),
                (-1., 0., 0.),
            ),
            (
                [(3, true), (8, true), (7, false), (11, false)],
                (0., 0., 0.),
                (-1., 0., 0.),
                (0., -1., 0.),
            ),
        ];
        let mut face_ids = vec![];
        for (loop_edges, o, z, x) in faces {
            let mut oes = vec![];
            for (e, fwd) in loop_edges {
                s.push_str(&format!(
                    "#{n}=ORIENTED_EDGE('',*,*,#{},{});\n",
                    ec[e],
                    if fwd { ".T." } else { ".F." }
                ));
                oes.push(format!("#{n}"));
                n += 1;
            }
            s.push_str(&format!("#{n}=EDGE_LOOP('',({}));\n", oes.join(",")));
            s.push_str(&format!("#{}=FACE_OUTER_BOUND('',#{n},.T.);\n", n + 1));
            s.push_str(&format!(
                "#{}=CARTESIAN_POINT('',({}.,{}.,{}.));\n",
                n + 2,
                o.0,
                o.1,
                o.2
            ));
            s.push_str(&format!(
                "#{}=DIRECTION('',({}.,{}.,{}.));\n",
                n + 3,
                z.0,
                z.1,
                z.2
            ));
            s.push_str(&format!(
                "#{}=DIRECTION('',({}.,{}.,{}.));\n",
                n + 4,
                x.0,
                x.1,
                x.2
            ));
            s.push_str(&format!(
                "#{}=AXIS2_PLACEMENT_3D('',#{},#{},#{});\n",
                n + 5,
                n + 2,
                n + 3,
                n + 4
            ));
            s.push_str(&format!("#{}=PLANE('',#{});\n", n + 6, n + 5));
            s.push_str(&format!(
                "#{}=ADVANCED_FACE('',(#{}),#{},.T.);\n",
                n + 7,
                n + 1,
                n + 6
            ));
            face_ids.push(n + 7);
            n += 8;
        }
        let fl: Vec<String> = face_ids.iter().map(|f| format!("#{f}")).collect();
        s.push_str(&format!("#{n}=CLOSED_SHELL('',({}));\n", fl.join(",")));
        s.push_str(&format!("#{}=MANIFOLD_SOLID_BREP('',#{n});\n", n + 1));
        s.push_str(&format!(
            "#{}=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{}),#9);\n",
            n + 2,
            n + 1
        ));
        s.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        s
    }

    pub(crate) const MM: &str = "#90=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));";

    fn bounds(s: &Soup) -> ([f64; 3], [f64; 3]) {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for p in &s.pos {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_block_becomes_twelve_outward_triangles() {
        let (s, st) = step_soup(
            &block_step(MM),
            &StepOptions::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(st.faces, 6);
        assert_eq!(st.skipped_faces, 0);
        assert_eq!(s.tris.len(), 12);
        let (lo, hi) = bounds(&s);
        assert_eq!((lo, hi), ([0.0; 3], [10.0, 20.0, 30.0]));
        // Outward normals: sum over triangles of (n . (centroid - centre)) > 0.
        let centre = [5.0, 10.0, 15.0];
        for t in &s.tris {
            let (a, b, c) = (
                s.pos[t[0] as usize],
                s.pos[t[1] as usize],
                s.pos[t[2] as usize],
            );
            let n = cross(sub(b, a), sub(c, a));
            let m = scale(add(add(a, b), c), 1.0 / 3.0);
            assert!(dot(n, sub(m, centre)) > 0.0);
        }
    }

    #[test]
    fn inch_files_are_scaled_to_millimetres() {
        let unit = "#91=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(25.4),#92);\n#92=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#90=(CONVERSION_BASED_UNIT('INCH',#91)LENGTH_UNIT()NAMED_UNIT(*));";
        let (s, _) = step_soup(
            &block_step(unit),
            &StepOptions::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let (_, hi) = bounds(&s);
        assert!((hi[0] - 254.0).abs() < 1e-9);
    }

    #[test]
    fn mapped_items_place_one_solid_many_times() {
        let mut t = block_step(MM);
        // Re-use the solid (id from the base builder) through a map.
        let solid = t
            .lines()
            .find(|l| l.contains("MANIFOLD_SOLID_BREP"))
            .and_then(|l| l.split('=').next())
            .unwrap()
            .trim_start_matches('#')
            .to_string();
        // Drop the plain representation, wrap the solid in a map, place twice.
        t = t
            .lines()
            .filter(|l| !l.contains("ADVANCED_BREP_SHAPE_REPRESENTATION"))
            .collect::<Vec<_>>()
            .join("\n");
        t = t.replace(
            "ENDSEC;\nEND-ISO-10303-21;",
            &format!(
                "#900=CARTESIAN_POINT('',(0.,0.,0.));\n#901=DIRECTION('',(0.,0.,1.));\n#902=DIRECTION('',(1.,0.,0.));\n#903=AXIS2_PLACEMENT_3D('',#900,#901,#902);\n\
#904=ADVANCED_BREP_SHAPE_REPRESENTATION('body',(#{solid}),#9);\n#905=REPRESENTATION_MAP(#903,#904);\n\
#910=CARTESIAN_POINT('',(100.,0.,0.));\n#911=AXIS2_PLACEMENT_3D('',#910,#901,#902);\n\
#912=MAPPED_ITEM('',#905,#903);\n#913=MAPPED_ITEM('',#905,#911);\n\
#914=SHAPE_REPRESENTATION('asm',(#912,#913),#9);\nENDSEC;\nEND-ISO-10303-21;"
            ),
        );
        let (s, st) = step_soup(&t, &StepOptions::default(), &AtomicBool::new(false)).unwrap();
        assert_eq!((st.solids, st.placements), (1, 2));
        assert_eq!(s.tris.len(), 24);
        let (_, hi) = bounds(&s);
        assert!((hi[0] - 110.0).abs() < 1e-9);
    }

    #[test]
    fn assembly_relationships_place_a_component_at_each_axis() {
        let mut t = block_step(MM);
        let solid = t
            .lines()
            .find(|l| l.contains("MANIFOLD_SOLID_BREP"))
            .and_then(|l| l.split('=').next())
            .unwrap()
            .trim_start_matches('#')
            .to_string();
        t = t
            .lines()
            .filter(|l| !l.contains("ADVANCED_BREP_SHAPE_REPRESENTATION"))
            .collect::<Vec<_>>()
            .join("\n");
        // Component (solid + its origin axis #903) and assembly (two axes),
        // as AP214 writes it; item 1 is the shared origin axis, item 2 sits
        // in the assembly, and the component is listed first in both.
        t = t.replace(
            "ENDSEC;\nEND-ISO-10303-21;",
            &format!(
                "#900=CARTESIAN_POINT('',(0.,0.,0.));\n#901=DIRECTION('',(0.,0.,1.));\n#902=DIRECTION('',(1.,0.,0.));\n#903=AXIS2_PLACEMENT_3D('',#900,#901,#902);\n\
#904=ADVANCED_BREP_SHAPE_REPRESENTATION('part',(#903,#{solid}),#9);\n\
#910=CARTESIAN_POINT('',(50.,0.,0.));\n#911=AXIS2_PLACEMENT_3D('',#910,#901,#902);\n\
#912=CARTESIAN_POINT('',(0.,70.,0.));\n#913=AXIS2_PLACEMENT_3D('',#912,#901,#902);\n\
#914=SHAPE_REPRESENTATION('asm',(#903,#911,#913),#9);\n\
#920=ITEM_DEFINED_TRANSFORMATION('','',#903,#911);\n\
#921=(REPRESENTATION_RELATIONSHIP('','',#904,#914)REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#920)SHAPE_REPRESENTATION_RELATIONSHIP());\n\
#922=ITEM_DEFINED_TRANSFORMATION('','',#903,#913);\n\
#923=(REPRESENTATION_RELATIONSHIP('','',#904,#914)REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#922)SHAPE_REPRESENTATION_RELATIONSHIP());\n\
ENDSEC;\nEND-ISO-10303-21;"
            ),
        );
        let (s, st) = step_soup(&t, &StepOptions::default(), &AtomicBool::new(false)).unwrap();
        assert_eq!((st.solids, st.placements), (1, 2));
        let (lo, hi) = bounds(&s);
        // One block at x 50..60, one at y 70..90.
        assert_eq!(lo, [0.0, 0.0, 0.0]);
        assert_eq!(hi, [60.0, 90.0, 30.0]);
    }

    #[test]
    fn files_without_solids_report_no_geometry() {
        let t = "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\nENDSEC;\nEND-ISO-10303-21;";
        assert_eq!(
            step_soup(t, &StepOptions::default(), &AtomicBool::new(false)).unwrap_err(),
            BuildError::NoGeometry
        );
        assert_eq!(
            step_soup("nope", &StepOptions::default(), &AtomicBool::new(false)).unwrap_err(),
            BuildError::NotStep
        );
    }

    #[test]
    fn a_passed_deadline_stops_the_build() {
        let opts = StepOptions {
            deadline: Some(Instant::now()),
            ..StepOptions::default()
        };
        let r = step_soup(&block_step(MM), &opts, &AtomicBool::new(false));
        assert_eq!(r.unwrap_err(), BuildError::TimedOut);
    }

    #[test]
    fn a_cancelled_build_stops() {
        let r = step_soup(
            &block_step(MM),
            &StepOptions::default(),
            &AtomicBool::new(true),
        );
        assert_eq!(r.unwrap_err(), BuildError::Cancelled);
    }
}
