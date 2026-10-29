use super::*;
use std::collections::HashSet;

/// The 40 icons of docs/design/visual-redesign.md section 9.2, by group.
const SPEC: &[(Group, &[&str])] = &[
    (Group::Workspace, &["schematic", "cabinet", "report"]),
    (
        Group::Component,
        &[
            "coil",
            "contact-no",
            "contact-nc",
            "contactor",
            "motor",
            "fuse",
            "breaker",
            "overload",
            "pushbutton",
            "estop",
            "lamp",
            "terminal",
            "terminal-strip",
            "transformer",
            "psu",
            "drive",
            "sensor",
            "plc",
            "ground",
            "potential",
        ],
    ),
    (
        Group::Wiring,
        &[
            "cable",
            "cable-section",
            "xref",
            "bridge",
            "wire-number",
            "net",
        ],
    ),
    (Group::Cabinet, &["din-rail", "duct", "plate", "enclosure"]),
    (Group::Production, &["ferrule", "bench"]),
    (Group::Workflow, &["macro", "place"]),
    (Group::Analysis, &["live", "fault", "emc"]),
];

#[test]
fn the_set_is_the_forty_icons_of_the_design() {
    assert_eq!(ALL.len(), 40);
    for (group, names) in SPEC {
        let have: Vec<&str> = Icon::of_group(*group).map(|i| i.name).collect();
        assert_eq!(&have, names, "group {}", group.id());
    }
    let unique: HashSet<_> = ALL.iter().map(|i| i.name).collect();
    assert_eq!(unique.len(), ALL.len(), "icon names are unique");
}

#[test]
fn names_are_lower_case_hyphenated_nouns_with_a_title() {
    for icon in ALL {
        assert!(
            icon.name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '-'),
            "{}",
            icon.name
        );
        assert!(!icon.title.is_empty(), "{} has no title", icon.name);
        assert!(!icon.shapes.is_empty(), "{} draws nothing", icon.name);
        assert_eq!(Icon::by_name(icon.name), Some(icon));
    }
    assert_eq!(Icon::by_name("no-such-icon"), None);
}

#[test]
fn every_icon_stays_inside_the_grid_and_the_live_area_but_for_named_overshoots() {
    // The live area is 2 to 22 (section 9.2). The centre line of the sensor's
    // outer wave bulges to 22.9, as drawn in the source; the stroke still fits
    // in the 24 box, which is the hard limit (nothing may be cut off).
    let overshoots: &[(&str, f32)] = &[("sensor", 0.95)];
    for icon in ALL {
        let b = icon.bounds();
        let allowed = overshoots
            .iter()
            .find(|(n, _)| *n == icon.name)
            .map_or(0.0, |(_, v)| *v);
        let (lo, hi) = (2.0 - 1e-2 - allowed, 22.0 + 1e-2 + allowed);
        for v in [b.min.x, b.min.y, b.max.x, b.max.y] {
            assert!(
                (lo..=hi).contains(&v),
                "{} leaves the live area: {b:?}",
                icon.name
            );
        }
        let half = STROKE / 2.0;
        assert!(
            b.min.x - half >= -1e-2 && b.max.x + half <= GRID + 1e-2,
            "{} is cut at the side: {b:?}",
            icon.name
        );
        assert!(
            b.min.y - half >= -1e-2 && b.max.y + half <= GRID + 1e-2,
            "{} is cut at top or bottom: {b:?}",
            icon.name
        );
    }
}

#[test]
fn only_junction_dots_are_filled() {
    // Section 9.2: outline only, the junction dot is the one exception.
    let mut with_dots = Vec::new();
    for icon in ALL {
        let dots = icon
            .prims(1.0)
            .iter()
            .filter(|p| matches!(p, Prim::Dot { .. }))
            .count();
        if dots > 0 {
            with_dots.push(icon.name);
            for p in icon.prims(1.0) {
                if let Prim::Dot { radius, .. } = p {
                    assert!(radius <= 1.5, "{}: a dot is a dot, not a disc", icon.name);
                }
            }
        }
    }
    assert_eq!(
        with_dots,
        ["terminal-strip", "plc", "bridge", "live"],
        "the icons that carry junction dots"
    );
}

#[test]
fn curves_are_sampled_more_finely_on_screen_than_in_the_grid() {
    let circle = Shape::Circle {
        cx: 12.0,
        cy: 12.0,
        r: 8.5,
        filled: false,
    };
    let count = |scale| match &circle.prims(scale)[0] {
        Prim::Stroke { points, .. } => points.len(),
        Prim::Dot { .. } => 0,
    };
    assert!(count(0.5) >= 24, "even a tiny circle is round");
    assert!(count(2.0) > count(0.5));
    // Every sample lies on the circle.
    if let Prim::Stroke { points, closed, .. } = &circle.prims(2.0)[0] {
        assert!(closed);
        for p in points {
            let d = ((p[0] - 12.0).powi(2) + (p[1] - 12.0).powi(2)).sqrt();
            assert!((d - 8.5).abs() < 1e-3, "{p:?} is {d} from the centre");
        }
    }
}

#[test]
fn the_emergency_stop_dome_is_the_upper_half_of_an_ellipse() {
    // The SVG arc `a7.5 6 0 0 1 15 0` from (4.5, 11.5): dome up (y smaller).
    let prims = ESTOP.shapes[0].prims(2.0);
    let Prim::Stroke { points, closed, .. } = &prims[0] else {
        panic!("a stroke")
    };
    assert!(closed);
    let top = points.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let bottom = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((top - 5.5).abs() < 1e-3, "dome top {top}");
    assert!((bottom - 11.5).abs() < 1e-3, "flat base {bottom}");
}

#[test]
fn dashed_shapes_keep_their_dash_and_close_the_outline() {
    let dashed = CONTACTOR
        .shapes
        .iter()
        .find(|s| matches!(s, Shape::Dashed { .. }))
        .expect("the contactor has its dashed coupling line");
    let prims = dashed.prims(2.0);
    assert!(matches!(
        prims[0],
        Prim::Stroke {
            dash: Some([on, off]),
            ..
        } if (on - 1.5).abs() < 1e-6 && (off - 2.0).abs() < 1e-6
    ));
    // A dashed rectangle (macro) goes round all four sides.
    let Prim::Stroke { points, closed, .. } = &MACRO.shapes[0].prims(1.0)[0] else {
        panic!("a stroke")
    };
    assert!(!closed);
    assert_eq!(points.first(), points.last());
}

#[test]
fn stroke_scales_with_size_and_never_drops_below_one_pixel() {
    assert!((stroke_width(24.0, 1.0) - 1.5).abs() < 1e-6);
    assert!((stroke_width(48.0, 1.0) - 3.0).abs() < 1e-6);
    assert!((stroke_width(16.0, 1.0) - 1.0).abs() < 1e-6);
    // 12 points would be 0.75: raised to one physical pixel.
    assert!((stroke_width(12.0, 1.0) - 1.0).abs() < 1e-6);
    assert!((stroke_width(12.0, 2.0) - 0.75).abs() < 1e-6);
    assert!((stroke_width(12.0, 1.25) - 0.8).abs() < 1e-6);
}

/// The element count of an SVG icon: rects, circles and one per sub-path.
fn svg_shape_count(svg: &str) -> usize {
    let mut count = 0;
    for tag in svg.split('<').skip(1) {
        let name = tag.split(|c: char| !c.is_ascii_alphabetic()).next();
        match name {
            Some("rect" | "circle") => count += 1,
            Some("path") => {
                let d = tag.split("d=\"").nth(1).and_then(|r| r.split('"').next());
                count += d.map_or(0, |d| d.matches(['M', 'm']).count());
            }
            _ => {}
        }
    }
    count
}

#[test]
fn every_icon_has_the_shape_count_of_its_svg() {
    // The Rust data is generated from koper_icons.py, which also writes these
    // SVGs: a count that differs means set.rs was not regenerated.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/design/visual-redesign/icons");
    for icon in ALL {
        let file = dir.join(format!("{}.svg", icon.name));
        let svg =
            std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        assert_eq!(
            icon.shape_count(),
            svg_shape_count(&svg),
            "{}: run docs/design/visual-redesign/tools/icons_to_rust.py",
            icon.name
        );
    }
}

/// Runs one frame that paints `icon` at `size` points and returns what was
/// drawn.
fn painted(icon: &Icon, size: f32, ppp: f32) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    let mut raw = egui::RawInput::default();
    raw.viewports
        .entry(raw.viewport_id)
        .or_default()
        .native_pixels_per_point = Some(ppp);
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0)));
    let mut output = ctx.run_ui(raw, |ui| {
        let rect = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::splat(size));
        paint(ui.painter(), rect, icon, Color32::WHITE);
    });
    output.textures_delta.clear();
    output.shapes
}

#[test]
fn painting_stays_inside_the_rect_at_every_size() {
    for icon in ALL {
        for size in SIZES {
            let shapes = painted(icon, size, 1.0);
            assert!(!shapes.is_empty(), "{} paints nothing at {size}", icon.name);
            let rect = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::splat(size)).expand(0.01);
            for clipped in &shapes {
                let bounds = clipped.shape.visual_bounding_rect();
                assert!(
                    rect.contains_rect(bounds),
                    "{} at {size}: {bounds:?} leaves {rect:?}",
                    icon.name
                );
            }
        }
    }
}

#[test]
fn painting_uses_one_colour() {
    let shapes = painted(&CONTACTOR, 24.0, 1.0);
    let mut seen = HashSet::new();
    for clipped in &shapes {
        collect_colours(&clipped.shape, &mut seen);
    }
    assert_eq!(seen.len(), 1, "colours used: {seen:?}");
    assert!(seen.contains(&Color32::WHITE));
}

fn collect_colours(shape: &Egui, seen: &mut HashSet<Color32>) {
    match shape {
        Egui::Circle(c) => {
            seen.insert(c.fill);
        }
        Egui::LineSegment { stroke, .. } => {
            seen.insert(stroke.color);
        }
        Egui::Path(p) => {
            if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                seen.insert(c);
            }
        }
        Egui::Vec(v) => v.iter().for_each(|s| collect_colours(s, seen)),
        _ => {}
    }
}

#[test]
fn a_zero_size_rect_paints_nothing() {
    assert!(painted(&MOTOR, 0.0, 1.0).is_empty());
}

fn demo(name: &str) -> IconSpec {
    IconSpec {
        name: name.into(),
        title: "Demo".into(),
        shapes: vec![
            ShapeSpec::Rect {
                x: 5.0,
                y: 3.0,
                w: 14.0,
                h: 18.0,
                r: 1.5,
            },
            ShapeSpec::Dashed {
                shape: Box::new(ShapeSpec::Line([5.0, 12.0, 19.0, 12.0])),
                on: 1.5,
                off: 2.0,
            },
            ShapeSpec::Circle {
                cx: 12.0,
                cy: 7.0,
                r: 1.0,
                filled: true,
            },
        ],
    }
}

#[test]
fn an_add_on_icon_is_found_by_name_paints_and_unloads() {
    let icon = register(demo("pt-icons.demo")).unwrap();
    assert_eq!(icon.group, Group::Addon);
    assert_eq!(icon.shape_count(), 3);
    assert_eq!(Icon::by_name("pt-icons.demo"), Some(icon));
    assert!(registered().iter().any(|i| i.name == "pt-icons.demo"));
    // The same engine paints it, inside its rect, at every size.
    for size in SIZES {
        let shapes = painted(icon, size, 1.0);
        assert!(!shapes.is_empty());
        let rect = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::splat(size)).expand(0.01);
        for clipped in &shapes {
            assert!(
                rect.contains_rect(clipped.shape.visual_bounding_rect()),
                "{size}"
            );
        }
    }
    // Registering the name again replaces it.
    register(demo("pt-icons.demo")).unwrap();
    assert_eq!(
        registered()
            .iter()
            .filter(|i| i.name == "pt-icons.demo")
            .count(),
        1
    );
    assert_eq!(unregister_prefix("pt-icons."), 1);
    assert_eq!(Icon::by_name("pt-icons.demo"), None);
    // The built-in set is untouched by any of this.
    assert_eq!(ALL.len(), 40);
}

#[test]
fn an_add_on_icon_must_follow_the_rules_of_the_set() {
    assert_eq!(register(demo("demo")).unwrap_err(), IconError::BadName);
    assert_eq!(register(demo("pt:x")).unwrap_err(), IconError::BadName);
    assert_eq!(register(demo("Demo.x")).unwrap_err(), IconError::BadName);
    assert_eq!(register(demo("coil")).unwrap_err(), IconError::BadName); // built-in names have no dot
    assert_eq!(register(demo(".x")).unwrap_err(), IconError::BadName);
    let mut no_title = demo("pt-bad.a");
    no_title.title = " ".into();
    assert_eq!(register(no_title).unwrap_err(), IconError::NoTitle);
    let mut empty = demo("pt-bad.b");
    empty.shapes.clear();
    assert_eq!(register(empty).unwrap_err(), IconError::NoShapes);
    let mut outside = demo("pt-bad.c");
    outside.shapes = vec![ShapeSpec::Line([-1.0, 12.0, 25.0, 12.0])];
    assert_eq!(register(outside).unwrap_err(), IconError::OutOfGrid);
    let mut many = demo("pt-bad.f");
    many.shapes = vec![ShapeSpec::Line([4.0, 4.0, 20.0, 4.0]); MAX_SHAPES + 1];
    assert_eq!(register(many).unwrap_err(), IconError::TooManyShapes);
    let mut nan = demo("pt-bad.d");
    nan.shapes = vec![ShapeSpec::Line([f32::NAN, 1.0, 5.0, 5.0])];
    assert_eq!(register(nan).unwrap_err(), IconError::OutOfGrid);
    let mut dash = demo("pt-bad.e");
    dash.shapes = vec![ShapeSpec::Dashed {
        shape: Box::new(ShapeSpec::Line([4.0, 4.0, 20.0, 4.0])),
        on: 0.0,
        off: 2.0,
    }];
    assert_eq!(register(dash).unwrap_err(), IconError::BadShape);
    assert!(registered().iter().all(|i| !i.name.starts_with("pt-bad.")));
}

#[test]
fn valid_names_are_namespaced() {
    assert!(valid_name("controllers.pendant"));
    assert!(valid_name("my-addon.radio_2"));
    for bad in [
        "pendant", "a.b.c", "A.b", "a.", ".b", "a b.c", "a:b", "coil",
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
}
