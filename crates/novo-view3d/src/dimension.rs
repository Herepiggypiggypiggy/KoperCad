//! Dimensions drawn over the 3D image: the enclosure's overall size and, for
//! one selected part, its size and the clear distance to what is next to it
//! or to the plate edge, in millimetres. GPU free: this only decides which
//! measuring lines exist; the widget projects and paints them.
//!
//! The distances are what the workshop needs to drill by hand: from the
//! part's edge to the plate edge or to its neighbour, along the plate.

use crate::math::Aabb;
use crate::scene::{ItemId, ItemKind, Scene};
use glam::Vec3;
use std::collections::BTreeSet;

/// Gaps smaller than this are not drawn (touching parts): millimetres.
const MIN_GAP_MM: f32 = 0.05;
/// How far the overall dimension lines stand off the enclosure: millimetres.
const STAND_OFF_MM: f32 = 25.0;
/// How far a part's size lines stand off the part: millimetres.
const PART_OFF_MM: f32 = 6.0;

/// What a line measures, for styling and for the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Width, height or depth of the enclosure.
    Overall,
    /// Width, height or depth of the selected part.
    Size,
    /// Clear distance from the selected part to a neighbour.
    Gap,
    /// Clear distance from the selected part to the plate edge.
    Edge,
}

/// One measuring line from `a` to `b` in world millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    pub a: Vec3,
    pub b: Vec3,
    pub role: Role,
    /// The measured length, mm.
    pub mm: f32,
    /// The text drawn on the line, for example `W 54 mm`.
    pub text: String,
}

/// Millimetres as written on a line: one decimal, none when it is whole.
pub fn format_mm(mm: f32) -> String {
    let tenths = (mm * 10.0).round() as i64;
    if tenths % 10 == 0 {
        format!("{} mm", tenths / 10)
    } else {
        format!("{:.1} mm", tenths as f32 / 10.0)
    }
}

fn line(a: Vec3, b: Vec3, role: Role, prefix: &str) -> Dimension {
    let mm = (b - a).length();
    let text = if prefix.is_empty() {
        format_mm(mm)
    } else {
        format!("{prefix} {}", format_mm(mm))
    };
    Dimension {
        a,
        b,
        role,
        mm,
        text,
    }
}

/// The dimensions to show for `scene` with `selection`. The overall size of
/// the enclosure (its shell, else the plate) is always there; one selected
/// part adds its size and its distances. Several selected parts add nothing:
/// the picture would be crowded and a distance needs one part.
pub fn dimensions(scene: &Scene, selection: &BTreeSet<ItemId>) -> Vec<Dimension> {
    let mut out = Vec::new();
    let plate = bounds_of(scene, ItemKind::Plate);
    let overall = bounds_of(scene, ItemKind::Enclosure).or(plate);
    if let Some(b) = overall.filter(|b| !b.is_empty()) {
        overall_lines(&b, &mut out);
    }
    let mut chosen = selection.iter().filter_map(|id| scene.get(*id));
    if let (Some(item), None) = (chosen.next(), chosen.next())
        && !matches!(
            item.id.kind,
            ItemKind::Plate | ItemKind::Enclosure | ItemKind::Door | ItemKind::Wire
        )
    {
        let b = scene.world_bounds(item);
        part_lines(scene, item.id, &b, plate.as_ref(), &mut out);
    }
    out
}

fn bounds_of(scene: &Scene, kind: ItemKind) -> Option<Aabb> {
    let mut b = Aabb::EMPTY;
    for i in scene.items.iter().filter(|i| i.id.kind == kind) {
        b = b.union(&scene.world_bounds(i));
    }
    (!b.is_empty()).then_some(b)
}

/// Width below the enclosure, height at its left, depth along its bottom
/// left edge, each standing off so the line does not cover the box.
fn overall_lines(b: &Aabb, out: &mut Vec<Dimension>) {
    let z = b.max.z;
    out.push(line(
        Vec3::new(b.min.x, b.min.y - STAND_OFF_MM, z),
        Vec3::new(b.max.x, b.min.y - STAND_OFF_MM, z),
        Role::Overall,
        "W",
    ));
    out.push(line(
        Vec3::new(b.min.x - STAND_OFF_MM, b.min.y, z),
        Vec3::new(b.min.x - STAND_OFF_MM, b.max.y, z),
        Role::Overall,
        "H",
    ));
    if b.max.z - b.min.z > 0.5 {
        out.push(line(
            Vec3::new(b.min.x - STAND_OFF_MM, b.min.y - STAND_OFF_MM, b.min.z),
            Vec3::new(b.min.x - STAND_OFF_MM, b.min.y - STAND_OFF_MM, b.max.z),
            Role::Overall,
            "D",
        ));
    }
}

fn part_lines(scene: &Scene, id: ItemId, b: &Aabb, plate: Option<&Aabb>, out: &mut Vec<Dimension>) {
    let z = b.max.z;
    let (cx, cy) = ((b.min.x + b.max.x) * 0.5, (b.min.y + b.max.y) * 0.5);
    // Size: width under the part, height on its left, depth on its lower right edge.
    out.push(line(
        Vec3::new(b.min.x, b.min.y - PART_OFF_MM, z),
        Vec3::new(b.max.x, b.min.y - PART_OFF_MM, z),
        Role::Size,
        "W",
    ));
    out.push(line(
        Vec3::new(b.min.x - PART_OFF_MM, b.min.y, z),
        Vec3::new(b.min.x - PART_OFF_MM, b.max.y, z),
        Role::Size,
        "H",
    ));
    if b.max.z - b.min.z > 0.5 {
        out.push(line(
            Vec3::new(b.max.x + PART_OFF_MM, b.min.y, b.min.z),
            Vec3::new(b.max.x + PART_OFF_MM, b.min.y, b.max.z),
            Role::Size,
            "D",
        ));
    }

    // Neighbours: what stands wholly beside the part and overlaps it across.
    let others: Vec<Aabb> = scene
        .items
        .iter()
        .filter(|i| i.id != id)
        .filter(|i| {
            matches!(
                i.id.kind,
                ItemKind::Device | ItemKind::Duct | ItemKind::Rail | ItemKind::Other
            )
        })
        .map(|i| scene.world_bounds(i))
        .collect();
    let overlap_y = |o: &Aabb| o.min.y < b.max.y && o.max.y > b.min.y;
    let overlap_x = |o: &Aabb| o.min.x < b.max.x && o.max.x > b.min.x;

    // (nearest neighbour edge, or the plate edge) on each side.
    let left = others
        .iter()
        .filter(|o| overlap_y(o) && o.max.x <= b.min.x)
        .map(|o| (o.max.x, true))
        .max_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.min.x, false)));
    let right = others
        .iter()
        .filter(|o| overlap_y(o) && o.min.x >= b.max.x)
        .map(|o| (o.min.x, true))
        .min_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.max.x, false)));
    let below = others
        .iter()
        .filter(|o| overlap_x(o) && o.max.y <= b.min.y)
        .map(|o| (o.max.y, true))
        .max_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.min.y, false)));
    let above = others
        .iter()
        .filter(|o| overlap_x(o) && o.min.y >= b.max.y)
        .map(|o| (o.min.y, true))
        .min_by(|a, c| a.0.total_cmp(&c.0))
        .or_else(|| plate.map(|p| (p.max.y, false)));

    let role = |neighbour: bool| if neighbour { Role::Gap } else { Role::Edge };
    let mut gap = |from: Vec3, to: Vec3, neighbour: bool| {
        if (to - from).length() >= MIN_GAP_MM {
            out.push(line(from, to, role(neighbour), ""));
        }
    };
    if let Some((x, n)) = left.filter(|(x, _)| *x <= b.min.x) {
        gap(Vec3::new(x, cy, z), Vec3::new(b.min.x, cy, z), n);
    }
    if let Some((x, n)) = right.filter(|(x, _)| *x >= b.max.x) {
        gap(Vec3::new(b.max.x, cy, z), Vec3::new(x, cy, z), n);
    }
    if let Some((y, n)) = below.filter(|(y, _)| *y <= b.min.y) {
        gap(Vec3::new(cx, y, z), Vec3::new(cx, b.min.y, z), n);
    }
    if let Some((y, n)) = above.filter(|(y, _)| *y >= b.max.y) {
        gap(Vec3::new(cx, b.max.y, z), Vec3::new(cx, y, z), n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, PlateFrame, Pose, Shape};

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 400.0,
            height: 300.0,
        });
        // Plate 400 x 300 at x 0..400, y 0..300, 3 mm thick below the surface.
        s.push(Item::new(
            ItemId::new(ItemKind::Plate, 1),
            Shape::Box,
            Pose::at(Vec3::new(200.0, 150.0, -3.0)),
            Vec3::new(400.0, 300.0, 3.0),
        ));
        // Three 50 mm wide devices on one row, 100 mm tall, 60 mm deep.
        for (n, x) in [(10u64, 100.0f32), (11, 200.0), (12, 330.0)] {
            s.push(Item::new(
                ItemId::new(ItemKind::Device, n),
                Shape::Box,
                Pose::at(Vec3::new(x, 150.0, 0.0)),
                Vec3::new(50.0, 100.0, 60.0),
            ));
        }
        s
    }

    fn pick(s: &Scene, n: u64) -> BTreeSet<ItemId> {
        let _ = s;
        BTreeSet::from([ItemId::new(ItemKind::Device, n)])
    }

    fn find<'a>(d: &'a [Dimension], role: Role, text: &str) -> Option<&'a Dimension> {
        d.iter().find(|d| d.role == role && d.text == text)
    }

    #[test]
    fn millimetres_are_written_with_one_decimal_only_when_needed() {
        assert_eq!(format_mm(54.0), "54 mm");
        assert_eq!(format_mm(54.04), "54 mm");
        assert_eq!(format_mm(54.5), "54.5 mm");
        assert_eq!(format_mm(0.26), "0.3 mm");
        assert_eq!(format_mm(1234.96), "1235 mm");
    }

    #[test]
    fn the_overall_size_of_the_plate_is_always_there() {
        let s = scene();
        let d = dimensions(&s, &BTreeSet::new());
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(find(&d, Role::Overall, "W 400 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Overall, "H 300 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Overall, "D 3 mm").is_some(), "{d:?}");
    }

    #[test]
    fn the_shell_gives_the_overall_size_when_there_is_one() {
        let mut s = scene();
        s.push(Item::new(
            ItemId::new(ItemKind::Enclosure, 2),
            Shape::Box,
            Pose::at(Vec3::new(200.0, 150.0, -200.0)),
            Vec3::new(420.0, 320.0, 200.0),
        ));
        let d = dimensions(&s, &BTreeSet::new());
        assert!(find(&d, Role::Overall, "W 420 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Overall, "D 200 mm").is_some(), "{d:?}");
    }

    #[test]
    fn one_part_shows_its_size_and_its_distances_to_neighbours_and_the_plate() {
        let s = scene();
        // The middle device spans x 175..225, y 100..200, z 0..60.
        let d = dimensions(&s, &pick(&s, 11));
        assert!(find(&d, Role::Size, "W 50 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Size, "H 100 mm").is_some(), "{d:?}");
        assert!(find(&d, Role::Size, "D 60 mm").is_some(), "{d:?}");
        // Left neighbour ends at 125: 50 mm; right starts at 305: 80 mm.
        let left = find(&d, Role::Gap, "50 mm").expect("gap to the left neighbour");
        assert_eq!((left.a.x, left.b.x), (125.0, 175.0));
        let right = find(&d, Role::Gap, "80 mm").expect("gap to the right neighbour");
        assert_eq!((right.a.x, right.b.x), (225.0, 305.0));
        // Nothing above or below: the plate edge, 100 mm each way.
        let edges: Vec<_> = d.iter().filter(|d| d.role == Role::Edge).collect();
        assert_eq!(edges.len(), 2, "{d:?}");
        assert!(edges.iter().all(|e| e.text == "100 mm"), "{edges:?}");
    }

    #[test]
    fn a_part_next_to_the_plate_edge_measures_to_the_edge() {
        let s = scene();
        // The first device spans x 75..125: 75 mm from the plate's left edge.
        let d = dimensions(&s, &pick(&s, 10));
        let edge = find(&d, Role::Edge, "75 mm").expect("distance to the left edge");
        assert_eq!((edge.a.x, edge.b.x), (0.0, 75.0));
        // And its right neighbour is the middle device: 175 - 125 = 50 mm.
        assert!(find(&d, Role::Gap, "50 mm").is_some(), "{d:?}");
    }

    #[test]
    fn touching_parts_have_no_gap_line_and_overlap_is_not_a_gap() {
        let mut s = scene();
        s.push(Item::new(
            ItemId::new(ItemKind::Device, 13),
            Shape::Box,
            Pose::at(Vec3::new(250.0, 150.0, 0.0)),
            Vec3::new(50.0, 100.0, 60.0),
        ));
        // Device 13 spans 225..275 and touches the middle device at 225.
        let d = dimensions(&s, &pick(&s, 11));
        assert!(
            d.iter()
                .all(|d| d.role != Role::Gap || d.mm > 1.0 || d.a.x < 225.0),
            "{d:?}"
        );
        let right: Vec<_> = d
            .iter()
            .filter(|d| d.role == Role::Gap && d.a.x >= 225.0)
            .collect();
        assert!(right.is_empty(), "no line for touching parts: {right:?}");
    }

    #[test]
    fn several_selected_parts_or_a_plate_add_no_part_lines() {
        let s = scene();
        let two = BTreeSet::from([
            ItemId::new(ItemKind::Device, 10),
            ItemId::new(ItemKind::Device, 11),
        ]);
        assert_eq!(dimensions(&s, &two).len(), 3, "overall lines only");
        let plate = BTreeSet::from([ItemId::new(ItemKind::Plate, 1)]);
        assert_eq!(dimensions(&s, &plate).len(), 3);
        let missing = BTreeSet::from([ItemId::new(ItemKind::Device, 99)]);
        assert_eq!(dimensions(&s, &missing).len(), 3);
    }

    #[test]
    fn an_empty_scene_has_no_dimensions() {
        let s = Scene::new(PlateFrame {
            width: 10.0,
            height: 10.0,
        });
        assert!(dimensions(&s, &BTreeSet::new()).is_empty());
    }
}
