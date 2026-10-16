//! Curved faces the reader tessellates on their true surface: B-spline
//! patches and spindle tori (synthetic faces and a synthetic STEP file;
//! no maker file is used).

use std::sync::Arc;

use novo_interop::part3d::brep::{Face, Surface};
use novo_interop::part3d::math::*;
use novo_interop::part3d::nurbs::SplineSurface;
use novo_interop::part3d::tess::{Outcome, Soup, tessellate};

fn area(s: &Soup) -> f64 {
    s.tris
        .iter()
        .map(|t| {
            len(cross(
                sub(s.pos[t[1] as usize], s.pos[t[0] as usize]),
                sub(s.pos[t[2] as usize], s.pos[t[0] as usize]),
            )) / 2.0
        })
        .sum()
}

/// Bicubic Bezier patch over 30 x 30 mm with a 9 mm bump; the border
/// control points lie in z = 0 on straight lines, so the border is straight.
fn bump() -> SplineSurface {
    let mut ctrl = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            let inner = (1..=2).contains(&i) && (1..=2).contains(&j);
            ctrl.push([
                i as f64 * 10.0,
                j as f64 * 10.0,
                if inner { 9.0 } else { 0.0 },
            ]);
        }
    }
    let k = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    SplineSurface::new((3, 3), (4, 4), ctrl, None, k.clone(), k).unwrap()
}

/// Area of a surface patch from a fine regular grid of (u, v).
fn grid_area(n: &SplineSurface, u: (f64, f64), v: (f64, f64)) -> f64 {
    let m = 120;
    let at = |i: usize, j: usize| {
        n.eval(
            u.0 + (u.1 - u.0) * i as f64 / m as f64,
            v.0 + (v.1 - v.0) * j as f64 / m as f64,
        )
    };
    let mut a = 0.0;
    for i in 0..m {
        for j in 0..m {
            let (p, q, r, s) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            a += len(cross(sub(q, p), sub(r, p))) / 2.0 + len(cross(sub(r, p), sub(s, p))) / 2.0;
        }
    }
    a
}

#[test]
fn a_bspline_patch_is_tessellated_on_its_bump_not_flat() {
    let n = bump();
    // The square border walked counter-clockwise seen from +z.
    let mut border = Vec::new();
    for k in 0..8 {
        border.push(n.eval(k as f64 / 8.0, 0.0));
    }
    for k in 0..8 {
        border.push(n.eval(1.0, k as f64 / 8.0));
    }
    for k in 0..8 {
        border.push(n.eval(1.0 - k as f64 / 8.0, 1.0));
    }
    for k in 0..8 {
        border.push(n.eval(0.0, 1.0 - k as f64 / 8.0));
    }
    let want = grid_area(&n, (0.0, 1.0), (0.0, 1.0));
    let face = Face {
        loops: vec![border],
        surface: Surface::BSpline,
        nurbs: Some(Arc::new(n)),
        same_sense: true,
    };
    let (soup, outcome) = tessellate(&face, 0.05);
    assert_eq!(outcome, Outcome::Exact, "on the true surface, not flat");
    let top = soup.pos.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
    assert!(top > 4.9, "the bump rises to {top}");
    let got = area(&soup);
    assert!(((got - want) / want).abs() < 0.01, "area {got} vs {want}");
    // Every triangle looks up (the bump faces +z).
    let up: f64 = soup
        .tris
        .iter()
        .map(|t| {
            cross(
                sub(soup.pos[t[1] as usize], soup.pos[t[0] as usize]),
                sub(soup.pos[t[2] as usize], soup.pos[t[0] as usize]),
            )[2]
        })
        .sum();
    assert!(up > 0.0);
}

#[test]
fn a_bspline_face_whose_boundary_is_off_the_surface_falls_back_to_flat() {
    let n = bump();
    let mut border: Vec<V3> = (0..8).map(|k| n.eval(k as f64 / 8.0, 0.0)).collect();
    border.extend((0..8).map(|k| n.eval(1.0, k as f64 / 8.0)));
    border.extend((0..8).map(|k| n.eval(1.0 - k as f64 / 8.0, 1.0)));
    border.extend((0..8).map(|k| n.eval(0.0, 1.0 - k as f64 / 8.0)));
    for p in &mut border {
        p[2] += 3.0;
    }
    let face = Face {
        loops: vec![border],
        surface: Surface::BSpline,
        nurbs: Some(Arc::new(n)),
        same_sense: true,
    };
    assert_eq!(tessellate(&face, 0.05).1, Outcome::Approx);
}

/// The torus point at (u, v).
fn torus_at(big: f64, small: f64, u: f64, v: f64) -> V3 {
    let rho = big + small * v.cos();
    [rho * u.cos(), rho * u.sin(), small * v.sin()]
}

#[test]
fn a_spindle_torus_patch_across_its_axis_has_the_area_of_the_surface() {
    let (big, small) = (2.0, 3.0);
    // cos v < -2/3 (v > 2.30) puts the tube on the far side of the axis.
    let (u0, u1, v0, v1) = (0.2, 1.1, 1.9, 4.4);
    let mut border = Vec::new();
    let steps = 40;
    for k in 0..steps {
        border.push(torus_at(
            big,
            small,
            u0 + (u1 - u0) * k as f64 / steps as f64,
            v0,
        ));
    }
    for k in 0..steps {
        border.push(torus_at(
            big,
            small,
            u1,
            v0 + (v1 - v0) * k as f64 / steps as f64,
        ));
    }
    for k in 0..steps {
        border.push(torus_at(
            big,
            small,
            u1 - (u1 - u0) * k as f64 / steps as f64,
            v1,
        ));
    }
    for k in 0..steps {
        border.push(torus_at(
            big,
            small,
            u0,
            v1 - (v1 - v0) * k as f64 / steps as f64,
        ));
    }
    // Area element small * |rho| du dv.
    let m = 4000;
    let mut inner = 0.0;
    for k in 0..m {
        let v = v0 + (v1 - v0) * (k as f64 + 0.5) / m as f64;
        inner += (big + small * v.cos()).abs() * (v1 - v0) / m as f64;
    }
    let want = small * (u1 - u0) * inner;
    let face = Face {
        loops: vec![border],
        surface: Surface::Torus(Frame::IDENTITY, big, small),
        nurbs: None,
        same_sense: true,
    };
    let (soup, outcome) = tessellate(&face, 0.01);
    assert_eq!(outcome, Outcome::Exact);
    let got = area(&soup);
    assert!(((got - want) / want).abs() < 0.2, "area {got} vs {want}");
    // Every vertex lies on the torus: (|p|^2 + R^2 - r^2)^2 = 4 R^2 (x^2 + y^2).
    for p in &soup.pos {
        let r2 = dot(*p, *p);
        let lhs = (r2 + big * big - small * small).powi(2);
        let rhs = 4.0 * big * big * (p[0] * p[0] + p[1] * p[1]);
        assert!(
            (lhs - rhs).abs() < 0.05 * (1.0 + rhs),
            "off the torus: {p:?}"
        );
    }
}

const MM: &str = "#9=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#8))REPRESENTATION_CONTEXT('',''));\n#8=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n";

/// An open shell of one face: the bump patch above, written as a STEP
/// B-spline surface (`complex` writes the two-part form with weights of 1)
/// bounded by four straight edges.
fn patch_step(complex: bool) -> String {
    let mut s = String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n");
    s.push_str(MM);
    let mut rows = Vec::new();
    for i in 0..4 {
        let mut row = Vec::new();
        for j in 0..4 {
            let id = 100 + i * 4 + j;
            let inner = (1..=2).contains(&i) && (1..=2).contains(&j);
            s.push_str(&format!(
                "#{id}=CARTESIAN_POINT('',({}.,{}.,{}.));\n",
                i * 10,
                j * 10,
                if inner { 9 } else { 0 }
            ));
            row.push(format!("#{id}"));
        }
        rows.push(format!("({})", row.join(",")));
    }
    let net = format!("({})", rows.join(","));
    if complex {
        s.push_str(&format!(
            "#200=(BOUNDED_SURFACE()B_SPLINE_SURFACE(3,3,{net},.UNSPECIFIED.,.F.,.F.,.F.)\
B_SPLINE_SURFACE_WITH_KNOTS((4,4),(4,4),(0.,1.),(0.,1.),.UNSPECIFIED.)\
RATIONAL_B_SPLINE_SURFACE(((1.,1.,1.,1.),(1.,1.,1.,1.),(1.,1.,1.,1.),(1.,1.,1.,1.)))\
SURFACE());\n"
        ));
    } else {
        s.push_str(&format!(
            "#200=B_SPLINE_SURFACE_WITH_KNOTS('',3,3,{net},.UNSPECIFIED.,.F.,.F.,.F.,(4,4),(4,4),(0.,1.),(0.,1.),.UNSPECIFIED.);\n"
        ));
    }
    // Corners (0,0) (30,0) (30,30) (0,30): points 100, 112, 115, 103.
    s.push_str(
        "#210=VERTEX_POINT('',#100);\n#211=VERTEX_POINT('',#112);\n#212=VERTEX_POINT('',#115);\n#213=VERTEX_POINT('',#103);\n\
#220=DIRECTION('',(1.,0.,0.));\n#221=DIRECTION('',(0.,1.,0.));\n#222=VECTOR('',#220,1.);\n#223=VECTOR('',#221,1.);\n\
#224=LINE('',#100,#222);\n#225=LINE('',#112,#223);\n#226=LINE('',#103,#222);\n#227=LINE('',#100,#223);\n\
#230=EDGE_CURVE('',#210,#211,#224,.T.);\n#231=EDGE_CURVE('',#211,#212,#225,.T.);\n\
#232=EDGE_CURVE('',#213,#212,#226,.T.);\n#233=EDGE_CURVE('',#210,#213,#227,.T.);\n\
#240=ORIENTED_EDGE('',*,*,#230,.T.);\n#241=ORIENTED_EDGE('',*,*,#231,.T.);\n\
#242=ORIENTED_EDGE('',*,*,#232,.F.);\n#243=ORIENTED_EDGE('',*,*,#233,.F.);\n\
#250=EDGE_LOOP('',(#240,#241,#242,#243));\n#251=FACE_OUTER_BOUND('',#250,.T.);\n\
#252=ADVANCED_FACE('',(#251),#200,.T.);\n#253=OPEN_SHELL('',(#252));\n\
#254=SHELL_BASED_SURFACE_MODEL('',(#253));\n#255=GEOMETRIC_SET('',(#254));\n\
#256=SHAPE_REPRESENTATION('',(#254),#9);\nENDSEC;\nEND-ISO-10303-21;\n",
    );
    s
}

#[test]
fn a_step_bspline_face_keeps_its_bump_in_both_spellings() {
    use std::sync::atomic::AtomicBool;
    for complex in [false, true] {
        let text = patch_step(complex);
        let g = novo_interop::part3d::geometry_from_step(
            text.as_bytes(),
            None,
            novo_interop::part3d::Quality::Normal,
            &AtomicBool::new(false),
            None,
        )
        .unwrap();
        let novo_interop::part3d::MeshOrigin::Step { stats, .. } = g.origin else {
            panic!("a model")
        };
        assert_eq!(stats.approximated_faces, 0, "complex={complex}: not flat");
        // The bump (5.06 mm high) is the smallest extent, so it is the depth.
        let size = g.size_mm();
        assert!(
            (f64::from(size[2]) - 5.0625).abs() < 0.05,
            "complex={complex}: {size:?}"
        );
    }
}

#[test]
fn a_step_bspline_with_an_over_cap_degree_or_infinite_knot_falls_back_to_flat() {
    use std::sync::atomic::AtomicBool;
    let base = patch_step(false);
    let hostile = [
        // Degree 40 in both directions (knot lists no longer fit).
        base.replace("',3,3,(", "',40,40,("),
        // An infinite knot.
        base.replace("(0.,1.),(0.,1.)", "(0.,1.0E999),(0.,1.)"),
    ];
    for text in hostile {
        let g = novo_interop::part3d::geometry_from_step(
            text.as_bytes(),
            None,
            novo_interop::part3d::Quality::Normal,
            &AtomicBool::new(false),
            None,
        )
        .unwrap();
        let novo_interop::part3d::MeshOrigin::Step { stats, .. } = g.origin else {
            panic!("a model")
        };
        assert_eq!(stats.approximated_faces, 1, "flat, not a stall");
    }
}
